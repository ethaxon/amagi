<img src="./assets/icons/cel/icon-1024.png" width="400" height="400" alt="amagi" align="right" />
<div align="center">

# Amagi

>
> — **𝓣𝓱𝓮 𝓔𝔁𝓬𝓮𝓹𝓽𝓲𝓸𝓷𝓪𝓵 𝓑𝓸𝓸𝓴𝓶𝓪𝓻𝓴-𝓜𝓪𝓷𝓪𝓰𝓲𝓷𝓰 𝓜𝓪𝓲𝓭 𝓐𝓷𝓭𝓻𝓸𝓲𝓭-**
>

<!-- placeholder for badgets -->

</div>

## 📖 Introduction

Amagi 是一个自托管的收藏夹控制平面（bookmark control plane）。

它不是单纯的书签同步器。项目围绕云端持有收藏状态、规则驱动投影、手动 preview/apply 同步、浏览器能力差异，以及默认不进入普通浏览器书签树的私密 vault library 来设计。

> **状态：早期原型**
>
> amagi 尚未生产可用。当前架构、API、数据库 schema、配置面、浏览器扩展打包方式和同步协议都仍允许变化。不要把当前代码或文档视为稳定 release contract。

---

[English](README.md) | [中文](README_zh.md)

## 使用 amagi

当前还没有稳定的打包发布版本。本仓库首先是开发工作区和架构原型。

预期产品形态包括：

- Rust + Axum API server，使用 PostgreSQL 作为持久化层
- SeaORM / SeaQuery schema 与 repository 边界
- 基于 SecurityDept token-set OIDC 的浏览器和 Dashboard 认证
- 用于 library、sync、vault、conflict 管理的 Dashboard Web UI
- 基于 WXT 的浏览器扩展壳层和共享 WebExtension adapter
- 用于手动 preview/apply 编排的共享 TypeScript sync client

当前实现是分阶段 baseline，不是完整产品。一些 surface 仍是骨架或窄纵切片，用来验证核心模型。

建议先阅读：

- [概览](docs/zh/000-OVERVIEW.md)
- [架构](docs/zh/001-ARCHITECTURE.md)
- [同步](docs/zh/005-SYNC.md)
- [浏览器适配](docs/zh/006-BROWSER-ADAPTERS.md)
- [安全](docs/zh/008-SECURITY.md)
- [仓库与交付](docs/zh/009-REPOSITORY-AND-DELIVERY.md)

## 开发本仓库

本地初始化：

```bash
just setup
```

启动本地开发依赖，包括 PostgreSQL 和本地 Dex OIDC provider：

```bash
just dev-deps
```

Iter12 happy path 中，`just dev` 和 `just dev-api` 会默认加载 `dev/amagi.config.local.toml`。本地 Dex 演示账号为 `amagi/amagi`，Dashboard 登录回跳到 `http://127.0.0.1:4174/auth/token-set/oidc/source/default/callback`，`devBearerToken` 现在只作为前端 SDK 排障时的 advanced fallback。

常用循环：

```bash
just dev-api
just dev-dashboard
just dev-extension-chrome
just lint
just typecheck
just test
just build
```

如果非交互 shell 找不到 `mise` 管理的工具，只在该 shell 中包一层执行：

```bash
mise exec --command "just lint"
```

不要仅因为 agent shell 没加载用户 shell 初始化，就把 `mise exec` 写进项目 recipe。

## 当前架构边界

- 云端数据库是 source of truth。浏览器原生书签树只是 projection。
- 同步是规则驱动且显式的。默认流程是 scan、preview、用户确认、apply、ack。
- normal library 和 vault library 是不同的安全与同步概念。vault 内容默认不得进入普通浏览器同步流。
- 协议绑定的认证接口使用稳定的 `/api/auth/...` facade；业务资源使用带版本的 `/api/v1/...` API。
- 浏览器扩展方向应收敛到 WXT + 共享 WebExtension adapter，而不是长期维护多套 per-browser adapter package。
- Safari 和移动浏览器在原生书签控制能力明确解决前，都属于降级能力目标。

## 文档地图

源文档位于 `docs/en` 与 `docs/zh`。

实现者入口：

- [领域模型](docs/zh/002-DOMAIN-MODEL.md)
- [数据库](docs/zh/003-DATABASE.md)
- [API](docs/zh/004-API.md)
- [同步](docs/zh/005-SYNC.md)
- [Web UI](docs/zh/007-WEB-UI.md)
- [仓库与交付](docs/zh/009-REPOSITORY-AND-DELIVERY.md)

文档应描述当前行为或明确的未来计划。历史实施过程放在 `CHANGELOG.md` 或 `temp/IMPL_*` 迭代文件中。

## 首阶段非目标

- Safari 原生书签树完整双向同步
- 移动端原生书签树控制
- 端到端加密搜索
- 实时 CRDT 协同
- 复杂多租户团队共享
- 稳定 public package 或 Docker release contract

这些能力可以后续增加，但应从已文档化的 source-of-truth、projection、sync 和 vault 模型增量演进，而不是反向推翻这些基础。

## 许可证

[MPL-2.0](LICENSE.md)

## 命名

项目名 `amagi` 取自《我是星际国家的恶德领主！》中的国家管理女仆机器人“天城”。

在本项目中，amagi 的职责是收藏状态、同步投影、设备协调、授权、解锁与审计的控制平面。

---

[English](README.md) | [中文](README_zh.md)
