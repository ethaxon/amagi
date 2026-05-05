# 001-ARCHITECTURE

## 1. Document Purpose

This document defines amagi's overall architecture, component boundaries, runtime responsibilities, and data flow.
For domain objects see `002-DOMAIN-MODEL.md`.
For database persistence see `003-DATABASE.md`.
For sync protocol details see `005-SYNC.md`.

---

## 2. Architecture Overview

amagi recommends starting as a monolith, but logically organizing by module boundary in advance:

- auth
- bookmark domain
- policy
- sync
- vault
- search/archive
- adapters-facing API
- dashboard-facing API

The logical structure is as follows:

1. Dashboard Web UI
2. Browser Extensions / Adapters
3. API Server
4. PostgreSQL
5. Optional workers / object storage / redis

---

## 3. Logical Components

### 3.1 Dashboard Web UI

Responsibilities:

- User login
- Library/tree/list management
- Search and tag operations
- Device and browser client management
- Sync profile / rules management
- Sync preview / conflict center
- Vault unlock entry and status display
- Audit and settings pages

See `007-WEB-UI.md`.

### 3.2 Browser Extensions / Adapters

Responsibilities:

- Read local bookmark tree
- Scan changes or subscribe to changes
- Execute local apply
- Maintain server session
- Initiate preview / apply workflow
- Present minimal sync UI

See `006-BROWSER-ADAPTERS.md` and `005-SYNC.md`.

### 3.3 API Server

Responsibilities:

- User authentication
- Library/tree CRUD
- Revision generation
- Policy evaluation
- Sync feed / push / ack / preview / apply
- Conflict calculation
- Vault unlock session issuance
- Audit logging

### 3.4 PostgreSQL

Responsibilities:

- Primary data storage
- Revision/outbox/sync cursor
- Policy/rules
- Devices/browser clients
- Vault unlock sessions
- Audit data
- Basic search index fields

### 3.5 Workers

Responsibilities:

- Fetch page titles and favicons
- Page archiving
- Metadata enrichment
- Low-priority index updates
- Expired unlock session cleanup
- Async event cleanup and compaction

---

## 4. Recommended Deployment Topology

### 4.1 First Stage: Monolith

Recommended as a single Rust service process, split by module into crates or internal modules.
Benefits:

- Fast to get started
- Avoids premature distribution
- Simplifies transactions and consistency
- Reduces agent implementation complexity

### 4.2 Future: Logical Splitting

When load or collaboration scale increases, it can be split into:

- `amagi-api`
- `amagi-worker`
- `amagi-web`
- `amagi-extension-core`

But at the model level, the boundary definitions in this document should not change.

---

## 5. Key Boundaries

### 5.1 Domain vs Adapter Boundary

Core domain models should not depend on specific browser APIs.
Browsers only expose capabilities through adapters, such as:

- load local tree
- apply ops
- scan local changes
- describe capabilities

### 5.2 Domain vs UI Boundary

The UI only consumes APIs; it should not own hidden core sync logic.
Sync rule evaluation and projection calculation should be done on the server side and in the shared sync core.

### 5.3 Vault vs Normal Library Boundary

The vault is not an additional UI state on top of a normal library, but a separate access level and sync level.
It must be handled separately in API, policy, search, and sync.

### 5.4 Sync vs CRUD Boundary

Plain CRUD is not a sync protocol.
Sync requires independent flows such as revision, cursor, preview, apply, conflict, and ack.

---

## 6. High-Level Data Flow

### 6.1 Dashboard Modifying Bookmarks

1. User modifies a node in the Dashboard
2. API writes to `bookmark_nodes` table
3. Generates revision
4. Updates library head
5. Relevant target receives delta on next pull

### 6.2 Browser Scanning Local Changes

1. Adapter reads local tree and local state
2. Computes local mutation set
3. Calls preview
4. Server evaluates rules, merges policies, detects conflicts
5. User confirms apply
6. Server receives push and generates revisions
7. Returns delta to apply locally
8. Adapter executes local apply
9. Adapter acks cursor

### 6.3 Vault Unlock

1. User accesses vault
2. Current base session does not meet requirements
3. Triggers step-up auth / WebAuthn
4. On success, generates unlock session
5. Vault content is readable only while unlock session is valid

---

## 7. Component Breakdown Recommendations

### 7.1 Auth Module

Responsible for:

- OIDC / token-set integration boundary
- Base session
- Step-up auth
- WebAuthn registration/assertion
- Vault unlock session

amagi does not implement its own OIDC protocol client within this repository. OIDC authorization-code / PKCE, callback exchange, refresh, userinfo, pending OAuth state, and related infrastructure use SecurityDept crates; the recommended Rust entry point is `securitydept-core`, enabling the `token-set-context` and backend-oidc required features, and consuming its re-exported `securitydept-token-set-context::backend_oidc_mode` product surface.

The primary authentication baseline shared by Dashboard, extension popup, side panel, and background sync API should be built around the SecurityDept `token-set-context` `backend-oidc` mode combination. Cookie sessions can optionally supplement same-origin Dashboard UX later, but must not become an implicit prerequisite for extension authentication or sync API.

Auth configuration must distinguish OIDC client / token-set from OAuth resource server / access-token substrate, and support map-like multi-OIDC sources. Each source should directly align with SecurityDept's `oidc`, `backend_oidc`, `frontend_oidc` and `access_token_substrate` typed configs, rather than wrapping another local `provider / oidc_client / *_override` adapter layer in amagi.

The amagi auth module still owns application-side responsibilities: `/api/auth/token-set/oidc/source/{source}/start` facade route wiring, token-set state receipt and storage strategy, extension/browser client session binding, OIDC account binding, auth user / domain user lookup, vault unlock session, domain authorization, and audit event writing. A SecurityDept token-set authenticated principal does not automatically grant vault access.

Authentication endpoints bound to protocols do not use the `/api/v1` prefix. OIDC, token-set, WebAuthn / authenticator paths are primarily constrained by RFC / security flows; bookmark, library, sync profile, sync preview/apply and other business resource interfaces continue to use `/api/v1`.

See `008-SECURITY.md`.

### 7.2 Bookmark Domain Module

Responsible for:

- Library / node / tree
- Title/url/meta
- Tag
- Move/delete/restore
- Node validation

See `002-DOMAIN-MODEL.md`.

### 7.3 Policy Module

Responsible for:

- Sync profile
- Include/exclude/readonly rules
- Target matching
- Vault visibility
- Search visibility

See `002-DOMAIN-MODEL.md` and `005-SYNC.md`.

### 7.4 Sync Module

Responsible for:

- Revision
- Delta feed
- Cursor
- Preview
- Apply
- Conflict
- Ack

See `005-SYNC.md`.

### 7.5 Archive/Search Module

Responsible for:

- URL normalization
- Metadata enrichment
- Search index preparation
- Archive asset handling

Only minimal implementation required in the first stage.

---

## 8. Key Architecture Decisions

### 8.1 Cloud as Source of Truth

Rationale:

- Supports manual sync
- Supports per-target tailoring
- Supports traceable conflicts
- Supports vault non-delivery
- Supports unified audit

### 8.2 Event-Based Revision + Cursor

Rationale:

- Easy incremental sync
- Easy debugging
- Easy replay
- Suitable for multi-client scaling

### 8.3 Platform Capability First

Rationale:

- Platform differences are real
- A unified abstraction can only be built on the lowest common capability
- Safari must be handled as a special case, not forced into consistency

### 8.4 Monolith First

Rationale:

- Current complexity is in sync and model, not service splitting
- Premature microservices would slow implementation and testing

---

## 9. External Dependency Recommendations

### Backend

- Rust
- Axum
- SeaORM + SeaQuery
- PostgreSQL
- OpenID Connect
  - Use [securitydept-core](https://github.com/ethaxon/securitydept) as Rust entry point
  - OIDC / token-set baseline uses SecurityDept `token-set-context` `backend-oidc` mode
  - Do not replicate authorization-code / PKCE / callback / refresh / userinfo flow in amagi route handlers
- WebAuthn
- OpenDAL
- Snafu (do not use thiserror or anyhow)
- Optional Redis, prefer in-memory implementation initially

### Frontend

- Vite (rolldown)
- React
- TanStack Router / Query / Table / Virtual (use TanStack solutions where available in the family)
- shadcn/ui
- Tailwind CSS
- pnpm (do not use npm)
- biomejs

### Extension

- TypeScript
- Shared sync core
- WXT extension shell
- Shared WebExtension adapter
- Platform capability detection

---

## 10. Observability and Audit

The system should at minimum record:

- User login and step-up
- Vault unlock and expiry
- Node changes
- Sync preview / apply
- Conflict creation and resolution
- Target registration and last seen

Logs and audit must not be conflated:

- Logs are for engineering operations
- Audit is for user behavior and system state

---

## 11. Failure Handling Principles

### 11.1 Local Apply Failure

Must not directly overwrite or give up.
Should preserve:

- Local error information
- Unapplied operations
- Current cursor
- Retryable state

### 11.2 Push Merge Failure

Should return a conflict description, not silently swallow.

### 11.3 Vault Unlock Failure

Must not degrade to normal read.
Must be strictly denied.

---

## 12. Next Reading

Continue reading:

- `002-DOMAIN-MODEL.md`
- `003-DATABASE.md`
- `005-SYNC.md`

---

[English](001-ARCHITECTURE.md) | [中文](../zh/001-ARCHITECTURE.md)
