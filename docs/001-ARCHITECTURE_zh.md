# 001-ARCHITECTURE

## 1. 本文档目的

本文档定义 amagi 的总体架构、组件边界、运行时职责与数据流。
领域对象请看 `002-DOMAIN-MODEL.md`。
数据库落地请看 `003-DATABASE.md`。
同步协议细节请看 `005-SYNC.md`。

---

## 2. 架构总览

amagi 推荐从单体应用起步，但逻辑上应提前按模块边界组织：

- auth
- bookmark domain
- policy
- sync
- vault
- search/archive
- adapters-facing API
- dashboard-facing API

逻辑结构如下：

1. Dashboard Web UI
2. Browser Extensions / Adapters
3. API Server
4. PostgreSQL
5. Optional workers / object storage / redis

---

## 3. 逻辑组件

### 3.1 Dashboard Web UI

职责：

- 用户登录
- library/tree/list 管理
- 搜索与标签操作
- 设备与浏览器客户端管理
- sync profile / rules 管理
- sync preview / conflict center
- vault 解锁入口与状态显示
- 审计与设置页面

详见 `007-WEB-UI.md`。

### 3.2 Browser Extensions / Adapters

职责：

- 读取本地书签树
- 扫描变更或订阅变更
- 执行本地 apply
- 维护服务端会话
- 发起 preview / apply 流程
- 呈现最小同步 UI

详见 `006-BROWSER-ADAPTERS.md` 与 `005-SYNC.md`。

### 3.3 API Server

职责：

- 用户认证
- library/tree CRUD
- revision 生成
- policy 评估
- sync feed / push / ack / preview / apply
- conflict 计算
- vault unlock session 发放
- 审计记录

### 3.4 PostgreSQL

职责：

- 主数据存储
- revision/outbox/sync cursor
- policy/rules
- devices/browser clients
- vault unlock session
- 审计数据
- 搜索索引基础字段

### 3.5 Workers

职责：

- 抓取页面标题与 favicon
- 页面归档
- 元数据补全
- 低优先级索引更新
- 过期 unlock session 清理
- 异步事件清理与压缩

---

## 4. 推荐运行形态

### 4.1 首阶段：单体应用

推荐单个 Rust 服务进程，按模块拆 crate 或 internal modules。
好处：

- 便于快速落地
- 避免过早分布式化
- 简化事务与一致性
- 降低 agent 实现复杂度

### 4.2 后续：逻辑拆分

当负载或协作规模变大时可拆分为：

- `amagi-api`
- `amagi-worker`
- `amagi-web`
- `amagi-extension-core`

但在模型层面不应改变本文档的边界定义。

---

## 5. 关键边界

### 5.1 Domain 与 Adapter 边界

核心领域模型不应依赖具体浏览器 API。
浏览器只通过 adapter 暴露能力，如：

- load local tree
- apply ops
- scan local changes
- describe capabilities

### 5.2 Domain 与 UI 边界

UI 只消费 API，不应拥有隐藏的核心同步逻辑。
同步规则判定和 projection 计算应在服务端和共享 sync core 中完成。

### 5.3 Vault 与 Normal Library 边界

vault 不是普通库上的附加 UI 状态，而是单独的访问层级与同步层级。
它必须在 API、policy、search、sync 上都被单独处理。

### 5.4 Sync 与 CRUD 边界

普通 CRUD 不是同步协议。
同步需要 revision、cursor、preview、apply、conflict、ack 等独立流程。

---

## 6. 高层数据流

### 6.1 Dashboard 修改收藏

1. 用户在 Dashboard 修改节点
2. API 写入 `bookmark_nodes` 等表
3. 生成 revision
4. 更新 library head
5. 相关 target 下次 pull 时收到 delta

### 6.2 Browser 扫描本地变更

1. adapter 读取本地树与本地状态
2. 计算本地 mutation set
3. 调用 preview
4. 服务端评估 rules、合并策略、冲突
5. 用户确认 apply
6. 服务端接收 push 并生成 revision
7. 返回要应用到本地的 delta
8. adapter 执行本地 apply
9. adapter ack cursor

### 6.3 Vault 解锁

1. 用户访问 vault
2. 当前 base session 不满足要求
3. 触发 step-up auth / WebAuthn
4. 成功后生成 unlock session
5. 仅在 unlock session 有效期间允许读取 vault 内容

---

## 7. 组件细分建议

### 7.1 auth 模块

负责：

- OIDC RP
- base session
- step-up auth
- WebAuthn registration/assertion
- vault unlock session

详见 `008-SECURITY.md`。

### 7.2 bookmark domain 模块

负责：

- library / node / tree
- title/url/meta
- tag
- move/delete/restore
- node validation

详见 `002-DOMAIN-MODEL.md`。

### 7.3 policy 模块

负责：

- sync profile
- include/exclude/readonly 规则
- target 匹配
- vault 可见性
- search 可见性

详见 `002-DOMAIN-MODEL.md` 与 `005-SYNC.md`。

### 7.4 sync 模块

负责：

- revision
- delta feed
- cursor
- preview
- apply
- conflict
- ack

详见 `005-SYNC.md`。

### 7.5 archive/search 模块

负责：

- URL normalize
- metadata enrich
- search index prepare
- archive asset handling

首阶段只需最小实现。

---

## 8. 重要架构决策

### 8.1 云端为真源

原因：

- 支持手动同步
- 支持按目标裁剪
- 支持冲突可追踪
- 支持 vault 不下发
- 支持统一审计

### 8.2 事件化 revision + cursor

原因：

- 易于增量同步
- 易于调试
- 易于重放
- 适合扩展多客户端场景

### 8.3 platform capability first

原因：

- 平台差异真实存在
- 统一抽象只能建立在最低公共能力之上
- Safari 必须被特判，而不是硬凑一致

### 8.4 单体优先

原因：

- 当前问题复杂度在同步与模型，而不在服务拆分
- 过早微服务化会拖慢实现与测试

---

## 9. 外部依赖建议

### 后端

- Rust
- Axum
- SeaORM + SeaQuery
- PostgreSQL
- OpenID Connect
  - 使用 [securitydept-core](https://github.com/ethaxon/securitydept) 库的 oidc feature 以及其重新导出的 openidconnect crate
- WebAuthn
- OpenDAL
- Snafu 不要使用 thiserror 和 anyhow
- 可选 Redis，初期尽量使用 in-memory 实现

### 前端

- Vite（rolldown）
- React
- TanStack Router / Query / Table / Virtual（有 tanstack 家族可以使用的尽量就使用 tanstack 方案）
- shadcn/ui
- Tailwind CSS
- pnpm 不要使用 npm
- biomejs

### 扩展

- TypeScript
- shared sync core
- Chromium adapter
- Firefox adapter

---

## 10. 可观测性与审计

系统至少应记录：

- 用户登录与 step-up
- vault 解锁与失效
- node 变更
- sync preview / apply
- conflict 产生与解决
- target 注册与 last seen

日志与审计不可混为一谈：

- 日志偏工程运维
- 审计偏用户行为与系统状态

---

## 11. 失败处理原则

### 11.1 本地 apply 失败

不得直接覆盖或放弃。
应保留：

- 本地错误信息
- 未应用操作
- 当前 cursor
- 可重试状态

### 11.2 push 合并失败

应返回 conflict 描述，而不是静默吞掉。

### 11.3 vault 解锁失败

不得退化成普通读取。
必须严格拒绝。

---

## 12. 下一步阅读

继续阅读：

- `002-DOMAIN-MODEL.md`
- `003-DATABASE.md`
- `005-SYNC.md`
