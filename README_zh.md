# amagi

amagi 是一个自托管的收藏夹控制平面（bookmark control plane）。

它不是单纯的“书签同步器”，而是一套围绕 **云端真源**、**策略驱动同步**、**设备/浏览器差异化投影**、**私密收藏库（vault）**、**手动同步优先** 设计的系统。

[English version](README.md)

目标能力：

- 自托管，服务端使用 Rust + PostgreSQL
- Dashboard Web UI 使用 Vite + React + TanStack 全家桶 + shadcn/ui + Tailwind CSS
- 多浏览器、多平台接入
- 云端集中管理收藏、文件夹、标签、同步规则、设备、冲突
- 支持按设备/浏览器/平台筛选同步范围
- 支持默认手动同步
- 支持私密收藏库（vault）与二次解锁
- 支持自定义 OIDC 登录，并为 vault 引入 step-up auth / WebAuthn

## 核心原则

1. **云端为真源**
   - 浏览器本地书签树不是数据库，只是云端状态的投影

2. **同步是规则驱动的投影**
   - 不同设备 / 浏览器拿到的内容可以不同

3. **普通收藏与私密收藏分层**
   - `normal` library 可映射到浏览器原生书签树
   - `vault` library 默认不映射到浏览器原生书签树

4. **手动同步优先**
   - 默认推荐 preview -> apply 的显式同步流程

## 当前边界

本项目明确区分：

- 云端收藏库
- 浏览器原生书签树
- 私密收藏库（vault）

其中 Safari / iOS / Android 的“原生浏览器书签树直接双向同步”不作为首阶段强承诺；详见：

- `docs/005-SYNC.md`
- `docs/006-BROWSER-ADAPTERS.md`

## 建议文档阅读顺序

### 面向人类读者

1. `docs/000-OVERVIEW.md`
2. `docs/001-ARCHITECTURE.md`
3. `docs/005-SYNC.md`
4. `docs/006-BROWSER-ADAPTERS.md`
5. `docs/008-SECURITY.md`
6. `docs/009-REPOSITORY-AND-DELIVERY.md`

### 面向实现者

1. `docs/002-DOMAIN-MODEL.md`
2. `docs/003-DATABASE.md`
3. `docs/004-API.md`
4. `docs/005-SYNC.md`
5. `docs/007-WEB-UI.md`
6. `docs/009-REPOSITORY-AND-DELIVERY.md`

## 非目标（首阶段）

以下能力不作为首阶段必须完成项：

- Safari 原生书签树完整双向同步
- 端到端加密搜索
- 实时 CRDT 协同
- 团队多租户复杂共享模型
- 全平台原生客户端

这些内容若需要推进，应以现有架构为基础增量演进，而不是反向重构核心同步模型。

## 许可证

[MPL-2.0](LICENSE.md)

## 命名

项目名 `amagi` 取自《我是星际国家的恶德领主！》中的国家管理女仆机器人“天城”。
在本项目中，amagi 的定位是：

- 收藏系统的控制平面
- 设备与浏览器之间的协调者
- 权限、同步、投影、解锁与审计的统一入口
