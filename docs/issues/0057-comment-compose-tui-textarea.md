---
type: Issue
title: "Comment compose adopts tui-textarea — caret/selection/undo editor behind Msg::ComposeInput(Input)"
description: Replace Compose.buffer (String, append/backspace only) with a tui_textarea::TextArea. The shell keeps intercepting Ctrl+S (submit) / Esc (cancel) and converts every other compose-mode key into the backend-neutral tui_textarea::Input, carried by one Msg::ComposeInput(Input); update() applies it to the TextArea (pure) and derives the body via editor.lines().join. Render the TextArea widget into the modal's inner Rect. The New/Edit/Submitting/Error lifecycle and the write path are unchanged.
status: open
labels: [tui, compose, comments, editor, tui-textarea, slice]
blocked_by:
tracker:
timestamp: 2026-07-14T00:00:00Z
---

## Comment compose adopts `tui-textarea`

Implements [ADR 0064](/adr/0064-comment-compose-adopts-tui-textarea.md), closing the
"richer caret editing" gap ADR 0034 deferred (PRD 0002). Amends ADR 0034's editing/buffer
model; the compose-as-a-Detail-mode structure ([ADR 0034](/adr/0034-comment-compose-mode-multiline.md)
/ [ADR 0047](/adr/0047-detail-overlay-as-one-typed-state.md)) and the modal render
([ADR 0039](/adr/0039-reusable-modal-overlay-for-compose-and-confirm.md)) stay.

### Problem

The compose buffer is a `String` that only grows/shrinks at the end (append char /
backspace / newline). There is no in-body caret movement, no selection, no word delete, no
undo/redo — inadequate for a multi-paragraph comment.

### Decision (from ADR 0064)

- **Editor replaces buffer:** `Compose.editor: tui_textarea::TextArea<'static>` in place of
  `buffer: String`. New → `TextArea::default()`; Edit → `TextArea::from(body.lines())`;
  submit body → `editor.lines().join("\n")`.
- **Shell keeps key authority:** `map_compose_key_event` maps `Ctrl+S -> ComposeSubmit`,
  `Esc -> ComposeCancel` (both intercepted before the editor), every other key ->
  `Msg::ComposeInput(tui_textarea::Input)` (converted from the crossterm `KeyEvent`). The
  granular `ComposeNewline`/`ComposeBackspace`/`ComposeInput(char)` messages are folded into
  `ComposeInput(Input)`.
- **`update()` stays pure:** `if let DetailOverlay::Compose(c) = overlay { c.editor.input(input); }`.
- **Render:** `view()` renders the `TextArea` widget into the modal inner content `Rect`
  (`render_modal` exposes it); caret/selection/scroll come from the widget.

### Scope

Included:

- `Cargo.toml` — add `tui-textarea`.
- `src/tui/model.rs` — `Compose.editor` field; edit pre-fill; submit-body derivation; the
  `ComposeInput(Input)` update arm; drop the folded arms.
- `src/tui/events.rs` + `src/tui/mod.rs` — `map_compose_key_event` → `ComposeInput(Input)`;
  `Msg::ComposeInput(tui_textarea::Input)`; remove folded `Msg`s.
- `src/tui/widgets/modal.rs` — expose the inner content `Rect`.
- `src/tui/view.rs` — render the `TextArea` into the modal inner Rect.
- Tests: `tests/unit/model.rs`, `tests/unit/tui_render.rs`.

Excluded: the image viewer (issues 0058/0059); `tui-textarea` regex search (deferred, YAGNI);
any change to the async write path (ADR 0035/0054) or the modal backdrop (ADR 0039).

### Acceptance

- AC1 — pure editing (unit): a sequence of `ComposeInput(Input)` (chars, `Enter`, caret
  Left/Right/Up/Down, Home/End, `Backspace`, undo, redo) through `update()` yields the
  expected `compose.editor.lines()`. (`verify_by: test`)
- AC2 — submit body (unit): `ComposeSubmit` emits `Cmd::SubmitComment` with
  `body == editor.lines().join("\n")` for a multi-line draft. (`verify_by: test`)
- AC3 — shell key authority (unit): `map_compose_key_event` maps `Ctrl+S -> ComposeSubmit`
  and `Esc -> ComposeCancel` (neither reaches the editor); every other compose key ->
  `ComposeInput(Input)`; `map_browse_key_event` unchanged; `'c'` opens compose only on
  Detail. (`verify_by: test`)
- AC4 — edit pre-fill (unit): `ComposeOpen(Edit{id})` seeds `editor.lines()` from the comment
  body split on `\n`; `New` starts empty. (`verify_by: test`)
- AC5 — draft survives failure (unit): `CommentMutationErr` leaves `editor.lines()` intact
  with `status = Error(_)`. (`verify_by: test`)
- AC6 — render (`TestBackend`): an open compose modal renders the editor content inside the
  centered box over the dimmed backdrop, with the in-box hint/status. (`verify_by: test`)
- CC — clean code: no superfluous comments / banners / commented-out code; well-named
  helpers. (`verify_by: inspection`)
- CX — complexity budget: cyclomatic ≤ 10 (≤ 8 new), cognitive ≤ threshold. (`verify_by: command`)
- TE — tests assert observable behavior (resulting `editor.lines()`, submitted body, mapped
  `Msg`s) and survive the mutation floor on changed lines. (`verify_by: command`)

### Plan

1. `Cargo.toml`: add `tui-textarea`.
2. `model.rs`: swap `buffer: String` → `editor: TextArea<'static>`; update New/Edit
   construction and the submit-body derivation.
3. `events.rs`/`mod.rs`: add `Msg::ComposeInput(Input)`, rewrite `map_compose_key_event`,
   remove the folded messages, add the pure `update` arm.
4. `widgets/modal.rs`: expose the inner content `Rect`.
5. `view.rs`: render the `TextArea` into the modal inner Rect.
6. Tests: editing sequences, submit body, key-authority mapping, edit pre-fill, error draft,
   compose modal render.

Observable end-to-end: press `c`, type a multi-line comment moving the caret back to fix a
typo and undo a change, `Ctrl+S`, thread reloads with the posted comment.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
</content>
