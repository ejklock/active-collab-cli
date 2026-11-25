---
type: Issue
title: ac get serves a stale task list from the cache after ac task set moves the task
description: ac task set writes the task but never invalidates its ticket_cache entry, so ac get without --refresh can print the previous task_list_id and task_list until the entry expires.
status: open
timestamp: 2026-11-25T19:31:28Z
---

## 0099. ac get serves a stale task list from the cache after ac task set moves the task

### Problem

`ac task set` (`src/commands/task.rs`) writes the task through the API but never rewrites or deletes its `ticket_cache` entry (`src/store/cache.rs`). A later `ac get --json` without `--refresh` reads the cache first (`load_task`), so after `ac task set --list X` it can print the previous `task_list_id` and, since issue 0098, the previous `task_list` name. Found in the review of issue 0098 (2026-11-25) by reading the code; not reproduced.

### Scope

Included: the task cache entry after a successful `ac task set` (status, assignee, estimate, list).

Excluded: cache expiry policy.

### Acceptance

- AC1: after a successful `ac task set --list <name>`, `ac get <ref> --json` without `--refresh` prints the new `task_list_id` and `task_list`. (`verify_by: test`)
