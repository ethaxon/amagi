# 002-DOMAIN-MODEL

## 1. Document Purpose

This document defines amagi's core domain model, including:

- Users, devices, browser clients
- Library, node, metadata
- Policy and sync profile
- Revision, cursor, conflict
- Vault and unlock session

For database persistence see `003-DATABASE.md`.
For API representation see `004-API.md`.

---

## 2. Domain View

amagi's domain can be divided into six subdomains:

1. identity
2. content
3. sync
4. policy
5. vault
6. audit

---

## 3. Identity Subdomain

### 3.1 User

Represents an owner within the amagi bookmark management domain.

`User` is the owning entity for bookmarks, devices, sync profiles, vault unlocks, and other domain objects. It is not equivalent to an OIDC provider account, and does not directly store OIDC claim keys.

Key attributes:

- `user_id`
- `email`
- `display_name`
- `created_at`
- `status`

Responsibilities:

- Owns libraries
- Owns devices
- Owns sync profiles
- Initiates vault unlock
- CRUD and sync on own data

### 3.2 AuthUser

Represents the amagi account principal at the authentication level.

Notes:

- Can be one-to-one with `User` initially
- Should still have an independent stable ID
- Should not assume one auth principal binds to only one OIDC source
- Should not put OIDC claims directly into the domain `User`

### 3.3 OidcAccountBinding

Represents the binding relationship between an external OIDC account and an amagi auth principal / domain owner.

Key attributes:

- `binding_id`
- `auth_user_id`
- `user_id`
- `oidc_source`
- `oidc_subject`
- `oidc_identity_key`
- `claims_snapshot_json`
- `last_seen_at`

Constraints:

- `(oidc_source, oidc_identity_key)` must be unique
- `oidc_subject` must always be stored structurally for subsequent bearer token principal resolution
- `oidc_identity_key` is determined by `oidc_identity_claim`, so it is not fixed to the OIDC `sub`
- Can later support one user binding to multiple OIDC sources or multiple external accounts
- `claims_snapshot_json` is only for audit, troubleshooting, and display assistance; not the sole basis for authorization

The application auth principal `AmagiPrincipal` is resolved from `ExternalOidcIdentity` + `AuthUser` + `User`. It represents the amagi application-layer bound principal, not an alias for the SecurityDept base authenticated principal; vault access still requires independent unlock / authorization decisions.

### 3.4 Device

Represents a physical or logical terminal.

Key attributes:

- `device_id`
- `user_id`
- `device_name`
- `device_type`
- `platform`
- `trust_level`
- `last_seen_at`

Notes:

- A device can have multiple browser clients
- The device itself is one of the policy matching dimensions

### 3.5 BrowserClient

Represents a specific browser instance or extension instance.

Key attributes:

- `browser_client_id`
- `device_id`
- `browser_family`
- `browser_profile_name`
- `extension_instance_id`
- `capabilities`

Notes:

- Sync target is primarily granular at the BrowserClient level
- Both Device and BrowserClient can participate in rule matching

---

## 4. Content Subdomain

### 4.1 Library

Represents a logical bookmark space.

Core attributes:

- `library_id`
- `owner_user_id`
- `kind`
- `name`
- `visibility_policy_id`

Where `kind` at least includes:

- `normal`
- `vault`

Constraints:

- `normal` libraries can participate in normal sync
- `vault` libraries do not participate in normal sync by default; access requires unlock

### 4.2 Node

Represents a node in the bookmark tree.

Node types:

- `folder`
- `bookmark`
- `separator`

Core attributes:

- `node_id`
- `library_id`
- `parent_id`
- `node_type`
- `title`
- `sort_key`
- `is_deleted`
- `created_at`
- `updated_at`

Additional notes:

- `bookmark` nodes have a `url`
- `folder` nodes can contain child nodes
- `separator` nodes have no URL

### 4.3 BookmarkMeta

Represents rich metadata attached to a bookmark.

Can include:

- `description`
- `tags`
- `canonical_url`
- `page_title`
- `favicon_asset_id`
- `reading_state`
- `starred`
- `extra_json`

Notes:

- Metadata should not be overly coupled with the core node table
- The first stage does not require full crawling and archiving

### 4.4 Current Implementation Status (Iter6)

The current bookmark domain implementation is located in `packages/amagi-bookmarks`. This phase has landed the first backend vertical slice for normal library / node / revision:

- Only `kind=normal` libraries can be created; `kind=vault` returns `vault_not_supported_in_iter6` and will not degrade to creating a normal library.
- Creating a library creates a root folder node with `parent_id=null`, `node_type=folder`, `sort_key=root`, and writes the initial revision.
- Dashboard tree response uses a flat adjacency list; UI / sync adapter builds the tree later.
- `bookmark` must provide a non-empty URL; `folder` and `separator` do not accept URLs. Current URL normalize baseline only guarantees trimming and empty-value rejection; scheme/host lowercasing is left for a future normalize pass.
- Delete is logical delete, setting only `is_deleted=true`; restore only restores the target node itself, not the subtree recursively.
- Root node is not allowed to be updated, moved, or deleted via business API.
- Normal create node must specify an undeleted folder parent within the same library; moving prohibits moving to self or descendant.

---

## 5. Tree Model Constraints

### 5.1 Single Parent Tree Structure

Each node has only one `parent_id`.
Multi-parent references are not supported.

### 5.2 Stable Ordering

Sibling node order is expressed by `sort_key`, not insertion time.
This is more suitable for sync and reordering.

### 5.3 Logical Deletion

Node deletion uses logical deletion and retains a tombstone.
Rationale:

- Supports incremental sync
- Supports conflict recovery
- Supports multi-client reconciliation

### 5.4 Path is Not a Primary Key

Folder path is only a display/matching semantic; it should not be the primary entity identifier.

---

## 6. Policy Subdomain

### 6.1 SyncProfile

Defines a configuration set for a class of sync behavior.

Key attributes:

- `profile_id`
- `user_id`
- `name`
- `mode`
- `default_direction`
- `conflict_policy`
- `enabled`

Where `mode` at least includes:

- `manual`
- `scheduled`
- `auto`

The first stage defaults to recommending `manual`.

### 6.2 SyncTargetSelector

Defines which targets a profile applies to.

Matching can be done by:

- platform
- device_type
- device_id
- browser_family
- browser_client_id

### 6.3 SyncRule

Defines include / exclude / readonly trimming rules for content.

Key attributes:

- `rule_order`
- `action`
- `matcher_type`
- `matcher_value`
- `options`

`action` at least includes:

- `include`
- `exclude`
- `readonly`

`matcher_type` can include:

- `folder_id`
- `folder_path`
- `library_kind`
- `tag`

Notes:

- A profile applies to targets
- A rule applies to content
- Together they form a projection

---

## 7. Sync Subdomain

### 7.1 Revision

Represents an ordered change event on the server.

Key attributes:

- `rev_id`
- `library_id`
- `node_id`
- `actor_type`
- `actor_id`
- `op_type`
- `payload`
- `logical_clock`
- `created_at`

Notes:

- Revisions are the basis for sync deltas
- Not all UI events need to be exposed to users, but must be usable for debugging and sync
- In Iter6, each bookmark tree mutation writes to `node_revisions`, with `actor_type=user` and `actor_id` as the bound amagi `user_id`

### 7.2 LibraryHead

Represents the current global logical clock for a library.

Key attributes:

- `library_id`
- `current_revision_clock`

The current implementation advances the clock via `library_heads.current_revision_clock = current_revision_clock + 1 ... RETURNING` within the same transaction, then writes the advanced clock to `node_revisions.logical_clock`. `library.create` initially advances to `1`.

### 7.3 SyncCursor

Represents the position up to which a BrowserClient has synced for a Library.

Key attributes:

- `browser_client_id`
- `library_id`
- `last_applied_clock`
- `last_ack_rev_id`
- `last_sync_at`

### 7.4 Client Mapping

Represents the mapping between server-side nodes and client-side local nodes.

Key attributes:

- `browser_client_id`
- `server_node_id`
- `client_external_id`
- `last_seen_hash`

Notes:

- The browser's local node id is only valid within the context of that client
- Must be bridged through a dedicated mapping table

### 7.5 Conflict

Represents an inconsistency that arises during push / merge / apply.

Example conflict types:

- concurrent update
- move to deleted parent
- delete vs update
- duplicate normalized URL candidate
- local apply blocked

A Conflict is not an exception log, but an explicit domain object that must be displayable and actionable.

---

## 8. Vault Subdomain

### 8.1 VaultLibrary

Essentially a `kind=vault` library, but should be treated separately semantically.

Characteristics:

- Requires unlock to read
- Does not participate in normal sync by default
- Not visible in normal search by default
- Unlock state has a TTL

### 8.2 UnlockSession

Represents temporary access authorization from a user to a vault.

Key attributes:

- `unlock_session_id`
- `user_id`
- `library_id`
- `auth_context`
- `acr`
- `amr`
- `expires_at`

### 8.3 VaultAccessPolicy

Defines the conditions required to access a vault.

Can include:

- Minimum `acr`
- Allowed `amr`
- Unlock TTL
- Whether WebAuthn assertion is required
- Whether to remember the device for a period

---

## 9. Audit Subdomain

At minimum, record the following actions:

- User login
- Step-up auth
- Vault unlock
- Node create/update/move/delete/restore
- Profile/rule changes
- Sync apply
- Conflict resolution

Audit and engineering logs are separate.
Audit should be searchable by user / device / browser client / library.

---

## 10. Core Invariants

### 10.1 Cloud Nodes Must Have Stable Identifiers

Once `server_node_id` is assigned, it is not rebuilt due to client changes.

### 10.2 Vault Must Not Fall Into the Normal Sync Flow by Default

Unless specifically designed and explicitly authorized, vault content must not appear in a normal projection.

### 10.3 Projection is Not a Full View

What a client sees may be only part of the library.
Therefore, a missing local tree entry does not mean it was deleted in the cloud.

### 10.4 Revisions Must Be Ordered

The revision clock within each library must increase monotonically.

### 10.5 Cursors Can Only Advance or Be Rebuilt

Under normal circumstances, cursors should not regress.
If a rebuild is needed, a reindex/resync flow should be explicitly triggered.

---

## 11. Domain Operations

### 11.1 Content Operations

- create folder
- create bookmark
- update title/url/meta
- move node
- reorder siblings
- delete node
- restore node

### 11.2 Sync Operations

- register target
- scan local changes
- preview sync
- apply sync
- ack cursor
- rebuild mapping

### 11.3 Policy Operations

- create profile
- attach target selector
- add rule
- reorder rule
- enable / disable profile

### 11.4 Security Operations

- login
- register passkey
- step-up auth
- unlock vault
- revoke unlock session

---

## 12. Next Reading

- Database storage: `003-DATABASE.md`
- API shape: `004-API.md`
- Sync behavior: `005-SYNC.md`

---

[English](002-DOMAIN-MODEL.md) | [中文](../zh/002-DOMAIN-MODEL.md)
