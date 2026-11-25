---
type: Issue
title: The served skill tells the agent to prefer the MCP tools
description: The text `ac skill active-collab` prints tells the agent to call the `ac mcp` tools when that server is connected, and to run the `ac` CLI otherwise.
status: closed
timestamp: 2026-11-25T00:00:00Z
---

## 0094. The served skill tells the agent to prefer the MCP tools

### Problem

`ac skill active-collab` (0.12.3) documents `ac mcp` and its tools `ac_read`, `ac_write` and `ac_download_attachments` only as an install step. Proof: `ac skill active-collab | grep -i prefer` prints nothing. An agent whose harness has the server connected still composes `ac … --json` shell commands, losing the typed parameters the tools give.

### Scope

Included: the served body, `.claude/skills/active-collab/SKILL.md`, which `src/commands/skill.rs` embeds with `include_str!`.

Excluded: the frontmatter `description`; the thin pointer copies under `.codex/`, `.github/`, `.opencode/`, `.pi/`, `packages/active-collab-agent/` and `plugins/active-collab/`, which already say to run `ac skill active-collab`; adding a `browse` operation to `ac_read`.

### Decision

- The body opens with `## Prefer the MCP tools` (when the server is connected, call its tools instead of the shell, with a table mapping each tool and operation to the command it replaces) followed by `## Otherwise use the CLI` (run the documented commands; `ac browse` has no MCP operation and always runs in the CLI), both before `## Commands`. The pattern follows the Bitbucket plugin skill. Not taken: a sentence inside the existing `## MCP server` section, which an agent reaches only after the CLI contract.

### Acceptance

- AC1: the served body has the prefer heading, then the otherwise heading, then `## Commands`; the prefer section names the three tools and the operations `get`, `current`, `mine`, `comment`, `log_time`, `set_task`, `time_report`; the otherwise section names `ac browse`. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Outcome

`.claude/skills/active-collab/SKILL.md` gains the two sections. Proof: `docker compose run --rm dev cargo test --bin ac skill_` (9 passed, the new `skill_prefers_mcp_tools_then_falls_back_to_cli_before_commands` red before the text change); full `cargo test -- --test-threads=1` (1881 binary tests plus every integration test) passes; `clippy --all-targets -D warnings`, `fmt --check` and `scripts/check-skill-stubs.sh` pass.
