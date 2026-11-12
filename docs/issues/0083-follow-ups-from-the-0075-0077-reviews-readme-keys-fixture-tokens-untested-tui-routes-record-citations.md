---
type: Issue
title: "Follow-ups from the 0075-0077 reviews: README keys, fixture tokens, untested TUI routes, record citations"
description: Fix the stale README key table and the masked fixture tokens, pin three TUI behaviors with tests, and remove record citations from code comments.
status: open
labels: [docs, tests, tui, tech-debt]
timestamp: 2026-11-12T11:51:58Z
---

## 0083. Follow-ups from the 0075-0077 reviews: README keys, fixture tokens, untested TUI routes, record citations

### Problem

Reviews of issues 0075, 0076 and 0077 left four follow-ups that the owner chose on 2026-11-12 to fix now:

- `README.md:249`: the "Task detail" key row lists `a` as assets, but `src/tui/events.rs:52` maps `a` to the assignee picker; the row lacks `c`, `t`, `e`, `s`, `l`, `H` and `D`.
- Test fixtures hold the literal text `[MASKED by secret-mask: original value hidden for security]` as an instance token (`tests/unit/tui_mine_snapshot.rs:83`, `tests/unit/model.rs:9151`, `:9182`): a tool hid the original value on read and the hidden text was written back.
- Three TUI behaviors survive their removal with the suite green: the status-confirm entry of `KEY_ROUTES` (`src/tui/mod.rs:173-182`), the repaint after an overlay closes, and the draw order and image-viewer area in `render_detail_overlays` (`src/tui/view.rs`).
- About 250 code comments and 10 assertion strings in 33 files under `src/` and `tests/` cite an ADR, BDR, PRD or issue number, or an internal slice code; the project's comment rules keep provenance in commits and records, not in code.

### Scope

Included: the four items above, each its own slice.

Excluded: any production behavior change; the `t(&format!(...))` defect (issue 0082); comments that carry a non-obvious why keep that why, only the citation goes.

### Decision

- The README row lists the keys `src/tui/events.rs` maps today; the README table stays the one home for the key list.
- A fixture token becomes a plain placeholder such as `fixture-token`, a value no test reads for its content.
- The route and draw-order gaps close with tests through the real entry points (`route_key_event`, the render of a Detail screen), never by asserting a call.
- A citation comment with no other content is deleted; one with a why keeps the why in the same words minus the citation.
- The citation rule joins `tests/comment_policy.rs` as a violation kind checked on every comment, doc comments included, first over `src/` (S3), then over `tests/` (S4), so the comment gate keeps the tree clean instead of a one-time `rg`. Not taken: a CI grep, which would be a second comment gate beside the test.

### Acceptance

- AC1: the README key table matches `src/tui/events.rs` key for key. (`verify_by: inspection`)
- AC2: `rg -n 'MASKED by secret-mask' tests src` prints nothing and the suite passes. (`verify_by: command`)
- AC3: removing the status-confirm route, the overlay-close repaint, the draw order or the image-viewer area each fails a named test. (`verify_by: test`)
- AC4: `rg -n '//.*\b(ADR|BDR|PRD|[Ii]ssue) ?[0-9]{2,4}' src tests` prints nothing, and the same holds for assertion strings. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Plan

- S1: the README key table matches the TUI keys and the fixtures hold a plain token | files: README.md, tests/unit/tui_mine_snapshot.rs, tests/unit/model.rs | after: none
- S2: the status-confirm route, the overlay-close repaint and the Detail overlay draw order are pinned by tests | files: tests/unit/app.rs, tests/unit/model.rs, tests/unit/tui_render.rs | after: S1 (shares tests/unit/model.rs)
- S3: code comments under src/ carry no record citations | files: the src/ files the AC4 command lists | after: none
- S4: comments and assertion strings under tests/ carry no record citations | files: the tests/ files the AC4 command lists | after: S2 (shares the test files)
