---
name: active-collab
description: Read ActiveCollab task data — a task, your assignments, comments, or projects — as machine-readable JSON from the `ac` CLI, non-interactively without the TUI. Use when an agent or script needs to fetch a task by id or URL, list the logged-in user's open tasks, read the task for the current git branch, or browse projects, and wants structured JSON instead of the interactive terminal UI. Covers `ac get`, `ac current`, `ac mine`, and `ac browse` with `--json` — the curated minified schemas, the round-trippable `ref`, and the cache/`--no-comments` flags.
---

# active-collab (thin pointer)

The full, authoritative ActiveCollab `--json` read contract is served by the CLI itself.

Run:

    ac skill active-collab

and follow its output. It documents the curated minified JSON schemas for
`ac get`, `ac current`, `ac mine`, and `ac browse` with `--json`, the round-trippable `ref`,
and the cache / `--no-comments` flags.
