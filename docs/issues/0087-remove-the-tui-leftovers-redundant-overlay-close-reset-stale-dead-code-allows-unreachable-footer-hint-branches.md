---
type: Issue
title: "Remove the TUI leftovers: redundant overlay-close reset, stale dead-code allows, unreachable footer hint branches"
description: Remove the redundant rendered_width reset, the stale allow(dead_code) in theme.rs and the detail_hint branches no caller reaches, with the TUI unchanged.
status: open
labels: [tui, tech-debt]
blocked_by: [0086]
timestamp: 2026-11-12T16:34:57Z
---

## 0087. Remove the TUI leftovers: redundant overlay-close reset, stale dead-code allows, unreachable footer hint branches

### Problem

Three leftovers in the TUI code, found by the 0083 reviews:

- `close_detail_overlay` (`src/tui/model.rs:2250`) sets `*rendered_width = usize::MAX`, but the data writers already clear the detail cache (`src/tui/model.rs:2453-2458`, `:2498-2503`) and the shell reflows every frame (`src/tui/mod.rs:331-333`); [0083](/issues/0083-follow-ups-from-the-0075-0077-reviews-readme-keys-fixture-tokens-untested-tui-routes-record-citations.md) dropped its tests for that reason and left the line to this issue.
- `src/tui/theme.rs` carries 15 `#[allow(dead_code)]`. Their old "unused until runtime theme selection is wired" comments were deleted as false in 0083 S3: selection is wired at `src/main.rs:115-116` (`theme_from_str`, `set_active`). An allow that hides nothing is noise, and one that hides dead code hides a real finding.
- `hint_for_screen` builds every `DetailHintOverlays` field as `None` since 0083 S3, so the overlay branches of `footer::detail_hint` (`src/tui/footer.rs:74`) are unreachable from it; other callers are unchecked.

### Scope

Included:

- The redundant reset is removed, with the TUI's visible behavior unchanged.
- Each `#[allow(dead_code)]` in `src/tui/theme.rs` is removed; an item the compiler then reports as dead is deleted, or keeps its allow with a why-comment when a caller outside the crate's bin target needs it.
- The overlay branches of `detail_hint` that no caller reaches are removed, after checking every caller.

Excluded: any key, render or text change a user sees.

### Decision

- Removal is proved by behavior, not by a test of the removed line: the existing TUI suite stays green and a rendered Detail frame after closing each overlay matches the frame before the change.
- A dead item is deleted, not kept for later; a later need brings it back from history.

### Acceptance

- AC1: `close_detail_overlay` no longer touches `rendered_width`, and closing a picker or the image viewer still shows the current Detail lines on the next frame. (`verify_by: test`)
- AC2: `git grep -n 'allow(dead_code)' -- src/tui/theme.rs` prints only allows that carry a why-comment, and `cargo clippy --all-targets -- -D warnings` passes. (`verify_by: command`)
- AC3: no branch of `detail_hint` is unreachable from its callers, and the footer hint for each screen and overlay is unchanged. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Plan

- S1: the three leftovers are removed with the TUI unchanged | files: src/tui/model.rs, src/tui/theme.rs, src/tui/footer.rs, the tests/unit files that pin the footer hint and the overlay close | after: 0086 (shares tests/unit files)
