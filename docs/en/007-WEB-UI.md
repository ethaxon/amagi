# 007-WEB-UI

## 1. Document Purpose

This document defines the information architecture, key pages, frontend data flow, and recommended directory structure for the amagi Dashboard Web UI.

The current Iter11 Dashboard Web baseline is implemented with:

- Vite
- React
- a lightweight typed API client
- local CSS and React state

TanStack Router / Query / Table / Virtual or other UI infrastructure should be introduced later when route count, async caching, and table complexity actually justify them; this round does not add them just to satisfy a checklist.

Current development-time calling convention:

- Dashboard Web continues to use an absolute API base URL from the dev connection panel
- the default local pairing is `http://localhost:4174` or `http://127.0.0.1:4174` -> API server `http://127.0.0.1:7800`
- the API server provides a restricted CORS baseline that allows those two Dashboard dev origins to send requests with `Authorization` and `X-Amagi-Oidc-Source`

For architecture see `001-ARCHITECTURE.md`.
For API see `004-API.md`.

---

## 2. Design Goals

The Dashboard is not a simple list page, but rather:

- A bookmark control panel
- A sync rule management center
- A device and client observation point
- A vault unlock entry point
- A conflict handling entry point

Therefore, the UI design should serve the following goals:

- Quick browsing and editing of library/tree
- Clear understanding of sync projection
- Display sync risks and conflicts
- Clear distinction between normal and vault
- Support for large trees and large lists

---

## 3. Recommended Information Architecture

Top-level navigation includes:

- Libraries
- Search
- Devices
- Sync
- Vault
- Conflicts
- Settings

---

## 4. Key Pages

### 4.1 Libraries Page

Main workspace.

Recommended layout:

- Left: library tree
- Center: current folder content list / cards
- Right: detail inspector
- Top: global search, quick actions

Supported operations:

- Create folder/bookmark
- Drag-and-drop move
- Batch tagging
- Delete/restore
- Star
- Open details

### 4.2 Search Page

Supports:

- Keyword search
- Tag filtering
- Starred filtering
- Library scope filtering
- Whether to include vault (requires unlock)

### 4.3 Devices Page

Displays:

- devices
- browser clients
- platform/browser family
- last seen
- trust level
- bound profiles

### 4.4 Sync Page

Displays:

- sync profiles
- target selectors
- include/exclude/readonly rules
- preview history
- last sync status

The current Iter11 baseline only implements the first sync profile management screen, including:

- auth panel: API base URL, OIDC source, SecurityDept backend-oidc login / clear local token-set state, authentication status display
- advanced dev fallback: collapsed `devBearerToken` textarea used only to bypass frontend SDK issues during debugging
- profile list: name, enabled, mode, default direction, conflict policy
- selected profile detail: edit profile, list targets, add/delete target, list rules, add/edit/delete rule
- error panel: display API error code / message

### 4.5 Vault Page

Displays:

- vault libraries
- current unlock status
- unlock TTL
- lock / unlock actions
- optional recent audit

### 4.6 Conflicts Page

Displays:

- open conflicts
- conflict type
- affected nodes
- server/local summary
- resolve actions

### 4.7 Settings Page

Displays:

- account
- OIDC provider info
- passkeys
- security preferences

---

## 5. State Management Strategy

### 5.1 Server State

The current Iter11 baseline still uses lightweight React state for this single-screen workflow; once Dashboard expands into multiple pages and shared caches, TanStack Query is the recommended next step. Future Query-owned state should include:

- current user
- libraries
- tree data
- search results
- devices/clients
- profiles/rules
- conflicts
- unlock state

### 5.2 Route State

The current Iter11 baseline is a single sync-management screen and does not yet use a router. In a later multi-page stage, TanStack Router + URL should own:

- selected library
- selected folder
- search keyword
- filters
- pagination
- active tab

### 5.3 Local UI State

Only retains short-lived interaction state, such as:

- modal open/close
- drag state
- selected rows
- right panel collapse
- inline form draft

---

## 6. Recommended Component Layers

### 6.1 Route Layer

Responsibilities:

- route params
- loader
- page assembly

### 6.2 Feature Layer

Responsibilities:

- libraries
- search
- sync
- vault
- conflicts

### 6.3 Shared UI

Responsibilities:

- tree view
- data table
- badges
- forms
- dialogs
- diff viewer

---

## 7. Page Detail Recommendations

### 7.1 Tree View

Requirements:

- Virtualization for large trees
- Expand/collapse support
- Icon and status markers
- Different styling for normal/vault
- Drag-and-drop move support

### 7.2 List/Table View

Recommend using TanStack Table.

Fields may include:

- title
- url
- tags
- starred
- updated_at
- source library
- visibility/sync markers

### 7.3 Details Inspector

Displays:

- title
- url
- normalized url
- description
- tags
- starred
- created/updated time
- revision summary
- sync visibility summary

### 7.4 Diff / Preview Viewer

This is a critical component. Should at minimum show:

- server -> local changes
- local -> server changes
- conflicts
- readonly violations
- affected counts

---

## 8. Vault Interaction Design

### 8.1 Default Isolation

Vault should not be mixed into the normal tree like a regular folder. It should be clearly distinguished at the navigation level.

### 8.2 Unlock Flow

When a user clicks on a vault library:

1. Check unlock state
2. If invalid, show unlock dialog
3. Complete step-up / WebAuthn
4. Obtain unlock session
5. Refresh vault content

### 8.3 Unlock State Display

Should explicitly show:

- remaining TTL
- current authentication method
- lock action

### 8.4 Vault in Search

Not included by default. If the user enables "include vault", unlock should be checked first.

---

## 9. Sync Management Interaction Design

### 9.1 Profile Editor

Should support:

- Basic info
- mode
- direction
- enabled
- target selectors
- ordered rules

### 9.2 Rule Editor

At minimum support:

- action
- matcher type
- matcher value
- reorder

### 9.3 Preview Page

Must be readable, not just raw JSON. Should include:

- summary cards
- grouped diff list
- conflict list
- apply button

### 9.4 Conflict Resolution

At minimum support:

- keep server
- keep local
- move to conflict folder
- mark resolved manually

---

## 10. Recommended Frontend Directory Structure

The current Iter11 baseline may stay lightweight, for example:

```
src/
  App.tsx
  api.ts
  constants.ts
  state.ts
  main.tsx
  styles.css
```

As Dashboard evolves into a real multi-page app, it can gradually move toward:

```
src/
  app/
    router.tsx
    providers.tsx
  routes/
    __root.tsx
    libraries.tsx
    search.tsx
    devices.tsx
    sync.tsx
    vault.tsx
    conflicts.tsx
    settings.tsx
  features/
    libraries/
    search/
    devices/
    sync/
    vault/
    conflicts/
    auth/
  components/
    tree/
    table/
    diff/
    forms/
    layout/
  lib/
    api/
    query/
    utils/
    auth/
  styles/
```

---

## 11. API Client Recommendations

The current Iter11 baseline already requires pulling requests out of component JSX. In the later multi-resource stage, establish `lib/api/`, organized by resource file:

- `me.ts`
- `libraries.ts`
- `nodes.ts`
- `search.ts`
- `devices.ts`
- `syncProfiles.ts`
- `sync.ts`
- `vault.ts`
- `conflicts.ts`

Do not write all requests inside components.

---

## 12. UI Style Recommendations

### 12.1 Normal vs Vault

Must have clear visual distinction:

- Vault uses lock/shield style markers
- Normal uses regular folder/bookmark icons

### 12.2 Risk Prompts

Sync and conflict-related operations must have clear prompts; they should not be understated.

### 12.3 High Density but Not Crowded

Designed for power users:

- List density can be slightly higher
- But partitions and status colors must be clear

---

## 13. Accessibility and Keyboard Operation

At minimum support:

- Tree keyboard navigation
- Table row selection
- Dialog focus trap
- Keyboard shortcut for search
- Keyboard shortcut for new bookmark

---

## 14. First-Stage Delivery Focus

1. Libraries main interface
2. Search
3. Devices
4. Sync Profiles: Iter11 now has the first management-screen baseline
5. Preview/Apply Viewer
6. Vault Unlock
7. Conflicts

Full historical timeline or complex multi-user sharing interface is not required in the first stage.

---

## 15. Relationship to Other Documents

- Architecture: `001-ARCHITECTURE.md`
- API: `004-API.md`
- Sync: `005-SYNC.md`
- Security and unlock: `008-SECURITY.md`

---

[English](007-WEB-UI.md) | [中文](../zh/007-WEB-UI.md)
