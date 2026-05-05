# AGENTS.md

_Single source of truth for agent identity, code standards, and project rules. Symlinked by `CLAUDE.md`; only edit `AGENTS.md` when needed._

Detailed product, architecture, database, API, sync, browser, security, and delivery rules live in `docs/`. Do not expand this file into a parallel design document.

## Identity & Communication

- **Role**: Expert coding assistant working inside the amagi repository.
- **Chat language**: Use the user's language. If the user writes Chinese, respond in Chinese.
- **Code and comments**: English only unless a file already clearly requires another language.
- **Documentation language**: See Multi-language Docs Section.
- **Style**: Concise, technical, action-oriented.

## Required Reading

Start with `docs/zh/000-OVERVIEW.md`, then read the topic documents relevant to the task:

- Architecture: `docs/zh/001-ARCHITECTURE.md`
- Domain model: `docs/zh/002-DOMAIN-MODEL.md`
- Database: `docs/zh/003-DATABASE.md`
- API: `docs/zh/004-API.md`
- Sync behavior: `docs/zh/005-SYNC.md`
- Browser extension / WXT / WebExtension adapter: `docs/zh/006-BROWSER-ADAPTERS.md`
- Dashboard Web UI: `docs/zh/007-WEB-UI.md`
- Authentication / vault / WebAuthn: `docs/zh/008-SECURITY.md`
- Repository layout, engineering rules, milestones: `docs/zh/009-REPOSITORY-AND-DELIVERY.md`

## Project Rules

- Treat docs as the authority for product and architecture decisions. If behavior changes, update the corresponding docs in the same change.
- README and AGENTS are entry points only. Keep durable design and engineering detail in `docs/`.
- Historical changes belong in `CHANGELOG.md` or iteration/review files under `temp/`, not as stale narrative in `docs/`.
- Use mature, modern libraries for established problems instead of hand-rolling core infrastructure, unless docs or the user explicitly require a custom implementation.
- Do not automatically create git commits. Stage or commit only when the user explicitly asks.

## Tooling & Commands

- Use `just` for repository workflows such as setup, lint, test, typecheck, build, and dev dependencies.
- Commands in project recipes should stay idiomatic and user-facing. Do not add `mise exec` to recipes only because an agent's non-interactive shell missed the user's shell initialization.
- Unless a command explicitly depends on a platform-specific prerequisite, commands in recipes, docs, and examples must remain cross-platform across macOS, Linux, and Windows. Prefer CLI flags or config files over inline `KEY=value command` shell syntax.
- When this agent shell cannot find `pnpm`, `node`, `cargo`, or another `mise`-managed tool, run the command through the agent environment wrapper, for example:

```sh
mise exec --command "pnpm lint"
```

- If a command result is version-sensitive, prefer the versions declared by `mise.toml`, `rust-toolchain.toml`, and workspace package manager config.
- Iteration close-out should run formatting first, then relevant lint/typecheck/build/test commands.

## Code Standards

- Comments explain why, not what.
- Keep boundaries clear: app crates/apps are thin composition layers; reusable logic belongs in `packages/*`.
- TypeScript public APIs should follow the coding standards in `docs/zh/009-REPOSITORY-AND-DELIVERY.md`, including options-object-first public API shape.
- For enum-like string domains, use `export const Foo = { ... } as const` + `export type Foo = (typeof Foo)[keyof typeof Foo]` as the canonical source of truth. Do not use `['a', 'b'] as const` + `[number]` as the primary domain definition. If stable iteration order is needed for UI or tests, derive a separate ordered array from the object const.
- Bash scripts should use `set -e`, `[[ ]]`, and quoted variables.
- YAML uses 2-space indentation and quotes only when necessary.

## Completion Standard

A task is complete only when:

- Code compiles or the relevant non-code artifact validates.
- Behavior is consistent with docs.
- New behavior has an appropriate test or verification path.
- Relevant docs are updated when product, architecture, API, database, sync, security, browser capability, or delivery semantics change.

### Multi-language Docs

**Directory Structure:**
- English docs: `docs/{lang}/00x-TITLE.md` (e.g., `docs/en/00x-TITLE.md`)

**Rules:**
- Translate user-facing docs only (README, docs/00x-*.md); do NOT translate machine-oriented docs (AGENTS.md, CLAUDE.md, etc.)
- Each doc should have bidirectional language links at the bottom: `[English](../en/xxx.md) | [中文](xxx.md)` (in Chinese docs) or `[English](xxx.md) | [中文](../zh/xxx.md)` (in English docs)
- Non-English docs must link to other docs in the same language folder when available (e.g., `docs/zh/` links point to `docs/zh/`)
- For future languages, create `docs/{lang}/` folder and follow the same pattern (e.g., `docs/es/`, `docs/ja/`)

**Current languages:**
- English: `docs/en/00x-TITLE.md`
- Chinese: `docs/zh/00x-TITLE.md`

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **amagi** (3657 symbols, 7439 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/amagi/context` | Codebase overview, check index freshness |
| `gitnexus://repo/amagi/clusters` | All functional areas |
| `gitnexus://repo/amagi/processes` | All execution flows |
| `gitnexus://repo/amagi/process/{name}` | Step-by-step execution trace |

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
