---
type: ADR
title: The flat commands.rs splits into a commands/ directory of deep modules — one input-resolution master (resolve), one presentation master (presenter), and one orchestration module per command family (setup / task / comment / mine), behind a thin re-exporting mod.rs
description: commands.rs is a 1035-line flat module where every command core (get_core, current_core, comment_core, mine_core, the setup family) serves three masters at once — it resolves input (parse_task_ref / parse_branch_ref / pick_instance / the branch-or-ref fallback), orchestrates the client + cache, and formats presentation (i18n text, JSON-vs-text, the out/err writers, exit codes). The presentation master leaks worst — the ADR 0042 401 re-auth literal "Token invalid or revoked — run `ac setup add` to re-authenticate." is copied into three command families (load_task, comment_core, mine_core), so a change to the re-auth guidance is a three-place edit. Split the file into src/commands/{mod,resolve,presenter,setup,task,comment,mine}.rs — resolve owns input resolution, presenter single-homes the user-facing strings (the re-auth message lands in one presenter::reauth_message()), each family module owns its orchestration, and mod.rs is a thin root that `pub use` re-exports the public surface so main.rs and tui/mod.rs call sites are untouched. Behavior identical; the existing command specs are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [commands, cli, refactor, locality, depth, module-structure]
timestamp: 2026-07-03T00:00:00Z
---

# 0055. The flat commands.rs splits into a commands/ directory of deep modules

## Context

`src/commands.rs` is a **1035-line flat module** holding the testable core of every
non-TUI command: the `setup` family (`setup_list`, `setup_remove`, `setup_language`,
`setup_test`/`setup_test_core`, `setup_add`, `run_connectivity_check`), the task read path
(`get_core`, `current_core`, `do_get_task`, `load_task`), the non-interactive write
(`comment_core`), and the aggregation path (`mine_core`, `collect_mine_rows`,
`fetch_mine_rows_checked`). Alongside them sit the input helpers (`parse_task_ref`,
`parse_branch_ref`, `pick_instance`, `resolve_task_ref_for_comment`, the two `OnceLock`
regexes) and the presentation helpers (`write_comment_success`, `write_comment_failure`).

Every command core **serves three masters at once**:

1. **Input resolution** — turning a CLI ref, a git branch, or an instance flag into typed
   ids / a chosen `Instance` (`parse_task_ref`, `parse_branch_ref`, `pick_instance`,
   `resolve_task_ref_for_comment`).
2. **Orchestration** — building the `ActiveCollabClient`, reading/writing the cache, issuing
   the fetch/mutation, threading the `JoinSet` for the aggregation path.
3. **Presentation** — the user-facing surface: `i18n::t(...)` strings, the JSON-vs-text
   branch, the `out`/`err` writers, and the exit codes.

The **presentation master leaks worst**. The [ADR 0042](/adr/0042-detect-401-and-guide-reauthentication.md)
401 re-auth literal — `"Token invalid or revoked — run \`ac setup add\` to re-authenticate."`
— is written in **three** separate command families: `load_task` (the read path),
`comment_core` (the write path), and `mine_core` (the aggregation path). A change to the
re-auth guidance is a three-place edit across three unrelated functions. Several cores also
carry `#[allow(clippy::too_many_arguments)]` — the presentation master (`out`, `err`, `json`)
is threaded as raw parameters through every signature. The module is **shallow in the large**:
one file, no seams, three concerns interwoven per function.

## Decision

Split the flat file into a `src/commands/` directory of **deep modules**, one master per
seam, behind a thin re-exporting root.

```
src/commands/
  mod.rs        thin root: `mod` declarations + `pub use` re-exports + `#[path] mod tests`
  resolve.rs    input-resolution master — parse_task_ref, parse_branch_ref, pick_instance,
                resolve_task_ref_for_comment, the task_url_re/branch_re regexes
  presenter.rs  presentation master — write_comment_success, write_comment_failure,
                and the single-homed reauth_message() -> String
  setup.rs      setup family orchestration (list/remove/language/test/add + connectivity)
  task.rs       task read orchestration (get_core, current_core, do_get_task, load_task, DisplayFlags)
  comment.rs    non-interactive write orchestration (comment_core)
  mine.rs       aggregation orchestration (mine_core, collect_mine_rows, fetch_mine_rows_checked, MineOutcome)
```

1. **`src/commands.rs` becomes `src/commands/mod.rs`.** Because the module gains a directory,
   its test include deepens one level: `#[path = "../tests/unit/commands.rs"]` →
   `#[path = "../../tests/unit/commands.rs"]`.

2. **The re-export seam stays stable.** `mod.rs` `pub use`s every symbol the external call
   sites and the test module consume (`get_core`, `current_core`, `comment_core`, `mine_core`,
   `collect_mine_rows`, `MineOutcome`, `pick_instance`, `DisplayFlags`, `SetupAddFields`, and
   the full setup family), so `main.rs` and `tui/mod.rs` (the only external callers) and the
   `use super::*` test module compile unchanged. Symbols moved out of `mod.rs` but referenced
   by the monolithic test file are `pub(crate)` + re-exported so `super::*` still resolves.

3. **The re-auth message lands in one home.** `presenter::reauth_message() -> String` returns
   the `i18n::t(...)` literal; `load_task`, `comment_core`, and `mine_core` call it instead of
   re-typing the string. The classification of *when* a 401 occurred already lives once per
   path (`HTTP_UNAUTHORIZED` / the typed `Unauthorized` error / `CommentWriteOutcome::Unauthorized`
   from [ADR 0054](/adr/0054-comment-write-outcome-typed-classification.md)); this ADR
   single-homes the *message* those three paths emit.

### Scope boundary

This is a **module restructuring**, not a signature redesign. The command cores keep their
current interfaces (still taking `out`/`err` writers and returning `i32` exit codes) — the
work moves functions to the master that owns them and single-homes the one duplicated string.
It does **not** rewrite the resolution helpers to return typed errors instead of writing to
`err`, nor collapse the `out`/`err`/`json` parameter triple into a presenter object — those are
larger, behavior-surface-touching passes deliberately left for later. The **test module stays
one file** (`tests/unit/commands.rs`, attached to `mod.rs` via `#[path]`); splitting it into
per-family test files under `tests/unit/commands/` is a follow-up, not this change.

### Guard / fitness function

- **Behavior preserved — invisible to the user.** Every command prints the same text, emits
  the same `--json`, and returns the same exit codes. The full existing command suite
  (`setup_*`, `get`/`current`, `comment_core`, `mine_core`) stays green — it is the
  characterization net for the move.
- **The re-auth message has one home.** `grep` finds the literal
  `Token invalid or revoked` exactly once (in `presenter::reauth_message`); `load_task`,
  `comment_core`, and `mine_core` call the function. Deleting `presenter` re-scatters the
  string back into the three families — the deletion test concentrates, it does not merely move.
- **The masters do not leak into each other.** `resolve.rs` imports no client/cache;
  `presenter.rs` imports only `i18n` + `agent_json` + `Write` (no client/controller). The
  family modules depend on `resolve` and `presenter`, never the reverse.
- **The re-export surface is the seam.** `main.rs` and `tui/mod.rs` are edited **not at all**;
  their `crate::commands::*` paths resolve through `mod.rs` re-exports. That untouched call
  surface is the proof the seam held.
- Full suite green; `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `cargo test --test comment_policy` clean; complexity within budget.

## Alternatives considered

- **Leave the flat file, extract only `presenter::reauth_message`.** Rejected: it fixes the
  one duplicated string but leaves the three masters interwoven in a 1035-line file — the
  structural friction (no seams, wide signatures, one file for four command families) remains.
  The user chose the full split precisely to give each master its own home.
- **Split by command family only (setup/task/comment/mine), no resolve/presenter masters.**
  Rejected: it would leave `parse_task_ref` and the re-auth literal duplicated *across* the
  family modules — the cross-cutting concerns need their own single home, or the split just
  redistributes the duplication.
- **Redesign the resolution helpers to return typed errors and fold `out`/`err`/`json` into a
  presenter object in the same change.** Rejected for now: it touches every error path's
  behavior surface at once — a large, risk-bearing change layered on top of the file move.
  Scoped out (see Scope boundary); the module tree this ADR lands makes it a smaller, later pass.

## Consequences

**Positive:** each of the three masters has one home — input resolution, presentation, and
per-family orchestration are separable and independently testable; the re-auth guidance is a
one-place edit; the `commands` module matches the established directory-module convention
(`store/`, `render/`, `tui/`); a new command family adds a file rather than growing a flat
god-module; and the external call surface (`main.rs`, `tui/mod.rs`) is provably untouched.

**Accepted trade-offs:** more files (seven where there was one) and a deeper test-include path;
the command cores still carry the `out`/`err`/`json` parameter triple (the presenter-object
collapse is a later pass); the test module stays monolithic for now, so per-family test
locality is not yet realized.

## Related

- ADR: [/adr/0042-detect-401-and-guide-reauthentication.md](/adr/0042-detect-401-and-guide-reauthentication.md) (the 401 re-auth message this single-homes)
- ADR: [/adr/0054-comment-write-outcome-typed-classification.md](/adr/0054-comment-write-outcome-typed-classification.md) (the write-path 401 classification whose message this consolidates)
- ADR: [/adr/0040-non-interactive-comment-write-command.md](/adr/0040-non-interactive-comment-write-command.md) (comment_core, moved into commands/comment.rs)
- ADR: [/adr/0004-tests-in-tests-dir-via-path-include.md](/adr/0004-tests-in-tests-dir-via-path-include.md) (the #[path] test-include convention the deepened path follows)
- ADR: [/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md](/adr/0049-split-render-into-text-measure-wrap-and-render-adapters.md) (the render/ directory-module split this mirrors)
</content>
</invoke>
