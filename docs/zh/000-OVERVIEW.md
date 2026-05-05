# 000-OVERVIEW

## 1. 文档目的

本文档是 amagi 的总体说明与导航入口。
它定义项目目标、边界、设计原则、核心对象以及文档之间的关系。

详细技术内容请继续阅读：

- 架构：`001-ARCHITECTURE.md`
- 领域模型：`002-DOMAIN-MODEL.md`
- 数据库：`003-DATABASE.md`
- API：`004-API.md`
- 同步：`005-SYNC.md`
- 浏览器适配：`006-BROWSER-ADAPTERS.md`
- Web UI：`007-WEB-UI.md`
- 安全：`008-SECURITY.md`
- 仓库与交付：`009-REPOSITORY-AND-DELIVERY.md`

---

## 2. 项目定义

amagi 是一个面向自托管场景的收藏夹控制平面（bookmark control plane）。

它面向以下问题：

- 同一个人的收藏需要跨浏览器、跨设备管理
- 不同浏览器不一定应该同步同一批文件夹
- 浏览器本地书签树难以表达权限、审计、冲突、策略
- 私密收藏不应默认暴露到所有终端的原生书签树
- 用户需要显式控制同步，而不是被动自动覆盖

因此，amagi 不把自己定义为 “另一个浏览器书签数据库”，而是定义为：

- 一个云端真源
- 一个策略驱动同步系统
- 一个设备/浏览器投影编排器
- 一个支持私密收藏库与二次解锁的收藏控制平面

---

## 3. 设计目标

### 3.1 功能目标

- 自托管
- Rust + PostgreSQL 后端
- Dashboard Web UI
- OIDC 登录
- vault 二次解锁
- 多设备、多浏览器同步
- 手动同步优先
- 可按设备 / 浏览器 / 平台过滤同步范围
- 支持冲突检测与解决
- 支持收藏、文件夹、标签、元数据管理

### 3.2 结构目标

- 清晰切分 domain / sync / policy / auth / adapters / ui
- 同步协议显式、可测试、可审计
- 浏览器适配器最小化
- WXT 仅作为扩展壳层、构建层与 UI 容器层
- Safari 单独降级处理
- 文档先行，便于 AI agents 与人工协作实现

---

## 4. 核心对象

amagi 处理三类不同对象：

### 4.1 云端收藏库

云端收藏库是 source of truth。
它包含：

- library
- folder
- bookmark
- tag
- metadata
- policy
- revision
- sync cursor
- audit

### 4.2 浏览器原生书签树

浏览器本地书签树是 projection。
它可以是：

- 云端普通库的局部镜像
- 按设备/浏览器裁剪后的结果
- 手动应用后的本地状态

它不是全局真源，也不一定完整。

### 4.3 私密收藏库（vault）

vault 是高敏感收藏空间。
默认行为：

- 不进入普通同步流
- 不映射到浏览器原生书签树
- 不出现在普通搜索
- 访问需要解锁态
- 解锁依赖 step-up auth / WebAuthn / 短期 unlock session

---

## 5. 核心设计原则

### 5.1 云端为真源

浏览器本地书签树不是主数据库。
所有变更最终都归并到云端。

### 5.2 同步是规则驱动的投影

同步不是“全量镜像”，而是：

- 依据 sync profile
- 依据 target device / browser / platform
- 依据 include/exclude/readonly 规则
- 生成目标环境可见的 projection

### 5.3 普通库与 vault 分层

不要试图把 vault 简化成一个普通 folder 加 `hidden=true`。
vault 在模型上是独立的 library kind。

### 5.4 手动同步优先

系统默认推荐：

- scan
- preview
- confirm
- apply

而不是后台无提示自动覆盖。

### 5.5 平台能力必须被承认

Chromium / Firefox 可以较强地控制原生书签树。
Safari / iOS / Android 不能假设拥有同等能力。
技术方案必须以实际平台能力为边界，而不是以理想统一 API 为边界。

---

## 6. 首阶段范围

首阶段应交付：

- Rust API server
- PostgreSQL schema
- Dashboard Web UI
- OIDC 登录
- vault 解锁基础设施
- Chromium extension
- Firefox extension
- sync preview/apply 工作流
- sync profile + rules
- conflict center 基础能力

详见 `009-REPOSITORY-AND-DELIVERY.md`。

---

## 7. 首阶段非目标

首阶段不要求：

- Safari 原生书签树完整双向同步
- 端到端加密搜索
- CRDT 实时协同
- 多租户企业级共享模型
- 移动端原生应用全覆盖

这些都可以在当前架构上逐步演进，但不应影响首阶段的数据模型与同步模型。

---

## 8. 文档使用方式

### 8.1 作为系统设计基线

所有新增代码、表结构、API、同步行为都应与 docs 保持一致。

### 8.2 作为 agent 操作规范

AI agents 不应仅凭局部文件猜测系统行为，必须结合本目录的文档切面工作。

### 8.3 作为架构裁决依据

发生实现分歧时，应优先比较：

- 是否满足云端真源
- 是否保持 vault 分层
- 是否保持策略驱动同步
- 是否承认平台差异

---

## 9. 术语表

### library

一个逻辑收藏空间。
可分为 `normal` 与 `vault`。

### node

收藏树节点，可能是：

- folder
- bookmark
- separator

### projection

某设备/浏览器所看到的本地投影状态。

### sync profile

定义同步方向、模式、规则与目标的配置单元。

### target

某次同步所面向的设备/浏览器实例。

### revision

服务端记录的有序变更事件。

### cursor

某 target 对某 library 已同步到的位置。

### unlock session

用户通过二次认证后获得的短期 vault 可访问状态。

---

## 10. 下一步阅读

继续阅读：

- `001-ARCHITECTURE.md`
- `002-DOMAIN-MODEL.md`

---

[English](../en/000-OVERVIEW.md) | [中文](000-OVERVIEW.md)
