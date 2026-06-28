---
type: Issue
title: "R7 — i18n (en + pt-BR) + asset open/download"
description: Full message catalogs through all output; open assets and download with host-scoped auth.
labels: [rust, i18n, assets, parity, security]
blocked_by: [7]
status: closed
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R7 — i18n (en + pt-BR) + asset open/download

Complete localization and asset handling. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) (requirements 7, 8); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R7 of plan
`rust-rewrite`.

### Scope

Included: English + Brazilian-Portuguese catalogs wired through all output; asset
open (http/https only); asset download attaching auth **only** on host match.
Kept: the token host-isolation non-negotiable.

### Acceptance

- Language switch parity with the Python `setup language`.
- Catalog completeness test (no missing keys per locale).
- Asset open restricted to http/https; download attaches a token only when the
  asset host matches the instance host (negative test off-host).

### Plan

Re-planned after R6 (provisional in plan `rust-rewrite`).
