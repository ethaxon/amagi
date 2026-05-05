# 007-WEB-UI

## 1. 本文档目的

本文档定义 amagi Dashboard Web UI 的信息架构、关键页面、前端数据流与推荐目录结构。

当前 Iter11 已落地的 Dashboard Web baseline 使用：

- Vite
- React
- 轻量 typed API client
- 本地 CSS 与 React state

后续当页面数量、路由和异步缓存复杂度真实出现后，再引入 TanStack Router / Query / Table / Virtual 或其他 UI 基础设施；本轮不为了满足清单而提前堆依赖。

当前开发期调用约定：

- Dashboard Web 继续使用 dev connection panel 中的绝对 API base URL
- 默认本地开发组合是 `http://localhost:4174` 或 `http://127.0.0.1:4174` -> API server `http://127.0.0.1:7800`
- API server 提供受限 CORS baseline 允许这两个 Dashboard dev origin 发起带 `Authorization` / `X-Amagi-Oidc-Source` 的请求

架构见 `001-ARCHITECTURE.md`。
API 见 `004-API.md`。

---

## 2. 设计目标

Dashboard 不是简单列表页，而是：

- 收藏控制面板
- 同步规则管理中心
- 设备与客户端观察点
- vault 解锁入口
- 冲突处理入口

因此 UI 设计应服务于以下目标：

- 快速浏览与编辑 library/tree
- 清晰理解 sync projection
- 显示同步风险与冲突
- 清晰区分 normal 与 vault
- 支持大树与大列表

---

## 3. 信息架构建议

顶层导航包含：

- Libraries
- Search
- Devices
- Sync
- Vault
- Conflicts
- Settings

---

## 4. 关键页面

### 4.1 Libraries 页面

主工作区。

推荐布局：

- 左侧：library tree
- 中间：当前 folder 内容列表 / 卡片
- 右侧：详情 inspector
- 顶部：全局搜索、quick actions

支持操作：

- 新建 folder/bookmark
- 拖拽移动
- 批量标签
- 删除/恢复
- 标星
- 打开详情

### 4.2 Search 页面

支持：

- 关键字搜索
- tag 过滤
- starred 过滤
- library 范围过滤
- 是否包含 vault（需 unlock）

### 4.3 Devices 页面

显示：

- devices
- browser clients
- platform/browser family
- last seen
- trust level
- bound profiles

### 4.4 Sync 页面

显示：

- sync profiles
- target selectors
- include/exclude/readonly rules
- preview history
- last sync status

当前 Iter11 baseline 只实现 sync profile 管理第一屏，包含：

- auth panel：API base URL、OIDC source、SecurityDept backend-oidc 登录 / 清理本地 token-set state、认证状态展示
- advanced dev fallback：折叠的 `devBearerToken` textarea，仅用于绕过前端 SDK 排障
- profile list：name、enabled、mode、default direction、conflict policy
- selected profile detail：编辑 profile、列出 targets、add/delete target、列出 rules、add/edit/delete rule
- error panel：显示 API error code / message

### 4.5 Vault 页面

显示：

- vault libraries
- 当前 unlock 状态
- unlock TTL
- lock / unlock 动作
- 可选最近审计

### 4.6 Conflicts 页面

显示：

- open conflicts
- conflict type
- affected nodes
- server/local summary
- resolve actions

### 4.7 Settings 页面

显示：

- account
- OIDC provider info
- passkeys
- security preferences

---

## 5. 状态管理策略

### 5.1 服务端状态

当前 Iter11 baseline 仍使用轻量 React state 驱动单屏交互；当 Dashboard 扩展到多页面和共享缓存后，再迁移到 TanStack Query。未来建议交给 Query 的内容包括：

- current user
- libraries
- tree data
- search results
- devices/clients
- profiles/rules
- conflicts
- unlock state

### 5.2 路由状态

当前 Iter11 只有单屏 sync management baseline，尚未引入 Router。未来多页面阶段建议交给 TanStack Router + URL：

- selected library
- selected folder
- search keyword
- filters
- pagination
- active tab

### 5.3 本地 UI 状态

只保留短期交互状态，例如：

- modal open/close
- drag state
- selected rows
- right panel collapse
- inline form draft

---

## 6. 组件分层建议

### 6.1 route layer

负责：

- route params
- loader
- page assembly

### 6.2 feature layer

负责：

- libraries
- search
- sync
- vault
- conflicts

### 6.3 shared ui

负责：

- tree view
- data table
- badges
- forms
- dialogs
- diff viewer

---

## 7. 页面细节建议

### 7.1 Tree View

要求：

- 支持大树虚拟化
- 支持展开/折叠
- 支持图标与状态标记
- 支持 normal/vault 差异样式
- 支持拖拽移动

### 7.2 List/Table View

推荐使用 TanStack Table。

字段可包括：

- title
- url
- tags
- starred
- updated_at
- source library
- visibility/sync markers

### 7.3 Details Inspector

显示：

- title
- url
- normalized url
- description
- tags
- starred
- created/updated time
- revision summary
- sync visibility summary

### 7.4 Diff / Preview Viewer

这是关键组件。至少显示：

- server -> local changes
- local -> server changes
- conflicts
- readonly violations
- affected counts

---

## 8. vault 交互设计

### 8.1 默认隔离

vault 不应像普通 folder 一样混在 normal tree 中。建议在导航层明确区分。

### 8.2 unlock 流程

用户点击 vault library 时：

1. 检查 unlock state
2. 若无效，弹出 unlock dialog
3. 完成 step-up / WebAuthn
4. 获取 unlock session
5. 刷新 vault 内容

### 8.3 unlock 状态显示

应显式显示：

- remaining TTL
- 当前认证方式
- lock action

### 8.4 搜索中的 vault

默认不包含。若用户开启"include vault"，应先检查 unlock。

---

## 9. Sync 管理交互设计

### 9.1 Profile 编辑器

应支持：

- 基本信息
- mode
- direction
- enabled
- target selectors
- ordered rules

### 9.2 Rule 编辑器

至少支持：

- action
- matcher type
- matcher value
- reorder

### 9.3 Preview 页面

必须可读，不只是原始 JSON。应有：

- summary cards
- grouped diff list
- conflict list
- apply button

### 9.4 Conflict Resolution

至少支持：

- keep server
- keep local
- move to conflict folder
- mark resolved manually

---

## 10. 推荐前端目录结构

当前 Iter11 baseline 允许保持轻量目录，例如：

```
src/
  App.tsx
  api.ts
  constants.ts
  state.ts
  main.tsx
  styles.css
```

当 Dashboard 扩展为多页面应用后，再逐步演进到：

```
src/
  app/
    router.tsx
    providers.tsx
  routes/
    __root.tsx
    libraries.tsx
    search.tsx
    devices.tsx
    sync.tsx
    vault.tsx
    conflicts.tsx
    settings.tsx
  features/
    libraries/
    search/
    devices/
    sync/
    vault/
    conflicts/
    auth/
  components/
    tree/
    table/
    diff/
    forms/
    layout/
  lib/
    api/
    query/
    utils/
    auth/
  styles/
```

---

## 11. API 客户端建议

当前 Iter11 baseline 已至少要求把请求从组件 JSX 中抽离。后续多资源阶段建议建立 `lib/api/`，按资源分文件：

- `me.ts`
- `libraries.ts`
- `nodes.ts`
- `search.ts`
- `devices.ts`
- `syncProfiles.ts`
- `sync.ts`
- `vault.ts`
- `conflicts.ts`

不要把所有请求写在组件里。

---

## 12. UI 风格建议

### 12.1 normal vs vault

必须有清晰视觉区分：

- vault 用 lock/shield 风格标识
- normal 用普通 folder/bookmark 图标

### 12.2 风险提示

同步与冲突相关操作必须有明确提示，不应弱化。

### 12.3 高密度但不拥挤

面向 power user：

- 列表密度可稍高
- 但分区与状态色必须清晰

---

## 13. 可访问性与键盘操作

至少应支持：

- tree 键盘导航
- table 行选择
- dialog focus trap
- 快捷键打开搜索
- 快捷键新建书签

---

## 14. 首阶段交付重点

1. Libraries 主界面
2. Search
3. Devices
4. Sync Profiles：Iter11 已有第一屏管理 baseline
5. Preview/Apply Viewer
6. Vault Unlock
7. Conflicts

不要求首阶段做到完整的历史时间线或复杂多用户共享界面。

---

## 15. 与其他文档关系

- 架构：`001-ARCHITECTURE.md`
- API：`004-API.md`
- 同步：`005-SYNC.md`
- 安全与解锁：`008-SECURITY.md`

---

[English](../en/007-WEB-UI.md) | [中文](007-WEB-UI.md)
