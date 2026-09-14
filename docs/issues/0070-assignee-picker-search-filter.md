---
type: Issue
title: TUI assignee-picker gains a live search/filter field
description: "Add a filter field to the TUI assignee-picker modal so the user narrows the candidate list by typed text instead of scrolling the whole user directory. Extends issue 0068."
status: open
timestamp: 2026-09-14T00:00:00Z
---

<!-- Status lives in frontmatter (`status`), not a body line. Settable values are
     exactly open | in-progress | closed. `living-docs supersede` sets Superseded on
     this issue -- never set it by hand -- when a later issue replaces it. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## TUI assignee-picker gains a live search/filter field

Extends [issue 0068](0068-edit-task-fields-status-assignee-time-estimate-from-cli-and-tui.md).

### Problem

The assignee-picker modal (`DetailOverlay::AssigneePicker` in `src/tui/model.rs`,
rendered by `render_assignee_picker_modal` in `src/tui/view.rs`) lists the whole
user directory with no way to narrow it. On an instance with many users, the user
must scroll through the full list to find one name.

### Decision (cheap-to-reverse, kept in this issue)

Add a `filter: String` field to the picker overlay:

- Typing filters the candidate list by name, case-insensitive substring match.
- Arrow keys (up/down) navigate the filtered list.
- `j`/`k` stop navigating the list and become filter text instead — a deliberate
  trade-off, since text entry needs the letter keys.
- Backspace deletes the last filter character.
- Enter/Ctrl+S submits the highlighted candidate from the filtered list.
- Esc cancels the picker, same as today.

### Scope

Included:

- `src/tui/model.rs` — add the `filter` field to `DetailOverlay::AssigneePicker`;
  add `Msg::AssigneePickerChar(char)` and `Msg::AssigneePickerBackspace`; update
  the up/down/submit handlers to operate on the filtered candidate list; reset the
  selected index to the first match whenever the filter changes.
- `src/tui/events.rs` — remap key input while the picker is open: `j`/`k` produce
  filter-text messages instead of navigation messages; other letters/digits feed
  the filter; arrow keys keep navigating.
- `src/tui/view.rs` — render a search line above the candidate list, an
  empty-match message when the filter matches nothing, and an updated hint line.
- `src/tui/footer.rs` — follow any type alias change to the picker view tuple.
- Tests: unit coverage for the filter/navigation/submit behavior and a render
  smoke test for the search line and empty-match state.

Explicitly kept: the picker's open/close lifecycle, the submit flow that calls
back into the assignee-update handler, and the CLI edit-assignee path — unchanged.

Excluded: fuzzy or scored matching, matching on user id or email, mouse
interaction, and any change to overlay architecture or module layout.

### Acceptance

- AC1 — substring filter (unit): typing text narrows the candidate list to names
  containing the typed text, case-insensitive. (`verify_by: test`)
- AC2 — selection reset (unit): the highlighted candidate resets to the first
  filtered match whenever the filter text changes; up/down navigate only the
  filtered list. (`verify_by: test`)
- AC3 — submit filtered candidate (unit): Enter/Ctrl+S submits the highlighted
  candidate from the current filtered list, not the unfiltered list.
  (`verify_by: test`)
- AC4 — key remap (unit): while the picker is open, `j` and `k` append to the
  filter text instead of navigating; arrow keys still navigate.
  (`verify_by: test`)
- AC5 — backspace (unit): Backspace removes the last filter character and
  re-filters the candidate list. (`verify_by: test`)
- AC6 — render (unit): the modal renders a search line showing the current
  filter text, an empty-match message when no candidate matches, and an updated
  hint line. (`verify_by: test`)
- AC7 — docs: this issue and the issues index record the change.
  (`verify_by: inspection`)

### Plan

1. `src/tui/model.rs`: add `filter` to `DetailOverlay::AssigneePicker`; add the
   two new `Msg` variants and their handlers; filter candidates before
   navigation/submit; reset the selected index on filter change.
2. `src/tui/events.rs`: remap `j`/`k`/other character keys to filter messages
   while the picker overlay is open.
3. `src/tui/view.rs`: render the search line, the empty-match message, and the
   updated hint.
4. `src/tui/footer.rs`: adjust the picker view type alias if its shape changes.
5. Tests: unit-test the filter/navigation/submit behavior in `model.rs` and add
   a render smoke test in `view.rs`.

### Verification commands

- `docker compose run --rm dev cargo test`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
