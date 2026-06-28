---
type: Issue
title: "ARCH — refactor render.rs: decompose meta-table, drop dead seams, relocate extract_assets"
description: Behavior-preserving maintainability pass; split build_meta_table_rows, remove the three #[allow(dead_code)] wrappers, move extract_assets to the controller/domain layer.
status: closed
labels: [refactor, render, controller, maintainability]
blocked_by: [10, 11, 12, 13]
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## ARCH — render.rs decompose / de-dead-code / relocate

The final pass the user asked to run **last**: eliminate the god function, the dead
seams, and the wrong-domain aggregation. Implements
[ADR 0016](/adr/0016-refactor-render-decompose-relocate.md). No observable behavior
change → parity is the contract (no BDR).

### Scope

Included: (1) decompose `build_meta_table_rows` (~100 lines) into a labeled-row
builder + field formatters, each within the complexity budget; (2) remove
`build_detail_lines`/`build_body_lines`/`build_comment_lines` (`#[allow(dead_code)]`)
and repoint their `tests/unit/render.rs` callers to the production
`_with_collector`/`build_detail_content` functions; (3) move `extract_assets` from
`src/render.rs` to the controller/domain layer next to its only caller. Excluded: a
broader `domain/` module split (deferred); any behavior change.

### Acceptance

- No function in `src/render.rs` exceeds the complexity budget; the meta-table is a
  short composition of named helpers.
- The three dead-code wrappers are gone; no `#[allow(dead_code)]` remains for them;
  tests call production functions and still pass.
- `extract_assets` lives in the controller/domain layer; `render.rs` imports none
  of the moved aggregation; the public behavior is identical.
- Full suite green, `clippy -D warnings` + `fmt` + comment-policy clean; the
  complexity command passes.
- Verified purely by parity — no assertion changes, only call-site repoints.

### Plan

Per ADR 0016: do it **after** V3/C1/R2/R3 land so the moved/decomposed code is the
final shape. Blocked by issues 0010–0013.
