---
type: ADR
title: Promote the Rust crate to the repo root and remove Python
description: At R8 cutover the Cargo crate is physically moved from rust/ to the repo root and the Python package is removed, making this a single-language repository.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, cutover, rust, structure]
timestamp: 2026-06-25T00:00:00Z
---

# 0006. Promote the Rust crate to the repo root and remove Python

## Context

During the rewrite ([ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md)) the
Cargo crate lived under `rust/` while the Python package (`src/active_collab/`,
`pyproject.toml`) occupied the repo root. This separation was intentional for the
transition period — the Python app remained the shipped product until the Rust
rewrite reached parity.

[PRD 0001](/prd/0001-rust-tui-cli-parity.md) and [Issue 0009](/issues/0009-r8-cutover.md)
define the cutover milestone: once parity is confirmed across R0–R7, the Rust
binary becomes the shipped app, Python is removed, and the repo is cleaned up.

Two layout options were evaluated at cutover:

1. **Keep the crate in `rust/`, delete Python.** Removes Python from the root
   without touching the Rust layout. Less churn, but leaves a non-idiomatic
   nested crate and a permanent `rust/` indirection every contributor must
   remember.

2. **Physically move the crate to the repo root, delete Python.** The crate
   becomes the repo — `Cargo.toml`, `src/`, `tests/`, `locales/`, `Dockerfile`,
   and `docker-compose.yml` all at top level. Standard Rust repository layout;
   no nested prefix.

The maintainer chose option 2.

## Decision

**Physically move the Cargo crate from `rust/` to the repo root** and **remove
the Python package entirely**.

Concretely:

- `Cargo.toml`, `src/`, `tests/`, `locales/` — moved to the repo root (previously
  under `rust/`).
- `Dockerfile` and `docker-compose.yml` — updated: `build.context ./`, volume
  `./:/app`, `working_dir /app`. `COPY locales` path updated accordingly.
- `.dockerignore` and `.gitignore` — moved/updated to the repo root.
- Python package (`src/active_collab/`, `tests/test_*.py`, `pyproject.toml`,
  `legacy/`) — removed.
- `CLAUDE.md` build-rule framing updated to describe the root-layout crate.

The `#[path]` unit-test stubs in `src/` point to `tests/unit/` via crate-relative
paths and are unaffected by the directory move.

**Rejected alternative:** keeping the crate in `rust/` and only removing Python
(option 1). Rejected because a permanent `rust/` subdirectory is non-idiomatic
for a single-language repository and forces every contributor to remember the
prefix when running `cargo` commands or referencing source paths.

## Consequences

**Positive:**

- Standard Rust repository layout — `cargo` commands work at the repo root without
  any `--manifest-path` or `cd rust/` qualifier.
- Single-language repo; Python tooling, CI steps, and interpreter are gone.
- `docker-compose.yml` context is `./` — the build context equals the repo, which
  is the standard Docker idiom.
- Simpler contributor mental model: the repo root is the crate.
- `.codegraph/` re-indexed against the new root paths after the move.

**Accepted trade-offs / one-time costs:**

- `docker-compose.yml`, `Dockerfile`, `CLAUDE.md`, and `docs/architecture.md`
  required edits to remove `rust/` path prefixes — these are one-time rewrites,
  not ongoing maintenance.
- Any bookmarks or muscle-memory to `rust/src/` paths need updating.

**Now forbidden:**

- Re-introducing a `rust/` directory or a nested Cargo crate without a new ADR.
- Re-introducing any Python package or interpreter dependency.

## Related

- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md)
- PRD: [/prd/0001-rust-tui-cli-parity.md](/prd/0001-rust-tui-cli-parity.md)
- Issue: [/issues/0009-r8-cutover.md](/issues/0009-r8-cutover.md)
