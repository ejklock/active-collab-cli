---
type: Issue
title: The mock ActiveCollab fixtures are the single source of the automated end-to-end tests
description: One shared, fictitious mock serves the walkthroughs and the automated CLI, TUI and MCP end-to-end tests.
status: open
labels: [tests, qa]
timestamp: 2026-11-13T00:37:53Z
---

## 0088. The mock ActiveCollab fixtures are the single source of the automated end-to-end tests

### Problem

Two copies of the mocked ActiveCollab exist. `examples/mock_collab.rs` seeds the instance, tasks, task lists, time records, a comment and its attachments for the terminal walkthroughs of [issue 0084](/issues/0084-replay-the-live-acceptance-criteria-as-terminal-walkthroughs-against-a-mock-activecollab.md). `tests/mcp_stdio.rs` mounts its own shapes with `mount_api`, `mount_json` and `mount_task_with_attachments`. The CLI and TUI end-to-end paths have no automated test against either one. A shape fixed in one copy stays wrong in the other.

### Scope

Included: the seeded data and routes of `examples/mock_collab.rs` move into code that both the example binary and the automated end-to-end tests use; `tests/mcp_stdio.rs` mounts its routes from that code; automated CLI and TUI end-to-end tests run against it.

Excluded: the walkthrough files under `qa/`, beyond what the move requires; any production behavior change; a real ActiveCollab.

### Decision

- The mock's fixtures are the single source of the automated end-to-end tests of the CLI, the TUI and MCP, shared with `tests/mcp_stdio.rs` (owner, 2026-11-12). Not taken: keeping a second set of shapes in each test file.
- The fixtures hold fictitious data only, never data copied from the real Collab: names such as `Ada Lovelace`, `example.com` addresses, invented task titles (owner, 2026-11-12).
- The TUI end-to-end proof is the terminal walkthroughs under `qa/`, replayed by `qa/run-walkthroughs.sh` against the mock built from the shared module, outside `cargo test` and CI; `run_app` is not refactored and no pseudo-terminal dev-dependency is added (owner, 2026-11-13). Not taken: making `run_app` generic over the ratatui backend and the event stream to drive it in-process with `TestBackend`; a pseudo-terminal test of the real binary with `expectrl` or `portable-pty`.

### Acceptance

- AC1: `examples/mock_collab.rs` and `tests/mcp_stdio.rs` mount their routes and seed data from one shared module, and no route shape is defined twice. (`verify_by: inspection`)
- AC2: an automated test runs `ac get`, `ac mine` and `ac task set --list` against the shared mock and checks their output. (`verify_by: test`)
- AC3: the terminal walkthroughs under `qa/` replay green through `qa/run-walkthroughs.sh` against the mock built from the shared module, with fictitious data only. (`verify_by: command`)
- AC4: every seeded name, address and title is fictitious. (`verify_by: inspection`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `bash qa/run-walkthroughs.sh`

### Plan

- S1: one shared module holds the mock's fictitious seed data and every route shape; `examples/mock_collab.rs` and `tests/mcp_stdio.rs` mount from it, and only status-only failure routes stay local to a test | files: tests/support/mock_collab.rs, examples/mock_collab.rs, tests/mcp_stdio.rs | after: none
- S2: an automated test runs `ac get`, `ac mine` and `ac task set --list` against the shared mock and checks their output | files: tests/cli_e2e.rs | after: S1
- S3: the terminal walkthroughs under `qa/` replay green through `qa/run-walkthroughs.sh` against the mock built from the shared module | files: none beyond what the replay requires | after: S1
