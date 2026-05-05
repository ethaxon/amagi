# 008-SECURITY

## 1. Document Purpose

This document defines amagi's authentication, authorization, vault access control, step-up auth, and WebAuthn-related design.

It covers:

- OIDC login
- Base session
- Step-up auth
- Vault unlock
- WebAuthn / passkey
- Basic audit requirements

Related documents:

- Architecture: `001-ARCHITECTURE.md`
- Domain Model: `002-DOMAIN-MODEL.md`
- API: `004-API.md`

---

## 2. Security Goals

### 2.1 Goals

- Use custom OIDC as the primary login
- Support session-based Dashboard access
- Support controlled browser extension access
- Support step-up unlock for vault
- Support WebAuthn / passkey as a strong authentication method
- Audit key security events

### 2.2 Non-Goals (First Stage)

- Full end-to-end encrypted bookmark system
- Zero plaintext cache architecture on the browser side
- Enterprise-grade multi-tenant complex IAM

---

## 3. Authentication Model

### 3.1 Base Login

Users log in via OIDC to obtain a base authentication identity and amagi-side session state. OIDC / token-set infrastructure uses SecurityDept crates; amagi does not implement its own OIDC protocol client.

The base authentication identity, amagi auth principal, and bookmark management domain owner are different concepts. They can be one-to-one mapped initially, but the database and authorization model must retain independent IDs and binding tables.

The base session can access:

- normal libraries
- devices
- profiles
- search (excluding vault)
- basic sync management pages

The base session does not automatically grant vault access.

For cross-host entrypoints such as Dashboard, extension popup, side panel, and background sync API, the primary authentication form should be based on SecurityDept `token-set-context` `backend-oidc` mode, not on cookie sessions that only suit same-origin Web apps.

### 3.2 Step-up Auth

When users access highly sensitive operations, a stronger authentication context is required.
Typical scenarios:

- Unlocking vault
- Viewing vault search results
- Performing sensitive security setting modifications

Step-up can be achieved via:

- Re-completing OIDC authentication with a higher ACR
- WebAuthn assertion

### 3.3 Unlock Session

After successful step-up, the system issues a short-term unlock session.
The unlock session scope at minimum includes:

- `user_id`
- `library_id`
- `expires_at`

The unlock session only affects vault visibility; it should not replace the normal login session.

---

## 4. OIDC Design

### 4.1 Roles

amagi acts as an OIDC Relying Party.

The Rust integration entry point uses `securitydept-core`, enabling the `token-set-context` and backend-oidc required features, and combining them through its re-exported SecurityDept product surface. amagi does not replicate SecurityDept's OIDC client, pending OAuth state, token exchange, refresh, or userinfo logic.

Before implementing, you must read and align with SecurityDept's auth context / mode documentation and `apps/server/src/config.rs` reference implementation, especially:

- OIDC provider shared defaults
- OIDC client union config
- backend-oidc override
- frontend-oidc override
- OAuth resource server / access-token substrate

The OIDC client and the OAuth resource server are different responsibilities. The former handles login, callback, token exchange, refresh, userinfo, and token-set state; the latter handles API bearer validation, audience / issuer / JWKS / introspection / propagation, and other resource-server semantics. amagi's configuration and runtime must not merge these two categories into a simplified `oidc` block.

SecurityDept's publicly exposed config source, resolved config, override config, token-set mode, and access-token substrate types should be reused directly where possible, or wrapped with thin newtypes / adapters. Do not redefine an entire set of local types with the same fields or similar semantics in amagi; only amagi-specific configurations such as facade paths, browser client binding, vault unlock policy, and audit policy should be owned by amagi.

When amagi needs to fix or restrict certain SecurityDept configuration fields, the ruling is as follows:

- Do not use `serde_json::Value` / `json::Value` as the primary configuration channel for OIDC / token-set. OIDC, token-set, and resource-server configuration must remain as typed models, and should reuse the config source / override / resolved types exported by SecurityDept.
- amagi expresses source key, application facade, browser client binding, vault unlock, and audit policy through its own wrapper; the wrapper resolves SecurityDept types into amagi runtime during compose / validate phase.
- The following paths are computed by amagi based on `source_key` and should not be exposed as user-configurable items:
  - backend-oidc `redirect_path`: `/api/auth/token-set/oidc/source/{source}/callback`
  - frontend-oidc `redirect_path`: `/auth/token-set/oidc/source/{source}/callback`
  - frontend-oidc `config_projection_path`: `/api/auth/token-set/oidc/source/{source}/config`
- If a configuration file explicitly sets one of the above fixed paths, configuration validation must error, not silently override; only a short-term migration window for legacy configuration compatibility may allow warning + override, and the removal plan must be documented in release notes / review.
- `token_propagation` is disabled in amagi. amagi is not a SecurityDept mesh scenario; any configuration that explicitly enables token propagation must be rejected as a security boundary error, not warned and overridden.
- `serde_json::Value` may only be used for explicit extension metadata, raw claim snapshots, or low-frequency debug payloads; it must not carry primary auth protocol configuration, bypass fixed paths, or enable disabled security capabilities.

### 4.2 Login Flow

Recommended:

- Authorization Code Flow
- PKCE

These protocol details are ultimately handled by SecurityDept `securitydept-oidc-client` and `securitydept-token-set-context::backend_oidc_mode`. Iter4 currently only establishes the application-level auth facade: `/api/auth/token-set/oidc/source/{source}/start` and callback paths validate source, return typed placeholders, and generate skeleton audit payloads, but do not execute the authorization-code flow, issue sessions, or fake account binding success in callbacks.

Iter5 current baseline has advanced from placeholders to real runtime/service integration:

- `GET /api/auth/token-set/oidc/source/{source}/start` -> SecurityDept backend-oidc login / authorize
- `GET /api/auth/token-set/oidc/source/{source}/callback` -> fragment redirect callback
- `POST /api/auth/token-set/oidc/source/{source}/callback` -> JSON body callback
- `POST /api/auth/token-set/oidc/source/{source}/refresh` -> refresh body return
- `POST /api/auth/token-set/oidc/source/{source}/metadata/redeem` -> metadata redemption
- `POST /api/auth/token-set/oidc/source/{source}/user-info` -> verified user-info + amagi principal resolution baseline

The frontend callback path `/auth/token-set/oidc/source/{source}/callback` remains a typed path consumed by the frontend app shell and is not mixed with the backend callback.

Authentication protocol endpoints do not use the `/api/v1` prefix. OIDC, token-set, WebAuthn / authenticator paths are constrained by explicit protocol and security flows, and their semantics should not change with business API version evolution like bookmark / sync. `/api/v1` is reserved for business resource APIs or auth-adjacent business operations such as vault unlock.

amagi must support multiple OIDC sources. The configuration structure should use a map-like shape with stable provider keys as map keys, e.g., `oidc_sources.<source_key>`; do not use arrays for provider lists, as arrays are difficult to partially merge by key via Figment. `oidc_source` must be threaded through facade routes, pending state, callbacks, account binding, and audit.

The current top-level auth config entry is located in `packages/amagi-config`, using `default_oidc_source` and `oidc_sources.<source_key>`. Each source directly aligns with SecurityDept typed config, distinguishing `oidc`, `backend_oidc`, `frontend_oidc`, and `access_token_substrate`; `token_set.facade_paths`, token-set storage policy, and browser client binding remain in the amagi application layer. Secret fields have been migrated to SecurityDept upstream `SecretString`, no longer maintaining amagi's own redacted secret wrapper.

The SecurityDept adapter entry is located in `packages/amagi-securitydept`. The current implementation has migrated to SecurityDept `0.3.x`'s typed config / resolved config / runtime/service boundaries; `packages/amagi-securitydept` only retains amagi-host-owned source-key and fixed path metadata, no longer maintaining a mirror projection nearly isomorphic to SecurityDept's resolved config. Protocol truth follows SecurityDept resolved config; amagi only adds route construction, account binding, principal, and audit semantics on top.

Fixed path fields no longer appear in config schema or examples. If a configuration file writes `redirect_url` or `config_projection_path`, it will fail at amagi config validation or SecurityDept fixed-redirect validator stage; the runtime always computes:

- backend-oidc callback: `/api/auth/token-set/oidc/source/{source}/callback`
- frontend-oidc callback: `/auth/token-set/oidc/source/{source}/callback`
- frontend config projection: `/api/auth/token-set/oidc/source/{source}/config`
- token-set OIDC start facade: `/api/auth/token-set/oidc/source/{source}/start`

If a Dashboard cookie/session OIDC flow is later added, it should use a separate namespace such as `/api/auth/session/oidc/source/{source}/start`, and must not reuse the token-set path.

`token_propagation` remains disabled at `packages/amagi-config` validation stage: not configured or explicitly false both do not enable forwarding; any configuration with a forwarding flag set to true will be rejected as a configuration error.

### 4.3 Session Binding

The server maintains its own session; it does not directly treat third-party tokens as the sole source of internal authority.

amagi owns token-set state receipt, storage strategy, extension/browser client session binding, OIDC account binding, auth user / domain user lookup, domain authorization, vault unlock session, and audit event writing. The SecurityDept token-set authenticated principal only represents a base authentication identity, not vault access.

`packages/amagi-auth` handles amagi application-side auth facade, frontend config projection, `ExternalOidcIdentity` / `AmagiPrincipal`, OIDC account binding repository, verified-claims principal resolution, and bearer principal lookup baseline. `packages/amagi-securitydept` continues to only handle SecurityDept typed config / resolved config / runtime projection, and does not handle amagi account binding, principal, or audit semantics.

Cookie sessions can be an optional later addition for Dashboard same-origin UX, but must not become a fundamental assumption for extension sync API.

### 4.4 OIDC Binding Model

OIDC login results should be bound to `oidc_account_bindings`, not written directly to the domain `users` table.

Binding constraints:

- `oidc_source`
- `oidc_subject`
- `oidc_identity_key`
- unique `(oidc_source, oidc_identity_key)`
- `auth_user_id`
- `user_id`
- `claims_snapshot_json`

Notes:

- `oidc_subject` is the protocol principal identifier and must always be stored structurally
- `oidc_identity_key` is determined by `oidc_identity_claim`, so it is not fixed to `sub`
- `oidc_identity_claim` uses typed config, with a default of `sub`, and reserves extension structure for `email`, `name`, `preferred_username`, and custom claims
- A single amagi user should not be assumed to bind to only one OIDC source
- Do not store raw tokens, client secrets, or refresh tokens in claim snapshots
- Claim snapshots may include necessary `email`, `name`, `acr`, `amr` and other audit/display information, but are not the sole basis for authorization lookup

The resource-server / bearer principal baseline also follows this boundary: bearer token validation is handled by SecurityDept access-token substrate; amagi only consumes verified bearer principal facts and looks up existing account bindings by `(oidc_source, oidc_subject)`. Must not reverse-lookup principal via `claims_snapshot_json`, nor use `oidc_identity_key` as the bearer lookup key.

### 4.5 Session Upgrade

If the OIDC Provider supports a higher authentication context, step-up login can be triggered during vault unlock.

---

## 5. WebAuthn / Passkey

### 5.1 Usage

WebAuthn is primarily used for:

- Vault step-up unlock
- High-sensitivity setting confirmation

### 5.2 Registration

Users can register passkeys while logged in.

### 5.3 Assertion

WebAuthn assertion can be required for vault unlock.

### 5.4 Device Remembering

Conservative approach recommended for the first stage:

- Do not default to "remember device for long-term bypass of step-up"
- If supported, should have a short TTL and explicit risk explanation

---

## 6. Vault Access Control

### 6.1 Default Invisibility

Vault does not appear by default in:

- Normal tree browsing
- Normal search results
- Normal sync feed
- Normal extension local cache

### 6.2 Access Conditions

Accessing a vault library requires at minimum:

- User is logged in
- The corresponding library's unlock session is valid

### 6.3 Unlock TTL

Should be configurable, for example:

- Default 5~30 minutes
- Re-step-up required after expiry

### 6.4 Active Locking

Users should be able to actively lock the current vault.

---

## 7. Authorization Model

### 7.1 First Stage

The first stage can use an owner-only model:

- Users only access their own libraries / devices / profiles
- PostgreSQL RLS is enabled simultaneously, using session variables like `amagi.current_user_id` as the database-side owner isolation contract
- API repository/query must still explicitly filter by owner; RLS is a safety net, not a replacement for clear business conditions

### 7.2 Future Extension

If sharing support is added, roles can include:

- viewer
- editor
- admin

But this should not affect the vault base model.

---

## 8. Extension-Side Security

### 8.1 Browser Extension Session

Extensions need their own controlled session, related to but not identical to the Dashboard session.

The extension authentication baseline should preferentially use the SecurityDept `token-set-context` `backend-oidc` mode product, bound by amagi to the specific browser client / extension instance. Do not treat the Dashboard cookie session as an implicit credential for extension background sync API.

### 8.2 Minimal Local Storage

Do not persist vault content in regular extension local state.
Do not persist highly sensitive unlock state long-term.

### 8.3 Capability Reporting

Extensions should report capabilities during registration, but the server cannot solely trust self-reported capabilities.
High-risk actions still require server-side policy restrictions.

---

## 9. Search and Visibility

### 9.1 Normal Search

By default, only search normal libraries.

### 9.2 Vault Search

Can only include vault content when unlock is valid.

### 9.3 Audit

Should record:

- When the user unlocked which vault
- How long the unlock lasted
- Whether the user actively locked it
- Whether unlock failed

---

## 10. Recommended Audit Events

At minimum record:

- user_login
- user_logout
- step_up_started
- step_up_succeeded
- step_up_failed
- webauthn_registered
- webauthn_asserted
- vault_unlock_succeeded
- vault_unlock_failed
- vault_locked
- profile_changed
- sync_apply

---

## 11. Future Encryption Extension

### 11.1 First Stage Recommendation

First implement server-controlled vault visibility and step-up unlock; do not implement end-to-end encryption.

### 11.2 Future Evolution Paths

If stronger privacy is needed later, consider:

- Encrypted storage for vault content
- User-side key wrapping
- Local decryption rendering
- Limited search capability

But this will significantly increase complexity and should not affect first-stage progress.

---

## 12. Security Boundary Summary

Must adhere to:

1. base session != vault access
2. unlock session is short-term and revocable
3. vault does not enter normal sync feed
4. vault does not enter normal extension cache
5. all critical security actions are audited
6. OIDC / token-set protocol infrastructure uses SecurityDept; amagi does not implement OAuth/OIDC flows in business handlers
7. SecurityDept token-set authenticated principal does not automatically grant vault library access

---

## 13. Relationship to Other Documents

- API: `004-API.md`
- Sync: `005-SYNC.md`
- Browser Adapters: `006-BROWSER-ADAPTERS.md`

---

[English](008-SECURITY.md) | [中文](../zh/008-SECURITY.md)
