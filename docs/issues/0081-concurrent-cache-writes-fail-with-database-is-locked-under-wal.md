---
type: Issue
title: Concurrent cache writes fail with database is locked under WAL
description: Two connections writing the shared SQLite cache at once sometimes fail with database is locked; the store must wait for the lock instead.
status: open
labels: [bug, store, mcp]
timestamp: 2026-11-12T01:08:08Z
---

## 0081. Concurrent cache writes fail with database is locked under WAL

### Problem

`store::tests::two_stores_write_the_task_cache_concurrently_without_lock_errors` (tests/unit/store/mod.rs:189), added with the WAL switch of [issue 0073](/issues/0073-ac-mcp-serve-the-cli-as-mcp-tools-over-stdio-for-parallel-agent-sessions.md), failed 2 of 40 isolated runs on 2026-11-11 with `database is locked` (`docker compose run --rm dev sh -c 'for i in $(seq 40); do cargo test -q --bin ac two_stores_write; done'`). Two `ac mcp` processes writing the shared cache at the same moment can therefore fail a call, which is the case the WAL switch exists for.

Two candidate causes, both inferences to confirm: `apply_pragmas` (src/store/mod.rs:69) sets `journal_mode=WAL` before `busy_timeout=5000`, so a connection opening while another switches the mode gets `SQLITE_BUSY` with no wait; or a deferred transaction that reads and then writes is refused at once in WAL, because SQLite skips the busy handler when a read lock must upgrade to a write lock.

### Scope

Included: the store opens and writes the cache from several connections at once without `database is locked` while another holds the lock for less than the busy timeout.

Excluded: cross-process write ordering, the cache schema, retries above the store.

### Decision

- Fix in the store's open and write path, using SQLite's own mechanisms (busy timeout, `BEGIN IMMEDIATE`), not a retry loop.
- Measured 2026-11-11 (200 rounds × 4 threads per scenario, raw statements of `Store::open`): only `PRAGMA journal_mode=WAL` fails, plain `SQLITE_BUSY` (extended code 5) despite a 5000 ms busy timeout, 28/800 opens on a fresh file and 29/800 on a DELETE-mode file, 0/800 on a WAL file. So the open tolerates `SQLITE_BUSY` on that one pragma and continues: the lock holder is a peer doing the same switch, WAL persists in the file, and the later statements wait on the busy timeout. Rejected: a bounded retry of the pragma (a homegrown loop), an advisory lock file around the switch (a sidecar file for a one-time event), reading the mode first (does not close the race on a fresh or DELETE file).

### Acceptance

- AC1: two and four connections writing the cache concurrently, repeated enough times that the current code fails in the same run, never get `database is locked`. (`verify_by: test`)
- AC2: opening a fresh store from several connections at once succeeds and leaves it in WAL. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
