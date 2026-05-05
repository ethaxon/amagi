# amagi

amagi is a self-hosted bookmark control plane.

It is not just a bookmark syncer. The project is designed around cloud-owned bookmark state, rule-driven projection, manual preview/apply sync, browser-specific capabilities, and private vault libraries that do not leak into ordinary browser bookmark trees by default.

> **Status: early prototype**
>
> amagi is not production-ready. The architecture, APIs, database schema, configuration surface, browser extension packaging, and sync protocol are still allowed to change. Do not treat the current code or docs as a stable release contract.

---

[English](README.md) | [中文](README_zh.md)

## Use amagi

There is no stable packaged release yet. Use this repository as a development workspace and architecture prototype.

The intended product shape is:

- Rust + Axum API server backed by PostgreSQL
- SeaORM / SeaQuery schema and repository boundary
- SecurityDept token-set OIDC integration for browser and dashboard authentication
- Dashboard Web UI for library, sync, vault, and conflict management
- WXT-based browser extension shell with a shared WebExtension adapter
- Shared TypeScript sync client for manual preview/apply orchestration

The current implementation is a staged baseline, not a finished product. Some surfaces exist only as skeletons or thin vertical slices while the core model is still being validated.

Start with:

- [Overview](docs/en/000-OVERVIEW.md)
- [Architecture](docs/en/001-ARCHITECTURE.md)
- [Sync](docs/en/005-SYNC.md)
- [Browser Adapters](docs/en/006-BROWSER-ADAPTERS.md)
- [Security](docs/en/008-SECURITY.md)
- [Repository and Delivery](docs/en/009-REPOSITORY-AND-DELIVERY.md)

## Develop This Repository

Local setup:

```bash
just setup
```

Start local development dependencies, including PostgreSQL and the local Dex OIDC provider:

```bash
just dev-deps
```

Common loops:

```bash
just dev-api
just dev-dashboard
just dev-extension
just lint
just typecheck
just test
just build
```

If your non-interactive shell cannot find tools managed by `mise`, wrap commands for that shell only:

```bash
mise exec --command "just lint"
```

Do not add `mise exec` noise to project recipes solely for agent shells.

## Current Architecture Boundaries

- The cloud database is the source of truth. A browser's native bookmark tree is only a projection.
- Sync is rule-driven and explicit. The default workflow is scan, preview, user confirmation, apply, then ack.
- Normal libraries and vault libraries are separate security and sync concepts. Vault content must not be sent through ordinary browser sync streams by default.
- Protocol-bound auth endpoints use the stable `/api/auth/...` facade shape; business resources use versioned `/api/v1/...` APIs.
- Browser extension work should converge on WXT plus a shared WebExtension adapter, not long-lived per-browser adapter packages.
- Safari and mobile browsers are degraded-capability targets until their native bookmark control constraints are explicitly solved.

## Documentation Map

Source documentation lives in `docs/en` and `docs/zh`.

For implementers:

- [Domain Model](docs/en/002-DOMAIN-MODEL.md)
- [Database](docs/en/003-DATABASE.md)
- [API](docs/en/004-API.md)
- [Sync](docs/en/005-SYNC.md)
- [Web UI](docs/en/007-WEB-UI.md)
- [Repository and Delivery](docs/en/009-REPOSITORY-AND-DELIVERY.md)

Documentation should describe current behavior or explicit future plans. Historical implementation notes belong in `CHANGELOG.md` or `temp/IMPL_*` iteration files.

## Non-Goals For The First Stage

- Full bidirectional sync with Safari's native bookmark tree
- Native mobile bookmark tree control
- End-to-end encrypted search
- Real-time CRDT collaboration
- Complex multi-tenant team sharing
- Stable public package or Docker release contract

These may be added later, but they should evolve from the documented source-of-truth, projection, sync, and vault model instead of reversing those foundations.

## License

[MPL-2.0](LICENSE.md)

## Naming

The project name `amagi` comes from "Amagi", a state-management maid robot from the light novel series "I'm the Villainous Lord of the Interstellar Nation!".

In this project, amagi's role is the control plane for bookmark state, sync projection, device coordination, authorization, unlock, and audit.

---

[English](README.md) | [中文](README_zh.md)
