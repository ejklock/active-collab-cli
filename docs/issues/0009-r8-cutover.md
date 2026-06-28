---
type: Issue
title: "R8 — cutover: promote Rust, remove Python"
description: Make the Rust binary the shipped app; remove the Python package; update docs/CI.
status: closed
labels: [rust, cutover, docs, ci]
blocked_by: [8]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R8 — cutover: promote Rust, remove Python

Once parity is confirmed, switch the project over to the Rust binary. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R8 of plan
`rust-rewrite`. This is a deliberately **horizontal** (layer-shaped) slice — no
single vertical user behavior; it is the migration/promotion step.

### Scope

Included: promote `rust/` to the repo root as the shipped app; remove the Python
package and `windows-curses`; update `README.md`, `CHANGELOG.md`, packaging, and
CI to the Rust binary; wire the ADR 0002 fitness functions as CI gates. Kept: the
preserved on-disk SQLite schema so existing users keep their data.

### Acceptance

- The Rust binary is the documented entry point; the Python package is removed.
- Docs and the ADR/PRD trail reflect the cutover.
- ADR 0002 fitness functions (clippy/fmt clean, static-binary check, BDR 0001
  regression tests) run as CI gates.

### Plan

Re-planned after parity (provisional in plan `rust-rewrite`).
