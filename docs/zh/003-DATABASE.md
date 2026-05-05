# 003-DATABASE

## 1. 本文档目的

本文档给出 amagi 的推荐数据库模型。
它不是最终 SQL DDL，但应作为建表与迁移的设计基线。

领域定义见 `002-DOMAIN-MODEL.md`。
API 见 `004-API.md`。

---

## 2. 总体原则

### 2.1 PostgreSQL 为主库

首阶段所有核心状态都放在 PostgreSQL：

- 用户与设备
- library 与 nodes
- revisions
- policies
- cursors
- vault unlock
- audit

### 2.2 避免过度 JSON 化

核心实体字段应结构化建模。
`jsonb` 仅用于：

- 低频扩展字段
- capability 描述
- 变化 payload
- 审计上下文

### 2.3 逻辑删除与 revision 并存

节点实体保留 `is_deleted`
同时使用 `node_revisions` 记录事件化历史。

### 2.4 优先显式索引

同步、搜索、路径匹配、目标匹配都会依赖索引。
不要等性能出问题再补索引策略。

### 2.5 当前迁移实现状态

Iter2 已建立后端迁移 crate：

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

迁移实现基线：

- 使用 SeaORM / `sea-orm-migration` 2.0.0-rc.x 系列。
- 优先使用 `sea-orm-migration` 高阶 migration API。
- SeaQuery 作为 migration API 下的表达层和 predicate / expression builder；不要绕过 migration API 直接堆裸 SeaQuery 或 raw SQL 作为主实现。
- PostgreSQL 专属且 SeaQuery / sea-orm-migration 无法直接表达的 DDL 可以使用 raw SQL，但必须收敛到小 helper 中，并用 `DeriveIden` / SeaQuery 渲染出的 identifier 或 predicate 作为输入。
- 表名、列名、索引、foreign key 和测试应复用共享的 `#[derive(DeriveIden)]` 定义，例如 `defs.rs`。
- UUIDv7 PK、JSONB 默认值、数组默认值、索引构造、RLS policy 生成等跨 migration 复用逻辑应放入共享 helper 模块，例如 `schema.rs`、`helpers.rs`、`rls.rs`，不要留在单个 migration 文件里复制。
- migration crate 应保持 lib + bin 双用途：外部 CLI 可执行 `up` / `down`，`packages/amagi-db` 通过配置消费 `Migrator` 实现受控 `auto_migrate`；`apps/api-server` 不直接依赖 migration crate。

当前实现中，`schema.rs` 负责 UUIDv7 PK、JSONB / array default、索引 helper；`rls.rs` 负责 owner predicate 的 SeaQuery builder、predicate 渲染和 PostgreSQL RLS DDL 外壳。首条 migration 只保留核心表结构和 policy 组合，不再内嵌通用 helper。

首条迁移 `m20260504_000001_create_core_tables` 的目标覆盖本文第 3-8 节核心表，包括 `users`、`auth_users`、`oidc_account_bindings`、`devices`、`browser_clients`、`libraries`、`bookmark_nodes`、`bookmark_meta`、`library_heads`、`node_revisions`、`sync_cursors`、`node_client_mappings`、`sync_previews`、`sync_conflicts`、`sync_profiles`、`sync_profile_targets`、`sync_profile_rules`、`vault_unlock_sessions`、`audit_events`。

本轮没有建立以下后续对象：

- `webauthn_credentials`
- `vault_keys`
- closure table / materialized path 加速结构
- archive assets / `node_assets`
- FTS generated columns 与 FTS GIN index
- team sharing / ACL 表

Iter3 已新增数据库运行时 crate：

```text
packages/amagi-db/
	Cargo.toml
	src/
		error.rs
		health.rs
		lib.rs
		migrate.rs
		rls.rs
		runtime.rs
```

当前数据库运行时行为：

- API server 启动时通过 `amagi_db::DatabaseService::initialize()` 读取 typed `DatabaseConfig`。
- `database.url` 未配置时，服务以 no-db mode 启动；`/healthz` 返回 `database.state=not_configured`，`/readyz` 返回 `503`。
- `database.url` 已配置时，启动阶段建立 SeaORM `DatabaseConnection`。
- `database.auto_migrate=true` 时，`amagi-db` 通过 migration crate library `Migrator` 执行 `up`，不 shell out 到外部 bin。
- auto-migrate 或连接失败不会把 database URL 放进错误消息；API server 只暴露脱敏状态，例如 `connection_failed`、`migration_failed`。
- readiness 检查会执行 ping，并检查 `public.users` 是否存在，以证明首条 core migration 已应用。

配置加载、环境变量映射和 bool-like 解析规则以 `009-REPOSITORY-AND-DELIVERY.md` 为准。database URL 与 OIDC client secret 一样属于 secret，不能通过 `Debug`、诊断日志或错误消息明文输出。

### 2.6 ID 生成策略

PostgreSQL 18 提供原生 `uuidv7()`。服务端生成的稳定实体 ID 应使用 `uuid primary key default uuidv7()`，避免后续大规模主键默认值迁移，并改善 btree index locality。

以下字段不生成新 ID，因此不应加 `uuidv7()` 默认值：

- 共享主键 / 外键镜像，例如 `bookmark_meta.node_id`、`library_heads.library_id`
- composite primary key 成员，例如 `sync_cursors.browser_client_id`、`sync_cursors.library_id`
- 纯引用字段，例如 `device_id`、`library_id`、`user_id`

### 2.7 Row-level Security 基线

owner-scoped 表必须从初始迁移开始启用 PostgreSQL RLS。应用查询层仍要显式过滤 owner，但不能把业务层过滤作为唯一安全边界。

推荐约定：

- 每个请求事务设置 `amagi.current_user_id`
- 普通用户策略通过 `current_setting('amagi.current_user_id', true)::uuid` 匹配 owner 字段
- 未设置当前用户时默认不可见 / 不可写
- service/admin 维护路径必须有明确角色或连接池隔离，并在代码和文档中说明
- `oidc_account_bindings` 在 principal 尚未解析出 `user_id` 前，可额外使用 `amagi.auth_oidc_source`、`amagi.auth_oidc_subject` 与 `amagi.auth_oidc_identity_key` 进行 select-only lookup；该能力只服务于 auth binding repository / 后续 bearer principal resolution，不替代 owner-scoped RLS

当前运行时 helper 位于 `packages/amagi-db/src/rls.rs`：

- `CurrentUserId` 使用 typed `Uuid`，不接受空字符串或非 UUID。
- `set_current_user_id()` 在 `DatabaseTransaction` 内调用 `set_config('amagi.current_user_id', ..., true)`，使用 transaction-local 语义。
- `AuthLookupIdentity` / `set_auth_lookup_identity()` 在 principal 解析前设置 `amagi.auth_oidc_source`、`amagi.auth_oidc_subject` 与可选的 `amagi.auth_oidc_identity_key`，供 `oidc_account_bindings` 的 select-only lookup policy 使用。
- auth binding repository 的普通 CRUD 通过 SeaORM Entity / ActiveModel 执行；raw SQL 只保留在 `set_config/current_setting` helper、`SELECT 1` ping、`to_regclass(...)` readiness check 与 migration / policy DDL 这些 ORM 无法直接承载的边界。
- helper 只是数据库侧隔离契约；repository/query 仍必须显式附带 owner filter。

RLS 至少覆盖 owner-scoped 核心表：`users`、`auth_users`、`oidc_account_bindings`、`devices`、`browser_clients`、`libraries`、`bookmark_nodes`、`bookmark_meta`、`library_heads`、`node_revisions`、`sync_cursors`、`node_client_mappings`、`sync_previews`、`sync_conflicts`、`sync_profiles`、`sync_profile_targets`、`sync_profile_rules`、`vault_unlock_sessions`、`audit_events`。

RLS policy predicate 应尽量用 SeaQuery query / expression builder 生成，例如 owner column comparison、`EXISTS` 子查询和 join owner 推导。`ALTER TABLE ... ENABLE ROW LEVEL SECURITY`、`FORCE ROW LEVEL SECURITY`、`CREATE POLICY` 这类 PostgreSQL policy DDL 若必须使用 raw SQL，应只保留最小 DDL 外壳，避免把 predicate 本身写成脆弱的字符串拼接。

当前实现通过 SeaQuery 生成 owner comparison 和多层 `EXISTS` 子查询 predicate，再由 `rls.rs` 把渲染结果嵌入 PostgreSQL policy DDL；真实 PostgreSQL 18-alpine 验证结果要求保持：18 条 policy 创建成功、18 张 owner-scoped 表同时开启 `relrowsecurity` 与 `relforcerowsecurity`。

---

## 3. 身份与终端

### 3.1 `users`

建议字段：

- `id uuid primary key default uuidv7()`
- `email text null`
- `display_name text null`
- `status text not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

说明：

- `users` 表示书签管理领域 owner
- 不在 `users` 本表保存 OIDC `sub` 或其他外部 claim key
- OIDC 与用户的绑定通过 `oidc_account_bindings` 表表达

索引建议：

- index on `email`

### 3.2 `auth_users`

建议字段：

- `id uuid primary key default uuidv7()`
- `user_id uuid not null unique references users(id)`
- `status text not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

说明：

- `auth_users` 是认证层主体
- 初期可以与 `users` 一对一
- 仍应保留独立 ID，避免把认证账号与书签领域 owner 永久绑死

### 3.3 `oidc_account_bindings`

建议字段：

- `id uuid primary key default uuidv7()`
- `auth_user_id uuid not null references auth_users(id)`
- `user_id uuid not null references users(id)`
- `oidc_source text not null`
- `oidc_subject text not null`
- `oidc_identity_key text not null`
- `claims_snapshot_json jsonb not null default '{}'::jsonb`
- `last_seen_at timestamptz null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

约束建议：

- unique `(oidc_source, oidc_identity_key)`

索引建议：

- index on `(auth_user_id)`
- index on `(user_id)`
- index on `(oidc_source, oidc_subject)`

说明：

- `oidc_subject` 必须结构化保存，不依赖 `claims_snapshot_json` 反查协议主体
- `oidc_identity_key` 由 `oidc_identity_claim` 决定，因此不假定 OIDC 关联 key 一定是 `sub`
- 一个用户后续可以绑定多个 OIDC 来源或多个外部账号
- `claims_snapshot_json` 只保存必要快照用于审计、排障和显示辅助，不保存 raw token 或 client secret
- `claims_snapshot_json` 不是授权 lookup 的唯一依据
- repository lookup 路径当前按 `(oidc_source, oidc_identity_key)` 命中唯一 binding；同时保留按 `(oidc_source, oidc_subject)` 的结构化 lookup 能力，供后续 OAuth resource server principal resolution 使用
- 初次 lookup 使用独立 auth lookup session contract，binding 落地后恢复 owner-scoped `amagi.current_user_id`

### 3.4 `devices`

建议字段：

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `device_name text not null`
- `device_type text not null`
- `platform text not null`
- `trust_level text not null`
- `last_seen_at timestamptz null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

索引建议：

- index on `(user_id, platform)`
- index on `(user_id, device_type)`

### 3.5 `browser_clients`

建议字段：

- `id uuid primary key default uuidv7()`
- `device_id uuid not null references devices(id)`
- `browser_family text not null`
- `browser_profile_name text null`
- `extension_instance_id text not null`
- `capabilities_json jsonb not null default '{}'::jsonb`
- `last_seen_at timestamptz null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

约束建议：

- unique `(device_id, extension_instance_id)`

索引建议：

- index on `(device_id, browser_family)`

---

## 4. 收藏内容

### 4.1 `libraries`

建议字段：

- `id uuid primary key default uuidv7()`
- `owner_user_id uuid not null references users(id)`
- `kind text not null`
- `name text not null`
- `visibility_policy_id uuid null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

约束建议：

- `kind in ('normal', 'vault')`

索引建议：

- index on `(owner_user_id, kind)`

### 4.2 `bookmark_nodes`

建议字段：

- `id uuid primary key default uuidv7()`
- `library_id uuid not null references libraries(id)`
- `node_type text not null`
- `parent_id uuid null references bookmark_nodes(id)`
- `sort_key text not null`
- `title text not null`
- `url text null`
- `url_normalized text null`
- `content_hash text null`
- `is_deleted boolean not null default false`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

约束建议：

- `node_type in ('folder', 'bookmark', 'separator')`
- `url is not null` only when `node_type='bookmark'`
- root folder 约定应在 domain 层控制

索引建议：

- index on `(library_id, parent_id, is_deleted)`
- index on `(library_id, url_normalized)`
- index on `(library_id, node_type, is_deleted)`

说明：

- 若需要路径加速，可另建 closure table 或 materialized path 字段
- 首阶段不必提前引入 closure table，除非规则匹配路径成为性能瓶颈

### 4.3 `bookmark_meta`

建议字段：

- `node_id uuid primary key references bookmark_nodes(id)`
- `description text null`
- `tags text[] not null default '{}'`
- `canonical_url text null`
- `page_title text null`
- `favicon_asset_id uuid null`
- `reading_state text null`
- `starred boolean not null default false`
- `extra_json jsonb not null default '{}'::jsonb`
- `updated_at timestamptz not null`

索引建议：

- gin index on `tags`
- index on `starred`

---

## 5. 版本与同步

### 5.1 `library_heads`

建议字段：

- `library_id uuid primary key references libraries(id)`
- `current_revision_clock bigint not null`
- `updated_at timestamptz not null`

### 5.2 `node_revisions`

建议字段：

- `rev_id uuid primary key default uuidv7()`
- `library_id uuid not null references libraries(id)`
- `node_id uuid not null references bookmark_nodes(id)`
- `actor_type text not null`
- `actor_id uuid null`
- `op_type text not null`
- `payload_json jsonb not null`
- `logical_clock bigint not null`
- `created_at timestamptz not null`

约束建议：

- unique `(library_id, logical_clock)`

### 5.3 当前实现状态（Iter6）

`packages/amagi-db/src/entities/` 当前已为内容与 revision 表补齐 SeaORM entity / ActiveModel：

- `libraries`
- `bookmark_nodes`
- `bookmark_meta`
- `library_heads`
- `node_revisions`

`packages/amagi-bookmarks` 的普通 CRUD 使用这些 Entity / ActiveModel / query builder 完成。当前唯一的 bookmark 领域 raw SQL 边界是 repository 内部 `next_library_clock(...)`，用于在单 transaction 内原子执行 `UPDATE library_heads ... RETURNING current_revision_clock`；该 helper 不扩散到 service 或 API route。

每个 owner-scoped bookmark service 方法都会在 transaction 内调用 `set_current_user_id(...)` 设置 `amagi.current_user_id`。service 层仍显式按 owner / library 校验可见性，PostgreSQL RLS policy 作为数据库兜底边界。

索引建议：

- index on `(library_id, logical_clock)`
- index on `(node_id, logical_clock)`
- index on `(actor_type, actor_id)`

说明：

- `payload_json` 用来表达 update/move/delete 等增量信息
- 可在后续引入 outbox 表，但首阶段可直接使用 `node_revisions` 作为 feed 来源

### 5.3 `sync_cursors`

建议字段：

- `browser_client_id uuid not null references browser_clients(id)`
- `library_id uuid not null references libraries(id)`
- `last_applied_clock bigint not null`
- `last_ack_rev_id uuid null`
- `last_sync_at timestamptz null`
- `updated_at timestamptz not null`

主键建议：

- primary key `(browser_client_id, library_id)`

### 5.4 `node_client_mappings`

建议字段：

- `browser_client_id uuid not null references browser_clients(id)`
- `server_node_id uuid not null references bookmark_nodes(id)`
- `client_external_id text not null`
- `last_seen_hash text null`
- `updated_at timestamptz not null`

主键建议：

- primary key `(browser_client_id, server_node_id)`

额外唯一约束建议：

- unique `(browser_client_id, client_external_id)`

说明：

- 本表非常关键
- 不允许直接拿本地 node id 代替服务端 node id

### 5.5 `sync_previews`

当前 Iter7 backend baseline 已落库。

字段：

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `browser_client_id uuid not null references browser_clients(id)`
- `library_id uuid not null references libraries(id)`
- `base_clock bigint not null`
- `to_clock bigint not null`
- `status text not null`
- `request_hash text not null`
- `summary_json jsonb not null default '{}'::jsonb`
- `server_ops_json jsonb not null default '[]'::jsonb`
- `accepted_local_mutations_json jsonb not null default '[]'::jsonb`
- `conflicts_json jsonb not null default '[]'::jsonb`
- `expires_at timestamptz not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`
- `applied_at timestamptz null`

约束与索引：

- `status in ('pending', 'applied', 'expired', 'conflicted')`
- index on `(user_id, created_at desc)`
- index on `(browser_client_id, library_id, status)`
- owner-scoped RLS：`user_id = current_user_id`
- `updated_at` 由共享 PostgreSQL auto-update trigger 维护

说明：

- preview record 是 apply 的唯一输入来源。
- `summary_json` 还会在 apply 成功后嵌入幂等回放所需的 `applyResult`。
- 预览默认有效期 10 分钟；过期后 apply 返回 `preview_expired`。

### 5.6 `sync_conflicts`

建议字段：

- `id uuid primary key default uuidv7()`
- `browser_client_id uuid not null references browser_clients(id)`
- `library_id uuid not null references libraries(id)`
- `conflict_type text not null`
- `state text not null`
- `summary text not null`
- `details_json jsonb not null`
- `created_at timestamptz not null`
- `resolved_at timestamptz null`
- `resolved_by uuid null references users(id)`

索引建议：

- index on `(browser_client_id, state)`
- index on `(library_id, state)`

---

## 6. 策略

### 6.1 `sync_profiles`

建议字段：

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `name text not null`
- `mode text not null`
- `default_direction text not null`
- `conflict_policy text not null`
- `enabled boolean not null default true`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

约束建议：

- `mode in ('manual', 'scheduled', 'auto')`

### 6.2 `sync_profile_targets`

建议字段：

- `id uuid primary key default uuidv7()`
- `profile_id uuid not null references sync_profiles(id)`
- `platform text null`
- `device_type text null`
- `device_id uuid null references devices(id)`
- `browser_family text null`
- `browser_client_id uuid null references browser_clients(id)`
- `created_at timestamptz not null`

说明：

- 一条 target selector 可是宽匹配，也可是精确匹配
- 匹配优先级与冲突处理应在 policy 层定义

### 6.3 `sync_profile_rules`

建议字段：

- `id uuid primary key default uuidv7()`
- `profile_id uuid not null references sync_profiles(id)`
- `rule_order integer not null`
- `action text not null`
- `matcher_type text not null`
- `matcher_value text not null`
- `options_json jsonb not null default '{}'::jsonb`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

约束建议：

- `action in ('include', 'exclude', 'readonly')`

索引建议：

- index on `(profile_id, rule_order)`

---

## 7. Vault 与安全

### 7.1 `vault_unlock_sessions`

建议字段：

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `library_id uuid not null references libraries(id)`
- `auth_context_json jsonb not null`
- `acr text null`
- `amr text[] not null default '{}'`
- `expires_at timestamptz not null`
- `created_at timestamptz not null`
- `revoked_at timestamptz null`

索引建议：

- index on `(user_id, library_id, expires_at)`
- partial index on active sessions if needed

### 7.2 `webauthn_credentials`

若首阶段直接接入 WebAuthn，可建：

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `credential_id bytea not null unique`
- `public_key bytea not null`
- `sign_count bigint not null`
- `transports text[] not null default '{}'`
- `aaguid uuid null`
- `nickname text null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

### 7.3 可选 `vault_keys`

只有当进入内容加密阶段才需要：

- `library_id uuid primary key references libraries(id)`
- `kek_wrapped_dek bytea not null`
- `kek_source text not null`
- `rotation_version integer not null`
- `updated_at timestamptz not null`

首阶段可以不建。

---

## 8. 审计

### 8.1 `audit_events`

建议字段：

- `id uuid primary key default uuidv7()`
- `user_id uuid null references users(id)`
- `device_id uuid null references devices(id)`
- `browser_client_id uuid null references browser_clients(id)`
- `library_id uuid null references libraries(id)`
- `event_type text not null`
- `payload_json jsonb not null`
- `created_at timestamptz not null`

索引建议：

- index on `(user_id, created_at desc)`
- index on `(library_id, created_at desc)`
- index on `(event_type, created_at desc)`

---

## 9. 搜索与归档（首阶段最小化）

### 9.1 PostgreSQL FTS

首阶段可在 `bookmark_nodes.title`、`bookmark_meta.description`、`bookmark_meta.page_title` 上建立全文搜索支持。

可选做法：

- 生成 `tsvector` 列
- GIN index

### 9.2 archive assets

若做 favicon/page snapshot，可额外建：

- `assets`
- `node_assets`

但不是首阶段必须。

---

## 10. 迁移策略建议

### 10.1 先建核心表

第一批迁移建议顺序：

1. users
2. auth_users
3. oidc_account_bindings
4. devices
5. browser_clients
6. libraries
7. bookmark_nodes
8. bookmark_meta
9. library_heads
10. node_revisions
11. sync_cursors
12. node_client_mappings
13. sync_previews
14. sync_conflicts
15. sync_profiles
16. sync_profile_targets
17. sync_profile_rules
18. vault_unlock_sessions
19. audit_events

目标实现中，以上表由 `packages/amagi-db-migration/src/m20260504_000001_create_core_tables.rs` 的 `up` 创建，并由 `down` 按依赖反序回滚。执行方式：

```sh
DATABASE_URL=postgres://amagi:<redacted>@localhost:5432/amagi cargo run -p amagi-db-migration -- up
DATABASE_URL=postgres://amagi:<redacted>@localhost:5432/amagi cargo run -p amagi-db-migration -- down
```

本地验证必须使用 `just dev-deps` 提供的 PostgreSQL 18-alpine 开发数据库真实执行 `up` / `down`，并检查关键约束、`uuidv7()` default 与 RLS policy 状态。

### 10.2 保守扩展

以下能力建议后续迁移追加，而不是一开始预埋过深：

- 加密 key 表
- closure table
- archive 资产表
- 更复杂共享权限表

---

## 11. 数据完整性约束建议

### 11.1 application-level invariants

以下最好由应用层同时保证：

- 一个 library 存在逻辑 root
- move 不形成循环
- folder 不能挂在 bookmark 下
- vault library 不参与普通 profile projection
- logical_clock 单调递增

### 11.2 database-level safety

以下建议由数据库直接约束：

- foreign keys
- unique constraints
- enum-like check
- non-null 主字段
- cursor 主键唯一
- mapping 唯一

---

## 12. 与其他文档的关系

- 领域含义：`002-DOMAIN-MODEL.md`
- API 请求/响应：`004-API.md`
- 同步读写流程：`005-SYNC.md`
- vault/step-up 安全约束：`008-SECURITY.md`

---

[English](../en/003-DATABASE.md) | [中文](003-DATABASE.md)
