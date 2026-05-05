# 002-DOMAIN-MODEL

## 1. 本文档目的

本文档定义 amagi 的核心领域模型，包括：

- 用户、设备、浏览器客户端
- library、node、metadata
- policy 与 sync profile
- revision、cursor、conflict
- vault 与 unlock session

数据库落地见 `003-DATABASE.md`。
API 表达见 `004-API.md`。

---

## 2. 领域视图

amagi 领域可以分为六个子域：

1. identity
2. content
3. sync
4. policy
5. vault
6. audit

---

## 3. Identity 子域

### 3.1 User

表示 amagi 书签管理领域中的一个 owner。

`User` 是收藏、设备、sync profile、vault unlock 等领域对象的归属主体。它不等同于 OIDC provider 的账号，也不直接保存 OIDC claim key。

关键属性：

- `user_id`
- `email`
- `display_name`
- `created_at`
- `status`

职责：

- 拥有 library
- 拥有设备
- 拥有 sync profile
- 发起 vault 解锁
- 对自己的数据做 CRUD 与同步

### 3.2 AuthUser

表示认证层面的 amagi 账号主体。

说明：

- 初期可以与 `User` 一对一
- 仍应拥有独立稳定 ID
- 不应假定一个认证主体只能绑定一个 OIDC 来源
- 不应把 OIDC claim 直接塞进领域 `User`

### 3.3 OidcAccountBinding

表示外部 OIDC 账号与 amagi 认证主体 / 领域 owner 的绑定关系。

关键属性：

- `binding_id`
- `auth_user_id`
- `user_id`
- `oidc_source`
- `oidc_subject`
- `oidc_identity_key`
- `claims_snapshot_json`
- `last_seen_at`

约束：

- `(oidc_source, oidc_identity_key)` 必须唯一
- `oidc_subject` 必须始终结构化保存，供后续 bearer token principal resolution 使用
- `oidc_identity_key` 由 `oidc_identity_claim` 决定，因此不固定等于 OIDC `sub`
- 后续可支持一个用户绑定多个 OIDC 来源或多个外部账号
- `claims_snapshot_json` 只用于审计、排障和显示辅助，不作为授权唯一依据

应用认证主体 `AmagiPrincipal` 由 `ExternalOidcIdentity` + `AuthUser` + `User` 解析得到。它表示 amagi 应用层的已绑定 principal，而不是 SecurityDept 的基础 authenticated principal 别名；vault access 仍需独立 unlock / authorization 决策。

### 3.4 Device

表示一个物理或逻辑终端。

关键属性：

- `device_id`
- `user_id`
- `device_name`
- `device_type`
- `platform`
- `trust_level`
- `last_seen_at`

说明：

- 一个设备下可有多个浏览器客户端
- 设备本身是 policy 匹配维度之一

### 3.5 BrowserClient

表示一个具体浏览器实例或扩展实例。

关键属性：

- `browser_client_id`
- `device_id`
- `browser_family`
- `browser_profile_name`
- `extension_instance_id`
- `capabilities`

说明：

- 同步 target 以 BrowserClient 为主要粒度
- Device 与 BrowserClient 都可参与规则匹配

---

## 4. Content 子域

### 4.1 Library

表示一个逻辑收藏空间。

核心属性：

- `library_id`
- `owner_user_id`
- `kind`
- `name`
- `visibility_policy_id`

其中 `kind` 至少包括：

- `normal`
- `vault`

约束：

- `normal` library 可参与普通同步
- `vault` library 默认不参与普通同步，访问需 unlock

### 4.2 Node

表示收藏树中的一个节点。

节点类型：

- `folder`
- `bookmark`
- `separator`

核心属性：

- `node_id`
- `library_id`
- `parent_id`
- `node_type`
- `title`
- `sort_key`
- `is_deleted`
- `created_at`
- `updated_at`

补充：

- `bookmark` 节点具有 `url`
- `folder` 节点可包含子节点
- `separator` 节点无 URL

### 4.3 BookmarkMeta

表示附着于 bookmark 的富元数据。

可包含：

- `description`
- `tags`
- `canonical_url`
- `page_title`
- `favicon_asset_id`
- `reading_state`
- `starred`
- `extra_json`

说明：

- 元数据不应与核心节点表过度耦合
- 首阶段不要求完整爬取与归档

### 4.4 当前实现状态（Iter6）

当前 bookmark 领域实现位于 `packages/amagi-bookmarks`。本阶段已落地 normal library / node / revision 的第一条后端纵切片：

- 只允许创建 `kind=normal` library；`kind=vault` 返回 `vault_not_supported_in_iter6`，不会降级创建普通 library。
- 创建 library 时会创建 root folder node，`parent_id=null`、`node_type=folder`、`sort_key=root`，并写入初始 revision。
- Dashboard tree response 使用 flat adjacency list；UI / sync adapter 后续自行组树。
- `bookmark` 必须提供非空 URL；`folder` 和 `separator` 不接受 URL。当前 URL normalize baseline 仅保证 trim 和空值拒绝，scheme/host 小写化留到后续 normalize 专项。
- delete 是 logical delete，只设置 `is_deleted=true`；restore 只恢复目标 node 自身，不递归恢复子树。
- root node 不允许通过业务 API 更新、移动或删除。
- 普通 create node 必须指定同 library 内未删除的 folder parent；移动时禁止移动到自身或 descendant 下。

---

## 5. Tree 模型约束

### 5.1 单 parent 树结构

每个 node 仅有一个 `parent_id`。
不支持多父引用。

### 5.2 稳定排序

同级节点顺序以 `sort_key` 表达，而不是依赖插入时间。
这样更适合同步与重排。

### 5.3 逻辑删除

节点删除采用逻辑删除，并保留 tombstone。
理由：

- 支持增量同步
- 支持冲突恢复
- 支持多客户端校正

### 5.4 路径不是主键

folder path 只是一种显示/匹配语义，不应作为实体主标识。

---

## 6. Policy 子域

### 6.1 SyncProfile

定义某类同步行为的配置集合。

关键属性：

- `profile_id`
- `user_id`
- `name`
- `mode`
- `default_direction`
- `conflict_policy`
- `enabled`

其中 `mode` 至少包括：

- `manual`
- `scheduled`
- `auto`

首阶段默认推荐 `manual`。

### 6.2 SyncTargetSelector

定义 profile 面向哪些目标。

可按以下维度匹配：

- platform
- device_type
- device_id
- browser_family
- browser_client_id

### 6.3 SyncRule

定义对内容的 include / exclude / readonly 裁剪规则。

关键属性：

- `rule_order`
- `action`
- `matcher_type`
- `matcher_value`
- `options`

`action` 至少包括：

- `include`
- `exclude`
- `readonly`

`matcher_type` 可包括：

- `folder_id`
- `folder_path`
- `library_kind`
- `tag`

说明：

- profile 对 target 生效
- rule 对 content 生效
- 二者组合形成 projection

---

## 7. Sync 子域

### 7.1 Revision

表示服务端有序变更事件。

关键属性：

- `rev_id`
- `library_id`
- `node_id`
- `actor_type`
- `actor_id`
- `op_type`
- `payload`
- `logical_clock`
- `created_at`

说明：

- revision 是同步增量的基础
- 不是所有 UI 事件都要一一暴露给用户，但必须可用于调试与同步
- Iter6 中每个 bookmark tree mutation 都写入 `node_revisions`，`actor_type=user`，`actor_id` 为已绑定 amagi `user_id`。

### 7.2 LibraryHead

表示某个 library 当前的全局逻辑时钟。

关键属性：

- `library_id`
- `current_revision_clock`

当前实现通过同一 transaction 内的 `library_heads.current_revision_clock = current_revision_clock + 1 ... RETURNING` 推进 clock，随后用推进后的 clock 写入 `node_revisions.logical_clock`。`library.create` 初始推进到 `1`。

### 7.3 SyncCursor

表示某 BrowserClient 对某 Library 已同步到的位置。

关键属性：

- `browser_client_id`
- `library_id`
- `last_applied_clock`
- `last_ack_rev_id`
- `last_sync_at`

### 7.4 Client Mapping

表示服务端节点与客户端本地节点之间的映射关系。

关键属性：

- `browser_client_id`
- `server_node_id`
- `client_external_id`
- `last_seen_hash`

说明：

- 浏览器本地 node id 只在本客户端上下文中有效
- 必须通过单独映射表桥接

### 7.5 Conflict

表示 push / merge / apply 中出现的不一致。

冲突类型示例：

- concurrent update
- move to deleted parent
- delete vs update
- duplicate normalized URL candidate
- local apply blocked

Conflict 不是异常日志，而是显式领域对象，必须可展示、可处理。

---

## 8. Vault 子域

### 8.1 VaultLibrary

本质上是 `kind=vault` 的 library，但在语义上应单独看待。

特点：

- 读取需要 unlock
- 默认不参与普通同步
- 普通搜索默认不可见
- 解锁状态有 TTL

### 8.2 UnlockSession

表示某用户对某 vault 的临时访问授权。

关键属性：

- `unlock_session_id`
- `user_id`
- `library_id`
- `auth_context`
- `acr`
- `amr`
- `expires_at`

### 8.3 VaultAccessPolicy

定义访问 vault 所需条件。

可包含：

- 最低 `acr`
- 允许的 `amr`
- unlock TTL
- 是否要求 WebAuthn assertion
- 是否允许记住本设备一段时间

---

## 9. Audit 子域

至少记录以下行为：

- 用户登录
- step-up auth
- vault unlock
- node create/update/move/delete/restore
- profile/rule 变更
- sync apply
- conflict resolution

审计与工程日志分开。
审计应能按 user / device / browser client / library 检索。

---

## 10. 核心不变量

### 10.1 云端节点必须稳定标识

`server_node_id` 一旦分配，不因客户端变化而重建。

### 10.2 Vault 默认不可落入普通同步流

除非专门设计并明确授权，否则 vault 内容不得出现在 normal projection 中。

### 10.3 Projection 不是完整视图

某客户端看到的内容可能只是 library 的一部分。
因此本地树缺失不等于云端删除。

### 10.4 Revision 必须有序

每个 library 内的 revision clock 必须单调增长。

### 10.5 Cursor 只能前进或重建

正常情况下 cursor 不应回退。
若需重建，应显式触发 reindex/resync 流程。

---

## 11. 领域操作列表

### 11.1 内容操作

- create folder
- create bookmark
- update title/url/meta
- move node
- reorder siblings
- delete node
- restore node

### 11.2 同步操作

- register target
- scan local changes
- preview sync
- apply sync
- ack cursor
- rebuild mapping

### 11.3 策略操作

- create profile
- attach target selector
- add rule
- reorder rule
- enable / disable profile

### 11.4 安全操作

- login
- register passkey
- step-up auth
- unlock vault
- revoke unlock session

---

## 12. 后续阅读

- 数据库存储：`003-DATABASE.md`
- API 形状：`004-API.md`
- 同步行为：`005-SYNC.md`

---

[English](../en/002-DOMAIN-MODEL.md) | [中文](002-DOMAIN-MODEL.md)
