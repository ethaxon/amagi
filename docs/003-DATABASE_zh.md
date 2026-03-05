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

---

## 3. 身份与终端

### 3.1 `users`

建议字段：

- `id uuid primary key`
- `oidc_subject text not null unique`
- `email text null`
- `display_name text null`
- `status text not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

索引建议：

- unique index on `oidc_subject`
- index on `email`

### 3.2 `devices`

建议字段：

- `id uuid primary key`
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

### 3.3 `browser_clients`

建议字段：

- `id uuid primary key`
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

- `id uuid primary key`
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

- `id uuid primary key`
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

- `rev_id uuid primary key`
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

### 5.5 `sync_conflicts`

建议字段：

- `id uuid primary key`
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

- `id uuid primary key`
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

- `id uuid primary key`
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

- `id uuid primary key`
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

- `id uuid primary key`
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

- `id uuid primary key`
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

- `id uuid primary key`
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
2. devices
3. browser_clients
4. libraries
5. bookmark_nodes
6. bookmark_meta
7. library_heads
8. node_revisions
9. sync_cursors
10. node_client_mappings
11. sync_profiles
12. sync_profile_targets
13. sync_profile_rules
14. vault_unlock_sessions
15. audit_events

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
