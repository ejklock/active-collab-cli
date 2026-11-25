---
type: Issue
title: ac mcp cuts off a running call five seconds after the client disconnects
description: On client EOF rmcp drains in-flight calls for at most 5 s, then the process exits and ends the blocking thread of a core still running, which can leave a download half-written.
status: open
timestamp: 2026-11-25T12:00:00Z
---

## 0097. ac mcp cuts off a running call five seconds after the client disconnects

### Problem

When the client closes stdin, rmcp 3.9.0 drains in-flight responses for at most 5 s (`src/service.rs`), then `serve` in `src/mcp/mod.rs` shuts the task manager down and `main` calls `process::exit`, ending the blocking thread of a command core still running. An attachment download can be left half-written. Synchronous calls behaved the same before issue 0095; found in its review (2026-11-25), from reading the code, not reproduced.

### Scope

Included: what `ac mcp` does with a core still running when the client disconnects.

Excluded: making the command cores interruptible.

### Acceptance

- AC1: a download running when the client disconnects either completes its files or leaves no partial file behind. (`verify_by: test`)
