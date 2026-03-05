# AGENTS.md

This file is intended for AI agents working within the amagi repository. It defines basic objectives, implementation constraints, reading order, design boundaries, and change requirements. It is referenced by `.cursorrules`, `CLAUDE.md`, and `GEMINI.md` via symbolic links.

For detailed design, always refer to the documentation in `docs/`.

If a task involves specific aspects, continue reading:

- Browser Extension: `docs/006-BROWSER-ADAPTERS.md`
- Dashboard Web UI: `docs/007-WEB-UI.md`
- Authentication / vault / WebAuthn: `docs/008-SECURITY.md`
- Repository Layout / Milestones: `docs/009-REPOSITORY-AND-DELIVERY.md`

## 2. Core Constraints That Must Not Be Violated

### 2.1 Cloud as Source of Truth

Do not treat the local browser bookmark tree as the primary database. The local bookmark tree is merely a projection / materialized view.

### 2.2 Normal Library and Vault Must Be Layered

Do not implement vault as just a `hidden=true` flag on a normal folder and then directly sync it to the browser's native bookmark tree.

### 2.3 Sync Must Be Rule-Driven

Do not implement "mindless full mirror of the entire tree" as the default model. Sync should center around sync profiles + rules.

### 2.4 Manual Sync Preferred by Default

Unless the task explicitly requires otherwise, do not skip the explicit sync flow of `preview -> apply`.

### 2.5 Safari Is a Special Case

Do not assume Safari has equivalent native bookmark API capabilities as Chromium / Firefox. When dealing with Safari, refer to the degradation strategy in `docs/006-BROWSER-ADAPTERS.md`.

## 3. Implementation Style Requirements

### 3.1 Prioritize Clear Boundaries

Prioritize establishing the following boundaries before piling on features:

- domain
- policy
- sync
- auth
- adapters
- ui

### 3.2 Prioritize Auditability

All critical state changes must be traceable, especially:

- bookmark node changes
- sync push / pull
- conflict resolution
- vault unlock
- policy changes

### 3.3 Prioritize Explicit Data Structures

Avoid stuffing core semantics into fuzzy JSON. Use `jsonb` only for extension fields or low-frequency metadata scenarios.

### 3.4 Prioritize Incremental Sync

Prioritize implementing the revision / delta / cursor model rather than full replacement every time.

## 4. Database and API Constraints

### 4.1 Primary Keys and Identifiers

- Server-side entities use stable IDs (UUID/ULID recommended)
- Do not use browser's local node ID as a global primary key
- Client external IDs must be mapped separately

### 4.2 Deletion Strategy

- Logical deletion preferred
- Tombstones must be retained long enough to support sync repair and conflict determination

### 4.3 API Versioning

- `/api/v1/...` preferred
- Backward compatible design for extension-side sync APIs

## 5. Browser Extension Constraints

### 5.1 Share Core Logic

Extension-internal sync / diff / normalize logic should be placed in shared packages as much as possible, rather than being tightly bound to a specific browser platform API.

### 5.2 Minimize Platform Adapters

Adapter layer is only responsible for:

- Reading local tree
- Applying operations
- Scanning changes or listening to changes
- Exposing capabilities

### 5.3 No Commitment to Mobile Native Bookmark Tree Control

Mobile side should prioritize support for:

- Browsing bookmarks
- Saving current page
- Search
- Opening dashboard
- Vault unlock

## 6. Documentation Sync Requirements

Any change that affects the following must update the corresponding docs:

- Domain Model -> `docs/002-DOMAIN-MODEL.md`
- Database -> `docs/003-DATABASE.md`
- API -> `docs/004-API.md`
- Sync Behavior -> `docs/005-SYNC.md`
- Browser Adapter -> `docs/006-BROWSER-ADAPTERS.md`
- Security Model -> `docs/008-SECURITY.md`
- Repository Layout / Milestones -> `docs/009-REPOSITORY-AND-DELIVERY.md`

Do not only change code without updating docs.

## 7. Task Priority Suggestions

Default priority:

1. domain model
2. database schema
3. sync protocol
4. auth / vault
5. dashboard read-path
6. chromium/firefox adapters
7. sync preview/apply UI
8. conflict resolution UI
9. archive/search enrichment
10. safari degraded support

## 8. Allowed Technical Directions

Commands run as justfile commands, like `just setup`, rather than `pnpm run setup` for auto-load dotenv files.

### Backend

- Rust
- Axum
- SeaORM + SeaQuery
- PostgreSQL
- WebAuthn
- OIDC
- OpenDAL
- Snafu

### Frontend

- Vite
- React
- TanStack Router / Query / Table / Virtual
- shadcn/ui
- Tailwind CSS
- pnpm
- biomejs

### Extension

- Shared TypeScript core
- Chromium adapter
- Firefox adapter
- Safari adapter treated separately

## 9. Prohibitions

- Prohibit defaulting vault content to下发 to ordinary browser sync streams
- Prohibit equating local bookmark tree with source of truth
- Prohibit enabling unexpected automatic bidirectional overwrite sync by default
- Prohibit introducing data models that require large-scale reverse migration without explanation
- Prohibit introducing key security behaviors without documentation

## 10. Delivery Standards

A task is considered complete when at least the following are met:

- Code compiles / runs
- Behavior is consistent with docs
- New structures have tests or minimal verification paths
- If deviating from docs, docs must be updated together with explanations
