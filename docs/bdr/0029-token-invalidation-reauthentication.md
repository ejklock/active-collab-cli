---
type: BDR
title: A revoked API token (HTTP 401) produces actionable re-authentication guidance — CLI prints a clear message and exits non-zero; the TUI shows a status-line pointing to `ac setup add`
description: Observable behavior for handling an invalidated/revoked ActiveCollab API token. When an authenticated request returns HTTP 401, the app no longer renders an empty task/list silently; instead it tells the user the token is invalid and to re-authenticate with `ac setup add`. The CLI exits non-zero; the TUI surfaces the guidance in the thin status line. Other non-200 responses (404/5xx) keep their current behavior — only 401 is treated as an auth failure.
status: Accepted
supersedes:
superseded_by:
tags: [auth, http, error-handling, cli, tui, resilience]
timestamp: 2026-06-29T00:00:00Z
---

# 0029. Token invalidation → re-authentication guidance

Implements [ADR 0042](/adr/0042-detect-401-and-guide-reauthentication.md). The
ActiveCollab token does not expire and has no refresh endpoint; it is invalidated only
by revocation (logout, password change, admin removal), after which every authenticated
request returns HTTP 401.

## Scenarios

### Scenario 1 — CLI task detail with a revoked token (issue 0042)

```
Given a configured instance whose stored token has been revoked
  And the user runs `ac get <project>/<task>` (or `ac current`)
When the task fetch returns HTTP 401
Then the CLI prints an actionable message naming the cause (token invalid/revoked)
  And instructing the user to re-authenticate with `ac setup add`
  And the process exits with a non-zero status
  And it does NOT print the generic "task not found" message
```

### Scenario 2 — CLI comment write with a revoked token (issue 0042)

```
Given a configured instance whose stored token has been revoked
  And the user runs `ac comment <task> -m "..."`
When the create-comment POST returns HTTP 401
Then the CLI prints the actionable re-auth message (not a generic HTTP error)
  And exits non-zero
  And with `--json`, the result is the failure shape (no false `"ok": true`)
```

### Scenario 3 — CLI mine list with a revoked token (issue 0043)

```
Given a configured instance whose stored token has been revoked
  And the user runs `ac mine`
When the open-tasks fetch returns HTTP 401 (raised as a typed Unauthorized)
Then the CLI prints the actionable re-auth message
  And exits non-zero
  And does NOT print an empty task table as if there were simply no tasks
```

### Scenario 4 — Other non-200 is unchanged (issues 0042, 0043)

```
Given a configured instance with a VALID token
When an authenticated request returns 404 or 500 (not 401)
Then the existing behavior is preserved (e.g. "task not found (HTTP 404)", empty list)
  And no re-auth message is shown (a missing task or server error is not an auth failure)
```

### Scenario 5 — TUI detail load with a revoked token (issue 0044)

```
Given the TUI is open on a task and the stored token has been revoked
When the detail load/refresh fetch returns HTTP 401
Then the thin status line shows the actionable re-auth message
  (session expired / token invalid — re-authenticate with `ac setup add`)
  And the app does not silently render an empty detail with no explanation
```

### Scenario 6 — TUI comment mutation with a revoked token (issue 0044)

```
Given the TUI is open on a task and the stored token has been revoked
When a comment create/edit/delete returns HTTP 401
Then the status line shows the same actionable re-auth message
  (distinct from the generic "failed to post comment" copy)
  And the compose buffer is preserved (no data loss)
```

### Scenario 7 — i18n (issues 0042–0044)

```
Given the active language is pt-BR
When any of the above re-auth messages is shown
Then it renders the pt-BR translation
  And the English source key is identity (ADR 0005)
```

## Test Design

| # | Scenario | Level | Technique | Instrument (test) |
|---|---|---|---|---|
| 1 | CLI detail 401 | unit | mocked-server (wiremock) | `tests/unit/commands.rs`: `get`/`load_task` against a MockServer returning 401 → asserts re-auth message on out + non-zero exit; assert NOT the "not found" string |
| 2 | CLI comment 401 | unit | mocked-server | `tests/unit/commands.rs`: `comment_core` with MockServer 401 → re-auth message + non-zero exit; `--json` failure shape, no `"ok":true` |
| 3 | CLI mine 401 | unit | mocked-server | `tests/unit/client.rs`: `fetch_open_tasks` 401 → `Err(Unauthorized)` (downcast); `tests/unit/commands.rs`: `mine_core` 401 → re-auth message + non-zero exit |
| 4 | Other non-200 unchanged | unit | mocked-server | `tests/unit/commands.rs` / `client.rs`: 404/500 keep current output/empty (regression: no re-auth message, exit/shape as before) |
| 5 | TUI detail-load 401 | unit | TestBackend buffer + pure update | `tests/unit/tui_render.rs`: drive the load-result path with a 401 outcome → status line renders the re-auth message in the rendered buffer |
| 6 | TUI mutation 401 | unit | pure update + buffer | `tests/unit/tui_render.rs` / `model.rs`: a 401 mutation result sets the auth-error status (distinct from generic write error); compose buffer retained |
| 7 | i18n | unit | catalog assertion | `tests/unit/*`: pt-BR renders the translated message; English key is identity |

**Floor:** mutation-sensitive — swapping the 401 check to a different status, or
dropping the re-auth message in favor of the generic copy, must fail a test. Tests
assert observable output (printed message, exit code, rendered status-line cells), not
internal calls.

## References

- [ADR 0042](/adr/0042-detect-401-and-guide-reauthentication.md). The decision.
- [ADR 0003](/adr/0003-http-transport-and-mocked-server-testing.md). The mocked-server test pattern.
- [ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md). The thin status line.
