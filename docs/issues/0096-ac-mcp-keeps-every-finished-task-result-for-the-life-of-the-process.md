---
type: Issue
title: ac mcp keeps every finished task result for the life of the process
description: Long calls run as tasks with no TTL so a running call is never cut, which also keeps each finished task's full result in memory until the server exits.
status: open
timestamp: 2026-11-25T12:00:00Z
---

## 0096. ac mcp keeps every finished task result for the life of the process

### Problem

`run_long` in `src/mcp/mod.rs` spawns tasks with `TaskOptions::new().with_ttl_ms(None)`, so the TTL cannot cut a running download or hours export. rmcp 3.9.0 has one TTL for running and finished tasks (`task_manager.rs`: finished tasks are "retained for the lifetime of the manager"), so every finished task keeps its whole result, for a download the full `ac get --json` payload, until `ac mcp` exits. A long agent session with many downloads grows memory without bound. Found in the review of issue 0095 (2026-11-25).

### Scope

Included: retention of finished task results in `ac mcp`.

Excluded: the TTL of running tasks, which must stay unbounded.

### Acceptance

- AC1: a finished task's result is dropped after a bounded retention, while a running task is never dropped by age. (`verify_by: test`)
