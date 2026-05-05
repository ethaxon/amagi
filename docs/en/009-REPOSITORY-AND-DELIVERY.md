# 009-REPOSITORY-AND-DELIVERY

## 1. Document Purpose

This document defines amagi's recommended repository structure, module organization, implementation order, and milestones. It is oriented toward concrete implementation and subsequent AI agents getting started.

Related documents:

- Overview: `000-OVERVIEW.md`
- Architecture: `001-ARCHITECTURE.md`
- API: `004-API.md`
- Sync: `005-SYNC.md`

---

## 2. Recommended Repository Structure

Monorepo recommended:

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
    amagi-auth/
    amagi-config/
    amagi-securitydept/
    amagi-db/
    amagi-db-migration/
    sync-core/
    browser-adapter-webext/
    dashboard-sdk/
```

---

## 3. Backend Structure Recommendations

Backend code should not be concentrated in a single `apps/api-server` crate long-term. Rust uses crates as the primary compilation unit; stuffing config, SecurityDept auth glue, domain, sync, migration helpers, and other reusable logic all into the API server will slow incremental compilation and hinder reuse by future CLI / job / migration runners.

Principles:

- `apps/api-server` should be a thin app crate, primarily responsible for process bootstrap, HTTP route wiring, state assembly, and app-local glue.
- Logic that can be shared by the API server, CLI, job worker, and migration runner should be moved down to `packages/*` Rust crates.
- A single Rust source file should not carry multiple boundaries. Config, schema, nested env overlay, SecurityDept mapping, runtime resolution, and tests should be split into clear modules.
- For stable types from upstream libraries, prefer reusing or newtype-wrapping them rather than duplicating a set of semantically similar local structs.

### 3.1 `apps/api-server`

Current `apps/api-server` remains a thin app crate:

```
src/
  main.rs
  app.rs
  error.rs
  http/
    routes/
    extractors/
    errors.rs
```

Config loading, schema, nested env overlay, and SecurityDept auth runtime/resolver are not placed in the app crate; they are moved down to `packages/amagi-config` and `packages/amagi-securitydept` respectively. Subsequent domain, policy, sync, vault, audit, db, jobs, and other shared logic should also continue to be split into `packages/*` crates; `apps/api-server` only handles process bootstrap, HTTP route wiring, state assembly, and app-local glue.

The current database migration crate is located at:

```text
packages/amagi-db-migration/
  Cargo.toml
  src/
    defs.rs
    lib.rs
    main.rs
    m20260504_000001_create_core_tables.rs
    rls.rs
    schema.rs
```

`packages/amagi-db-migration` is part of the Rust workspace, with package / binary name `amagi-db-migration`, used for executing `up` / `down` in SeaORM migration CLI style. The migration crate must also retain a library `Migrator` export, for `packages/amagi-db` to use in controlled `auto_migrate` when `database.auto_migrate=true`; shared `DeriveIden` definitions converge in `src/defs.rs`. Migration logic is not placed in `apps/api-server/src/main.rs`, and `apps/api-server` does not directly depend on the migration crate.

Migration implementation requirements:

- Use SeaORM / `sea-orm-migration` 2.0.0-rc.x series.
- Prefer the high-level `sea-orm-migration` API; SeaQuery serves as its expression layer, predicate builder, or necessary helpers.
- Raw SQL is only allowed for PostgreSQL-specific DDL that SeaQuery / sea-orm-migration cannot directly express, and must be converged into small helpers; do not write owner predicates, joins, `EXISTS`, and other structurally expressible logic as large string concatenations.
- Table names, column names, indexes, foreign keys, and tests reuse shared `#[derive(DeriveIden)]` definitions.
- General-purpose schema helpers, RLS helpers, UUIDv7 / JSONB / array default / index helpers must be placed in shared modules such as `schema.rs`, `helpers.rs`, `rls.rs`, and should not remain in private functions of the first migration waiting to be copied.
- PostgreSQL 18's `uuidv7()` is the server-side UUID primary key default strategy.
- Owner-scoped core tables enable RLS from the first migration, and adopt the `amagi.current_user_id` session variable contract.

In the current implementation, `schema.rs` has taken over UUIDv7 / JSONB / array default / index helpers, `rls.rs` has taken over the SeaQuery predicate builder and PostgreSQL policy DDL shell; `m20260504_000001_create_core_tables.rs` only composes core tables and policies.

The current database runtime boundary is located in `packages/amagi-db`:

- Holds `DbRuntime` with a SeaORM `DatabaseConnection`.
- `DatabaseService` handles no-db mode, connection attempts, controlled auto-migrate, database health/readiness state.
- `migrate.rs` executes controlled `up` through the migration crate library `Migrator`, without shelling out to an external binary.
- `entities/` provides SeaORM entity / ActiveModel for `users`, `auth_users`, `oidc_account_bindings`, `audit_events`, `libraries`, `bookmark_nodes`, `bookmark_meta`, `library_heads`, `node_revisions`; auth binding and bookmark domain repository normal CRUD establish static schema boundaries here.
- `rls.rs` provides transaction-local `amagi.current_user_id` helpers, and `amagi.auth_oidc_source` / `amagi.auth_oidc_subject` / `amagi.auth_oidc_identity_key` lookup helpers for the pre-resolution phase of `oidc_account_bindings`, to avoid the business layer writing `SET` SQL manually.
- Raw SQL is retained only in `set_config/current_setting` helpers, `SELECT 1` ping, `to_regclass(...)` readiness check, and migration / policy DDL boundaries that SeaORM does not directly cover.

The API server currently calls `DatabaseService::initialize()` when assembling `AppState` at startup:

- When `database.url` is not configured, the service continues to start, but `/readyz` returns `503` with `database.state=not_configured`.
- When `database.url` is configured, a connection attempt is made at startup.
- When `database.auto_migrate=true`, migration crate library `Migrator::up()` is executed at startup.
- When connection fails or auto-migrate fails, the service retains sanitized state and exposes it via `/healthz` and `/readyz`; the database URL must not be output.

### 3.2 Crate Organization Recommendations

The backend workspace should gradually be split into multiple crates, reserving at minimum:

- `amagi-domain`
- `amagi-policy`
- `amagi-sync`
- `amagi-auth`
- `amagi-config`
- `amagi-securitydept`
- `amagi-db`
- `amagi-api`

Where:

- `amagi-auth`: amagi auth facade, frontend config projection, verified-claims principal resolution, bearer subject lookup baseline, principal/account binding repository.
- `amagi-config`: typed config model, Figment loading, nested env overlay, schema generation, example validation.
- `amagi-securitydept`: SecurityDept config surface adapter, OIDC source resolution, per-source lazy token-set/resource-server runtime/service wrapper.
- `amagi-db`: SeaORM runtime, auto-migrate wiring, readiness/health check, RLS session helper.
- `amagi-bookmarks`: bookmark library / node / meta / revision domain model, repository, service, RLS transaction orchestration, and API-facing DTO. `apps/api-server` dashboard route handlers do not directly write SeaORM or advance revision clocks.
- `amagi-api` / `apps/api-server`: HTTP server assembly.

Early on, a single crate can be retained briefly, but once a file exceeds a clear single boundary, or logic is clearly reusable by CLI / job / tests, it should be extracted into a package crate rather than continuing to add private structures within modules.

The current auth integration boundary is split into two layers:

- `packages/amagi-securitydept`: amagi uses `securitydept-core` as the Rust entry point, combining `token-set-context`'s `backend-oidc`, `frontend-oidc`, and `access_token_substrate` typed config surface, and lazily builds runtime/service wrappers per source key; do not hand-write OIDC authorization-code / PKCE / callback / refresh / userinfo flow in `main.rs` or route handlers.
- `packages/amagi-auth`: handles amagi application-side auth facade, source resolution, frontend config projection, `ExternalOidcIdentity` / `AmagiPrincipal`, verified-claims principal resolution, OIDC account binding repository, and bearer lookup baseline. `apps/api-server` only mounts routes; does not directly compose binding SQL or audit JSON.
- `packages/amagi-bookmarks`: handles the cloud bookmark source-of-truth vertical slice starting from Iter6. All normal library / node mutations must go through the service, setting `amagi.current_user_id` in an owner-scoped transaction, verifying ownership, writing `node_revisions`, and advancing `library_heads.current_revision_clock`.

Integration testing conventions continue to use crate-local `tests/integration/main.rs` as the entry point. Current Postgres/Dex/container-related test utilities are centralized in `packages/amagi-test-utils`; business crates reuse this layer instead of duplicating testcontainers composition logic in their own integration binaries.

---

## 4. Frontend Structure Recommendations

### 4.1 `apps/dashboard-web`

See `007-WEB-UI.md`.

Recommended:

```
src/
  app/
  routes/
  features/
  components/
  lib/
```

### 4.2 `apps/extension-web`

Recommended:

```
entrypoints/
  background.ts
  popup/
  options/
  sidepanel/
src/
  adapter/
  shared/
wxt.config.ts
```

WXT is recommended as the extension shell for this application:

- Handles development scaffolding
- Handles manifest and multi-entrypoint organization
- Handles build output for Chrome/Edge/Firefox/Safari and other target browsers
- Handles popup/options/side panel UI container assembly
- Handles build-time differences via WXT target / manifest version mechanism

Shared logic should not be piled directly into extension entrypoints; it should be moved down to `packages/`.
`apps/extension-web` should not become the layer where sync core implementation lives.

---

## 5. Shared Package Recommendations

### 5.1 `packages/amagi-sync`

Responsibilities:

- BrowserClient register / session / feed / preview / apply / cursor ack service orchestration
- Sync preview / conflict / cursor / mapping repository and DTO
- In-transaction reuse of `packages/amagi-bookmarks`'s transaction-scoped mutation boundary
- Keep `apps/api-server` as a thin app crate; sync business logic is not written in route handlers

### 5.2 `packages/amagi-webext`

Responsibilities:

- WXT / WebExtension bookmarks API encapsulation
- Local node id / tree extraction
- Apply ops
- Extension sync state persistence via `browser.storage.local`
- Platform capability detection
- Necessary Chrome/Firefox/Safari compatibility overrides

The current Iter9 baseline completes the migration from the transitional implementation to a shared package:

- `src/browser-bookmarks.ts`
- `src/browser-storage.ts`
- `src/capabilities.ts`
- Fake `browser`-driven Node tests

Do not expand this back into three long-term maintained adapter packages for Chromium / Firefox / Safari; future browser differences should continue to converge inside `packages/amagi-webext` and WXT target configuration.

### 5.3 Safari Degraded Adapter

Safari first-stage responsibilities are limited:

- Save current page
- Search entry
- Lightweight bridging capability
- Explicitly declare through capability detection that full native bookmark tree sync is not supported

WXT supporting Safari builds does not mean amagi promises full bidirectional Safari native bookmark tree sync; if stronger capabilities are needed later, evaluate a native wrapper.

### 5.4 `packages/dashboard-sdk`

Responsibilities:

- API client
- Shared DTO types
- Auth/session helpers

---

## 6. Implementation Order Recommendations

### Phase 0: Documentation and Baseline

Goals:

- Confirm docs are complete
- Confirm repository skeleton
- Confirm naming and boundaries

Deliverables:

- Current docs
- README
- AGENTS

### Phase 1: Backend Minimum Skeleton

Goals:

- API server starts
- Health/config/logging
- SecurityDept backend-oidc / token-set auth boundary
- users/auth_users/oidc_account_bindings/devices/browser_clients/libraries/bookmark_nodes tables

Deliverables:

- Runnable service
- Initial migrations
- `/healthz` and basic config / logging / error shell
- amagi auth facade and SecurityDept product-side runtime boundary baseline
- SeaORM migration crate and initial core table migration

The first migration targets core tables: `users`, `auth_users`, `oidc_account_bindings`, `devices`, `browser_clients`, `libraries`, `bookmark_nodes`, `bookmark_meta`, `library_heads`, `node_revisions`, `sync_cursors`, `node_client_mappings`, `sync_previews`, `sync_conflicts`, `sync_profiles`, `sync_profile_targets`, `sync_profile_rules`, `vault_unlock_sessions`, `audit_events`.

The following tables and capabilities remain for subsequent migrations or phases: `webauthn_credentials`, `vault_keys`, archive assets, FTS generated columns / indexes, closure table, team sharing / ACL.

### Phase 2: Content Domain and Dashboard Read/Write

Goals:

- Libraries/tree CRUD
- bookmark_meta
- Basic search
- Basic audit

Deliverables:

- Dashboard can browse and edit normal libraries

### Phase 3: Sync Core

Goals:

- Revisions
- library_heads
- Cursors
- Preview/apply
- Mapping
- Basic conflicts

Deliverables:

- Server-side sync API works

### Phase 4: WXT Desktop Extension MVP

Goals:

- Establish extension shell and cross-browser build output based on WXT
- Browser client register
- Local scan
- Preview/apply
- Ack

Deliverables:

- Actual desktop browser manual sync loop

Current Iter10 has advanced this phase to the following baseline:

- `packages/amagi-sync-client` provides typed Sync API client, local tree normalization, diff baseline, apply plan baseline, manual sync orchestrator, and Node tests.
- `packages/amagi-webext` provides the WebExtension bookmarks/storage adapter baseline, capability detection, and fake-browser tests.
- `apps/extension-web` now provides the WXT-based background/popup/options build output baseline, validated for Chrome MV3 build + real extension load smoke, Firefox build + manifest smoke, and Safari build + manifest smoke.
- Mapping reconciliation for server-created local nodes is now completed by `amagi-sync-client` + `amagi-webext`.

Current gaps:

- Automatic background sync, conflict resolution UI, and full options/popup state management are not yet implemented.
- The extension now has a minimal token-set login loop via `packages/amagi-auth-client`, and background manual sync can inject the bearer principal from shared auth state; optional permission authorization for production self-hosted hosts remains future work.

### Phase 5: Sync Profiles and Rules

Goals:

- Target selectors
- Include/exclude/readonly
- Projection tailoring

Deliverables:

- Different devices for the same user see different bookmark sets

### Phase 6: Vault and Step-Up Unlock

Goals:

- Vault libraries
- WebAuthn
- Unlock session
- Vault search and access control

Deliverables:

- Private bookmark library loop

### Phase 7: Conflict Center and Enhanced UI

Goals:

- Conflict list
- Manual resolution
- Better preview viewer

### Phase 8: Safari Degraded Support / Mobile Web

Goals:

- Safari save current page
- Search entry
- PWA basic access

---

## 7. First-Stage MVP Definition

The first-stage MVP now centers first on "desktop browser + Dashboard + a real local auth/sync happy path". Basic vault capabilities remain important, but they are no longer blockers before the experiential demo loop is working.

Must complete:

- OIDC login
- Normal library CRUD
- Revisions + cursors
- Manual preview/apply sync
- WXT Chromium extension MVP
- WXT Firefox build baseline
- Sync profile + rules basics
- SecurityDept token-set frontend SDK integration
- Local Dex login, account binding, and Dashboard/extension manual sync loop
- Conflicts basic view

Can defer:

- Vault library + unlock session
- WebAuthn basics
- Archive worker
- Advanced search relevance
- Safari native tree sync
- Native mobile shell
- Team sharing

---

## 8. Testing Strategy

### 8.1 Backend Must Cover

- Node CRUD
- Move/reorder
- Revision generation
- Preview/apply
- Cursor advance
- Vault visibility
- Unlock expiry

### 8.2 Frontend Must Cover at Minimum

- Route loading
- Tree/list interaction
- Preview/apply UI
- Vault unlock flow

### 8.3 Extension Must Cover at Minimum

- Register
- Load tree
- Diff/scan
- Apply ops
- Ack

---

## 9. Data and Configuration Management

### 9.1 Recommended Configuration Items

- database url: currently a configuration skeleton, does not mean the API server has established a connection pool or connects to the database at startup
- database auto migrate: default off; when enabled, should execute through the migration crate library `Migrator`, not shell out to an external binary
- oidc sources: map-like, multi-source configuration, key corresponds to `oidc_source`
- oidc client union / backend-oidc override / frontend-oidc override
- oauth resource server / access-token substrate
- external base url
- session secret
- webauthn rp id/name
- object storage settings (optional)

OIDC / token-set configuration should map to the SecurityDept `backend-oidc` mode configuration surface. The amagi configuration layer can retain facade path, external base URL, token-set storage policy, browser client binding, and vault unlock policy, but should not replicate SecurityDept's OIDC client configuration parsing and protocol state machine.

The configuration model must distinguish between OIDC client and OAuth resource server:

- OIDC client / token-set: authorization-code, PKCE, callback, token exchange, refresh, userinfo, pending state, backend/frontend OIDC mode override.
- OAuth resource server / access-token substrate: API bearer validation, issuer, audience, JWKS / introspection, token propagation.

Implementation rulings for SecurityDept config reuse:

- OIDC / token-set primary configuration must not degrade to `serde_json::Value` / `json::Value` dynamic validation; must remain typed config, and should preferentially use SecurityDept exported config source / override / resolved config / access-token substrate types, adapted with thin newtypes / wrappers for Figment, schema, or amagi policy where necessary.
- The amagi wrapper handles source key, application facade, browser client binding, vault unlock, audit, and other application-layer policies; SecurityDept types handle provider, OIDC client union, backend/frontend mode override, and OAuth resource-server semantics.
- backend-oidc `redirect_path` is fixed to `/api/auth/token-set/oidc/source/{source}/callback`, frontend-oidc `redirect_path` is fixed to `/auth/token-set/oidc/source/{source}/callback`, frontend-oidc `config_projection_path` is fixed to `/api/auth/token-set/oidc/source/{source}/config`. These values are computed by amagi at compose / validate phase based on `source_key` and are not exposed as user-configurable items.
- If a user explicitly configures one of the above fixed paths, it must default to a configuration error; only short-term compatibility migration may allow warning + override, and the removal plan must be documented in review / release records.
- `token_propagation` is disabled in amagi; any explicit enablement must be a configuration error, not warned and overridden. amagi is currently not a mesh / outpost token propagation scenario.
- `serde_json::Value` is only allowed for extension metadata, OIDC claim snapshots, and similar non-primary protocol configuration; must not be used to bypass typed validation or carry disabled security capabilities.

Multiple OIDC sources must use a map-like structure, e.g., `oidc_sources.<source_key>`, rather than arrays. This allows Figment env / file overlay to merge individual sources by provider key. `source_key` should also serve as the stable value for `oidc_account_bindings.oidc_source`, facade route / callback state, and audit.

`database.url` / `AMAGI_DATABASE__URL` and `oidc.client_secret` / `AMAGI_OIDC_SOURCES__<source>__OIDC__CLIENT_SECRET` must both be treated as secrets. The config structure's `Debug`, diagnostic output, and error paths must not output plaintext database URLs or OIDC secrets; when troubleshooting is needed, only non-sensitive derived information such as whether configured, host/port, or redacted forms may be output.

### 9.2 Configuration Loading Standards

Project configuration should be defined as a reusable top-level config model and loaded with Figment, rather than manually writing `std::env::var` parsing item by item in `main.rs` or `config.rs`.

Requirements:

- Configuration loading should prioritize `Figment`, supporting config file + environment overlay.
- Config file should support TOML at minimum; if JSON / YAML is introduced, they should access the same typed model through Figment providers.
- Env overlay uses SecurityDept server-like `__` nesting delimiter strategy, e.g., `AMAGI_SERVER__HOST`, `AMAGI_DATABASE__URL`, `AMAGI_DATABASE__AUTO_MIGRATE`.
- The initial development phase should not introduce legacy config aliases. Only when there is a real published configuration surface, a clear migration window, and a removal plan, may compatibility aliases be introduced with a time-limited deprecation policy.
- OIDC / token-set configuration should prioritize reusing SecurityDept exposed config source / resolver types; if the current upstream version has not yet exposed a directly reusable Figment provider, maintain a boundary mapping in the amagi typed config to avoid falling back to item-by-item manual env parsing.
- Before implementing OIDC / token-set / OAuth resource server configuration, you must read `~/workspace/securitydept/docs/en/020-AUTH_CONTEXT_AND_MODES.md`, `~/workspace/securitydept/docs/en/007-CLIENT_SDK_GUIDE.md`, and `~/workspace/securitydept/apps/server/src/config.rs`.
- Bool-like configuration must not use ad-hoc `matches!` manual parsing. Use a reusable serde representation such as this project's `BooleanLike` newtype or a confirmed `serde_with` / community helper.
- Invalid bool-like values must produce a configuration error, not silently degrade to `false`.
- Secret fields should uniformly use a redacted wrapper; `Debug`, error paths, and diagnostic output must not leak plaintext.

The current implementation entry is `amagi-config::ApiServerConfig::load()`: it uses Figment to merge `amagi.config.toml` (falling back to `amagi.toml`) with the `AMAGI_` prefix, `__`-delimited formal env overlay. Only formal typed config keys are accepted as env entries, e.g., `AMAGI_SERVER__HOST`, `AMAGI_DATABASE__AUTO_MIGRATE`, `AMAGI_OIDC_SOURCES__default__OIDC__CLIENT_ID`; legacy aliases like `AMAGI_API_HOST`, `AMAGI_OIDC_CLIENT_ID`, `AMAGI_DATABASE_URL` are no longer accepted.

`packages/amagi-config` still retains the amagi host-side typed config entry, used to express the `oidc_sources` map, fixed routes/policies, schema, and env overlay; secret fields have already been migrated to SecurityDept upstream `SecretString`. `packages/amagi-securitydept` has been shrunk to a thin adapter; resolver/runtime directly consumes SecurityDept `0.3.x`'s typed config, resolved config, and runtime/service types, only adding amagi-owned source metadata and fixed paths on top. Do not maintain a mirror projection in amagi that is essentially isomorphic to SecurityDept's resolved config.

### 9.3 Configuration Schema and Example Files

The configuration model must have machine-verifiable schema and example files to avoid structural drift in TOML / JSON / env.

Requirements:

- Provide a JSON Schema aligned with the Rust typed config, preferably generated by `schemars` or equivalent tools from the config struct.
- If OpenAPI 3.1 schema is chosen later, it must also be able to validate the config document structure; do not only maintain natural language field descriptions.
- Provide `amagi.config.example.toml`, covering the full structure, including server, database, multi-OIDC map, OIDC client union / backend override, OAuth resource server, and other non-sensitive examples.
- Provide `.env.example`, containing only a few basic and sensitive items suitable for env, such as config file path, port, `AMAGI_DATABASE__URL`, `AMAGI_OIDC_SOURCES__<source>__OIDC__CLIENT_SECRET`; complex structures should be recommended for TOML / JSON / YAML config files.
- CI or tests must verify that the example config can be parsed by the current config loader, and that the schema and example structure remain in sync.

The current repository has committed `amagi.config.schema.json`, `amagi.config.example.toml`, and `.env.example`. Tests in `packages/amagi-config` verify that the example config can be parsed by the loader, and that the committed schema matches `schemars` generation output.

### 9.4 Environment Tiers

Recommend at minimum:

- local
- dev
- prod

### 9.5 Seed Data

Development seeds can be provided:

- Demo normal library
- Sample sync profile
- Demo vault library should be added when the vault iteration begins; the current happy-path demo must not depend on vault seed data

---

## 10. Engineering Collaboration and Coding Standards

AGENTS / README only retain entry points and brief execution rules. Long-standing engineering standards go in this document; product and architecture details go in the corresponding topic documents.

### 10.1 General Standards

- Comments explain why, not the obvious what.
- If the community already has a mature, modern, well-maintained library covering a general capability, prefer using the library rather than writing infrastructure by hand. Typical examples include WXT, SecurityDept token-set / OIDC, SeaORM migration, testcontainers, Figment, etc.
- App crate / app package is a composition layer; it should not permanently carry reusable business logic. Reusable domain, sync, auth, db, config, adapter, and SDK logic should be moved down to `packages/*`.
- Do not introduce data models, API shapes, or config entry points for short-term demos that will create long-term migration debt; early development allows direct model adjustments and development database cleanup.
- Historical process records go in `CHANGELOG.md` or `temp/IMPL_*` iteration documents, not permanently retained in `docs/` body text.

### 10.2 TypeScript Standards

- Use workspace TypeScript project references for inter-package dependency management.
- Node / browser TypeScript packages default to targeting modern ESM hosts; avoid unnecessary CommonJS compatibility layers.
- For enum-like string domains, use `export const Foo = { ... } as const` + `export type Foo = (typeof Foo)[keyof typeof Foo]`; do not scatter raw string unions and magic strings.
- Repeated strings such as public contracts, message types, audit / telemetry vocabulary, and API path segments should be extracted into named constants.
- Optional parameters for public functions use an options object. Only allow a second positional parameter when the parameter is semantically unique, naturally positional, and unlikely to ever need extension.
- Once a public API needs to add a second or further optional parameter, change the second parameter entirely to an options object, even if this is a breaking change in the early stages.
- Options object naming should be stable and readable, avoiding field names like `flag1`, `mode2` that cannot be maintained long-term.
- Test fakes / fixtures should implement minimal interfaces; do not stuff browser global objects or server API responses carelessly as `any`.

### 10.3 Rust Standards

- Reuse mature crates and upstream types; avoid duplicating semantically similar local structures. SecurityDept-exported OIDC / token-set / resource-server types should be used directly or thinly wrapped.
- Use Snafu for error types; error messages must not leak secrets, database URLs, OIDC client secrets, access tokens, or refresh tokens.
- SeaORM entity / ActiveModel is the default boundary for normal CRUD; raw SQL is only retained in small boundaries such as migration / RLS DDL, `set_config/current_setting`, and readiness probes that ORM cannot easily express.
- Tests requiring a real database or OIDC provider should go in `tests/integration/` and preferentially manage Postgres / Dex via testcontainers.

### 10.4 Shell / YAML / Configuration

- Bash scripts use `set -e`, conditionals use `[[ ]]`, variable expansions are quoted.
- YAML uses 2-space indentation, quoting only when necessary.
- Project commands are exposed through `just`; avoid encouraging scattered commands that bypass dotenv / workspace toolchain in documentation.
- Do not hardcode `mise exec` into user-facing just recipes just because an agent shell did not load the user's `.zshrc` / profile. When the agent itself needs it, wrap execution with `mise exec --command "..."`.

### 10.5 Iteration Close-Out

When completing a full implementation round, format first, then verify health. At minimum cover:

- Relevant formatting / lint fix.
- Relevant lint.
- Relevant TypeScript typecheck / Rust check.
- Relevant build.
- Relevant unit / integration tests.

If a validation cannot be run due to external dependencies or environment constraints, the summary / review response must explicitly record which items were not run and why.

---

## 11. Documentation Maintenance Rules

Any of the following changes must be synchronized with docs:

- Domain model changes: `002-DOMAIN-MODEL.md`
- New tables or table semantic changes
- Database changes: `003-DATABASE.md`
- New APIs or API semantic changes: `004-API.md`
- Sync behavior changes: `005-SYNC.md`
- Browser capability boundary changes: `006-BROWSER-ADAPTERS.md`
- Web UI structure or interaction semantic changes: `007-WEB-UI.md`
- Vault / auth / WebAuthn / token-set semantic changes: `008-SECURITY.md`
- Repository structure, engineering standards, iteration plan changes: `009-REPOSITORY-AND-DELIVERY.md`

README and AGENTS only retain entry-point information. Details are uniformly moved down to `docs/` to avoid duplication.

### 11.1 Multi-Language Documentation Rules

Currently, amagi's authoritative user documentation is primarily in Chinese, located in `docs/zh/`. When English or more languages are introduced later, continue to use a multi-language source document model similar to SecurityDept, rather than casually adding parallel files:

- Only user-facing documents need multi-language support; `AGENTS.md`, tool configuration, and machine-readable schemas are not translated.
- The target structure is `docs/zh/00x-TITLE.md` and `docs/en/00x-TITLE.md`; future languages continue using `docs/{lang}/`.
- Different language versions of the same document should be semantically equivalent, with bidirectional language links at the bottom.
- Non-English documents linking to other docs should preferentially link to the same language version.
- Do not create pseudo-translation documents that only copy or roughly summarize another language; only add a corresponding language `00x-TITLE.md` when semantic equivalence can be maintained long-term.
- README can retain multi-language entry points; long-term details remain in `docs/`.

Implementation iteration documents are uniformly named: Guide uses `temp/IMPL_ITERn_GUIDE_zh.md`, Summary uses `temp/IMPL_ITERn_SUMMARY_zh.md`, Review uses `temp/IMPL_ITERn_REVIEWx.md`. Reviews within the same iteration are numbered starting from 1; review fix summaries are appended directly to the corresponding review file, not as separate fix files.

---

## 12. Recommended Initial Task List

### T1 Initialize monorepo and directory structure

### T2 Establish Rust API server, config, logging, health check

### T3 Establish PostgreSQL migrations: users/auth_users/oidc_account_bindings/devices/browser_clients/libraries/bookmark_nodes/bookmark_meta

### T4 Implement OIDC login and basic session

### T5 Implement normal library CRUD API

### T6 Implement Dashboard Libraries page

### T7 Implement revisions/library_heads/cursors

### T8 Implement sync preview/apply API

### T9 Migrate WXT extension MVP

### T10 Converge the WebExtension adapter, complete mapping reconcile, and establish a Chrome/Firefox/Safari smoke baseline

Additional constraints:

- WXT handles extension host, manifest, entrypoint, and build output
- Chromium / Firefox / Safari differences are primarily handled through WXT target, manifest version, entrypoint include-exclude, and runtime feature detection
- Do not maintain three sets of browser adapter packages long-term; only retain WXT/WebExtension adapter and necessary platform overrides
- `sync-core` does not directly depend on WXT
- Chrome MV3 must have a real extension load smoke; Firefox / Safari stay at build + manifest smoke baseline
- `<all_urls>` does not enter the current manifest; localhost host permissions and the dev-only bearer token remain a development-only entry path

### T11 Implement sync profiles / rules UI and API

Additional constraints:

- In Iter11, `apps/dashboard-web` may first ship as a single-screen sync-management baseline instead of a full multi-page app
- The dev bearer token + localStorage flow is only a development entry point, not a production login UX
- TanStack Router / Query or similar infrastructure should be introduced only when real multi-page routing and shared async state needs appear

### T12 Complete the local SecurityDept token-set login and manual sync happy path

Additional constraints:

- `just dev` should start Postgres, Dex, the API server, Dashboard Web, and the WXT extension, and provide a locally explorable loop.
- Dashboard Web and the extension must no longer use manually entered `devBearerToken` as the main path; the main path must come from the SecurityDept token-set frontend SDK auth snapshot / authorization header.
- Local Dex uses `amagi/amagi`; the OIDC source is `default`; client id / secret must match the committed local demo config.
- The demo loop must at least cover login, account binding, creating or reading a normal library, extension register/start session, preview, apply, and ack or equivalent cursor advancement verification.
- The dev-only bearer token can remain temporarily as a test fallback, but UI and docs must clearly state that it is not the default happy path.

### T13 Implement vault library + unlock session + WebAuthn

---

## 13. Delivery Criteria

A feature can be considered "complete" when it meets at minimum:

- Consistent with the corresponding docs
- Has minimum tests or verification path
- Error paths are explainable
- Does not break the cloud source of truth model
- Does not break the vault layering model
- Does not introduce undocumented behavior

---

## 13. Final Reminder

The real difficulty of amagi is not ordinary CRUD, but:

- Versioned tree
- Policy-driven projection
- Preview/apply sync
- Mapping repair
- Vault visibility boundary
- Platform capability variance

Therefore, implementation should prioritize ensuring these baselines are solid, rather than piling on surface-level features first.

---

## 14. Relationship to Other Documents

- Overview: `000-OVERVIEW.md`
- Architecture: `001-ARCHITECTURE.md`
- Sync: `005-SYNC.md`
- Browser Adapters: `006-BROWSER-ADAPTERS.md`
- Security: `008-SECURITY.md`

---

[English](009-REPOSITORY-AND-DELIVERY.md) | [中文](../zh/009-REPOSITORY-AND-DELIVERY.md)
