# 006-BROWSER-ADAPTERS

## 1. Document Purpose

This document defines amagi's adaptation strategy across different browsers/platforms. It does not attempt to fabricate a unified capability, but designs based on actual platform capabilities as boundaries.

For sync protocol see `005-SYNC.md`.
For Web UI see `007-WEB-UI.md`.

---

## 2. Core Principles

### 2.1 Platform Capability Before Abstract Unification

Do not assume all platforms can directly read and write the native bookmark tree in pursuit of uniformity.

### 2.2 Shared Core Logic, Minimal Adapters

Place diff, projection, preview parsing, and other logic in shared packages. Platform adapters are only responsible for interacting with the browser API.

### 2.3 WXT as Extension Shell, Not Core Layer

The project defaults to using WXT as the browser extension engineering shell, rather than continuing to hand-write manifests, entrypoints, and build wrappers for each browser.

WXT's positioning should be limited to:

- Extension development scaffolding
- Build and cross-browser output layer for manifest/background/popup/options/side panel
- Extension UI container and entry orchestration layer
- Accessing browser runtime capabilities via `wxt/browser` / WebExtension API

The following capabilities should not be frozen in the WXT app shell:

- Sync protocol semantics
- Diff / normalization / projection logic
- Policy decisions
- Domain logic beyond platform capability detection

These capabilities should continue to reside in shared packages or thin adapter packages. WXT can smooth over many extension engineering differences, but cannot replace amagi's own sync semantics, projection rules, vault boundaries, and audit requirements.

### 2.4 Safari Handled Separately

Safari is a special case. The first stage should not promise full bidirectional native bookmark tree sync.

---

## 3. Adapter Abstract Interface

The current Iter8 baseline landed shared abstractions in `packages/sync-client`:

```typescript
interface SyncAdapter {
  getCapabilities(): Promise<AdapterCapabilities>;
  loadTree(): Promise<LocalBookmarkNode[]>;
  applyLocalPlan(plan: LocalApplyOp[]): Promise<void>;
}
```

Notes:

- Sync core handles local tree normalization, diff, preview/apply orchestration, server ops -> local apply plan.
- WXT / WebExtension adapter only handles real browser API calls, capability probing, and local state persistence.
- This round does not implement change event driving; still uses manual scan as the primary method.

---

## 4. Chromium Family

Coverage:

- Chrome
- Edge
- Brave
- Vivaldi
- Opera (if API compatible)

### 4.1 First-Stage Capabilities

Considered the primary platform, targeting support for:

- Reading native bookmark tree
- Writing to native bookmark tree
- Local scanning
- Manual sync
- Background message shell
- Popup/options placeholder UI

The current Iter8 baseline implements a pre-migration Chromium-only baseline:

- `packages/browser-adapter-chromium`
  - `createChromiumBookmarkAdapter(chromeLike)`
  - `createChromiumStorage(chrome.storage.local)`
  - `chrome.bookmarks.getTree()` -> `LocalBookmarkNode[]`
  - `LocalApplyOp[]` -> `chrome.bookmarks.create/update/move/remove/removeTree`
- `apps/extension-web`
  - MV3 manifest generation
  - Background service worker
  - Popup/options shell
  - `amagi.sync.preview` / `amagi.sync.apply` / `amagi.sync.status` message baseline

Currently not yet implemented:

- Automatic background sync
- Side panel
- Complete preview/apply UI interaction
- Conflict resolution UI
- Complete mapping reconciliation for server-created local nodes

### 4.2 Recommended Extension Architecture

Default to using WXT to build the extension application, outputting Chrome/Edge/Firefox/Safari builds per target browser. Chromium family delivered as MV3 in the first stage, including:

- Background service worker
- Options page
- Popup
- Side panel (optional but recommended)

WXT is only used for:

- Organizing the above extension entrypoints
- Producing build output for Chromium / Firefox
- Hosting React / Vite and other UI page containers

The actual sync flow orchestration should call shared packages and WXT/WebExtension adapter, rather than writing business logic directly in extension entrypoint files.

### 4.3 Recommended Local State Storage

- browser_client_id
- Dev-only auth config placeholder
- Local mapping cache
- Last normalized tree snapshot
- Pending apply state
- Profile selection

### 4.4 Minimal UI

Should at minimum include:

- Login/connection status
- Current profile
- Preview summary
- Apply button
- Last sync status
- Conflict count

### 4.5 Things Not Recommended

- Do not default to silent background bidirectional auto-sync
- Do not mix vault content directly into the local bookmark tree

---

## 5. Firefox

### 5.1 Overall Strategy

Firefox is no longer planned as a separate first-class adapter package by default. It should reuse the same WXT app and WebExtension adapter, handling differences through WXT's browser target, manifest version target, and runtime feature detection.

### 5.2 Difference Handling

Differences are only encapsulated where necessary:

- Bookmarks API details
- Permission differences
- Storage differences
- Event compatibility differences

If the differences are only in manifest, entrypoint, or build targets, they should be handled in WXT config / entrypoint include-exclude / target branches, rather than creating a new Firefox-specific sync implementation.

### 5.3 First-Stage Goal

Feature parity with Chromium version where possible:

- load tree
- apply ops
- manual preview/apply
- conflict reporting

---

## 6. Safari

### 6.1 Basic Position

Safari is not a first-stage "full bidirectional native bookmark tree sync" platform.

### 6.2 First-Stage Support Goals

Priority should be given to supporting:

- Save current page to amagi
- Search amagi bookmarks
- Open Dashboard
- Import/export bridge
- Vault access entry (if running within a controlled UI)

### 6.3 Non-Promised Capabilities

The first stage does not promise:

- Full read access to Safari's native bookmark tree
- Full write access to Safari's native bookmark tree
- Real-time bidirectional tree sync consistent with Chromium/Firefox

### 6.4 Engineering Strategy Recommendation

The first stage still uses capabilities that WXT/Safari Web Extension can cover for lightweight entry points. If stronger native capabilities are needed later, add a native wrapper; do not promise full Safari native bookmark tree sync just because WXT supports Safari builds.

### 6.5 Future Expansion Path

If investment increases in the future, consider:

- macOS app + Safari Web Extension collaboration
- Import/export bridge
- Management within a controlled app rather than forcibly controlling the native bookmark tree

---

## 7. iOS / Android Mobile

### 7.1 Native Browser Bookmark Tree Sync is Not the Goal

Mobile browser APIs are not uniform and are generally unsuitable for the same level of native tree control as desktop.

### 7.2 First-Stage Recommended Product Forms

- Responsive Web UI
- PWA
- System share entry (later)

### 7.3 Supported Capabilities

- Browse bookmarks
- Search
- Save current page (via share)
- Open links
- Vault unlock
- View sync status (read-only)

### 7.4 Future Enhancement

If stronger biometric capabilities are needed, add native shells:

- iOS: Face ID / Touch ID
- Android: BiometricPrompt

But this does not change the cloud sync model.

---

## 8. Shared Extension Core Recommendations

Recommend establishing:

- `packages/sync-client`
- `packages/browser-adapter-webext` or `apps/extension-web/src/adapter`
- `apps/extension-web` (WXT-based extension shell)

`packages/browser-adapter-chromium` is an Iter8 transitional baseline. The path forward should not expand into three separate packages (`browser-adapter-chromium`, `browser-adapter-firefox`, `browser-adapter-safari`), but converge into a WXT/WebExtension adapter with limited platform capability overrides.

### 8.1 `sync-client` Responsibilities

- Local tree normalization
- Diff
- Preview response handling
- Apply plan
- Mapping helper
- Error model
- Manual sync orchestrator
- Typed Sync API client

### 8.2 Platform Adapter Responsibilities Only

- Platform API calls
- Local node id resolution
- Capability reporting
- Extension local sync state persistence adaptation

### 8.3 `apps/extension-web` / WXT Responsibilities Only

- Manifest and entrypoint declarations
- Background/popup/options/side panel hosting and assembly
- Build, packaging, cross-browser output
- Injecting shared UI shell and calling shared packages

Do not deposit the following in this layer:

- Browser tree diff algorithms
- Preview/apply rule interpretation
- Mapping repair strategies
- Safari / Firefox / Chromium platform difference logic

---

## 9. Local Data Model Recommendations

The local extension side needs at minimum:

- Current session
- browser_client_id
- Selected profile
- Last known cursor per library
- Local mapping cache
- Pending preview result
- Pending apply result
- Sync logs

Note:

- Local cache is not the source of truth
- Lost local cache should be recoverable via rebuild/relogin

---

## 10. Local Operation Recommendations

### 10.1 Scan

Prefer tree scanning over fully relying on event streams.

### 10.2 Apply

Use the explicit op list returned by the server; do not "guess the final state" yourself.

### 10.3 Rollback

Full transactional rollback is not required, but ensure:

- Apply failure does not advance cursor
- User can re-preview or rebuild

---

## 11. Browser-Side Minimum Deliverables

### Desktop WebExtension MVP

- Login
- Register browser client
- Read bookmark tree
- Scan local changes
- Preview
- Apply
- Ack
- Minimal conflict display

Default to first verifying the complete loop with WXT-produced Chromium MV3 build, then use the same app to output Firefox builds and fill in necessary compatibility overrides.

### Safari MVP

- Save current page
- Open dashboard
- Search bookmarks
- Read-only access to basic lists
- No full tree sync promises

---

## 12. Risk Checklist

### 12.1 User Manually Modifying Local Tree

Can cause mapping mismatch, requiring rebuild.

### 12.2 Platform API Differences

Do not let shared logic depend on any single platform feature.

### 12.3 Unreliable Local Storage

All critical state must ultimately be recoverable from the server side.

### 12.4 Vault Leak Risk

Do not cache vault content into regular extension local state unless there is a very clear security design.

---

## 13. Relationship to Other Documents

- Sync semantics: `005-SYNC.md`
- Web UI: `007-WEB-UI.md`
- Security boundaries: `008-SECURITY.md`
- Delivery plan: `009-REPOSITORY-AND-DELIVERY.md`

---

[English](006-BROWSER-ADAPTERS.md) | [中文](../zh/006-BROWSER-ADAPTERS.md)
