# 000-OVERVIEW

## 1. Document Purpose

This document is the general description and navigation entry point for amagi.
It defines the project's goals, boundaries, design principles, core objects, and relationships between documents.

For detailed technical content, continue reading:

- Architecture: `001-ARCHITECTURE.md`
- Domain Model: `002-DOMAIN-MODEL.md`
- Database: `003-DATABASE.md`
- API: `004-API.md`
- Sync: `005-SYNC.md`
- Browser Adapters: `006-BROWSER-ADAPTERS.md`
- Web UI: `007-WEB-UI.md`
- Security: `008-SECURITY.md`
- Repository & Delivery: `009-REPOSITORY-AND-DELIVERY.md`

---

## 2. Project Definition

amagi is a self-hosted bookmark control plane.

It addresses the following problems:

- A single person's bookmarks need to be managed across browsers and devices
- Different browsers should not necessarily sync the same set of folders
- The browser's native bookmark tree struggles to express permissions, audit, conflicts, and policies
- Private bookmarks should not be exposed by default to all terminals' native bookmark trees
- Users need explicit control over sync, not passive automatic overwriting

Therefore, amagi defines itself not as "yet another browser bookmark database", but as:

- A cloud source of truth
- A policy-driven sync system
- A device/browser projection orchestrator
- A bookmark control plane supporting private vault libraries and step-up unlock

---

## 3. Design Goals

### 3.1 Functional Goals

- Self-hosted
- Rust + PostgreSQL backend
- Dashboard Web UI
- OIDC login
- Vault step-up unlock
- Multi-device, multi-browser sync
- Manual sync first
- Filter sync scope by device / browser / platform
- Conflict detection and resolution
- Bookmark, folder, tag, and metadata management

### 3.2 Structural Goals

- Clear separation of domain / sync / policy / auth / adapters / UI
- Explicit, testable, auditable sync protocol
- Minimal browser adapters
- WXT as extension shell, build layer, and UI container layer only
- Safari handled separately with degraded support
- Documentation-first, enabling collaboration between AI agents and humans

---

## 4. Core Objects

amagi deals with three distinct types of objects:

### 4.1 Cloud Bookmark Library

The cloud bookmark library is the source of truth.
It contains:

- library
- folder
- bookmark
- tag
- metadata
- policy
- revision
- sync cursor
- audit

### 4.2 Browser Native Bookmark Tree

The browser's local bookmark tree is a projection.
It can be:

- A partial mirror of a cloud normal library
- The result of per-device/browser tailoring
- A local state after manual apply

It is not the global source of truth, nor is it necessarily complete.

### 4.3 Private Vault Library

The vault is a high-sensitivity bookmark space.
Default behavior:

- Not included in the normal sync flow
- Not mapped to the browser's native bookmark tree
- Not shown in normal search results
- Requires unlock state for access
- Unlock depends on step-up auth / WebAuthn / short-term unlock session

---

## 5. Core Design Principles

### 5.1 Cloud as Source of Truth

The browser's local bookmark tree is not the primary database.
All changes ultimately converge to the cloud.

### 5.2 Sync is Policy-Driven Projection

Sync is not "full mirroring", but rather:

- Based on sync profiles
- Based on target device / browser / platform
- Based on include/exclude/readonly rules
- Generates a projection visible to the target environment

### 5.3 Normal and Vault Libraries are Separate Layers

Do not attempt to simplify a vault into a normal folder with `hidden=true`.
A vault is an independent library kind at the model level.

### 5.4 Manual Sync First

The system defaults to recommending:

- scan
- preview
- confirm
- apply

Rather than silent automatic background overwriting.

### 5.5 Platform Capabilities Must Be Acknowledged

Chromium / Firefox can exercise stronger control over the native bookmark tree.
Safari / iOS / Android cannot be assumed to have equivalent capability.
Technical solutions must be bounded by actual platform capabilities, not by an idealized unified API.

---

## 6. First-Stage Scope

The first stage should deliver:

- Rust API server
- PostgreSQL schema
- Dashboard Web UI
- OIDC login
- Vault unlock infrastructure
- Chromium extension
- Firefox extension
- Sync preview/apply workflow
- Sync profile + rules
- Basic conflict center capabilities

See `009-REPOSITORY-AND-DELIVERY.md` for details.

---

## 7. First-Stage Non-Goals

The first stage does not require:

- Full bidirectional sync with Safari's native bookmark tree
- End-to-end encrypted search
- CRDT real-time collaboration
- Multi-tenant enterprise sharing model
- Full mobile native client coverage

These can all evolve incrementally on the current architecture, but should not affect the first-stage data model and sync model.

---

## 8. How to Use This Documentation

### 8.1 As System Design Baseline

All new code, table structures, APIs, and sync behavior should be consistent with the docs.

### 8.2 As Agent Operation Guidelines

AI agents should not guess system behavior based solely on local files; they must work in conjunction with the documentation in this directory.

### 8.3 As Architecture Arbitration

When implementation divergence occurs, prioritize checking:

- Whether it satisfies cloud as source of truth
- Whether it maintains vault separation
- Whether it maintains policy-driven sync
- Whether it acknowledges platform differences

---

## 9. Glossary

### library

A logical bookmark space.
Can be classified as `normal` or `vault`.

### node

A tree node in the bookmark collection, which may be:

- folder
- bookmark
- separator

### projection

The local projection state visible to a specific device/browser.

### sync profile

A configuration unit defining sync direction, mode, rules, and targets.

### target

The device/browser instance targeted by a sync operation.

### revision

An ordered change event recorded on the server side.

### cursor

The position up to which a target has synced for a given library.

### unlock session

A short-term vault-accessible state obtained by a user after completing step-up authentication.

---

## 10. Next Reading

Continue reading:

- `001-ARCHITECTURE.md`
- `002-DOMAIN-MODEL.md`

---

[English](000-OVERVIEW.md) | [中文](../zh/000-OVERVIEW.md)
