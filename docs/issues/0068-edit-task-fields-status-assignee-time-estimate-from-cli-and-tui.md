---
type: Issue
title: Edit task fields — status, assignee, time estimate from CLI and TUI
description: "Add task-field edits — status, assignee, and time estimate — from CLI and TUI, reusing the ADR 0033 host-gated write seam with per-field vertical slices. Shares issue 0067's scope gate: a constitution amendment plus a product ADR making active-collab-cli a limited write client."
status: open
timestamp: 2026-09-14T14:26:32Z
---

<!-- Status lives in frontmatter (`status`), not a body line. Settable values are
     exactly open | in-progress | closed. `living-docs supersede` sets Superseded on
     this issue -- never set it by hand -- when a later issue replaces it. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## Edit task fields — status, assignee, time estimate

Reuses the authenticated write seam from
[ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md).

### Problem

From the task list / detail, the user wants to change a task's **status**, **assignee**, and
**time estimate** without leaving the terminal — today these edits require the ActiveCollab web
UI. This issue adds those three field edits from both the CLI and the TUI.

### Scope gate — read before implementing

This is a **write beyond comments** and shares the scope decision with issue 0067. The
[constitution](/constitution.md) still lists task writes as explicitly out of scope. Before
implementation, record the **constitution amendment** + product-direction **ADR** that make
active-collab-cli a limited write client. Do not implement until that decision is recorded.

Field-specific care:

- **Status**: ActiveCollab distinguishes a task's *completion* (open/completed, a dedicated
  endpoint) from *task lists*/columns. Confirm which "status" the user means at planning — the
  first read of the request is open ↔ completed plus, if the instance uses them, moving between
  task lists/columns.
- **Assignee**: the payload is a user id (`assignee_id`); the UI needs the project's member list
  to offer a pick. Confirm how members are fetched.
- **Time estimate**: an `estimate` value in hours on the task update endpoint.

### Scope

Included (once the scope gate above is cleared):

- `src/client.rs` — task-update write method(s) over the host-gated seam (expected:
  `PUT /api/v1/projects/{project_id}/tasks/{task_id}` with the changed fields; completion may be
  a separate open/complete endpoint — verify at planning).
- CLI — non-interactive subcommands (e.g. `ac task set-status`, `ac task set-assignee`,
  `ac task set-estimate`, or one `ac task set --status/--assignee/--estimate`), `--json` result,
  resolving the task ref the same way `comment` does.
- TUI — detail-view affordances to change status, assignee (picker over project members), and
  estimate, each posting over the client method with a server-truth refresh after the write.
- Tests: client writes (payload + typed outcome), CLI parsing + wiring, TUI pure update
  transitions for each edit.

Excluded: creating or deleting tasks; editing name/description/due date/labels (separate
follow-ups if wanted); time logging (issue 0067); bulk edits.

### Acceptance

- AC0 — scope gate: the constitution amendment + product ADR are recorded before this issue is
  implemented. (`verify_by: inspection`)
- AC1 — client writes (unit): each field update posts the resolved payload over the host-gated
  seam and classifies the response into a typed outcome (Ok / Unauthorized / Failed(status)),
  mirroring the comment write outcomes (ADR 0054). (`verify_by: test`)
- AC2 — CLI (unit): the status/assignee/estimate subcommand(s) parse the task ref and the new
  value, thread them into the client call, and print a `--json` result; an invalid value is a
  usage error (exit 2). (`verify_by: test`)
- AC3 — TUI (unit): the pure `update` core transitions through each edit flow (open → choose →
  submit → refresh) with no terminal/network dependency; the assignee picker is sourced from the
  project member list. (`verify_by: test`)
- AC4 — server-truth refresh: after a successful write the detail view re-fetches from the
  instance rather than trusting local state (ADR 0035 parity). (`verify_by: test`)
- CC — clean code: no superfluous comments / banners / commented-out code. (`verify_by: inspection`)

### Plan

Slice after the scope gate clears; likely per-field vertical slices so each ships independently:

1. Client task-update write method + typed outcome + unit tests.
2. Status edit — CLI + TUI (confirm completion vs task-list semantics first).
3. Assignee edit — CLI + TUI (needs project-member fetch for the picker).
4. Time-estimate edit — CLI + TUI.

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
