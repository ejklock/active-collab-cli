---
type: Issue
title: Claude Code drops ac_read and ac_write because their schema is a top-level oneOf
description: ac_read and ac_write publish a flat object schema with an operation enum so Claude Code lists them, and the stale active_collab.py setup hint becomes ac setup add.
status: open
timestamp: 2026-11-13T19:19:16Z
---

## 0091. Claude Code drops ac_read and ac_write because their schema is a top-level oneOf

### Problem

Claude Code shows only `ac_download_attachments` from `ac mcp`: `ac_read` and `ac_write` publish an `inputSchema` whose top level is `oneOf` (derived from the `ReadOperation` and `WriteOperation` enums in `src/mcp/mod.rs:105` and `:148`), and Claude's tool API rejects `oneOf`, `anyOf` and `allOf` at the top level, so the client drops both tools. Proof: `tools/list` from `~/.local/bin/ac mcp` in a temporary store returns `ac_read` and `ac_write` with keys `$schema, oneOf, type`; the `/mcp` panel lists 1 tool. `tests/mcp_stdio.rs` never caught it because its rmcp client accepts any schema.

Separately, three error messages still tell the user to run `active_collab.py setup add` (`src/commands/resolve.rs:21`, `:85`, `src/commands/setup.rs:23`, `locales/pt_BR.json:33-34`), the Python tool's name, and `ac mcp --help` lists only `ac_read, ac_write` (`src/cli.rs:58`).

### Scope

Included: a flat top-level object schema for `ac_read` and `ac_write`; a test that every published tool schema has no top-level `oneOf`/`anyOf`/`allOf`; the stale `active_collab.py` hint and `ac mcp` help text.

Excluded: splitting the tools per operation; changing operations, their fields or the server-side parse.

### Decision

- `ac_read` and `ac_write` keep their operations and publish a flat schema: top-level `type: object`, a required `operation` enum, every operation's fields as optional properties, each described with the operations it applies to; the server still parses into the operation enums and rejects a field the operation does not take (owner, 2026-11-13). Not taken: one tool per operation, superseding ADR 0072; leaving the schema as is.
- The flat schema and the enums are two declarations of one contract, kept equal by a test.
- `ac_write` rejects a field its operation does not take, as `ac_read` already does: `WriteOperation` gets `deny_unknown_fields`, so a `comment` sent with `hours` fails as `invalid_params` and writes nothing (owner, 2026-11-13). Before, it posted the comment and dropped the extra fields silently; the flat schema offers every write field to every write operation, which makes that mistake likelier. Not taken: documenting that `ac_write` ignores extra fields.
- The `operation` property's description lists each operation with its own description, and each field's description names the operations that take it and whether each requires it; a test checks that text exactly.

### Acceptance

- AC1: `tools/list` returns, for every tool, an `inputSchema` with top-level `type: object` and no top-level `oneOf`, `anyOf` or `allOf`. (`verify_by: test`)
- AC2: every field each `ReadOperation` and `WriteOperation` variant accepts appears in its tool's flat schema, and the `operation` enum lists exactly the variants. (`verify_by: test`)
- AC3: an operation given a field it does not take, or no `operation`, still fails as `invalid_params`. (`verify_by: test`)
- AC4: Claude Code lists `ac_read`, `ac_write` and `ac_download_attachments` from the plugin. (`verify_by: manual`)
- AC5: no message or test mentions `active_collab.py`; the hint reads `ac setup add`; `ac mcp --help` names the three tools. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)
