---
type: ADR
title: Multi-line comment compose is a mode on the Detail screen, driven by mode-aware key mapping in the shell
description: The TUI has no text-input mode; the pure update() reacts to a fixed Msg vocabulary and the shell maps each crossterm key to a Msg context-free. Comment compose needs the same keys (Enter, printable chars) to mean text, not navigation, while composing. Model compose as an Option<Compose> field on Screen::Detail (not a new Screen variant) so it shares the open task's context and Back/Esc semantics stay local. Keep update() pure: the shell's handle_input_event reads the model's compose state and picks map_compose_key_event vs map_browse_key_event; both still emit Msgs. Enter inserts a newline, Ctrl+S submits (Cmd::SubmitComment), Esc cancels.
status: Accepted
supersedes:
superseded_by:
tags: [tui, model, events, compose, comments, input, ratatui]
timestamp: 2026-06-28T00:00:00Z
---

# 0034. Multi-line comment compose as a mode on the Detail screen

## Context

The TUI is a strict Elm/TEA core ([ADR 0007](/adr/0007-tui-module-structure.md),
[ADR 0008](/adr/0008-async-event-loop-with-eventstream-and-select.md)): `update(Model, Msg) -> (Model,
Vec<Cmd>)` is **pure** (no terminal, no async), and the shell maps each crossterm
event to a `Msg` **context-free** in `events.rs::map_browse_key_event` (e.g.
`'q' -> Quit`, `Enter -> Select`, `'r' -> Refresh`, arrows -> navigation).

There is **no text-input mode anywhere in the app**. Every key currently means a
navigation/command action regardless of state. Comment compose ([PRD 0002](/prd/0002-task-comment-authoring.md))
breaks that assumption: while composing, `Enter` must insert a **newline** (the body
is multi-line), printable characters must append to a buffer, `Backspace` must delete,
and only `Ctrl+S` submits / `Esc` cancels. The *same physical keys* must mean
different things depending on whether the user is composing.

Two design questions follow:

1. **Where does the compose state live** — a new `Screen` variant, or a field on the
   existing `Screen::Detail`?
2. **How do keys get interpreted by mode** without polluting the pure `update()` with
   terminal concerns or exploding the `Msg` vocabulary into raw key events?

## Decision

**Compose is a field on `Screen::Detail`, not a new screen.**

```rust
enum ComposeKind { New, Edit { comment_id: i64 } }   // Edit lands in ADR 0036's slice
enum ComposeStatus { Editing, Submitting, Error(String) }
struct Compose { kind: ComposeKind, buffer: String, status: ComposeStatus }
// Screen::Detail gains:  compose: Option<Compose>
```

Rationale (locality force): composing is an interaction *within* the open task — it
shares `instance` / `project_id` / `task_id`, and `Esc`/`Back` should cancel the
compose *before* popping the Detail screen off the stack. A separate
`Screen::CommentCompose` variant would duplicate that task context and entangle the
navigation stack (Back would have to distinguish "cancel compose" from "leave
detail", and reflow/scroll caches would need a second home). A field keeps the fact
in one place.

**Key interpretation is mode-aware in the shell, and `update()` stays pure.** The
shell's `handle_input_event` already holds the `Model`. It reads
`model.top()`'s compose state and chooses the mapping function:

- compose active (`Some(Editing)`) → `events::map_compose_key_event(key)`:
  - printable char → `Msg::ComposeInput(char)`
  - `Enter` → `Msg::ComposeNewline`
  - `Backspace` → `Msg::ComposeBackspace`
  - `Ctrl+S` → `Msg::ComposeSubmit`
  - `Esc` → `Msg::ComposeCancel`
- compose inactive → the existing `map_browse_key_event` (unchanged), plus a new
  `'c' -> Msg::ComposeOpen(New)` arm to start a new comment on the open Detail.

Both paths still emit ordinary `Msg`s; the pure `update()` gains arms that mutate the
compose buffer/status and, on `ComposeSubmit`, emit `Cmd::SubmitComment { instance,
project_id, task_id, body }` and set `status = Submitting`. This preserves the
pure/shell split exactly: the **shell** decides *which keys are text* (a terminal-mode
concern it is allowed to own, like it already owns mouse-capture toggling), the
**pure core** decides *what each Msg does to the model*.

The async write follows the established `LoadDetail` pattern: `dispatch_cmds` spawns
`spawn_submit_comment`, which calls the client write method and sends back
`Msg::CommentMutationOk` (→ ADR 0035 refresh) or `Msg::CommentMutationErr(reason)`
(→ `status = Error(reason)`, buffer preserved).

### Guard / fitness function

- **Pure state machine (unit, headless):** `update()` tests drive the full sequence —
  `ComposeOpen` sets `compose = Some(Editing{buffer:""})`; `ComposeInput`/`ComposeNewline`/
  `ComposeBackspace` mutate the buffer (including embedded `\n`); `ComposeSubmit` emits
  `Cmd::SubmitComment` with the typed body and sets `Submitting`; `ComposeCancel`
  clears compose. No terminal, no async.
- **Mode-aware mapping (unit):** `map_compose_key_event` maps `Enter -> ComposeNewline`
  (not `Select`) and a printable key -> `ComposeInput`, while `map_browse_key_event`
  is unchanged; a test pins that `'c'` opens compose only on Detail.
- **Buffer survives failure:** a `CommentMutationErr` test asserts the buffer is intact
  and `status = Error(_)` (no lost draft).

## Alternatives considered

- **A separate `Screen::CommentCompose` variant.** Rejected: duplicates the task
  context, complicates Back/stack semantics, and needs its own reflow/scroll plumbing.
  Compose is modal *within* Detail — a field models that truthfully.
- **A raw `Msg::Key(KeyEvent)` passthrough so `update()` interprets keys.** Rejected:
  it drags crossterm key semantics into the pure core and dissolves the clean Msg
  vocabulary that makes `update()` exhaustively testable. The shell is the right place
  to know "these keystrokes are text right now".
- **Single-line input (Enter submits).** Rejected per the product decision (PRD 0002):
  comments are multi-line, so Enter must be a newline and submit needs a distinct
  chord (Ctrl+S).

## Consequences

**Positive:** the app gains a reusable text-compose mode with a pure, fully-testable
state machine; create and edit ([ADR 0036](/adr/0036-permission-aware-comment-targeting.md))
share it (edit just seeds `buffer` and sets `kind = Edit`). The pure/shell boundary is
preserved — no terminal types enter `update()`. The Msg vocabulary grows by a small,
named set rather than a raw key passthrough.

**Accepted trade-offs:** `Screen::Detail` gains a field and the shell's
`handle_input_event` gains a mode branch (a small, well-contained complexity). The
first version's editing is append/backspace + newline at the end of the buffer — no
in-body caret movement; richer caret editing is a deferred follow-up (PRD 0002 open
question). The compose region must be reflow/scroll-aware in the renderer
([BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md) Test Design).

## Related

- PRD: [/prd/0002-task-comment-authoring.md](/prd/0002-task-comment-authoring.md)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md) (the write Cmd::SubmitComment calls into)
- ADR: [/adr/0035-server-truth-refresh-after-comment-mutation.md](/adr/0035-server-truth-refresh-after-comment-mutation.md) (the CommentMutationOk handler)
- ADR: [/adr/0036-permission-aware-comment-targeting.md](/adr/0036-permission-aware-comment-targeting.md) (edit reuses this compose mode)
- ADR: [/adr/0007-tui-module-structure.md](/adr/0007-tui-module-structure.md), [/adr/0008-async-event-loop-with-eventstream-and-select.md](/adr/0008-async-event-loop-with-eventstream-and-select.md) (the TEA core this extends)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md)
