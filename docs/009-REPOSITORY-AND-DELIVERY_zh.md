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
    sync-core/
    browser-adapter-chromium/
    browser-adapter-firefox/
    browser-adapter-safari/
    dashboard-sdk/
```

---

## 3. 后端结构建议

### 3.1 `apps/api-server`

推荐逻辑模块：

```
src/
  main.rs
  app.rs
  config.rs
  http/
    routes/
    extractors/
    errors.rs
  auth/
  domain/
    libraries/
    nodes/
    metadata/
    policy/
    sync/
    vault/
    audit/
  db/
  jobs/
```

### 3.2 crate 组织建议

若使用 workspace，可进一步拆成：

- `amagi-domain`
- `amagi-policy`
- `amagi-sync`
- `amagi-auth`
- `amagi-api`

首阶段也可先保留单 crate + clear modules。

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
src/
  background/
  popup/
  options/
  sidepanel/
  shared/
```

建议以 `extension.js` 作为该应用的扩展壳层：

- 负责开发脚手架
- 负责 manifest 与多入口组织
- 负责 Chromium / Firefox 构建输出
- 负责 popup/options/side panel 等 UI 容器装配

共享逻辑不要直接堆在扩展入口中，应下沉到 `packages/`。
`apps/extension-web` 不应变成同步核心实现所在层。

---

## 5. 共享包建议

### 5.1 `packages/sync-core`

职责：

- local tree normalization
- mapping helpers
- diff helpers
- preview/apply result model
- conflict model

### 5.2 `packages/browser-adapter-chromium`

职责：

- Chromium bookmarks API 封装
- 本地 node id / tree 提取
- apply ops

### 5.3 `packages/browser-adapter-firefox`

职责与 Chromium 类似，但处理平台差异。

### 5.4 `packages/browser-adapter-safari`

首阶段职责有限：

- 保存当前页
- 搜索入口
- 轻量桥接能力

### 5.5 `packages/dashboard-sdk`

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
- OIDC 基础登录
- users/devices/browser_clients/libraries/bookmark_nodes 表

产物：

- 可启动服务
- 初始 migrations
- 基础 session

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

### Phase 4: Chromium / Firefox 扩展 MVP

目标：

- 基于 `extension.js` 建立扩展壳层与跨浏览器构建输出
- browser client register
- local scan
- preview/apply
- ack

产物：

- 实际桌面浏览器手动同步闭环

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
- Chromium extension MVP
- Firefox extension MVP
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

- database url
- oidc issuer
- oidc client id/secret
- session secret
- webauthn rp id/name
- object storage settings（可选）

### 9.2 环境分层

建议至少：

- local
- dev
- prod

### 9.3 seed 数据

可提供开发用 seed：

- demo normal library
- demo vault library
- sample sync profile

---

## 10. 文档维护规则

任何以下变更，都必须同步 docs：

- 新表或表语义变化
- 新 API 或 API 语义变化
- 同步行为变化
- vault 可见性变化
- 浏览器能力边界变化

README 与 AGENTS 只保留入口信息。细节统一下沉到 `docs/`，避免重复。

---

## 11. 建议的首批任务单

### T1 初始化 monorepo 与目录结构

### T2 建立 Rust API server、配置、日志、健康检查

### T3 建立 PostgreSQL migrations：users/devices/browser_clients/libraries/bookmark_nodes/bookmark_meta

### T4 实现 OIDC 登录与基础 session

### T5 实现 normal library CRUD API

### T6 实现 Dashboard Libraries 页面

### T7 实现 revisions/library_heads/cursors

### T8 实现 sync preview/apply API

### T9 实现 Chromium adapter + extension MVP

### T10 实现 Firefox adapter + extension MVP

补充约束：

- `extension.js` 负责扩展宿主与构建输出
- Chromium / Firefox 差异继续收敛在 adapter 包
- `sync-core` 不直接依赖 `extension.js`

### T11 实现 sync profiles / rules UI 与 API

### T12 实现 vault library + unlock session + WebAuthn

---

## 12. 交付判断标准

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
