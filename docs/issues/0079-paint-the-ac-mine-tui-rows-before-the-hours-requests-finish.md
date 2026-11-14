---
type: Issue
title: Paint the ac mine TUI rows before the hours requests finish
description: The ac mine TUI shows refreshed rows as soon as the task lists arrive and fills week and total hours in a second message.
status: closed
labels: [tui, performance]
timestamp: 2026-11-12T00:21:46Z
---

## 0079. Paint the ac mine TUI rows before the hours requests finish

### Problem

The `ac mine` TUI revalidation (`Cmd::LoadMineTasks`, src/tui/mod.rs:555) awaits `collect_mine_rows`, which finishes only after every hours request: one time-records call per instance and one task call per listed task. The refreshed list therefore waits for the slowest hours request before it paints. Found in the review of commit 2db5bca.

### Scope

Included:

- The TUI revalidation sends the refreshed rows as soon as the task lists arrive, then sends the hours in a second message when the hours requests finish.
- Until the hours arrive, a row keeps the hours it already showed for the same instance and task (from the snapshot or the previous load); a new row shows them blank.
- The mine snapshot is written after each of the two messages.

Excluded: the `ac mine` table, `--json` and the MCP `mine`, which keep waiting for the hours because they print once; request concurrency limits.

### Decision

- Two messages instead of one per task: one paint for the list, one for the hours, keeping `update` pure.
- The estimate arrives with the rows, because it comes from the task list.
- The mine handlers find the Tasks screen anywhere in the screen stack, so a message that lands while Detail is open still updates the list and the snapshot.
- `update` (cyclomatic 16) and `dispatch_cmds` (12) exceed the gate's ceiling of 10 already on the base commit; the owner chose to refactor both inside this issue (2026-11-11), by grouping their arms into message and command families in the existing sub-handler pattern. Not taken: a named baseline entry plus a separate issue; leaving the issue uncommitted; splitting `Msg` into nested per-family enums.

### Acceptance

- AC1: after the task lists arrive, the TUI shows the refreshed rows before the hours requests finish. (`verify_by: test`)
- AC2: the hours message fills week and total hours by instance and task; a row the message has no entry for keeps the hours it shows; an entry whose value is unknown sets that value unknown; an entry for a task no longer listed adds no row. (`verify_by: test`)
- AC3: rows keep their earlier hours until the hours message lands. (`verify_by: test`)
- AC4: the snapshot read after a restart carries the hours of the last hours message. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
