# 008-SECURITY

## 1. 本文档目的

本文档定义 amagi 的认证、授权、vault 访问控制、step-up auth 与 WebAuthn 相关设计。

它覆盖：

- OIDC 登录
- base session
- step-up auth
- vault unlock
- WebAuthn / passkey
- 基本审计要求

相关文档：

- 架构：`001-ARCHITECTURE.md`
- 领域模型：`002-DOMAIN-MODEL.md`
- API：`004-API.md`

---

## 2. 安全目标

### 2.1 目标

- 使用自定义 OIDC 作为主登录
- 支持基于 session 的 Dashboard 访问
- 支持浏览器扩展受控访问
- 支持 vault 的二次解锁
- 支持 WebAuthn / passkey 作为强认证手段
- 审计关键安全事件

### 2.2 非目标（首阶段）

- 完整端到端加密收藏系统
- 隐身浏览器端零明文缓存架构
- 企业级多租户复杂 IAM

---

## 3. 认证模型

### 3.1 Base Login

用户通过 OIDC 登录，获得基础认证身份与 amagi 侧会话状态。OIDC / token-set 基础设施采用 SecurityDept crates；amagi 不自研 OIDC protocol client。

基础认证身份、amagi 认证主体和书签管理领域 owner 是不同概念。初期可以一对一映射，但数据库和授权模型必须保留独立 ID 与绑定表。

基础会话可访问：

- normal libraries
- devices
- profiles
- search（不含 vault）
- 基础同步管理页

基础会话不自动授予 vault 访问权。

对 Dashboard、extension popup、side panel、background sync API 这类跨宿主入口，主认证形态应以 SecurityDept `token-set-context` 的 `backend-oidc` mode 为基线，而不是依赖只适合同域 Web app 的 cookie session。

### 3.2 Step-up Auth

当用户访问高敏感操作时，需要更强认证上下文。
典型场景：

- 解锁 vault
- 查看 vault 搜索结果
- 执行敏感安全设置修改

step-up 可通过：

- 重新完成更高 ACR 的 OIDC 认证
- WebAuthn assertion

### 3.3 Unlock Session

step-up 成功后，系统发放短期 unlock session。
unlock session 作用域至少包括：

- `user_id`
- `library_id`
- `expires_at`

unlock session 仅影响 vault 可见性，不应替代普通登录 session。

---

## 4. OIDC 设计

### 4.1 角色

amagi 作为 OIDC Relying Party。

Rust 集成入口使用 `securitydept-core`，打开 `token-set-context` 与 backend-oidc 所需 feature，并通过其 re-export 的 SecurityDept 产品面组合实现。amagi 不复制 SecurityDept 的 OIDC client、pending OAuth state、token exchange、refresh 或 userinfo 逻辑。

实现前必须阅读并对齐 SecurityDept 的 auth context / mode 文档与 `apps/server/src/config.rs` 参考实现，尤其是：

- OIDC provider shared defaults
- OIDC client union config
- backend-oidc override
- frontend-oidc override
- OAuth resource server / access-token substrate

OIDC client 与 OAuth resource server 是不同职责。前者负责登录、callback、token exchange、refresh、userinfo 与 token-set state；后者负责 API bearer 验证、audience / issuer / JWKS / introspection / propagation 等 resource-server 语义。amagi 配置和运行时不得把这两类配置合并成一个简化 `oidc` block。

SecurityDept 已经公开的 config source、resolved config、override config、token-set mode 与 access-token substrate 类型应优先直接复用，或通过薄 newtype / adapter 包装。不要在 amagi 中重新定义一整套字段相同或语义相近的本地类型；只有 amagi 独有的配置，如 facade path、browser client binding、vault unlock policy、audit policy，才应由 amagi 自己拥有。

当 amagi 需要固定或禁止部分 SecurityDept 配置字段时，裁决如下：

- 不使用 `serde_json::Value` / `json::Value` 作为 OIDC / token-set 主配置通道。OIDC、token-set、resource-server 配置必须保持 typed model，并优先复用 SecurityDept 导出的 config source / override / resolved 类型。
- amagi 通过自有 wrapper 表达 source key、应用 facade、browser client binding、vault unlock 与 audit policy；wrapper 在 compose / validate 阶段把 SecurityDept 类型解析为 amagi runtime。
- 以下路径由 amagi 根据 `source_key` 计算，不应作为用户可配置项暴露：
  - backend-oidc `redirect_path`: `/api/auth/token-set/oidc/source/{source}/callback`
  - frontend-oidc `redirect_path`: `/auth/token-set/oidc/source/{source}/callback`
  - frontend-oidc `config_projection_path`: `/api/auth/token-set/oidc/source/{source}/config`
- 如果配置文件显式设置上述固定路径，配置校验必须报错，而不是静默覆盖；只有为了兼容旧配置的短期迁移窗口，才允许 warning + override，并且必须在 release note / review 文档中标明移除计划。
- `token_propagation` 在 amagi 中禁用。amagi 不是 SecurityDept mesh 场景，任何显式启用 token propagation 的配置都必须作为安全边界错误拒绝，不能 warning 后覆盖。
- `serde_json::Value` 仅可用于明确的扩展 metadata、原始 claim snapshot 或低频调试载荷，不能承载认证协议主配置、固定路径绕过或被禁用的安全能力。

### 4.2 登录流程

推荐：

- Authorization Code Flow
- PKCE

这些协议细节最终仍由 SecurityDept `securitydept-oidc-client` 与 `securitydept-token-set-context::backend_oidc_mode` 承担。Iter4 当前仅建立应用级 auth facade：`/api/auth/token-set/oidc/source/{source}/start` 与 callback 路径会校验 source、返回 typed placeholder、生成 skeleton audit payload，但不会执行 authorization-code flow、不会签发 session，也不会在 callback 中伪造 account binding 成功。

Iter5 当前基线已经把 placeholder 推进到真实 runtime/service integration：

- `GET /api/auth/token-set/oidc/source/{source}/start` -> SecurityDept backend-oidc login / authorize
- `GET /api/auth/token-set/oidc/source/{source}/callback` -> fragment redirect callback
- `POST /api/auth/token-set/oidc/source/{source}/callback` -> JSON body callback
- `POST /api/auth/token-set/oidc/source/{source}/refresh` -> refresh body return
- `POST /api/auth/token-set/oidc/source/{source}/metadata/redeem` -> metadata redemption
- `POST /api/auth/token-set/oidc/source/{source}/user-info` -> verified user-info + amagi principal resolution baseline

frontend callback path `/auth/token-set/oidc/source/{source}/callback` 仍保留为 frontend app shell 消费的 typed path，不与 backend callback 混用。

认证协议 endpoint 不使用 `/api/v1` 前缀。OIDC、token-set、WebAuthn / authenticator 这类路径由明确协议和安全流程约束，语义不应随 bookmark / sync 等业务 API 版本演进而变化。`/api/v1` 保留给业务资源 API 或 auth-adjacent 业务操作，例如 vault unlock。

amagi 必须支持多个 OIDC 来源。配置结构应使用 map-like shape，以稳定 provider key 为 map key，例如 `oidc_sources.<source_key>`；不要用数组表达 provider 列表，因为数组难以通过 Figment 按 key 局部合并。`oidc_source` 必须贯穿 facade route、pending state、callback、account binding 与 audit。

当前实现的顶层 auth config 入口位于 `packages/amagi-config`，采用 `default_oidc_source` 与 `oidc_sources.<source_key>`。每个 source 直接对齐 SecurityDept typed config，区分 `oidc`、`backend_oidc`、`frontend_oidc` 与 `access_token_substrate`；`token_set.facade_paths`、token-set storage policy、browser client binding 仍保留在 amagi 应用层。secret 字段已切到 SecurityDept 上游 `SecretString`，不再维护 amagi 自己的 redacted secret wrapper。

SecurityDept 适配入口位于 `packages/amagi-securitydept`。当前实现已经切到 SecurityDept `0.3.x` 的 typed config / resolved config / runtime/service 边界，`packages/amagi-securitydept` 只保留 amagi host 自己拥有的 source-key 与 fixed path metadata，不再维护一套与 SecurityDept resolved config 几乎同构的 mirror projection。协议 truth 以 SecurityDept resolved config 为准；amagi 只在其外层附加 route construction、account binding、principal 与 audit 语义。

固定路径字段不再出现在 config schema 或 example 中。配置文件如果写入 `redirect_url` 或 `config_projection_path`，会在 amagi config 校验或 SecurityDept fixed-redirect validator 阶段失败；运行时始终计算：

- backend-oidc callback: `/api/auth/token-set/oidc/source/{source}/callback`
- frontend-oidc callback: `/auth/token-set/oidc/source/{source}/callback`
- frontend config projection: `/api/auth/token-set/oidc/source/{source}/config`
- token-set OIDC start facade: `/api/auth/token-set/oidc/source/{source}/start`

后续如果增加 Dashboard cookie/session OIDC flow，应使用单独命名空间，例如 `/api/auth/session/oidc/source/{source}/start`，不得复用 token-set 路径。

`token_propagation` 在 `packages/amagi-config` 校验阶段保持禁用：未配置或显式 false 均不会开启 forwarding，任何 forwarding flag 为 true 的配置都会作为配置错误拒绝。

### 4.3 会话绑定

服务端维护自身 session，不直接把第三方 token 当内部权限载体的唯一来源。

amagi 拥有 token-set state 接收、存储策略、extension/browser client session binding、OIDC account binding、auth user / domain user lookup、domain authorization、vault unlock session 与 audit event writing。SecurityDept 的 token-set authenticated principal 只表示基础认证身份，不等于 vault access。

`packages/amagi-auth` 承接 amagi 应用侧 auth facade、frontend config projection、`ExternalOidcIdentity` / `AmagiPrincipal`、OIDC account binding repository、verified-claims principal resolution 与 bearer principal lookup baseline。`packages/amagi-securitydept` 继续只负责 SecurityDept typed config / resolved config / runtime projection，不承接 amagi account binding、principal 或 audit 语义。

cookie session 可以作为 Dashboard 同域 UX 的后续可选补充，但不得成为 extension sync API 的基础假设。

### 4.4 OIDC 绑定模型

OIDC 登录结果应绑定到 `oidc_account_bindings`，而不是直接写入领域 `users` 表。

绑定约束：

- `oidc_source`
- `oidc_subject`
- `oidc_identity_key`
- unique `(oidc_source, oidc_identity_key)`
- `auth_user_id`
- `user_id`
- `claims_snapshot_json`

说明：

- `oidc_subject` 是协议主体标识，必须始终结构化保存
- `oidc_identity_key` 由 `oidc_identity_claim` 决定，因此不固定等于 `sub`
- `oidc_identity_claim` 采用 typed config，默认值为 `sub`，并为 `email`、`name`、`preferred_username` 与自定义 claim 预留扩展结构
- 不假定一个 amagi 用户只能绑定一个 OIDC 来源
- 不保存 raw token、client secret 或 refresh token 到 claim snapshot
- claim snapshot 可包含必要的 `email`、`name`、`acr`、`amr` 等审计/显示信息，但不是授权 lookup 的唯一依据

resource-server / bearer principal baseline 也遵循这一边界：bearer token 验证由 SecurityDept access-token substrate 承担；amagi 只消费 verified bearer principal facts，并按 `(oidc_source, oidc_subject)` 查找已有 account binding。不得通过 `claims_snapshot_json` 反查 principal，也不得把 `oidc_identity_key` 当作 bearer lookup key。

### 4.5 会话升级

若 OIDC Provider 支持更高认证上下文，可在 vault unlock 时触发 step-up 登录。

---

## 5. WebAuthn / Passkey

### 5.1 用途

WebAuthn 主要用于：

- vault 二次解锁
- 高敏感设置确认

### 5.2 注册

用户在已登录状态下可注册 passkey。

### 5.3 断言

vault unlock 时可要求 WebAuthn assertion。

### 5.4 设备记忆

首阶段建议保守：

- 不默认“记住设备长期免二次解锁”
- 若要支持，也应有较短 TTL 与显式风险说明

---

## 6. Vault 访问控制

### 6.1 默认不可见

vault 默认不出现在：

- 普通 tree 浏览
- 普通搜索结果
- 普通 sync feed
- 普通扩展本地缓存

### 6.2 访问条件

访问某 vault library 至少要求：

- 用户已登录
- 对应 library 的 unlock session 有效

### 6.3 unlock TTL

建议可配置，例如：

- 默认 5~30 分钟
- 过期后需重新 step-up

### 6.4 主动锁定

应支持用户主动 lock 当前 vault。

---

## 7. 授权模型

### 7.1 首阶段

首阶段可采用 owner-only 模型：

- 用户仅访问自己的 libraries / devices / profiles
- PostgreSQL RLS 同步启用，使用 `amagi.current_user_id` 这类 session variable 作为数据库侧 owner 隔离契约
- API repository/query 仍需显式过滤 owner，RLS 是兜底安全边界，不是替代清晰业务条件

### 7.2 后续扩展

若支持共享，可增加：

- viewer
- editor
- admin

但不应影响 vault 基础模型。

---

## 8. 扩展侧安全

### 8.1 浏览器扩展会话

扩展需要自己的受控会话，与 Dashboard session 关联但不完全等同。

扩展认证基线应优先使用 SecurityDept `token-set-context` 的 `backend-oidc` mode 产物，并由 amagi 绑定到具体 browser client / extension instance。不要把 Dashboard cookie session 当作扩展后台同步 API 的隐含凭据。

当前实现中，Dashboard 与 extension 共享 `packages/amagi-auth-client` 作为前端 auth boundary；extension 通过 `browser.storage.local` 适配 SecurityDept record store，让 popup / options / background 共享同一份 token-set state。

### 8.2 本地存储最小化

不要在普通扩展本地状态中持久化 vault 内容。
不要长期保存高敏感 unlock 状态。

### 8.3 capability 报告

扩展注册时应报告能力，但服务端不能仅信任自报。
对于高风险动作仍需服务端 policy 限制。

---

## 9. 搜索与可见性

### 9.1 普通搜索

默认只搜索 normal libraries。

### 9.2 vault 搜索

只有在 unlock 有效时才能包含 vault 内容。

### 9.3 审计

应记录：

- 用户何时解锁了哪个 vault
- 解锁持续多久
- 是否主动锁定
- 是否解锁失败

---

## 10. 审计事件建议

至少记录：

- user_login
- user_logout
- step_up_started
- step_up_succeeded
- step_up_failed
- webauthn_registered
- webauthn_asserted
- vault_unlock_succeeded
- vault_unlock_failed
- vault_locked
- profile_changed
- sync_apply

---

## 11. 未来加密扩展

### 11.1 首阶段建议

先做服务端控制的 vault 可见性与二次解锁，不做端到端加密。

### 11.2 后续可演进方向

若未来需要更强隐私，可引入：

- vault 内容加密存储
- 用户侧密钥包装
- 本地解密渲染
- 有限制的搜索能力

但这将显著增加复杂度，不应影响首阶段推进。

---

## 12. 安全边界总结

必须坚持：

1. base session != vault access
2. unlock session 是短期且可撤销的
3. vault 不进入普通 sync feed
4. vault 不进入普通扩展缓存
5. 所有关键安全行为都有审计
6. OIDC / token-set protocol infrastructure 采用 SecurityDept，amagi 不在业务 handler 内自研 OAuth/OIDC flow
7. SecurityDept token-set authenticated principal 不自动授予 vault library 访问权

---

## 13. 与其他文档关系

- API：`004-API.md`
- 同步：`005-SYNC.md`
- 浏览器适配：`006-BROWSER-ADAPTERS.md`

---

[English](../en/008-SECURITY.md) | [中文](008-SECURITY.md)
