# amagi

amagi is a self-hosted bookmark control plane.

It is not merely a "bookmark syncer", but a complete system built around **cloud as source of truth**, **policy-driven sync**, **device/browser-specific projection**, **private vault**, and **manual sync first**.

[中文版本](README_zh.md)

> **Status: Prototype / Not Production-Ready**
>
> This project is currently in the prototype stage and is NOT production-ready.
> No stable releases, APIs, or data formats are guaranteed at this time.

Core capabilities:

- Self-hosted, server built with Rust + PostgreSQL
- Dashboard Web UI using Vite + React + TanStack suite + shadcn/ui + Tailwind CSS
- Multi-browser, multi-platform support
- Centralized cloud management for bookmarks, folders, tags, sync rules, devices, and conflicts
- Filter sync scope by device/browser/platform
- Manual sync by default
- Private vault (vault) with step-up authentication
- Custom OIDC login support, with step-up auth / WebAuthn for vault access

## Core Principles

1. **Cloud as Source of Truth**
   - Browser's local bookmark tree is not the database, just a projection of cloud state

2. **Sync is Policy-Driven Projection**
   - Different devices / browsers can receive different content

3. **Separation of Normal and Private Bookmarks**
   - `normal` libraries can be mapped to browser's native bookmark tree
   - `vault` libraries are NOT mapped to browser's native bookmark tree by default

4. **Manual Sync First**
   - Recommended flow: explicit preview -> apply sync process

## Current Boundaries

This project clearly distinguishes:

- Cloud bookmark repository
- Browser's native bookmark tree
- Private vault (vault)

"Full bidirectional sync with native browser bookmark tree" for Safari / iOS / Android is NOT strongly committed as a first-stage goal; see:

- `docs/005-SYNC_zh.md`
- `docs/006-BROWSER-ADAPTERS_zh.md`

## Suggested Documents Reading Order

### For Human Readers

1. `docs/000-OVERVIEW_zh.md`
2. `docs/001-ARCHITECTURE_zh.md`
3. `docs/005-SYNC_zh.md`
4. `docs/006-BROWSER-ADAPTERS_zh.md`
5. `docs/008-SECURITY_zh.md`
6. `docs/009-REPOSITORY-AND-DELIVERY_zh.md`

### For Implementers

1. `docs/002-DOMAIN-MODEL_zh.md`
2. `docs/003-DATABASE_zh.md`
3. `docs/004-API_zh.md`
4. `docs/005-SYNC_zh.md`
5. `docs/007-WEB-UI_zh.md`
6. `docs/009-REPOSITORY-AND-DELIVERY_zh.md`

## Non-Goals (First Stage)

The following capabilities are NOT required for the first stage:

- Full bidirectional sync with Safari's native bookmark tree
- End-to-end encrypted search
- Real-time CRDT collaboration
- Complex multi-tenant team sharing models
- Full-platform native clients

If these capabilities need to be added, they should be built incrementally on top of the existing architecture, rather than refactoring the core sync model in reverse.

## LICENSE

[MPL-2.0](LICENSE.md)

## Naming

The project name `amagi` comes from "Amagi", a state-management maid robot from the light novel series "I'm the Villainous Lord of the Interstellar Nation!".

In this project, amagi's position is:

- Control plane for the bookmark system
- Coordinator between devices and browsers
- Unified entry point for permissions, sync, projection, unlock, and audit
