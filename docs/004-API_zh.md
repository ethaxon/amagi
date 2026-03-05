# 004-API

## 1. 本文档目的

本文档定义 amagi 的 API 分层与主要接口形状。它不是最终 OpenAPI 文档，但应作为接口设计与实现基线。

数据库结构见 `003-DATABASE.md`。
同步语义见 `005-SYNC.md`。
安全与鉴权见 `008-SECURITY.md`。

---

## 2. API 分层

建议按用途分为三类：

1. Dashboard API
2. Sync API
3. Auth / Security API

统一前缀建议：

- `/api/v1/dashboard/...`
- `/api/v1/sync/...`
- `/api/v1/auth/...`

---

## 3. 通用约定

### 3.1 数据格式

- JSON request / response
- 时间统一使用 ISO 8601 / RFC 3339
- ID 使用字符串表示 UUID/ULID

### 3.2 错误格式

建议统一错误响应：

```json
{
  "error": {
    "code": "conflict_detected",
    "message": "A sync conflict was detected",
    "details": {}
  }
}
```

### 3.3 分页列表接口

建议支持：

- `limit`
- `cursor`

### 3.4 版本化

所有新接口放在 `/api/v1/`
不要在无版本前缀下暴露核心 API。

---

## 4. Dashboard API

### 4.1 当前用户

#### `GET /api/v1/dashboard/me`

返回当前用户与会话基本信息。

示例响应：

```json
{
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "display_name": "User"
  },
  "session": {
    "is_step_up": false
  }
}
```

### 4.2 Libraries

#### `GET /api/v1/dashboard/libraries`

列出用户可见的 libraries。

#### `POST /api/v1/dashboard/libraries`

创建 library。

示例请求：

```json
{
  "name": "Default",
  "kind": "normal"
}
```

说明：

- `kind` 为 `vault` 时，需要按安全策略创建

### 4.3 Tree

#### `GET /api/v1/dashboard/libraries/:libraryId/tree`

返回完整树或指定根节点下的子树。

查询参数建议：

- `rootNodeId`
- `includeDeleted=false`
- `includeMeta=true`

#### `POST /api/v1/dashboard/libraries/:libraryId/nodes`

创建节点。

示例请求：

```json
{
  "node_type": "bookmark",
  "parent_id": "uuid",
  "title": "Example",
  "url": "https://example.com"
}
```

#### `PATCH /api/v1/dashboard/nodes/:nodeId`

更新节点基础字段。

#### `POST /api/v1/dashboard/nodes/:nodeId/move`

移动节点。

示例请求：

```json
{
  "new_parent_id": "uuid",
  "new_sort_key": "m100"
}
```

#### `POST /api/v1/dashboard/nodes/:nodeId/delete`

逻辑删除节点。

#### `POST /api/v1/dashboard/nodes/:nodeId/restore`

恢复节点。

### 4.4 Metadata

#### `PATCH /api/v1/dashboard/nodes/:nodeId/meta`

更新 bookmark metadata。

示例请求：

```json
{
  "description": "sample",
  "tags": ["read", "rust"],
  "starred": true
}
```

### 4.5 Search

#### `GET /api/v1/dashboard/search`

查询参数建议：

- `q`
- `libraryId`
- `includeVault=false`
- `tags`
- `starred`
- `limit`
- `cursor`

注意：

- 若 `includeVault=true`，必须验证 unlock 状态

### 4.6 Devices / Clients

#### `GET /api/v1/dashboard/devices`

#### `GET /api/v1/dashboard/browser-clients`

### 4.7 Sync Profiles

#### `GET /api/v1/dashboard/sync-profiles`

#### `POST /api/v1/dashboard/sync-profiles`

创建 profile。

示例请求：

```json
{
  "name": "Desktop Browsers",
  "mode": "manual",
  "default_direction": "bidirectional",
  "conflict_policy": "default"
}
```

#### `POST /api/v1/dashboard/sync-profiles/:profileId/targets`

新增 target selector。

#### `POST /api/v1/dashboard/sync-profiles/:profileId/rules`

新增规则。

示例请求：

```json
{
  "rule_order": 10,
  "action": "include",
  "matcher_type": "folder_id",
  "matcher_value": "uuid"
}
```

#### `PATCH /api/v1/dashboard/sync-profiles/:profileId/rules/:ruleId`

#### `DELETE /api/v1/dashboard/sync-profiles/:profileId/rules/:ruleId`

### 4.8 Conflicts

#### `GET /api/v1/dashboard/conflicts`

#### `POST /api/v1/dashboard/conflicts/:conflictId/resolve`

---

## 5. Sync API

同步 API 面向 BrowserClient。详见语义说明 `005-SYNC.md`。

### 5.1 注册

#### `POST /api/v1/sync/clients/register`

示例请求：

```json
{
  "device": {
    "device_name": "My PC",
    "device_type": "desktop",
    "platform": "windows"
  },
  "browser_client": {
    "browser_family": "chrome",
    "browser_profile_name": "Default",
    "extension_instance_id": "ext-123",
    "capabilities": {
      "can_read_bookmarks": true,
      "can_write_bookmarks": true
    }
  }
}
```

响应返回：

- `device_id`
- `browser_client_id`
- 基础会话信息
- 可匹配 profile 摘要

### 5.2 会话开始

#### `POST /api/v1/sync/session/start`

输入：

- `browser_client_id`
- 当前 profile 意向
- 当前本地能力摘要

输出：

- 认证状态
- 可用 profile
- 服务器建议模式
- 当前 cursor 状态

### 5.3 Feed 拉取

#### `GET /api/v1/sync/feed`

查询参数建议：

- `browserClientId`
- `libraryId`
- `fromClock`
- `profileId`

返回：

```json
{
  "library_id": "uuid",
  "from_clock": 10,
  "to_clock": 20,
  "events": [
    {
      "rev_id": "uuid",
      "clock": 11,
      "op": "update",
      "node_id": "uuid",
      "payload": {}
    }
  ]
}
```

说明：

- feed 已经按 profile/rules 裁剪
- vault 默认不会出现在普通 feed 中

### 5.4 Preview

#### `POST /api/v1/sync/preview`

请求建议包含：

```json
{
  "browser_client_id": "uuid",
  "profile_id": "uuid",
  "library_id": "uuid",
  "base_clock": 120,
  "local_snapshot_summary": {
    "root_hash": "hash"
  },
  "local_mutations": [
    {
      "client_mutation_id": "uuid",
      "op": "create",
      "parent_client_external_id": "b-100",
      "node_type": "bookmark",
      "title": "Example",
      "url": "https://example.com"
    }
  ]
}
```

响应建议包含：

```json
{
  "preview_id": "uuid",
  "summary": {
    "server_to_local": 3,
    "local_to_server": 1,
    "conflicts": 1
  },
  "server_ops": [],
  "accepted_local_mutations": [],
  "conflicts": []
}
```

### 5.5 Apply

#### `POST /api/v1/sync/apply`

请求示例：

```json
{
  "preview_id": "uuid",
  "confirm": true
}
```

响应示例：

```json
{
  "applied": true,
  "new_clock": 125,
  "server_ops_to_apply_locally": []
}
```

### 5.6 Push（可选拆分）

若实现上不希望 preview/apply 全承载，也可额外提供：

#### `POST /api/v1/sync/push`

但首阶段推荐优先走 preview/apply 双阶段。

### 5.7 Ack

#### `POST /api/v1/sync/ack`

请求示例：

```json
{
  "browser_client_id": "uuid",
  "library_id": "uuid",
  "last_applied_clock": 125,
  "last_ack_rev_id": "uuid"
}
```

### 5.8 Rebuild Mapping

#### `POST /api/v1/sync/rebuild-mapping`

用于客户端映射损坏或本地树被手动大改后的修复流程。

---

## 6. Auth / Security API

### 6.1 OIDC

#### `GET /api/v1/auth/oidc/start`

#### `GET /api/v1/auth/oidc/callback`

### 6.2 Session

#### `POST /api/v1/auth/logout`

### 6.3 WebAuthn 注册

#### `POST /api/v1/auth/webauthn/register/start`

#### `POST /api/v1/auth/webauthn/register/finish`

### 6.4 WebAuthn 断言

#### `POST /api/v1/auth/webauthn/assert/start`

#### `POST /api/v1/auth/webauthn/assert/finish`

### 6.5 Vault Unlock

#### `POST /api/v1/auth/vaults/:libraryId/unlock`

请求示例：

```json
{
  "method": "webauthn"
}
```

响应示例：

```json
{
  "unlock_session": {
    "id": "uuid",
    "expires_at": "2026-03-07T10:00:00Z"
  }
}
```

#### `POST /api/v1/auth/vaults/:libraryId/lock`

主动撤销当前 unlock session。

---

## 7. 响应形状建议

### 7.1 Node DTO

```json
{
  "id": "uuid",
  "library_id": "uuid",
  "node_type": "bookmark",
  "parent_id": "uuid",
  "sort_key": "m100",
  "title": "Example",
  "url": "https://example.com",
  "is_deleted": false,
  "meta": {
    "tags": ["read"],
    "starred": true
  }
}
```

### 7.2 Conflict DTO

```json
{
  "id": "uuid",
  "conflict_type": "delete_vs_update",
  "state": "open",
  "summary": "Node was deleted on server but updated locally",
  "details": {}
}
```

### 7.3 Profile DTO

```json
{
  "id": "uuid",
  "name": "Desktop Browsers",
  "mode": "manual",
  "default_direction": "bidirectional",
  "enabled": true,
  "targets": [],
  "rules": []
}
```

---

## 8. 授权与可见性

### 8.1 Dashboard API

需要用户登录 session。

### 8.2 Sync API

需要已注册 browser client 的受控 session。

### 8.3 Vault

任何涉及 vault 内容的 API 都必须额外检查：

- base 登录身份
- 对应 library 的 unlock session 是否有效

---

## 9. 向后兼容要求

- 扩展端 API 改动需尽量向后兼容
- `preview/apply/ack` 语义一旦上线，变更必须谨慎
- 新字段应尽量追加，不要破坏旧客户端解析

---

## 10. 与其他文档关系

- 领域语义：`002-DOMAIN-MODEL.md`
- 数据库存储：`003-DATABASE.md`
- 同步语义：`005-SYNC.md`
- 安全与 vault：`008-SECURITY.md`
