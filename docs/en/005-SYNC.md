# 005-SYNC

## 1. Document Purpose

This document defines amagi's sync model, sync protocol, conflict handling, projection rules, and implementation suggestions.
This is one of the most critical documents in the entire system.

Related documents:

- Architecture: `001-ARCHITECTURE.md`
- Domain Model: `002-DOMAIN-MODEL.md`
- API: `004-API.md`
- Browser Adapters: `006-BROWSER-ADAPTERS.md`

---

## 2. Sync Design Principles

### 2.1 Cloud as Source of Truth

The browser's local bookmark tree is only a projection.
The goal of sync is to gradually align the local state with what the target should see from the cloud, not to give any single local endpoint the final say.

### 2.2 Sync is Projection, Not Mirroring

Different devices / browsers can see different content.
Therefore:

- A missing local folder does not necessarily mean it was deleted in the cloud
- A folder existing in the cloud does not necessarily need to be delivered to the current target

### 2.3 Manual Sync First

The default workflow is:

1. scan local state
2. generate mutations
3. preview
4. user confirm
5. apply
6. ack

### 2.4 revision + cursor Driven

The server delivers deltas to the target via revision feed.
The target indicates the applied position via cursor.

### 2.5 Conflict is an Explicit Object

Conflicts are not logs; they are displayable and actionable domain objects.

---

## 3. Sync Participants

### 3.1 Source of Truth

Cloud library + revisions.

### 3.2 Target

A specific BrowserClient.
It is described by:

- device
- platform
- browser family
- capabilities
- matched profile

### 3.3 Sync Profile

Defines how the target should sync:

- mode
- direction
- conflict policy
- include/exclude/readonly rules

---

## 4. Sync Modes

### 4.1 Manual

Preferred mode.
Only executes preview/apply when the user explicitly acts.

### 4.2 Scheduled

Executes scan and sync at fixed intervals.
Suitable for desktop, but optional in the first stage.

### 4.3 Auto

Near real-time automatic sync.
Not recommended for default enablement in the first stage.

---

## 5. Sync Directions

### 5.1 Pull-only

Only pulls from the cloud and applies locally.

### 5.2 Push-only

Only sends local changes to the cloud.
Rare, but may be useful during import phase.

### 5.3 Bidirectional

Bidirectional sync.
Still recommended to explicitly confirm via preview/apply.

---

## 6. Projection Rules

### 6.1 Rule Inputs

Rule matching dimensions include:

- target attributes
  - platform
  - device_type
  - device_id
  - browser_family
  - browser_client_id
- content attributes
  - library kind
  - folder id
  - folder path
  - tag

### 6.2 Rule Actions

- `include`
- `exclude`
- `readonly`

### 6.3 Recommended Evaluation Process

1. Select the active profile
2. Evaluate whether the target matches the profile
3. Evaluate rules top-down on the library tree
4. Generate the visible projection for the current target
5. Compute delta / merge / apply on this projection

### 6.4 Default Behavior Recommendations

If a profile matches but no more granular rules apply:

- Normal library defaults to include
- Vault library defaults to exclude

---

## 7. Sync Data Model

### 7.1 Server-Side Revision Event

Each event should at minimum contain:

- rev id
- clock
- op
- node id
- relevant payload

`op` may include:

- create
- update
- move
- delete
- restore

Current Iter7 has implemented the server-side sync backend baseline:

- `POST /api/v1/sync/clients/register`
- `POST /api/v1/sync/session/start`
- `GET /api/v1/sync/feed`
- `POST /api/v1/sync/preview`
- `POST /api/v1/sync/apply`
- `POST /api/v1/sync/cursors/ack`

This baseline is still the minimum viable version:

- Bearer principal remains the only business API authentication baseline
- Feed reads directly from `node_revisions`
- Preview/apply persists two-phase states via `sync_previews`
- Apply reuses `packages/amagi-bookmarks`'s transaction-scoped mutation boundary within a single transaction
- Vault library is excluded by default and does not enter normal sync feed
- This round does not implement full rule engine, automatic background sync, complex three-way merge, or mapping rebuild API

### 7.2 Client Mutation

Local changes reported by the client to the server should include:

- client_mutation_id
- base_clock
- op
- local node reference
- node payload

### 7.3 Mapping

The client must maintain:

- server node id <-> client external id

Otherwise, move/update/delete cannot be performed safely.

---

## 8. Standard Sync Flow

### 8.1 Registration Phase

1. Extension registers BrowserClient after installation
2. Server returns client identity
3. Matches available sync profiles

### 8.2 Regular Sync Phase

1. Client reads local tree
2. Calls feed with local cursor's `lastAppliedClock` as `fromClock` to get server-side delta after that clock
3. Generates local change summary and local mutations
4. Calls preview with the same locally applied clock as `baseClock`; must not directly use `feed.currentClock` as the preview baseline
4. Server:
  - Validates browser client / owner / profile / library
  - Pulls server-side delta
  - Evaluates base projection
  - Accepts or rejects local mutations
  - Generates conflicts
5. User views preview
6. User confirms apply
7. Server writes bookmark mutation, revision, and mapping in a single transaction
8. Returns final local apply ops and new clock
9. Client applies local operations in stages per apply plan
10. Client acks cursor

---

## 9. Preview / Apply Model

### 9.1 Why Preview is Needed

Reasons:

- Users prefer not to auto-sync by default
- Need to let users see the impact before overwriting
- Need to prevent blind writes on conflict

### 9.2 Preview Output

Preview should at minimum return:

- Server -> local ops count
- Local -> server accepted count
- Conflict count
- Readable summary
- Clear conflict detail
- Persisted preview id and expiration time

### 9.3 Apply Semantics

Apply should be based on the preview result; the client should not silently alter behavior by recalculating on its own.
If the preview has expired, a new preview should be requested.
An already `applied` preview must be idempotently replayable and must not create duplicate nodes, revisions, or mappings.

Current Iter8 client baseline:

- `packages/amagi-sync-client` provides `runManualSync()`, with a fixed sequence of register -> session -> feed -> preview -> confirm -> apply -> local apply -> ack.
- If preview has conflicts, saves the pending preview and returns `needs-user-resolution`; does not apply, does not ack.
- If the user has not confirmed apply, saves the pending preview and returns `awaiting-confirmation`.
- If server-side apply succeeds but local adapter apply fails, saves pending recovery state, does not ack cursor.
- If the local cursor is behind and there are no local mutations this round, still converts server ops from preview/apply into a local apply plan; the adapter must successfully apply before acking cursor.
- If the local cursor is behind and there are local mutations this round, the server can return `stale_base_clock` conflict; the client should save the pending preview, not apply, not ack, wait to pull/apply new server ops first, then retry preview.
- The local apply plan currently executes in four phases: create -> update -> move -> delete.
- When the client parses revision payload, field sources prioritize `payload.node.*`: `payload.node.nodeType`, `payload.node.parentId`, `payload.node.title`, `payload.node.url`, `payload.node.sortKey`; for `node.move`, the target parent first takes the top-level `payload.parentId`, then falls back to `payload.node.parentId`.

---

## 10. Conflict Handling

### 10.1 Recommended Conflict Types

#### mapping_missing

The client has lost the mapping between local id and server id.

#### stale_base_clock

Local mutations are based on an outdated clock; this round requires pulling/applying server ops first, then re-previewing.

#### invalid_parent

The parent node resolved by create/move is not a live folder within the current library.

#### unsupported_vault_sync

Vault library does not participate in normal sync feed / preview / apply.

#### projection_violation

The client attempts to push a node that the current profile does not allow exposing.

### 10.2 Default Conflict Strategy Recommendations

#### title/url/meta updates

Last-writer-wins, auditable.

#### move

If the target parent is invalid, place in a conflict holding folder or mark as unresolved.

#### delete vs update

Delete wins by default, but restore entry is retained.

#### duplicate normalized URL

Do not auto-dedupe by default, only provide suggestions.

### 10.3 Conflict Display

Both Dashboard and extension side should at least display:

- Conflict type
- Affected nodes
- Server state summary
- Local state summary
- Recommended resolution

---

## 11. Tombstone and Restore

### 11.1 Why Tombstone is Needed

Without a tombstone, the client struggles to determine:

- Whether something never existed
- Or existed but was deleted

### 11.2 Tombstone Lifecycle

Recommend retaining long enough to span:

- Multiple manual sync cycles
- Offline device reconnection cycles

### 11.3 Restore

Restore is essentially a new revision, not the deletion of tombstone history.

---

## 12. Local Apply Strategy

### 12.1 Idempotency Requirement

Local apply should be designed to be idempotent where possible.
Receiving the same op repeatedly should not corrupt the tree.

### 12.2 Phased Apply

Recommend local apply to execute in at least these phases:

1. Create missing containers
2. Update node payload
3. Move / reorder
4. Delete / cleanup

### 12.3 Failure Recovery

If apply is interrupted:

- Do not advance ack
- Record failure position
- Support retry
- Trigger rebuild mapping if necessary

---

## 13. Rebuild / Resync

### 13.1 When Rebuild is Needed

- User makes extensive manual changes to the tree in the browser
- Local extension state is lost
- Mapping table is corrupted
- Cross-browser migration

### 13.2 Rebuild Goals

- Re-establish server node id to client external id mapping
- Identify obviously matching nodes
- Generate minimal diff repair

### 13.3 Rebuild is Not Blind Full Overwrite

Unless the user explicitly chooses to reset the local tree, prefer matching and repairing first.

---

## 14. Vault and Sync

### 14.1 Default Rule

Vault does not participate in normal sync feed by default.

### 14.2 Special Cases

If future support for certain clients accessing the vault is needed, it must satisfy:

- Target explicitly authorized
- Current unlock session is valid
- Projection explicitly allows
- Local storage risks have been assessed

### 14.3 First Stage Recommendation

Do not deliver vault content to the browser's native bookmark tree.
Vault is only accessed in Web UI / controlled app shell.

---

## 15. Performance Recommendations

### 15.1 First Stage

Prioritize correctness over complex real-time performance.

### 15.2 Scan Strategy

Start with:

- Manually triggered scan
- Periodic lightweight scan
- Change detection based on root hash or subtree hash

### 15.3 Incrementalization

Revision feed should be primarily indexed by `(library_id, logical_clock)`.

---

## 16. Testing Recommendations

At minimum cover:

- create/update/move/delete/restore
- projection include/exclude/readonly
- local create + server create concurrent
- delete vs update
- missing mapping rebuild
- preview expiry apply failure
- cursor idempotent ack
- vault content not entering normal feed

---

## 17. Implementation Priority Recommendations

### First Priority

- Revision model
- Cursor model
- Preview/apply
- Mapping table
- Basic conflict types

### Second Priority

- Rule engine
- Readonly projection
- Rebuild mapping

### Third Priority

- Scheduled sync
- Event listeners
- Richer diff UI

---

## 18. Relationship to Other Documents

- API shape: `004-API.md`
- Browser platform implementation: `006-BROWSER-ADAPTERS.md`
- Security boundaries: `008-SECURITY.md`

---

[English](005-SYNC.md) | [中文](../zh/005-SYNC.md)
