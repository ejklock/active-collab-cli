---
type: Issue
title: Log time on a task — add a time entry from CLI and TUI
description: "Add a 'log time on a task' write from CLI (ac time log) and TUI, reusing the ADR 0033 host-gated write seam. Gated on a scope decision: a constitution amendment plus a product ADR making active-collab-cli a limited write client, since this extends writes beyond comments."
status: closed
timestamp: 2026-09-14T14:26:32Z
---

<!-- Status lives in frontmatter (`status`), not a body line. Settable values are
     exactly open | in-progress | closed. `living-docs supersede` sets Superseded on
     this issue -- never set it by hand -- when a later issue replaces it. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## Log time on a task

Reuses the authenticated write seam from
[ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md).

### Problem

The tool can read a task and (since PRD 0002) comment on it, but it cannot record worked time.
A developer who lives in the terminal still has to open the ActiveCollab web UI to log a time
entry against the task behind their branch. This issue adds "log time on a task" from both the
CLI and the TUI.

### Scope gate — CLEARED

This is a **write beyond comments**. The product-direction decision is now recorded:
[ADR 0069](/adr/0069-active-collab-cli-becomes-a-limited-write-client.md) (Accepted) and
constitution Amendment 1 make active-collab-cli a limited write client, with time records
explicitly in scope. This issue is now implementable.

### Scope

Included (once the scope gate above is cleared):

- `src/client.rs` — a write method to create a time record for a task over the existing
  host-gated write seam. Verify the exact ActiveCollab endpoint and required fields against the
  API at planning time (expected: `POST /api/v1/projects/{project_id}/time-records` with
  `task_id`, `value` (hours), `record_date`, `job_type_id`, optional `summary`/`user_id`).
- CLI — a non-interactive subcommand (e.g. `ac time log <task-ref> --hours <H> [--date …]
  [--summary …]`, `--json` result), resolving the task ref the same way `comment` does.
- TUI — a "log time" affordance on the detail view: a small compose/modal for hours + optional
  summary, posting over the same client method, with a server-truth refresh after the write.
- Tests: client write (payload + outcome classification), CLI arg parsing + wiring, TUI pure
  update transitions for the log-time flow.

Excluded: editing or deleting existing time records; job-type management; reporting/rollups of
logged time; timers/stopwatch. Task-field edits (status, assignee, estimate) are issue 0068.

### Acceptance

- AC0 — scope gate: the constitution amendment + product ADR are recorded before this issue is
  implemented. (`verify_by: inspection`)
- AC1 — client write (unit): the time-record method posts the resolved payload over the
  host-gated seam and classifies the response into a typed outcome (Ok / Unauthorized /
  Failed(status)), mirroring the comment write outcomes (ADR 0054). (`verify_by: test`)
- AC2 — CLI (unit): `ac time log` parses the task ref, hours, and optional date/summary, threads
  them into the client call, and prints a `--json` result; an invalid/empty hours value is a
  usage error (exit 2). (`verify_by: test`)
- AC3 — TUI (unit): the pure `update` core transitions through the log-time flow (open → input →
  submit → refresh) with no terminal/network dependency. (`verify_by: test`)
- AC4 — server-truth refresh: after a successful write the detail view re-fetches from the
  instance rather than trusting local state (ADR 0035 parity). (`verify_by: test`)
- CC — clean code: no superfluous comments / banners / commented-out code. (`verify_by: inspection`)

### Plan

Slice after the scope gate clears; likely three slices:

1. Client write method + typed outcome + unit tests.
2. CLI `time log` subcommand (arg parsing, wiring, `--json`).
3. TUI log-time affordance (pure update flow + modal + server-truth refresh).

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
