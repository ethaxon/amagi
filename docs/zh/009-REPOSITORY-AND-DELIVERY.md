# 009-REPOSITORY-AND-DELIVERY

## 1. 本文档目的

本文档定义 amagi 的推荐仓库结构、模块切分、实现顺序与里程碑。它面向具体落地与后续 AI agents 开工。

相关文档：

- 总览：`000-OVERVIEW.md`
- 架构：`001-ARCHITECTURE.md`
- API：`004-API.md`
- 同步：`005-SYNC.md`

---

## 2. 推荐仓库结构

建议采用 monorepo：

```
amagi/
  README.md
  AGENTS.md
  docs/
  apps/
    dashboard-web/
    extension-web/
    api-server/
  packages/
    amagi-auth/
    amagi-config/
    amagi-securitydept/
    amagi-db/
    amagi-db-migration/
    sync-core/
    browser-adapter-webext/
    dashboard-sdk/
```

---

## 3. 后端结构建议

后端代码不要长期集中在 `apps/api-server` 单 crate 内。Rust 以 crate 为主要编译单元；把 config、SecurityDept auth glue、domain、sync、migration helpers 等可复用逻辑全部塞进 API server 会拖慢增量编译，也会阻碍后续 CLI / job / migration runner 复用。

原则：

- `apps/api-server` 应是 thin app crate，主要负责 process bootstrap、HTTP route wiring、state assembly 与 app-local glue。
- 可被 API server、CLI、job worker、migration runner 共享的逻辑应下沉到 `packages/*` Rust crates。
- 单个 Rust source file 不应承载多个边界。配置、schema、nested env overlay、SecurityDept mapping、runtime resolution、tests 应拆成清晰模块。
- 对上游库已有的稳定类型，优先复用或 newtype 包装，不要复制一套语义相近的本地 struct。

### 3.1 `apps/api-server`

当前 `apps/api-server` 保持 thin app crate：

```
src/
  main.rs
  app.rs
  error.rs
  http/
    routes/
    extractors/
    errors.rs
```

配置加载、schema、nested env overlay 与 SecurityDept auth runtime/resolver 不放在 app crate 内，分别下沉到 `packages/amagi-config` 与 `packages/amagi-securitydept`。后续 domain、policy、sync、vault、audit、db、jobs 等共享逻辑也应继续拆入 `packages/*` crates，`apps/api-server` 只负责 process bootstrap、HTTP route wiring、state assembly 与 app-local glue。

当前数据库迁移 crate 位于：

```text
packages/amagi-db-migration/
  Cargo.toml
  src/
    defs.rs
    lib.rs
    main.rs
    m20260504_000001_create_core_tables.rs
    rls.rs
    schema.rs
```

`packages/amagi-db-migration` 纳入 Rust workspace，package / binary 名为 `amagi-db-migration`，用于通过 SeaORM migration CLI 风格执行 `up` / `down`。migration crate 必须同时保留 library `Migrator` 导出，供 `packages/amagi-db` 在 `database.auto_migrate=true` 时接入受控 `auto_migrate`；共享 `DeriveIden` 定义收敛在 `src/defs.rs`。迁移逻辑不放入 `apps/api-server/src/main.rs`，`apps/api-server` 也不直接依赖 migration crate。

迁移实现要求：

- 使用 SeaORM / `sea-orm-migration` 2.0.0-rc.x 系列。
- 优先使用 `sea-orm-migration` 高阶 API；SeaQuery 作为其表达层、predicate builder 或必要 helper。
- raw SQL 只允许用于 SeaQuery / sea-orm-migration 无法直接表达的 PostgreSQL 专属 DDL，并且必须收敛到小 helper；不要把 owner predicate、join、`EXISTS` 等可结构化表达的逻辑写成大段字符串拼接。
- 表名、列名、索引、foreign key 和测试复用共享 `#[derive(DeriveIden)]` 定义。
- 通用 schema helper、RLS helper、UUIDv7 / JSONB / array default / index helper 必须进入共享模块，例如 `schema.rs`、`helpers.rs`、`rls.rs`，不要留在第一条 migration 私有函数里等待复制。
- PostgreSQL 18 的 `uuidv7()` 是服务端生成 UUID 主键默认策略。
- owner-scoped 核心表从首条迁移启用 RLS，并约定 `amagi.current_user_id` 这类 session variable contract。

当前实现中，`schema.rs` 已承接 UUIDv7 / JSONB / array default / index helper，`rls.rs` 已承接 SeaQuery predicate builder 与 PostgreSQL policy DDL 外壳；`m20260504_000001_create_core_tables.rs` 只组合核心表与 policy。

当前数据库运行时边界位于 `packages/amagi-db`：

- 持有 SeaORM `DatabaseConnection` 的 `DbRuntime`。
- `DatabaseService` 负责 no-db mode、连接尝试、受控 auto-migrate、数据库 health/readiness 状态。
- `migrate.rs` 通过 migration crate library `Migrator` 执行受控 `up`，不 shell out 到外部 bin。
- `entities/` 提供 `users`、`auth_users`、`oidc_account_bindings`、`audit_events`、`libraries`、`bookmark_nodes`、`bookmark_meta`、`library_heads`、`node_revisions` 的 SeaORM entity / ActiveModel，auth binding 和 bookmark domain repository 的普通 CRUD 在这里建立静态 schema 边界。
- `rls.rs` 提供 transaction-local `amagi.current_user_id` helper，以及 `oidc_account_bindings` 预解析阶段使用的 `amagi.auth_oidc_source` / `amagi.auth_oidc_subject` / `amagi.auth_oidc_identity_key` lookup helper，避免业务层自行拼 `SET` SQL。
- raw SQL 仅保留在 `set_config/current_setting` helper、`SELECT 1` ping、`to_regclass(...)` readiness check 和 migration / policy DDL 这些 SeaORM 不直接覆盖的边界。

API server 当前在启动阶段组装 `AppState` 时调用 `DatabaseService::initialize()`：

- `database.url` 未配置时，服务继续启动，但 `/readyz` 返回 `503` 且 `database.state=not_configured`。
- `database.url` 已配置时，启动阶段尝试建立连接。
- `database.auto_migrate=true` 时，启动阶段执行 migration crate library `Migrator::up()`。
- 连接失败或 auto-migrate 失败时，服务保留脱敏状态并通过 `/healthz`、`/readyz` 暴露；不得输出 database URL。

### 3.2 crate 组织建议

后端 workspace 应逐步拆成多个 crates，至少预留：

- `amagi-domain`
- `amagi-policy`
- `amagi-sync`
- `amagi-auth`
- `amagi-config`
- `amagi-securitydept`
- `amagi-db`
- `amagi-api`

其中：

- `amagi-auth`：amagi auth facade、frontend config projection、verified-claims principal resolution、bearer subject lookup baseline、principal/account binding repository。
- `amagi-config`：typed config model、Figment loading、nested env overlay、schema generation、example validation。
- `amagi-securitydept`：SecurityDept config surface adapter、OIDC source resolution、per-source lazy token-set/resource-server runtime/service wrapper。
- `amagi-db`：SeaORM runtime、auto-migrate 接线、readiness/health check、RLS session helper。
- `amagi-bookmarks`：bookmark library / node / meta / revision 的领域模型、repository、service、RLS transaction orchestration 与 API-facing DTO。`apps/api-server` 的 dashboard route handler 不直接写 SeaORM，不直接推进 revision clock。
- `amagi-api` / `apps/api-server`：HTTP server assembly。

早期可以短暂保留单 crate，但一旦某个文件超过清晰单一边界，或逻辑显然会被 CLI / job / tests 复用，就应拆出 package crate，而不是继续加模块内私有结构。

当前 auth 集成边界拆为两层：

- `packages/amagi-securitydept`：amagi 使用 `securitydept-core` 作为 Rust 入口，组合 `token-set-context` 的 `backend-oidc`、`frontend-oidc` 与 `access_token_substrate` typed config surface，并按 source key 惰性构建 runtime/service wrapper；不要在 `main.rs` 或 route handler 内手写 OIDC authorization-code / PKCE / callback / refresh / userinfo flow。
- `packages/amagi-auth`：承接 amagi 应用侧 auth facade、source resolution、frontend config projection、`ExternalOidcIdentity` / `AmagiPrincipal`、verified-claims principal resolution、OIDC account binding repository 与 bearer lookup baseline。`apps/api-server` 只挂 route，不直接拼 binding SQL 或 audit JSON。
- `packages/amagi-bookmarks`：承接 Iter6 起的 cloud bookmark source-of-truth 纵切片。所有 normal library / node mutation 都必须经 service 在 owner-scoped transaction 内设置 `amagi.current_user_id`、校验 ownership、写 `node_revisions` 并推进 `library_heads.current_revision_clock`。

集成测试约定继续使用 crate-local `tests/integration/main.rs` 作为入口。当前 Postgres/Dex/container 相关测试辅助集中在 `packages/amagi-test-utils`，业务 crate 复用该层，不在各自 integration binary 中复制 testcontainers 组合逻辑。

---

## 4. 前端结构建议

### 4.1 `apps/dashboard-web`

见 `007-WEB-UI.md`。

建议：

```
src/
  app/
  routes/
  features/
  components/
  lib/
```

### 4.2 `apps/extension-web`

建议：

```
entrypoints/
  background.ts
  popup/
  options/
  sidepanel/
src/
  adapter/
  shared/
wxt.config.ts
```

建议以 WXT 作为该应用的扩展壳层：

- 负责开发脚手架
- 负责 manifest 与多入口组织
- 负责 Chrome/Edge/Firefox/Safari 等目标浏览器构建输出
- 负责 popup/options/side panel 等 UI 容器装配
- 负责通过 WXT target / manifest version 机制处理构建期差异

共享逻辑不要直接堆在扩展入口中，应下沉到 `packages/`。
`apps/extension-web` 不应变成同步核心实现所在层。

---

## 5. 共享包建议

### 5.1 `packages/amagi-sync`

职责：

- BrowserClient register / session / feed / preview / apply / cursor ack 服务编排
- sync preview / conflict / cursor / mapping 的 repository 与 DTO
- 事务内复用 `packages/amagi-bookmarks` 的 transaction-scoped mutation boundary
- 保持 `apps/api-server` 为 thin app crate，不在 route handler 内写同步业务逻辑

### 5.2 `packages/browser-adapter-webext`

职责：

- WXT / WebExtension bookmarks API 封装
- 本地 node id / tree 提取
- apply ops
- `browser.storage.local` / `chrome.storage.local` 上的扩展 sync state 持久化
- platform capability detection
- 必要的 Chrome/Firefox/Safari compatibility override

当前 Iter8 baseline 的 `packages/browser-adapter-chromium` 是过渡实现：

- `src/chromium-bookmarks.ts`
- `src/chromium-storage.ts`
- fake `chrome` 驱动的 Node 测试

后续应迁移/收敛到 WXT/WebExtension adapter，而不是继续扩张成 Chromium / Firefox / Safari 三套长期维护的 adapter package。

### 5.3 Safari 降级 adapter

Safari 首阶段职责有限：

- 保存当前页
- 搜索入口
- 轻量桥接能力
- 通过 capability detection 显式声明不支持完整原生书签树同步

WXT 支持 Safari 构建不等于 amagi 承诺 Safari 原生书签树完整双向同步；如后续需要更强能力，再评估 native wrapper。

### 5.4 `packages/dashboard-sdk`

职责：

- API client
- shared DTO types
- auth/session helpers

---

## 6. 实现顺序建议

### Phase 0: 文档与基线

目标：

- 确认 docs 完整
- 确认仓库骨架
- 确认命名与边界

产物：

- 当前 docs
- README
- AGENTS

### Phase 1: 后端最小骨架

目标：

- API server 跑起来
- health/config/logging
- SecurityDept backend-oidc / token-set auth boundary
- users/auth_users/oidc_account_bindings/devices/browser_clients/libraries/bookmark_nodes 表

产物：

- 可启动服务
- 初始 migrations
- `/healthz` 与基础配置 / 日志 / 错误壳层
- amagi auth facade 与 SecurityDept 产品面的运行时边界 baseline
- SeaORM migration crate 与首条核心表迁移

当前首条迁移的目标覆盖核心表：`users`、`auth_users`、`oidc_account_bindings`、`devices`、`browser_clients`、`libraries`、`bookmark_nodes`、`bookmark_meta`、`library_heads`、`node_revisions`、`sync_cursors`、`node_client_mappings`、`sync_previews`、`sync_conflicts`、`sync_profiles`、`sync_profile_targets`、`sync_profile_rules`、`vault_unlock_sessions`、`audit_events`。

以下表和能力仍属于后续迁移或后续阶段：`webauthn_credentials`、`vault_keys`、archive assets、FTS generated columns / indexes、closure table、team sharing / ACL。

### Phase 2: 内容域与 Dashboard 读写

目标：

- libraries/tree CRUD
- bookmark_meta
- basic search
- audit 基础

产物：

- Dashboard 可浏览与编辑 normal library

### Phase 3: Sync 核心

目标：

- revisions
- library_heads
- cursors
- preview/apply
- mapping
- basic conflicts

产物：

- 服务端 sync API 可工作

### Phase 4: WXT 桌面扩展 MVP

目标：

- 基于 WXT 建立扩展壳层与跨浏览器构建输出
- browser client register
- local scan
- preview/apply
- ack

产物：

- 实际桌面浏览器手动同步闭环

当前 Iter8 已进入该阶段的迁移前 baseline：

- `packages/sync-client` 已提供 typed Sync API client、local tree normalization、diff baseline、apply plan baseline、manual sync orchestrator 和 Node tests。
- `packages/browser-adapter-chromium` 已提供 Chromium bookmarks/storage adapter baseline 和 fake-chrome tests。
- `apps/extension-web` 已提供 MV3 manifest/background/popup/options 的构建产物 baseline。

当前缺口：

- 尚未把 `apps/extension-web` 迁移为 WXT app。
- 尚未把 Chromium-only adapter 收敛为 WXT/WebExtension adapter。
- 尚未用同一 WXT app 验证 Firefox 构建。
- 尚未实现自动后台 sync、conflict resolution UI、完整 options/popup 状态管理。
- 尚未实现 server-created local node 的完整 mapping reconcile。

### Phase 5: Sync Profiles 与规则

目标：

- target selectors
- include/exclude/readonly
- projection 裁剪

产物：

- 同一用户不同设备看到不同书签集

### Phase 6: Vault 与二次解锁

目标：

- vault libraries
- WebAuthn
- unlock session
- vault 搜索与访问控制

产物：

- 私密收藏库闭环

### Phase 7: 冲突中心与增强 UI

目标：

- conflict list
- manual resolution
- better preview viewer

### Phase 8: Safari 降级支持 / 移动 Web

目标：

- Safari 保存当前页
- 搜索入口
- PWA 基础访问

---

## 7. 首阶段 MVP 定义

首阶段 MVP 以"桌面浏览器 + Dashboard + vault 基础能力"为核心。

必须完成：

- OIDC 登录
- normal library CRUD
- revisions + cursors
- manual preview/apply sync
- WXT Chromium extension MVP
- WXT Firefox build baseline
- sync profile + rules 基础
- vault library + unlock session
- WebAuthn 基础
- conflicts basic view

可以延后：

- archive worker
- advanced search relevance
- Safari 原生树同步
- native mobile shell
- team sharing

---

## 8. 测试策略

### 8.1 后端必须覆盖

- node CRUD
- move/reorder
- revision generation
- preview/apply
- cursor advance
- vault visibility
- unlock expiry

### 8.2 前端至少覆盖

- route loading
- tree/list interaction
- preview/apply UI
- vault unlock flow

### 8.3 扩展至少覆盖

- register
- load tree
- diff/scan
- apply ops
- ack

---

## 9. 数据与配置管理

### 9.1 配置项建议

- database url：当前为配置 skeleton，不代表 API server 已建立连接池或启动时连接数据库
- database auto migrate：默认关闭；后续启用时应通过 migration crate 的 library `Migrator` 执行，而不是 shell out 到外部 bin
- oidc sources：map-like，多来源配置，key 对应 `oidc_source`
- oidc client union / backend-oidc override / frontend-oidc override
- oauth resource server / access-token substrate
- external base url
- session secret
- webauthn rp id/name
- object storage settings（可选）

OIDC / token-set 配置应映射到 SecurityDept `backend-oidc` mode 的配置面。amagi 配置层可以保留 facade path、external base URL、token-set storage policy、browser client binding 与 vault unlock 策略，但不应复制 SecurityDept 的 OIDC client 配置解析与协议状态机。

配置模型必须区分 OIDC client 与 OAuth resource server：

- OIDC client / token-set：authorization-code、PKCE、callback、token exchange、refresh、userinfo、pending state、backend/frontend OIDC mode override。
- OAuth resource server / access-token substrate：API bearer 验证、issuer、audience、JWKS / introspection、token propagation。

SecurityDept 配置复用的实现裁决：

- OIDC / token-set 主配置不得退化为 `serde_json::Value` / `json::Value` 动态验证；必须保持 typed config，并优先使用 SecurityDept 导出的 config source / override / resolved config / access-token substrate 类型，必要时用薄 newtype / adapter 适配 Figment、schema 或 amagi policy。
- amagi wrapper 负责 source key、应用 facade、browser client binding、vault unlock、audit 等应用层策略；SecurityDept 类型负责 provider、OIDC client union、backend/frontend mode override 与 OAuth resource-server 语义。
- backend-oidc `redirect_path` 固定为 `/api/auth/token-set/oidc/source/{source}/callback`，frontend-oidc `redirect_path` 固定为 `/auth/token-set/oidc/source/{source}/callback`，frontend-oidc `config_projection_path` 固定为 `/api/auth/token-set/oidc/source/{source}/config`。这些值由 amagi compose / validate 阶段按 `source_key` 计算，不作为用户配置项暴露。
- 若用户显式配置上述固定 path，默认必须报配置错误；只有短期兼容迁移才允许 warning + override，并必须在 review / release 记录中注明移除计划。
- `token_propagation` 在 amagi 中禁用；任何显式启用都必须是配置错误，不能 warning 后覆盖。amagi 当前不是 mesh / outpost token propagation 场景。
- `serde_json::Value` 只允许用于 extension metadata、OIDC claim snapshot 这类非主协议配置，不得用于绕过 typed validation 或承载被禁用的安全能力。

多 OIDC 来源必须使用 map-like 结构，例如 `oidc_sources.<source_key>`，而不是数组。这样 Figment env / file overlay 可以按 provider key 合并单个来源。`source_key` 也应作为 `oidc_account_bindings.oidc_source`、facade route / callback state 与 audit 的稳定值。

`database.url` / `AMAGI_DATABASE__URL` 与 `oidc.client_secret` / `AMAGI_OIDC_SOURCES__<source>__OIDC__CLIENT_SECRET` 都必须按 secret 处理。配置结构的 `Debug`、诊断输出和错误路径不得输出明文 database URL 或 OIDC secret；需要排障时只能输出是否配置、host/port 等非敏感派生信息，或使用脱敏形式。

### 9.2 配置加载规范

项目配置应定义为一个可复用的顶层 config model，并用 Figment 加载，而不是在 `main.rs` 或 `config.rs` 中逐项手写 `std::env::var` 解析。

要求：

- 配置加载优先采用 `Figment`，支持 config file + environment overlay。
- config file 至少应支持 TOML；如果引入 JSON / YAML，只能通过 Figment provider 接入同一套 typed model。
- env overlay 使用 SecurityDept server 类似的 `__` nesting 分隔策略，例如 `AMAGI_SERVER__HOST`、`AMAGI_DATABASE__URL`、`AMAGI_DATABASE__AUTO_MIGRATE`。
- 初始开发阶段不引入 legacy config alias。只有出现真实已发布配置面、明确迁移窗口和移除计划时，才允许以有时限的 deprecation policy 引入兼容 alias。
- OIDC / token-set 配置优先复用 SecurityDept 暴露的 config source / resolver 类型；若当前上游版本尚未暴露可直接复用的 Figment provider，则应在 amagi typed config 中保持一份边界映射，避免回退到逐项手写 env parser。
- 实现 OIDC / token-set / OAuth resource server 配置前，必须阅读 `~/workspace/securitydept/docs/zh/020-AUTH_CONTEXT_AND_MODES.md`、`~/workspace/securitydept/docs/zh/007-CLIENT_SDK_GUIDE.md` 与 `~/workspace/securitydept/apps/server/src/config.rs`。
- bool-like 配置不得用临时 `matches!` 手写解析。应使用 serde 可复用表示，例如本项目 `BooleanLike` newtype，或经确认合适的 `serde_with` / 社区 helper。
- 非法 bool-like 值必须报配置错误，不得静默降级为 `false`。
- secret 字段统一使用 redacted wrapper，`Debug`、错误路径、诊断输出不得泄露明文。

当前实现入口为 `amagi-config::ApiServerConfig::load()`：它使用 Figment 合并 `amagi.config.toml`（回退 `amagi.toml`）与 `AMAGI_` 前缀、按 `__` 分层的正式 env overlay。支持的 env 入口只有正式 typed config key，例如 `AMAGI_SERVER__HOST`、`AMAGI_DATABASE__AUTO_MIGRATE`、`AMAGI_OIDC_SOURCES__default__OIDC__CLIENT_ID`；不会再接受 `AMAGI_API_HOST`、`AMAGI_OIDC_CLIENT_ID`、`AMAGI_DATABASE_URL` 这类早期 legacy alias。

`packages/amagi-config` 仍保留 amagi host 侧 typed config 入口，用来表达 `oidc_sources` map、固定 route/policy、schema 与 env overlay；其中 secret 字段已经复用 SecurityDept 上游 `SecretString`。`packages/amagi-securitydept` 已经收缩为薄 adapter，resolver/runtime 直接消费 SecurityDept `0.3.x` 的 typed config、resolved config 与 runtime/service 类型，并只在外层补上 amagi 自己拥有的 source metadata 与固定 path。不要再在 amagi 内维护一套与 SecurityDept resolved config 基本同构的 mirror projection。

### 9.3 配置 Schema 与示例文件

配置模型必须有机器可验证的 schema 与示例文件，避免 TOML / JSON / env 的结构漂移。

要求：

- 提供与 Rust typed config 对齐的 JSON Schema，推荐由 `schemars` 或等价工具从 config struct 生成。
- 如果后续选择 OpenAPI 3.1 schema，也必须能验证配置文档结构；不要只维护自然语言字段说明。
- 提供 `amagi.config.example.toml`，覆盖完整结构，包括 server、database、multi OIDC map、OIDC client union / backend override、OAuth resource server 等非敏感示例。
- 提供 `.env.example`，只放适合 env 的少量基础与敏感项，例如 config file path、port、`AMAGI_DATABASE__URL`、`AMAGI_OIDC_SOURCES__<source>__OIDC__CLIENT_SECRET`；复杂结构应推荐放到 TOML / JSON / YAML 配置文件。
- CI 或测试至少验证 example config 能被当前 config loader 解析，并验证 schema 与示例结构保持同步。

当前仓库已提交 `amagi.config.schema.json`、`amagi.config.example.toml` 与 `.env.example`。`packages/amagi-config` 中的测试会校验 example config 可被 loader 解析，并校验 committed schema 与 `schemars` 生成结果一致。

### 9.4 环境分层

建议至少：

- local
- dev
- prod

### 9.5 seed 数据

可提供开发用 seed：

- demo normal library
- demo vault library
- sample sync profile

---

## 10. 工程协作与编码规范

AGENTS / README 只保留入口和简短执行规则。长期有效的工程规范放在本文档；产品与架构细节放在对应专题文档。

### 10.1 通用规范

- 注释解释 why，不解释显而易见的 what。
- 如果社区已有成熟、现代、维护良好的库能覆盖某个通用能力，应优先使用库，不要手写基础设施。典型例子包括 WXT、SecurityDept token-set / OIDC、SeaORM migration、testcontainers、Figment 等。
- app crate / app package 是组合层，不应长期承载可复用业务逻辑。可复用的 domain、sync、auth、db、config、adapter、SDK 逻辑应下沉到 `packages/*`。
- 不要为了短期演示引入会制造长期迁移债的数据模型、API 形状或配置入口；早期开发阶段允许直接调整模型和清理开发数据库。
- 历史过程记录放在 `CHANGELOG.md` 或 `temp/IMPL_*` 迭代文档，不放在 `docs/` 正文中长期保留。

### 10.2 TypeScript 规范

- 使用 workspace TypeScript project references 管理包间依赖。
- Node / browser TypeScript 包默认面向现代 ESM host，避免不必要的 CommonJS 兼容层。
- enum-like string domain 使用 `export const Foo = { ... } as const` + `export type Foo = (typeof Foo)[keyof typeof Foo]`，不要散落 raw string union 和魔法字符串。
- public contract、message type、audit / telemetry vocabulary、API path segment 等重复字符串应抽为命名常量。
- public function 的可选参数使用 options object。只有参数语义唯一、位置天然清晰、且未来几乎不会扩展时，才允许第二个位置参数。
- 一旦 public API 需要新增第二个以上可选项，应把第二个参数整体改为 options object，即使这在早期阶段是 breaking change。
- options object 的命名应稳定、可读，避免 `flag1`、`mode2` 这类无法长期维护的字段名。
- 测试 fake / fixture 应实现最小接口，不要把浏览器全局对象或服务端 API response 随意塞成 `any`。

### 10.3 Rust 规范

- 复用成熟 crate 与上游类型，避免复制一套语义接近的本地结构。SecurityDept 已导出的 OIDC / token-set / resource-server 类型应优先使用或薄包装。
- 错误类型使用 Snafu，错误消息不得泄露 secret、database URL、OIDC client secret、access token、refresh token。
- SeaORM entity / ActiveModel 是普通 CRUD 的默认边界；raw SQL 只保留在 migration / RLS DDL、`set_config/current_setting`、readiness probe 等 ORM 难以表达的小边界。
- 需要真实数据库或 OIDC provider 的覆盖应进入 `tests/integration/`，并优先通过 testcontainers 管理 Postgres / Dex。

### 10.4 Shell / YAML / 配置

- Bash 脚本使用 `set -e`，条件判断使用 `[[ ]]`，变量展开加引号。
- YAML 使用 2 空格缩进，只在必要时加引号。
- 项目命令通过 `just` 暴露，避免在文档中鼓励直接绕过 dotenv / workspace toolchain 的散乱命令。
- 不要因为某个 agent shell 没加载用户 `.zshrc` / profile，就把 `mise exec` 硬编码进用户面向的 just recipe。agent 自己需要时用 `mise exec --command "..."` 包裹执行。

### 10.5 迭代收尾

完整实现轮次收尾时，应先格式化，再验证健康状态。至少覆盖：

- 相关格式化 / lint fix。
- 相关 lint。
- 相关 TypeScript typecheck / Rust check。
- 相关 build。
- 相关 unit / integration tests。

如果某项验证因为外部依赖或环境限制无法运行，summary / review response 必须明确记录未运行项和原因。

---

## 11. 文档维护规则

任何以下变更，都必须同步 docs：

- 领域模型变化：`002-DOMAIN-MODEL.md`
- 新表或表语义变化
- 数据库变化：`003-DATABASE.md`
- 新 API 或 API 语义变化：`004-API.md`
- 同步行为变化：`005-SYNC.md`
- 浏览器能力边界变化：`006-BROWSER-ADAPTERS.md`
- Web UI 结构或交互语义变化：`007-WEB-UI.md`
- vault / auth / WebAuthn / token-set 语义变化：`008-SECURITY.md`
- 仓库结构、工程规范、迭代计划变化：`009-REPOSITORY-AND-DELIVERY.md`

README 与 AGENTS 只保留入口信息。细节统一下沉到 `docs/`，避免重复。

### 11.1 多语言文档规则

当前 amagi 的权威用户文档以中文为主，位于 `docs/zh/`。后续引入英文或更多语言时，继续采用与 SecurityDept 类似的多语言源文档模式，而不是随意增加平行文件：

- 用户面向文档才需要多语言；`AGENTS.md`、工具配置、机器读取 schema 不翻译。
- 目标结构为 `docs/zh/00x-TITLE.md` 与 `docs/en/00x-TITLE.md`，未来语言继续使用 `docs/{lang}/`。
- 同一篇文档的不同语言版本语义应等价，底部提供双向语言链接。
- 非英文文档链接到其它 docs 时，应优先链接同语言版本。
- 不要创建只复制或粗略摘要另一种语言的伪翻译文档；只有能长期维护语义等价时，才新增对应语言的 `00x-TITLE.md`。
- README 可以保留多语言入口；长期详细内容仍放在 `docs/`。

implementation iteration 文档统一命名：Guide 使用 `temp/IMPL_ITERn_GUIDE_zh.md`，Summary 使用 `temp/IMPL_ITERn_SUMMARY_zh.md`，Review 使用 `temp/IMPL_ITERn_REVIEWx.md`。同一 iteration 内 review 编号从 1 开始；review fix summary 直接追加到对应 review 文件，不单独新建 fix 文件。

---

## 12. 建议的首批任务单

### T1 初始化 monorepo 与目录结构

### T2 建立 Rust API server、配置、日志、健康检查

### T3 建立 PostgreSQL migrations：users/auth_users/oidc_account_bindings/devices/browser_clients/libraries/bookmark_nodes/bookmark_meta

### T4 实现 OIDC 登录与基础 session

### T5 实现 normal library CRUD API

### T6 实现 Dashboard Libraries 页面

### T7 实现 revisions/library_heads/cursors

### T8 实现 sync preview/apply API

### T9 迁移 WXT extension MVP

### T10 收敛 WebExtension adapter 并验证 Firefox 构建

补充约束：

- WXT 负责扩展宿主、manifest、entrypoint 与构建输出
- Chromium / Firefox / Safari 差异优先通过 WXT target、manifest version、entrypoint include-exclude 和运行期 feature detection 处理
- 长期不维护三套浏览器 adapter package；只保留 WXT/WebExtension adapter 与必要平台 override
- `sync-core` 不直接依赖 WXT

### T11 实现 sync profiles / rules UI 与 API

### T12 实现 vault library + unlock session + WebAuthn

---

## 13. 交付判断标准

一项功能可以视为"完成"，至少满足：

- 与对应 docs 一致
- 有最小测试或验证路径
- 错误路径可解释
- 不破坏云端真源模型
- 不破坏 vault 分层模型
- 不引入 undocumented behavior

---

## 13. 最终提醒

amagi 的真正难点不是普通 CRUD，而是：

- versioned tree
- policy-driven projection
- preview/apply sync
- mapping repair
- vault visibility boundary
- platform capability variance

因此实现时应优先保证这些基线成立，而不是优先堆叠表面功能。

---

## 14. 与其他文档关系

- 总览：`000-OVERVIEW.md`
- 架构：`001-ARCHITECTURE.md`
- 同步：`005-SYNC.md`
- 浏览器适配：`006-BROWSER-ADAPTERS.md`
- 安全：`008-SECURITY.md`

---

[English](../en/009-REPOSITORY-AND-DELIVERY.md) | [中文](009-REPOSITORY-AND-DELIVERY.md)
