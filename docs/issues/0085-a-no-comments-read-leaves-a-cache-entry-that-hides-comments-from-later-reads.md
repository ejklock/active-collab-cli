---
type: Issue
title: A --no-comments read leaves a cache entry that hides comments from later reads
description: A --no-comments read caches an empty comment list that later reads and attachment downloads serve as if the task had no comments.
status: closed
labels: [cli, cache, bug]
blocked_by: []
timestamp: 2026-11-12T14:35:12Z
---

## 0085. A --no-comments read leaves a cache entry that hides comments from later reads

### Problem

`ac get <ref> --no-comments` stores the task in the cache with an empty comment list (`src/commands/task.rs:78-91`). A later `ac get <ref>` or `ac get <ref> --download-attachments` is served from that entry (`src/commands/task.rs:40-49`); `TaskCache::read` (`src/store/cache.rs:24`) never expires an entry, so only `--refresh` clears it, so it shows no comments and downloads none of their attachments, without any request to the server. Found by the review of issue 0084's second slice on 2026-11-12: against the mock ActiveCollab, `ac get 10/102 --no-comments` then `ac get 10/102 --download-attachments` printed "Downloaded 0 of 0 attachment(s)" and the mock saw no request.

### Scope

Included: a read that wants comments never takes them from a cache entry written by a `--no-comments` read.

Excluded: the cache lifetime and the cache layout beyond what the fix needs.

### Decision

- `load_task` caches the full comment list the API returned, whatever `--no-comments` says; `--no-comments` drops the comments only from what the read returns, on a cache hit as on a miss, so the cached entry is the same for both reads (session, 2026-11-12). Not taken: skipping the cache write on a `--no-comments` read, which sends every such read to the network and still lets a hit return comments the flag hides.
- `TaskCache::write` marks each entry as holding the full comment list with a format key inside the stored JSON, and `TaskCache::read` treats an entry without that key as a miss, so an entry an earlier version wrote with an emptied list is fetched again once instead of hiding comments until `--refresh` (session, 2026-11-12). Every reader of the task cache (CLI, MCP, TUI detail) gets this through `TaskCache`; each entry written before the change costs one fetch. Not taken: a new column, a schema migration for a cache; leaving old entries to `--refresh`, which nothing tells the user to run.

### Acceptance

- AC1: after `ac get <ref> --no-comments`, `ac get <ref>` shows the task's comments and `--download-attachments` downloads their attachments. (`verify_by: test`)
- AC2: a `--no-comments` read after a full read still prints no comments. (`verify_by: test`)
- AC3: a cache entry without the format key, as an earlier version wrote it, is fetched again on its next read and then served from the cache. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)
