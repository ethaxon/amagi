# 003-DATABASE

## 1. Document Purpose

This document provides amagi's recommended database model.
It is not the final SQL DDL, but should serve as the design baseline for table creation and migrations.

Domain definitions see `002-DOMAIN-MODEL.md`.
API see `004-API.md`.

---

## 2. General Principles

### 2.1 PostgreSQL as Primary Database

All core state resides in PostgreSQL in the first stage:

- Users and devices
- Libraries and nodes
- Revisions
- Policies
- Cursors
- Vault unlock
- Audit

### 2.2 Avoid Over-JSON-ification

Core entity fields should be modeled structurally.
`jsonb` is only used for:

- Low-frequency extension fields
- Capability descriptions
- Change payloads
- Audit context

### 2.3 Logical Deletion and Revision Coexist

Entity records retain `is_deleted`
While using `node_revisions` to record event history.

### 2.4 Prefer Explicit Indexes

Sync, search, path matching, and target matching all depend on indexes.
Do not wait for performance issues before adding an index strategy.

### 2.5 Current Migration Implementation Status

Iter2 has established the backend migration crate:

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

Migration implementation baseline:

- Uses SeaORM / `sea-orm-migration` 2.0.0-rc.x series.
- Prefer the high-level `sea-orm-migration` migration API.
- SeaQuery serves as the expression layer and predicate / expression builder under the migration API; do not bypass the migration API to stack raw SeaQuery or raw SQL as the primary implementation.
- PostgreSQL-specific DDL that cannot be directly expressed via SeaQuery / sea-orm-migration may use raw SQL, but must be converged into small helpers, with identifiers or predicates rendered via `DeriveIden` / SeaQuery as input.
- Table names, column names, indexes, foreign keys, and tests should reuse shared `#[derive(DeriveIden)]` definitions, e.g., `defs.rs`.
- UUIDv7 PKs, JSONB defaults, array defaults, index construction, RLS policy generation, and other cross-migration reusable logic should be placed in shared helper modules such as `schema.rs`, `helpers.rs`, `rls.rs`, and should not remain copied in individual migration files.
- The migration crate should maintain both lib + bin dual purpose: external CLI can execute `up` / `down`, `packages/amagi-db` consumes the `Migrator` through configuration for controlled `auto_migrate`; `apps/api-server` does not directly depend on the migration crate.

In the current implementation, `schema.rs` handles UUIDv7 PK, JSONB / array default, and index helpers; `rls.rs` handles SeaQuery builder for owner predicates, predicate rendering, and PostgreSQL RLS DDL shell. The first migration only keeps core table structures and policy combinations, and no longer embeds general-purpose helpers.

The first migration `m20260504_000001_create_core_tables` targets the core tables in sections 3-8 of this document, including `users`, `auth_users`, `oidc_account_bindings`, `devices`, `browser_clients`, `libraries`, `bookmark_nodes`, `bookmark_meta`, `library_heads`, `node_revisions`, `sync_cursors`, `node_client_mappings`, `sync_previews`, `sync_conflicts`, `sync_profiles`, `sync_profile_targets`, `sync_profile_rules`, `vault_unlock_sessions`, `audit_events`.

The following subsequent objects are not established in this round:

- `webauthn_credentials`
- `vault_keys`
- closure table / materialized path acceleration structures
- archive assets / `node_assets`
- FTS generated columns and FTS GIN index
- team sharing / ACL tables

Iter3 has added a database runtime crate:

```text
packages/amagi-db/
	Cargo.toml
	src/
		error.rs
		health.rs
		lib.rs
		migrate.rs
		rls.rs
		runtime.rs
```

Current database runtime behavior:

- When the API server starts, it calls `amagi_db::DatabaseService::initialize()` which reads the typed `DatabaseConfig`.
- When `database.url` is not configured, the service starts in no-db mode; `/healthz` returns `database.state=not_configured`, `/readyz` returns `503`.
- When `database.url` is configured, a SeaORM `DatabaseConnection` is established at startup.
- When `database.auto_migrate=true`, `amagi-db` executes `up` through the migration crate library `Migrator`, without shelling out to an external binary.
- Auto-migrate or connection failures do not include the database URL in error messages; the API server only exposes sanitized states like `connection_failed`, `migration_failed`.
- Readiness checks execute a ping and check whether `public.users` exists, to verify that the first core migration has been applied.

Configuration loading, environment variable mapping, and bool-like parsing rules follow `009-REPOSITORY-AND-DELIVERY.md`. Database URLs and OIDC client secrets are secrets and must not be output in plaintext via `Debug`, diagnostic logs, or error messages.

### 2.6 ID Generation Strategy

PostgreSQL 18 provides native `uuidv7()`. Server-generated stable entity IDs should use `uuid primary key default uuidv7()` to avoid future large-scale primary key default migrations and improve btree index locality.

The following fields do not generate new IDs and therefore should not have `uuidv7()` defaults:

- Shared primary keys / foreign key mirrors, e.g., `bookmark_meta.node_id`, `library_heads.library_id`
- Composite primary key members, e.g., `sync_cursors.browser_client_id`, `sync_cursors.library_id`
- Pure reference fields, e.g., `device_id`, `library_id`, `user_id`

### 2.7 Row-Level Security Baseline

Owner-scoped tables must enable PostgreSQL RLS from the initial migration. The application query layer must still explicitly filter by owner, but the business layer filter cannot be the only security boundary.

Recommended conventions:

- Set `amagi.current_user_id` per request transaction
- Regular user policies match the owner field via `current_setting('amagi.current_user_id', true)::uuid`
- Default invisible / unwritable when no current user is set
- Service/admin maintenance paths must have clear roles or connection pool isolation, documented in code and docs
- `oidc_account_bindings`, before the principal resolves `user_id`, may additionally use `amagi.auth_oidc_source`, `amagi.auth_oidc_subject`, and `amagi.auth_oidc_identity_key` for select-only lookup; this capability serves only auth binding repository / subsequent bearer principal resolution, not as a replacement for owner-scoped RLS

Current runtime helpers are located in `packages/amagi-db/src/rls.rs`:

- `CurrentUserId` uses typed `Uuid`, does not accept empty strings or non-UUID values.
- `set_current_user_id()` calls `set_config('amagi.current_user_id', ..., true)` within `DatabaseTransaction`, using transaction-local semantics.
- `AuthLookupIdentity` / `set_auth_lookup_identity()` sets `amagi.auth_oidc_source`, `amagi.auth_oidc_subject`, and optional `amagi.auth_oidc_identity_key` before principal resolution, for select-only lookup policies on `oidc_account_bindings`.
- The auth binding repository's normal CRUD executes through SeaORM Entity / ActiveModel; raw SQL is retained only in `set_config/current_setting` helpers, `SELECT 1` ping, `to_regclass(...)` readiness check, and migration / policy DDL boundaries that ORM cannot directly handle.
- Helpers are only the database-side isolation contract; repository/query must still explicitly include owner filters.

RLS covers at minimum the owner-scoped core tables: `users`, `auth_users`, `oidc_account_bindings`, `devices`, `browser_clients`, `libraries`, `bookmark_nodes`, `bookmark_meta`, `library_heads`, `node_revisions`, `sync_cursors`, `node_client_mappings`, `sync_previews`, `sync_conflicts`, `sync_profiles`, `sync_profile_targets`, `sync_profile_rules`, `vault_unlock_sessions`, `audit_events`.

RLS policy predicates should ideally be generated using SeaQuery query / expression builders, such as owner column comparison, `EXISTS` subqueries, and join owner derivation. `ALTER TABLE ... ENABLE ROW LEVEL SECURITY`, `FORCE ROW LEVEL SECURITY`, `CREATE POLICY` and similar PostgreSQL policy DDL, if raw SQL is necessary, should keep only the minimal DDL shell and avoid writing predicates as fragile string concatenation.

The current implementation generates owner comparison and multi-layer `EXISTS` subquery predicates via SeaQuery, then embeds the rendered result into PostgreSQL policy DDL via `rls.rs`; real PostgreSQL 18-alpine verification requires: 18 policies created successfully, all 18 owner-scoped tables having both `relrowsecurity` and `relforcerowsecurity` enabled.

---

## 3. Identity and Terminals

### 3.1 `users`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `email text null`
- `display_name text null`
- `status text not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Notes:

- `users` represents the bookmark management domain owner
- Do not store OIDC `sub` or other external claim keys in the `users` table itself
- OIDC to user binding is expressed through the `oidc_account_bindings` table

Index recommendations:

- index on `email`

### 3.2 `auth_users`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `user_id uuid not null unique references users(id)`
- `status text not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Notes:

- `auth_users` is the authentication layer principal
- Can be one-to-one with `users` initially
- Should still retain an independent ID to avoid permanently binding the auth account to the bookmark domain owner

### 3.3 `oidc_account_bindings`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `auth_user_id uuid not null references auth_users(id)`
- `user_id uuid not null references users(id)`
- `oidc_source text not null`
- `oidc_subject text not null`
- `oidc_identity_key text not null`
- `claims_snapshot_json jsonb not null default '{}'::jsonb`
- `last_seen_at timestamptz null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Constraint recommendations:

- unique `(oidc_source, oidc_identity_key)`

Index recommendations:

- index on `(auth_user_id)`
- index on `(user_id)`
- index on `(oidc_source, oidc_subject)`

Notes:

- `oidc_subject` must be stored structurally, not relying on `claims_snapshot_json` for protocol principal lookup
- `oidc_identity_key` is determined by `oidc_identity_claim`, so the OIDC associated key is not assumed to be `sub`
- A user can later bind to multiple OIDC sources or multiple external accounts
- `claims_snapshot_json` only stores necessary snapshots for audit, troubleshooting, and display assistance; does not store raw tokens or client secrets
- `claims_snapshot_json` is not the sole basis for authorization lookup
- The repository lookup path currently matches a unique binding by `(oidc_source, oidc_identity_key)`; structured lookup capability by `(oidc_source, oidc_subject)` is also retained for subsequent OAuth resource server principal resolution
- Initial lookup uses an independent auth lookup session contract; after binding is established, restore owner-scoped `amagi.current_user_id`

### 3.4 `devices`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `device_name text not null`
- `device_type text not null`
- `platform text not null`
- `trust_level text not null`
- `last_seen_at timestamptz null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Index recommendations:

- index on `(user_id, platform)`
- index on `(user_id, device_type)`

### 3.5 `browser_clients`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `device_id uuid not null references devices(id)`
- `browser_family text not null`
- `browser_profile_name text null`
- `extension_instance_id text not null`
- `capabilities_json jsonb not null default '{}'::jsonb`
- `last_seen_at timestamptz null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Constraint recommendations:

- unique `(device_id, extension_instance_id)`

Index recommendations:

- index on `(device_id, browser_family)`

---

## 4. Bookmark Content

### 4.1 `libraries`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `owner_user_id uuid not null references users(id)`
- `kind text not null`
- `name text not null`
- `visibility_policy_id uuid null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Constraint recommendations:

- `kind in ('normal', 'vault')`

Index recommendations:

- index on `(owner_user_id, kind)`

### 4.2 `bookmark_nodes`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `library_id uuid not null references libraries(id)`
- `node_type text not null`
- `parent_id uuid null references bookmark_nodes(id)`
- `sort_key text not null`
- `title text not null`
- `url text null`
- `url_normalized text null`
- `content_hash text null`
- `is_deleted boolean not null default false`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Constraint recommendations:

- `node_type in ('folder', 'bookmark', 'separator')`
- `url is not null` only when `node_type='bookmark'`
- Root folder conventions should be controlled at the domain layer

Index recommendations:

- index on `(library_id, parent_id, is_deleted)`
- index on `(library_id, url_normalized)`
- index on `(library_id, node_type, is_deleted)`

Notes:

- If path acceleration is needed, a closure table or materialized path field can be added later
- The first stage does not require early introduction of a closure table unless rule path matching becomes a performance bottleneck

### 4.3 `bookmark_meta`

Recommended fields:

- `node_id uuid primary key references bookmark_nodes(id)`
- `description text null`
- `tags text[] not null default '{}'`
- `canonical_url text null`
- `page_title text null`
- `favicon_asset_id uuid null`
- `reading_state text null`
- `starred boolean not null default false`
- `extra_json jsonb not null default '{}'::jsonb`
- `updated_at timestamptz not null`

Index recommendations:

- gin index on `tags`
- index on `starred`

---

## 5. Versioning and Sync

### 5.1 `library_heads`

Recommended fields:

- `library_id uuid primary key references libraries(id)`
- `current_revision_clock bigint not null`
- `updated_at timestamptz not null`

### 5.2 `node_revisions`

Recommended fields:

- `rev_id uuid primary key default uuidv7()`
- `library_id uuid not null references libraries(id)`
- `node_id uuid not null references bookmark_nodes(id)`
- `actor_type text not null`
- `actor_id uuid null`
- `op_type text not null`
- `payload_json jsonb not null`
- `logical_clock bigint not null`
- `created_at timestamptz not null`

Constraint recommendations:

- unique `(library_id, logical_clock)`

### 5.3 Current Implementation Status (Iter6)

`packages/amagi-db/src/entities/` has currently completed SeaORM entity / ActiveModel for content and revision tables:

- `libraries`
- `bookmark_nodes`
- `bookmark_meta`
- `library_heads`
- `node_revisions`

The normal CRUD in `packages/amagi-bookmarks` uses these Entity / ActiveModel / query builders. The current sole raw SQL boundary in the bookmark domain is `next_library_clock(...)` within the repository, used for atomically executing `UPDATE library_heads ... RETURNING current_revision_clock` in a single transaction; this helper does not spread to service or API route layers.

Each owner-scoped bookmark service method calls `set_current_user_id(...)` within the transaction to set `amagi.current_user_id`. The service layer still explicitly checks visibility by owner / library; PostgreSQL RLS policy serves as the database-level safety net.

Index recommendations:

- index on `(library_id, logical_clock)`
- index on `(node_id, logical_clock)`
- index on `(actor_type, actor_id)`

Notes:

- `payload_json` expresses incremental information such as update/move/delete
- An outbox table can be introduced later, but the first stage can use `node_revisions` directly as the feed source

### 5.3 `sync_cursors`

Recommended fields:

- `browser_client_id uuid not null references browser_clients(id)`
- `library_id uuid not null references libraries(id)`
- `last_applied_clock bigint not null`
- `last_ack_rev_id uuid null`
- `last_sync_at timestamptz null`
- `updated_at timestamptz not null`

Primary key recommendation:

- primary key `(browser_client_id, library_id)`

### 5.4 `node_client_mappings`

Recommended fields:

- `browser_client_id uuid not null references browser_clients(id)`
- `server_node_id uuid not null references bookmark_nodes(id)`
- `client_external_id text not null`
- `last_seen_hash text null`
- `updated_at timestamptz not null`

Primary key recommendation:

- primary key `(browser_client_id, server_node_id)`

Additional unique constraint recommendation:

- unique `(browser_client_id, client_external_id)`

Notes:

- This table is critical
- Local node ids must not be used as substitutes for server node ids

### 5.5 `sync_previews`

Currently persisted as part of the Iter7 backend baseline.

Fields:

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `browser_client_id uuid not null references browser_clients(id)`
- `library_id uuid not null references libraries(id)`
- `base_clock bigint not null`
- `to_clock bigint not null`
- `status text not null`
- `request_hash text not null`
- `summary_json jsonb not null default '{}'::jsonb`
- `server_ops_json jsonb not null default '[]'::jsonb`
- `accepted_local_mutations_json jsonb not null default '[]'::jsonb`
- `conflicts_json jsonb not null default '[]'::jsonb`
- `expires_at timestamptz not null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`
- `applied_at timestamptz null`

Constraints and indexes:

- `status in ('pending', 'applied', 'expired', 'conflicted')`
- index on `(user_id, created_at desc)`
- index on `(browser_client_id, library_id, status)`
- owner-scoped RLS: `user_id = current_user_id`
- `updated_at` is maintained by a shared PostgreSQL auto-update trigger

Notes:

- The preview record is the sole input source for apply.
- `summary_json` also embeds the `applyResult` needed for idempotent replay after a successful apply.
- Preview default validity is 10 minutes; apply returns `preview_expired` after expiry.

### 5.6 `sync_conflicts`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `browser_client_id uuid not null references browser_clients(id)`
- `library_id uuid not null references libraries(id)`
- `conflict_type text not null`
- `state text not null`
- `summary text not null`
- `details_json jsonb not null`
- `created_at timestamptz not null`
- `resolved_at timestamptz null`
- `resolved_by uuid null references users(id)`

Index recommendations:

- index on `(browser_client_id, state)`
- index on `(library_id, state)`

---

## 6. Policy

### 6.1 `sync_profiles`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `name text not null`
- `mode text not null`
- `default_direction text not null`
- `conflict_policy text not null`
- `enabled boolean not null default true`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Constraint recommendations:

- `mode in ('manual', 'scheduled', 'auto')`

### 6.2 `sync_profile_targets`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `profile_id uuid not null references sync_profiles(id)`
- `platform text null`
- `device_type text null`
- `device_id uuid null references devices(id)`
- `browser_family text null`
- `browser_client_id uuid null references browser_clients(id)`
- `created_at timestamptz not null`

Notes:

- A target selector can be broad or exact
- Matching priority and conflict handling should be defined in the policy layer

### 6.3 `sync_profile_rules`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `profile_id uuid not null references sync_profiles(id)`
- `rule_order integer not null`
- `action text not null`
- `matcher_type text not null`
- `matcher_value text not null`
- `options_json jsonb not null default '{}'::jsonb`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

Constraint recommendations:

- `action in ('include', 'exclude', 'readonly')`

Index recommendations:

- index on `(profile_id, rule_order)`

---

## 7. Vault and Security

### 7.1 `vault_unlock_sessions`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `library_id uuid not null references libraries(id)`
- `auth_context_json jsonb not null`
- `acr text null`
- `amr text[] not null default '{}'`
- `expires_at timestamptz not null`
- `created_at timestamptz not null`
- `revoked_at timestamptz null`

Index recommendations:

- index on `(user_id, library_id, expires_at)`
- partial index on active sessions if needed

### 7.2 `webauthn_credentials`

If WebAuthn is directly integrated in the first stage, create:

- `id uuid primary key default uuidv7()`
- `user_id uuid not null references users(id)`
- `credential_id bytea not null unique`
- `public_key bytea not null`
- `sign_count bigint not null`
- `transports text[] not null default '{}'`
- `aaguid uuid null`
- `nickname text null`
- `created_at timestamptz not null`
- `updated_at timestamptz not null`

### 7.3 Optional `vault_keys`

Only needed when entering the content encryption phase:

- `library_id uuid primary key references libraries(id)`
- `kek_wrapped_dek bytea not null`
- `kek_source text not null`
- `rotation_version integer not null`
- `updated_at timestamptz not null`

Not needed in the first stage.

---

## 8. Audit

### 8.1 `audit_events`

Recommended fields:

- `id uuid primary key default uuidv7()`
- `user_id uuid null references users(id)`
- `device_id uuid null references devices(id)`
- `browser_client_id uuid null references browser_clients(id)`
- `library_id uuid null references libraries(id)`
- `event_type text not null`
- `payload_json jsonb not null`
- `created_at timestamptz not null`

Index recommendations:

- index on `(user_id, created_at desc)`
- index on `(library_id, created_at desc)`
- index on `(event_type, created_at desc)`

---

## 9. Search and Archiving (First Stage Minimal)

### 9.1 PostgreSQL FTS

In the first stage, full-text search support can be established on `bookmark_nodes.title`, `bookmark_meta.description`, and `bookmark_meta.page_title`.

Optional approach:

- Generate `tsvector` column
- GIN index

### 9.2 Archive Assets

If favicon/page snapshots are needed, additional tables can be created:

- `assets`
- `node_assets`

But this is not a first-stage requirement.

---

## 10. Migration Strategy Recommendations

### 10.1 Create Core Tables First

Recommended order for the first batch of migrations:

1. users
2. auth_users
3. oidc_account_bindings
4. devices
5. browser_clients
6. libraries
7. bookmark_nodes
8. bookmark_meta
9. library_heads
10. node_revisions
11. sync_cursors
12. node_client_mappings
13. sync_previews
14. sync_conflicts
15. sync_profiles
16. sync_profile_targets
17. sync_profile_rules
18. vault_unlock_sessions
19. audit_events

In the target implementation, the above tables are created by `packages/amagi-db-migration/src/m20260504_000001_create_core_tables.rs`'s `up`, and rolled back in reverse dependency order by `down`. Execution method:

```sh
DATABASE_URL=postgres://amagi:<redacted>@localhost:5432/amagi cargo run -p amagi-db-migration -- up
DATABASE_URL=postgres://amagi:<redacted>@localhost:5432/amagi cargo run -p amagi-db-migration -- down
```

Local verification must use the PostgreSQL 18-alpine development database provided by `just dev-deps` to actually execute `up` / `down`, and check key constraints, `uuidv7()` defaults, and RLS policy status.

### 10.2 Conservative Extension

The following capabilities are recommended for subsequent migration additions, rather than pre-embedding too deeply from the start:

- Encryption key tables
- Closure table
- Archive asset tables
- More complex sharing/permission tables

---

## 11. Data Integrity Constraint Recommendations

### 11.1 Application-Level Invariants

The following should ideally be guaranteed by the application layer:

- Each library has a logical root
- Move does not create cycles
- A folder cannot be placed under a bookmark
- Vault libraries do not participate in normal profile projection
- `logical_clock` increases monotonically

### 11.2 Database-Level Safety

The following should be directly constrained by the database:

- Foreign keys
- Unique constraints
- Enum-like checks
- Non-null primary fields
- Cursor primary key uniqueness
- Mapping uniqueness

---

## 12. Relationship to Other Documents

- Domain meaning: `002-DOMAIN-MODEL.md`
- API request/response: `004-API.md`
- Sync read/write flow: `005-SYNC.md`
- Vault/step-up security constraints: `008-SECURITY.md`

---

[English](003-DATABASE.md) | [中文](../zh/003-DATABASE.md)
