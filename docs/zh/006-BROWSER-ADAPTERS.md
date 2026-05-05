# 006-BROWSER-ADAPTERS

## 1. 本文档目的

本文档定义 amagi 在不同浏览器/平台上的适配策略。它不试图伪造统一能力，而是以平台真实能力为边界进行设计。

同步协议见 `005-SYNC.md`。
Web UI 见 `007-WEB-UI.md`。

---

## 2. 核心原则

### 2.1 平台能力优先于抽象统一

不要为追求统一而假设所有平台都能直接读写原生书签树。

### 2.2 共享核心逻辑，最小适配器

把 diff、projection、preview 解析等逻辑放在共享包中。平台适配器只负责与浏览器 API 交互。

### 2.3 WXT 作为扩展壳层，不作为核心层

项目后续默认采用 WXT 作为浏览器扩展工程壳层，而不是继续手写 manifest、entrypoint 与每个浏览器的构建包装。

WXT 的定位应限制为：

- 扩展开发脚手架
- manifest/background/popup/options/side panel 的构建与跨浏览器输出层
- 扩展 UI 容器与入口编排层
- 通过 `wxt/browser` / WebExtension API 访问浏览器运行时能力

不应把以下能力固化在 WXT app 壳层：

- sync protocol 语义
- diff / normalization / projection 逻辑
- policy 判定
- 平台 capability 判定之外的领域逻辑

这些能力应继续位于共享包或薄 adapter 包中。WXT 可以抹平大量扩展工程差异，但不能替代 amagi 自己的同步语义、projection 规则、vault 边界和审计要求。

### 2.4 Safari 单独处理

Safari 是特例。首阶段不应承诺完整原生书签树双向同步。

---

## 3. 适配器抽象接口

当前 Iter8 baseline 已落地的共享抽象位于 `packages/sync-client`：

```typescript
interface SyncAdapter {
  getCapabilities(): Promise<AdapterCapabilities>;
  loadTree(): Promise<LocalBookmarkNode[]>;
  applyLocalPlan(plan: LocalApplyOp[]): Promise<void>;
}
```

说明：

- sync core 负责 local tree normalization、diff、preview/apply orchestration、server ops -> local apply plan。
- WXT / WebExtension adapter 只负责真实浏览器 API 调用、能力探测与本地 state 落盘。
- 本轮没有实现 change event 驱动；仍然以 manual scan 为主。

---

## 4. Chromium 系

覆盖范围：

- Chrome
- Edge
- Brave
- Vivaldi
- Opera（若 API 兼容）

### 4.1 首阶段能力

可视为主战场平台，目标支持：

- 读取原生书签树
- 写入原生书签树
- 本地扫描
- 手动同步
- background message shell
- popup/options 占位 UI

当前 Iter8 baseline 已实现的是迁移前的 Chromium-only 基线：

- `packages/browser-adapter-chromium`
  - `createChromiumBookmarkAdapter(chromeLike)`
  - `createChromiumStorage(chrome.storage.local)`
  - `chrome.bookmarks.getTree()` -> `LocalBookmarkNode[]`
  - `LocalApplyOp[]` -> `chrome.bookmarks.create/update/move/remove/removeTree`
- `apps/extension-web`
  - MV3 manifest 生成
  - background service worker
  - popup/options shell
  - `amagi.sync.preview` / `amagi.sync.apply` / `amagi.sync.status` message baseline

当前仍未实现：

- 自动后台 sync
- side panel
- 完整 preview/apply UI 交互
- conflict resolution UI
- server-created local node 的完整 mapping 回填

### 4.2 扩展形态建议

默认使用 WXT 建立扩展应用，按目标浏览器输出 Chrome/Edge/Firefox/Safari 等构建产物。Chromium 系首阶段按 MV3 交付，包含：

- background service worker
- options page
- popup
- side panel（可选但推荐）

WXT 只用于：

- 组织上述扩展入口
- 产出 Chromium / Firefox 所需构建结果
- 承载 React / Vite 等 UI 页面容器

真正的同步流程编排应调用共享包和 WXT/WebExtension adapter，而不是直接把业务逻辑写死在扩展入口文件里。

### 4.3 本地状态建议存储

- browser_client_id
- dev-only auth config 占位
- local mapping cache
- last normalized tree snapshot
- pending apply state
- profile selection

### 4.4 最小 UI

至少包含：

- 登录/连接状态
- 当前 profile
- preview summary
- apply 按钮
- last sync status
- conflict count

### 4.5 不建议做的事情

- 不要默认后台静默双向 auto sync
- 不要把 vault 内容直接混入本地书签树

---

## 5. Firefox

### 5.1 总体策略

Firefox 不再默认规划单独的第一等 adapter package。后续应复用同一个 WXT app 和 WebExtension adapter，通过 WXT 的 browser target、manifest version target 与运行期 feature detection 处理差异。

### 5.2 差异处理

差异只在必要位置封装：

- bookmarks API 细节
- 权限差异
- 存储差异
- 事件兼容差异

如果差异只是 manifest、entrypoint 或构建目标差异，应优先放在 WXT config / entrypoint include-exclude / target 分支里，而不是新建一套 Firefox 专用同步实现。

### 5.3 首阶段目标

与 Chromium 版本尽量同等功能：

- load tree
- apply ops
- manual preview/apply
- conflict reporting

---

## 6. Safari

### 6.1 基本立场

Safari 不作为首阶段"原生书签树完整双向同步"平台。

### 6.2 首阶段支持目标

应优先支持以下能力：

- 当前页保存到 amagi
- 搜索 amagi 收藏
- 打开 Dashboard
- 导入/导出桥接
- vault 访问入口（若运行在受控 UI 内）

### 6.3 不承诺能力

首阶段不承诺：

- 完整读取 Safari 原生书签树
- 完整写入 Safari 原生书签树
- 与 Chromium/Firefox 一致的实时双向树同步

### 6.4 工程策略建议

首阶段仍使用 WXT/Safari Web Extension 能覆盖的能力做轻量入口。若后续需要更强原生能力，再增加 native wrapper；不要因为 WXT 支持 Safari 构建就承诺完整 Safari 原生书签树同步。

### 6.5 后续扩展路线

若未来投入更高，可研究：

- macOS app + Safari Web Extension 协作
- 导入/导出桥接
- 受控应用内管理，而不是强行控制原生书签树

---

## 7. iOS / Android 移动端

### 7.1 不以原生浏览器书签树同步为目标

移动端浏览器 API 能力不统一，也通常不适合做与桌面一致的原生树控制。

### 7.2 首阶段推荐产品形态

- 响应式 Web UI
- PWA
- 系统分享入口（后续）

### 7.3 可支持能力

- 浏览收藏
- 搜索
- 保存当前页（通过分享）
- 打开链接
- vault 解锁
- 查看 sync 状态（只读）

### 7.4 后续增强

若需要更强生物识别能力，可增加原生壳：

- iOS: Face ID / Touch ID
- Android: BiometricPrompt

但这不改变云端同步模型。

---

## 8. 共享扩展核心建议

建议建立：

- `packages/sync-client`
- `packages/browser-adapter-webext` 或 `apps/extension-web/src/adapter`
- `apps/extension-web`（基于 WXT 的扩展壳层）

`packages/browser-adapter-chromium` 是 Iter8 的过渡基线。后续不应继续沿着 `browser-adapter-chromium`、`browser-adapter-firefox`、`browser-adapter-safari` 三套包扩张，而应收敛成 WXT/WebExtension adapter 加少量平台 capability override。

### 8.1 `sync-client` 负责

- local tree normalization
- diff
- preview response handling
- apply plan
- mapping helper
- error model
- manual sync orchestrator
- typed Sync API client

### 8.2 平台 adapter 只负责

- 平台 API 调用
- 本地 node id 解析
- capability 报告
- 扩展本地 sync state 的落盘适配

### 8.3 `apps/extension-web` / WXT 只负责

- manifest 与入口声明
- background/popup/options/side panel 的宿主装配
- 构建、打包、跨浏览器输出
- 注入共享 UI shell 与调用共享包

不要在这一层直接沉淀：

- 浏览器树 diff 算法
- preview/apply 规则解释
- mapping 修复策略
- Safari / Firefox / Chromium 平台差异逻辑

---

## 9. 本地数据模型建议

本地扩展侧至少需要：

- current session
- browser_client_id
- selected profile
- last known cursor per library
- local mapping cache
- pending preview result
- pending apply result
- sync logs

注意：

- 本地缓存不是 source of truth
- 本地缓存丢失后应可通过 rebuild/relogin 修复

---

## 10. 本地操作建议

### 10.1 scan

优先用树扫描，而不是完全依赖事件流。

### 10.2 apply

使用服务端返回的明确 op 列表，不要自行"猜测最终状态"。

### 10.3 rollback

不要求完整事务式回滚，但要保证：

- apply 失败不推进 cursor
- 用户可重新 preview 或 rebuild

---

## 11. 浏览器端最小交付范围

### 桌面 WebExtension MVP

- 登录
- 注册 browser client
- 读取书签树
- 扫描本地变更
- preview
- apply
- ack
- 最小 conflict 显示

默认先以 WXT 产出的 Chromium MV3 构建验证完整闭环，再用同一 app 输出 Firefox 构建并补足必要 compatibility override。

### Safari MVP

- 保存当前页
- 打开 dashboard
- 搜索收藏
- 只读访问基础列表
- 不做完整树同步承诺

---

## 12. 风险清单

### 12.1 用户手动改动本地树

会导致 mapping 失配，需要 rebuild。

### 12.2 平台 API 差异

不要让共享逻辑依赖某一平台特性。

### 12.3 本地存储不可靠

所有关键状态最终都必须可由服务端恢复。

### 12.4 vault 泄露风险

不要把 vault 内容缓存进普通扩展本地状态，除非有非常明确的安全设计。

---

## 13. 与其他文档关系

- 同步语义：`005-SYNC.md`
- Web UI：`007-WEB-UI.md`
- 安全边界：`008-SECURITY.md`
- 交付计划：`009-REPOSITORY-AND-DELIVERY.md`

---

[English](../en/006-BROWSER-ADAPTERS.md) | [中文](006-BROWSER-ADAPTERS.md)
