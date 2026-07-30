---
type: ADR
title: Control characters stripped at the untrusted-text render boundary
description: A single pure sanitizer (src/sanitize.rs) removes C0/C1 control characters — everything except newline and tab — from every server-supplied string before it reaches a terminal, applied at the CLI string-building boundary and at the three richtext entity-decode sites, closing the ANSI/terminal escape injection confirmed as issue 0062 C1.
status: Proposed # Proposed | Accepted | Superseded | Deprecated
supersedes:                 # NNNN of the ADR this replaces, if any
superseded_by:              # NNNN, set when a later ADR replaces this one
tags: [security, render, terminal, injection, ansi, tui, cli]
timestamp: 2026-07-30T14:37:22Z
---

# 0068. Control characters stripped at the untrusted-text render boundary

## Context

Everything the CLI prints about a task comes from the ActiveCollab API, and most of it is
written by other people: the task name, the body HTML, comment bodies, comment author
names, project names. That is untrusted input, and it currently reaches the terminal
byte-for-byte.

`cli_render::html_to_text` (`src/render/cli_render.rs:35-46`) strips tags and then calls
`html_escape::decode_html_entities` with **no control-character filter**. `&#27;` /
`&#x1b;` in a body decodes to a literal `ESC` (0x1B); every other C0/C1 byte survives
verbatim. The result is written straight to stdout by `render::render_task`
(`src/render/mod.rs:532`), reachable through `ac get <ref>` and `ac current` without
`--json`/`--short`.

A validation pass over [issue 0062](/issues/0062-security-audit-findings-pending-validation.md)
(candidate C1 — **confirmed**) found the crate has **no** control-character stripping
anywhere, and that the exposed surface is wider than the entity-decode path:

- `task.name`, emitted raw in the human view (`cli_render.rs:185`) and by `--short`
  (`src/commands/task.rs:145-146`);
- `created_by_name`, emitted raw (`cli_render.rs:128`);
- `body_plain_text`, used verbatim when present — it never passes through `html_to_text`
  at all (`cli_render.rs:114-126`);
- the `ac mine` table rows (`src/render/mod.rs:429`);
- user display names from the user map (`cli_render.rs:55`);
- the TUI: the three richtext entity-decode sites (`src/richtext.rs:277,283,615`).

**Abuser story.** Any collaborator who can create or comment on a task the victim views
posts a body containing `&#27;[…]` sequences. HTML entities pass server-side raw-byte
filtering because they are valid HTML text. When the victim runs `ac get <task>`, the
sequences reach their terminal: reliable output spoofing (forge a success line, hide or
overwrite text, corrupt the display) and, in emulators that honor them, OSC sequences
(clipboard write, window-title set/query).

The `--json` path is already safe — `serde_json` turns every control character into a JSON
unicode escape (ESC becomes a six-character backslash-u sequence,
`src/commands/task.rs:140`) — and must stay byte-identical, because it is the agent
contract.

Rejected alternative — *escape control characters into a visible form* (caret notation,
or a printable unicode-escape rendering):
it changes what benign text looks like and invites a second round of "unescape it for
display". Dropping the bytes is unambiguous and idempotent.

Rejected alternative — *sanitize only inside `html_to_text`*: it misses `body_plain_text`,
`name`, author names, and the `mine` table, all of which reach the same terminal. The
boundary is "server-supplied string about to be rendered", not "HTML being converted".

## Decision

We will add one pure module, `src/sanitize.rs`, with a single public function:

```rust
/// Remove control characters from server-supplied text before it reaches a terminal.
/// Keeps `\n` and `\t`; drops C0 (U+0000-U+001F), DEL (U+007F), and C1 (U+0080-U+009F).
pub fn strip_control_chars(s: &str) -> String
```

Allocation-free on the common path (return the input unchanged when nothing is filtered is
*not* required; a straightforward `chars().filter().collect()` is acceptable — this runs
once per rendered field, not per frame).

It is applied at two boundaries — **at the write seam, not per field**:

1. **CLI stdout** — every assembled string is sanitized immediately before it is written:
   `render::render_task` (`src/render/mod.rs`, the `ac get`/`ac current` human view, which
   carries the whole `cli_render` output: task name, meta, description, comment authors and
   bodies), `render::render_mine_table` (`src/render/mod.rs`, the `ac mine` rows), and the
   `--short` line in `src/commands/task.rs`. Sanitizing the assembled string rather than
   each field is deliberate: a field added to a renderer later is covered automatically,
   and the operation is idempotent on our own labels.
2. **TUI richtext** (`src/richtext.rs`) — on the decoded string at all three
   `decode_html_entities` sites, so the escape can never reach the crossterm backend
   regardless of what ratatui's cell buffer does with a zero-width control char. This is
   defense in depth, not a claim about ratatui's behavior.

`--json` output is **not** sanitized: `serde_json` already escapes control characters, and
the agent contract is byte-stable.

## Consequences

**Easier / gained:**
- One named seam an auditor can grep for, and one place to change the policy.
- Both human-facing renderers (CLI text and TUI) are covered by the same rule.

**Harder / accepted trade-offs:**
- The byte-for-byte parity contract with the legacy Python `render.py` now diverges **for
  inputs containing control characters** — deliberately. Parity for all other input is
  unchanged.
- A future CLI command that writes server-derived text through a new path must sanitize at
  its own write site. The seam is per-writer, because CLI output is assembled as plain
  `String`s in several modules rather than passing through one writer.

**Follow-ups:**
- If a future refactor gives the CLI a single write seam, move the call there and delete
  the per-writer calls.

## Verification

**Implementation impact:** `src/sanitize.rs` (new), `src/main.rs` (module declaration),
`src/render/mod.rs`, `src/commands/task.rs`, `src/commands/mine.rs`, `src/richtext.rs`,
`tests/unit/sanitize.rs` (new), `tests/unit/render.rs`, `tests/unit/richtext.rs`.

**Verification criteria:**
- `strip_control_chars` drops ESC, BEL, DEL, and C1 bytes, and preserves `\n`, `\t`, and
  multi-byte UTF-8 (emoji, accented characters) exactly.
- A task body of `&#27;[2J` rendered by `render_task` produces output containing **no**
  `0x1B` byte — the negative test bound to issue 0062 C1.
- A comment with `body_plain_text` containing a raw ESC renders with no `0x1B` byte.
- A task `name` containing a raw ESC produces no `0x1B` byte in both the human view and
  `--short`.
- The richtext parser given `&#27;[31m` yields spans whose text contains no `0x1B` byte.
- `--json` output for a body containing ESC still contains the JSON unicode escape for
  ESC, not a raw byte (the agent contract is unchanged).
