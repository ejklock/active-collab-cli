---
type: ADR
title: The comment-write seam returns a typed CommentWriteOutcome — the client classifies 401 / success / failure once, retiring the duplicated HTTP_UNAUTHORIZED status compare at the shell and CLI call sites
description: The comment-mutation methods (create_comment, update_comment, delete_comment) return a raw HTTP status tuple, so BOTH callers — the TUI shell's spawn_comment_write and the CLI's comment_core — independently re-classify the status: each re-checks status == HTTP_UNAUTHORIZED for 401 and (200..=299) for success. The 401 detection (ADR 0042) is thus duplicated across two call sites over a raw status the client hands back un-judged, and the write seam (ADR 0033) is the odd one out — the read methods already classify once (fetch_open_tasks returns a typed Unauthorized error; fetch_task's status is mapped once into the controller's FetchResult enum). Make the client own the write-seam classification: the three comment-mutation methods return a typed CommentWriteOutcome { Ok(Option<Value>), Unauthorized, Failed(u16) }; the shell and CLI match the typed outcome instead of comparing raw status. Behavior identical — same Msg::AuthExpired, same CLI re-auth message and exit code; the existing comment-write specs are the characterization net.
status: Accepted
supersedes:
superseded_by:
tags: [client, http, auth, comments, refactor, locality, depth]
timestamp: 2026-07-01T00:00:00Z
---

# 0054. The comment-write seam returns a typed CommentWriteOutcome

## Context

The authenticated comment-write seam ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md))
exposes three methods on `ActiveCollabClient` — `create_comment`, `update_comment`, `delete_comment`
— that each return a **raw HTTP status** (`Result<(u16, Option<Value>)>` for create/update,
`Result<u16>` for delete). The client does not judge the status; each **caller re-classifies it**:

- The TUI shell's `spawn_comment_write` (`src/tui/mod.rs:677`) maps every write to a bare `u16`, then
  matches: `(200..=299)` → `Msg::CommentMutationOk`, `status == HTTP_UNAUTHORIZED` → `Msg::AuthExpired`,
  else → `Msg::CommentMutationErr`.
- The CLI's `comment_core` (`src/commands.rs:791`) matches the `create_comment` tuple:
  `(200..=299)` → success + exit 0, `HTTP_UNAUTHORIZED` → re-auth message + exit 1, other status →
  generic HTTP error + exit 1, transport `Err` → failure + exit 1.

So the [ADR 0042](/adr/0042-detect-401-and-guide-reauthentication.md) 401 detection for writes lives
in **two places**, each re-deriving it from a raw status the client handed back un-judged, and each
re-implementing the success-range check. The write seam is the **odd one out**: the read methods
already classify once — `fetch_open_tasks` returns a typed `Unauthorized` error, and `fetch_task`'s
status is mapped a single time into the controller's `FetchResult` enum. The write path never got
that treatment.

## Decision

Make the client own the write-seam classification: the comment-mutation methods return a typed
outcome, and the callers match it instead of comparing a raw status.

1. **A typed outcome.** Introduce `CommentWriteOutcome` in `client.rs`:

   ```
   pub enum CommentWriteOutcome {
       Ok(Option<Value>),  // 2xx — carries the response body when present (None for delete)
       Unauthorized,       // 401
       Failed(u16),        // any other status
   }
   ```

2. **The client classifies once.** `create_comment`, `update_comment`, and `delete_comment` return
   `Result<CommentWriteOutcome>`: `(200..=299)` → `Ok(body)` (delete → `Ok(None)`),
   `HTTP_UNAUTHORIZED` → `Unauthorized`, otherwise → `Failed(status)`. The success-range and 401
   checks live once, inside the client.

3. **Callers match the outcome.** `spawn_comment_write` matches
   `Ok(CommentWriteOutcome::Ok(_))` → `CommentMutationOk`, `Ok(CommentWriteOutcome::Unauthorized)` →
   `AuthExpired`, else → `CommentMutationErr`. `comment_core` matches `Ok(CommentWriteOutcome::Ok(body))`
   → success + exit 0 (extracting the comment id from `body`), `Ok(CommentWriteOutcome::Unauthorized)`
   → the re-auth message + exit 1, `Ok(CommentWriteOutcome::Failed(status))` → the `HTTP {status}`
   error + exit 1, transport `Err` → failure + exit 1. No `== HTTP_UNAUTHORIZED` or `(200..=299)`
   comparison remains at either call site.

### Scope boundary

This ADR unifies the **write** seam only. The read methods are deliberately left as-is: `fetch_task`
is already classified once in `controller::fetch_from_network` (the `FetchResult` enum) and
`fetch_open_tasks` already returns a typed `Unauthorized` error — neither duplicates the check, so
neither needs this change. A full client-wide outcome type is a possible future pass, not this one.

### Guard / fitness function

- **Behavior preserved — invisible to the user.** The TUI still sends `AuthExpired` on a write 401
  (→ `auth_error` status line) and `CommentMutationOk`/`CommentMutationErr` otherwise; the CLI still
  prints the same success/re-auth/HTTP-error text and returns the same exit codes. All existing
  comment-write specs (client, `comment_core`, shell mutation) stay green.
- **The 401 classification for writes has one home.** Grep finds no `HTTP_UNAUTHORIZED` comparison
  and no `(200..=299)` write-status check in `spawn_comment_write` or `comment_core`; both match
  `CommentWriteOutcome`. The classification lives once, in `client.rs`.
- **The interface is the test surface.** `client` unit tests assert each mutation method returns
  `Ok`/`Unauthorized`/`Failed` for 2xx/401/other status against the mocked server; the caller tests
  assert the mapped `Msg` / exit code from the typed outcome.
- **The deletion test passes.** Deleting `CommentWriteOutcome` would push the success-and-401
  classification back into both callers — it concentrates the write-path classification, not merely
  moves it.
- Full suite green; `cargo clippy --all-targets -D warnings`, `cargo fmt --check`, `comment_policy`
  clean; complexity within budget.

## Alternatives considered

- **A shared `is_unauthorized(status) -> bool` predicate.** Rejected: it single-homes the literal
  `== 401` but leaves each caller still branching on raw status for success vs failure — a shallow
  helper, not a classification the client owns.
- **One client-wide `ApiOutcome<T>` for reads and writes.** Rejected for now: the read methods are
  not duplicated (each already classifies once), so folding them in would be a large, risk-bearing
  refactor of the fetch path for little locality gain. Scoped to the write seam where the duplication
  actually lives.
- **Leave the split (status quo).** Rejected: the write 401 detection lives in two call sites over a
  raw status, so a change to how a write 401 is handled (or the success range) is a two-place edit —
  exactly the inconsistency the read methods already avoid.

## Consequences

**Positive:** the comment-write 401/success/failure classification has one home (the client); the
shell and CLI consume a typed outcome instead of re-deriving HTTP semantics; a future write method
(or a change to the 401 policy) inherits the classification instead of copying the status compare;
the write seam becomes symmetric with the already-classified read methods.

**Accepted trade-offs:** a new public enum (`CommentWriteOutcome`) on the client surface — a
deliberate, small type that raises the seam's depth (callers learn one outcome instead of the raw
HTTP status vocabulary); `delete_comment`'s return type widens from `u16` to the shared outcome
(consistent, at the cost of one enum wrap for a method that ignores the body).

## Related

- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md) (the comment-write seam this types)
- ADR: [/adr/0042-detect-401-and-guide-reauthentication.md](/adr/0042-detect-401-and-guide-reauthentication.md) (the 401 detection whose write-path duplication this single-homes)
- ADR: [/adr/0040-non-interactive-comment-write-command.md](/adr/0040-non-interactive-comment-write-command.md) (the CLI `comment` command that matches the outcome)
