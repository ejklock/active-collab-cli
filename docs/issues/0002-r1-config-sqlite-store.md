---
type: Issue
title: "R1 — Config + SQLite store parity"
description: rusqlite-backed instances/settings/task-cache on the existing on-disk schema.
status: closed
labels: [rust, store, parity]
blocked_by: [1]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R1 — Config + SQLite store parity

Re-implement the persistence layer in Rust against the **same on-disk SQLite
schema** so an existing user's database keeps working. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R1 of plan
`rust-rewrite`.

### Scope

Included: cross-platform config/cache path resolution; `rusqlite` connection
(tokio-safe — no thread-bound footgun); `InstanceRepository`, `SettingsRepository`,
`TaskCache` (read/write/delete-for-instance). Kept: schema compatibility with the
Python app.

### Acceptance

- Persist/load an `Instance` round-trips against the real schema.
- Settings get/set; default language read path.
- Task cache write → read → delete-for-instance behaves as the Python cache.
- Behavior tests at unit level; no thread-bound connection errors under async.

### Plan

Re-planned after R0 (provisional in plan `rust-rewrite`). Mirror the Python
`store.py` repositories; preserve table/column names.
