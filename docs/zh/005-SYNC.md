# 005-SYNC

## 1. 本文档目的

本文档定义 amagi 的同步模型、同步协议、冲突处理、投影规则与实现建议。
这是整个系统最关键的文档之一。

相关文档：

- 架构：`001-ARCHITECTURE.md`
- 领域模型：`002-DOMAIN-MODEL.md`
- API：`004-API.md`
- 浏览器适配：`006-BROWSER-ADAPTERS.md`

---

## 2. 同步设计原则

### 2.1 云端为真源

浏览器本地书签树只是 projection。
同步目标是让本地状态与云端的“该目标应见内容”逐步一致，而不是让任何一个本地端天然拥有最终裁决权。

### 2.2 同步是 projection，不是镜像

不同设备 / 浏览器能看到不同内容。
因此：

- 本地缺少某 folder，不一定代表云端删除
- 云端有某 folder，不一定应下发给当前目标

### 2.3 手动同步优先

默认工作流是：

1. scan local state
2. generate mutations
3. preview
4. user confirm
5. apply
6. ack

### 2.4 revision + cursor 驱动

服务端通过 revision feed 向目标下发增量。
目标通过 cursor 表示已应用位置。

### 2.5 conflict 是显式对象

冲突不是日志，而是可展示、可处理的领域对象。

---

## 3. 同步参与方

### 3.1 Source of Truth

云端 library + revisions。

### 3.2 Target

一个具体 BrowserClient。
它由以下因素描述：

- device
- platform
- browser family
- capabilities
- matched profile

### 3.3 Sync Profile

定义该 target 应如何同步：

- mode
- direction
- conflict policy
- target selectors
- include/exclude/readonly rules

当前 Iter11 baseline 额外约束：

- Dashboard 管理 API 保证每个 user 至少存在一个 enabled manual profile
- sync session 中返回的 `selectedProfile` / `availableProfiles` 继续来自云端 profile 配置，而不是浏览器本地状态

---

## 4. 同步模式

### 4.1 Manual

首选模式。
仅当用户明确操作时执行 preview/apply。

### 4.2 Scheduled

按固定周期执行扫描与同步。
适合桌面端，但当前只保留领域位点；Iter11 UI/API 不允许创建该 mode。

### 4.3 Auto

近实时自动同步。
不推荐首阶段默认启用；Iter11 UI/API 不允许创建该 mode。

---

## 5. 同步方向

### 5.1 Pull-only

仅从云端拉取并应用到本地。

### 5.2 Push-only

仅把本地变更送到云端。
少见，但在导入期可能有用。

### 5.3 Bidirectional

双向同步。
仍然建议通过 preview/apply 明确确认。

---

## 6. Projection 规则

### 6.1 规则输入

规则匹配维度包括：

- target attributes
  - platform
  - device_type
  - device_id
  - browser_family
  - browser_client_id
- content attributes
  - library kind
  - folder id
  - folder path
  - tag

当前 Iter11 dashboard 管理 API 中，JSON 字段使用 camelCase，例如 `matcherType` / `matcherValue`；但 matcher 的 canonical value 仍然使用：

- `library_kind`
- `folder_id`
- `folder_path`
- `tag`

### 6.2 规则动作

- `include`
- `exclude`
- `readonly`

### 6.3 推荐评估方式

1. 选择生效 profile
2. 评估 target 是否命中 profile
3. 对 library tree 自顶向下评估规则
4. 生成当前 target 的可见 projection
5. 在此 projection 上计算 delta / merge / apply

### 6.4 默认行为建议

若 profile 已命中而无更细规则：

- normal library 默认 include
- vault library 默认 exclude

当前 Iter11 `ensure_default_profile()` 还会保证默认 profile 至少包含两条规则：

- include `library_kind:normal`
- exclude `library_kind:vault`

---

## 7. 同步数据模型

### 7.1 服务端 revision event

每条 event 应至少包含：

- rev id
- clock
- op
- node id
- 相关 payload

`op` 可能包括：

- create
- update
- move
- delete
- restore

当前 Iter7 已实现服务端 sync backend baseline：

- `POST /api/v1/sync/clients/register`
- `POST /api/v1/sync/session/start`
- `GET /api/v1/sync/feed`
- `POST /api/v1/sync/preview`
- `POST /api/v1/sync/apply`
- `POST /api/v1/sync/cursors/ack`

该 baseline 仍然是最小可用版本：

- bearer principal 仍是唯一业务 API 认证基线
- feed 直接读取 `node_revisions`
- preview/apply 通过 `sync_previews` 持久化两阶段状态
- apply 在单个事务内复用 `packages/amagi-bookmarks` 的 transaction-scoped mutation 边界
- vault library 默认排除，不进入普通 sync feed
- 本轮不实现完整 rule engine、自动后台 sync、复杂三方 merge 或 mapping rebuild API

### 7.2 客户端 mutation

客户端向服务端上报的本地变更，应包含：

- client_mutation_id
- base_clock
- op
- local node reference
- node payload

### 7.3 Mapping

客户端必须维护：

- server node id <-> client external id

对于服务端下发的 create/restore，本地 apply 的 create op 必须同时携带 `serverNodeId`。adapter 在浏览器侧真正创建节点后，应返回新的 `clientExternalId`，由 orchestrator 在 ack cursor 前合并回 mapping。

否则无法安全做 move/update/delete。

---

## 8. 标准同步流程

### 8.1 注册阶段

1. 扩展安装后注册 BrowserClient
2. 服务端返回 client identity
3. 匹配可用 sync profiles

### 8.2 常规同步阶段

1. 客户端读取本地树
2. 以本地 cursor 的 `lastAppliedClock` 作为 `fromClock` 调用 feed，获取该 clock 之后的 server-side delta
3. 生成本地变化摘要和 local mutations
4. 以同一个本地已应用 clock 作为 `baseClock` 调用 preview，不能直接把 `feed.currentClock` 当作 preview 基线
4. 服务端：
  - 校验 browser client / owner / profile / library
  - 拉取 server-side delta
  - 评估基础 projection
  - 接纳或拒绝 local mutations
  - 生成 conflicts
5. 用户查看 preview
6. 用户确认 apply
7. 服务端在单事务内写 bookmark mutation、revision 与 mapping
8. 返回最终 local apply ops 与新 clock
9. 客户端按 apply plan 分阶段应用本地操作，并合并服务端 apply 返回的 `createdMappings` 与本地 apply create 返回的 `createdMappings`
10. 客户端 reload tree、保存 merged mappings 后 ack cursor

---

## 9. Preview / Apply 模型

### 9.1 为什么需要 preview

原因：

- 用户希望默认不自动同步
- 需要让用户在覆盖前看到影响
- 需要在冲突时阻止盲写

### 9.2 Preview 输出

preview 至少应返回：

- server -> local ops 数量
- local -> server accepted 数量
- conflict 数量
- 可读 summary
- 明确的 conflict detail
- 持久化 preview id 与过期时间

### 9.3 Apply 语义

apply 应以 preview 结果为基础，不应在客户端自行重算后偷偷改变行为。
若 preview 过期，应要求重新 preview。
同一个已 `applied` preview 必须可幂等回放，不得重复创建节点、revision 或 mapping。

当前 Iter8 client baseline：

- `packages/amagi-sync-client` 提供 `runManualSync()`，顺序固定为 register -> session -> feed -> preview -> confirm -> apply -> local apply -> ack。
- 若 preview 有 conflict，则保存 pending preview 并返回 `needs-user-resolution`，不会 apply、不会 ack。
- 若用户未确认 apply，则保存 pending preview 并返回 `awaiting-confirmation`。
- 若服务端 apply 成功但 local adapter apply 失败，则保存 pending recovery state，不 ack cursor。
- 若本地 cursor 落后且本轮没有 local mutations，则仍应把 preview/apply 返回的 server ops 转成 local apply plan，adapter 成功应用后才能 ack cursor。
- 若本地 cursor 落后且本轮还带 local mutations，服务端可返回 `stale_base_clock` conflict；客户端应保存 pending preview，不 apply、不 ack，等待先 pull/apply 新 server ops 后再重试 preview。
- 对于 server-created local node，local apply create 必须返回 `createdMappings`，`runManualSync()` 合并后再 reload tree / ack cursor。
- local apply plan 目前按 create -> update -> move -> delete 四阶段执行。
- client 解析 revision payload 时，字段来源以 `payload.node.*` 为主：`payload.node.nodeType`、`payload.node.parentId`、`payload.node.title`、`payload.node.url`、`payload.node.sortKey`；其中 `node.move` 的目标父节点优先取顶层 `payload.parentId`，其次才回退到 `payload.node.parentId`。

---

## 10. 冲突处理

### 10.1 冲突类型建议

#### mapping_missing

客户端失去了本地 id 与服务端 id 的映射。

#### stale_base_clock

本地 mutation 基于过旧 clock，本轮要求先 pull/apply server ops 再重新 preview。

#### invalid_parent

create/move 解析出的父节点不是当前 library 内的 live folder。

#### unsupported_vault_sync

vault library 不进入普通 sync feed / preview / apply。

#### projection_violation

客户端尝试 push 一个当前 profile 不允许暴露的节点。

### 10.2 默认冲突策略建议

#### title/url/meta 更新

last-writer-wins，可记录审计。

#### move

若目标父节点无效，放到冲突收容 folder 或标记 unresolved。

#### delete vs update

默认 delete 优先，但保留恢复入口。

#### duplicate normalized URL

默认不自动 dedupe，只给出建议。

### 10.3 冲突展示

Dashboard 与扩展侧都应至少能展示：

- 冲突类型
- 涉及节点
- server state 摘要
- local state 摘要
- 推荐处理方式

---

## 11. Tombstone 与恢复

### 11.1 为什么需要 tombstone

没有 tombstone，客户端很难判断：

- 这是从未存在
- 还是曾经存在但已删除

### 11.2 Tombstone 生命周期

建议保留足够长时间，至少跨越：

- 多个手动同步周期
- 离线设备回连周期

### 11.3 恢复

restore 本质上是一条新的 revision，而不是删除 tombstone 历史。

---

## 12. 本地 apply 策略

### 12.1 幂等要求

本地 apply 尽量设计为幂等。
重复收到相同 op 不应造成树损坏。

### 12.2 分阶段 apply

建议本地 apply 至少按以下阶段执行：

1. create missing containers
2. update node payload
3. move / reorder
4. delete / cleanup

### 12.3 失败恢复

若 apply 中断：

- 不推进 ack
- 记录失败位置
- 支持重试
- 必要时触发 rebuild mapping

---

## 13. Rebuild / Resync

### 13.1 何时需要 rebuild

- 浏览器用户手工大量改树
- 本地扩展状态丢失
- mapping 表损坏
- 跨浏览器迁移

### 13.2 rebuild 目标

- 重新建立 server node id 与 client external id 映射
- 识别明显相同节点
- 生成最小差异修复

### 13.3 rebuild 不是全量盲覆盖

除非用户明确选择 reset local tree，否则应优先尝试匹配与修复。

---

## 14. Vault 与同步

### 14.1 默认规则

vault 默认不进入普通 sync feed。

### 14.2 特殊情况

若未来允许某类客户端访问 vault，也必须满足：

- target 明确授权
- 当前 unlock session 有效
- projection 明确允许
- 本地存储风险已评估

### 14.3 首阶段建议

不要把 vault 下发到浏览器原生书签树。
vault 只在 Web UI / 受控应用壳中访问。

---

## 15. 性能建议

### 15.1 首阶段

优先正确性，不优先复杂实时性。

### 15.2 扫描策略

先采用：

- 手动触发扫描
- 周期性轻量扫描
- 基于 root hash 或 subtree hash 的变更判断

### 15.3 增量化

revision feed 应以 `(library_id, logical_clock)` 为主索引。

---

## 16. 测试建议

至少覆盖：

- create/update/move/delete/restore
- projection include/exclude/readonly
- local create + server create 并发
- delete vs update
- missing mapping rebuild
- preview 过期 apply 失败
- cursor 幂等 ack
- vault 内容不进入普通 feed

---

## 17. 实现优先级建议

### 第一优先级

- revision model
- cursor model
- preview/apply
- mapping table
- basic conflict types

### 第二优先级

- rule engine
- readonly projection
- rebuild mapping

### 第三优先级

- scheduled sync
- event listeners
- richer diff UI

---

## 18. 与其他文档关系

- API 形状：`004-API.md`
- 浏览器平台实现：`006-BROWSER-ADAPTERS.md`
- 安全边界：`008-SECURITY.md`

---

[English](../en/005-SYNC.md) | [中文](005-SYNC.md)
