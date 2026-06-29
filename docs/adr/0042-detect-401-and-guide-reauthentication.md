---
type: ADR
title: Detect HTTP 401 as a distinct condition and surface actionable re-authentication guidance (CLI message + non-zero exit; TUI status line)
description: ActiveCollab's REST API has no refresh-token endpoint — its issue-token credential (an ApiSubscription row) is long-lived and durable, valid until it is revoked (logout, password reset, admin removal). So there is no token to "refresh"; the only recovery from an invalidated token is to re-authenticate (re-run issue-token, which the idempotent INSERT OR REPLACE setup already supports). The codebase currently collapses every non-200 (including 401) into empty data, so a revoked token shows an empty task/list with no explanation and no recovery path. Decision: detect a 401 from any authenticated request as a distinct, typed condition (never silently collapsed into empty), and translate it at each surface into actionable re-authentication guidance — the CLI prints a clear message and exits non-zero; the TUI shows a status-line message pointing the user to `ac setup add`. No in-app re-auth modal (deferred); re-auth happens by re-running setup.
status: Accepted
supersedes:
superseded_by:
tags: [auth, http, error-handling, cli, tui, resilience, i18n]
timestamp: 2026-06-29T00:00:00Z
---

# 0042. Detect HTTP 401 and guide re-authentication

## Context

ActiveCollab's REST API authenticates with a token obtained from
`POST /api/v1/issue-token` (email + password → token). Confirmed against the
ActiveCollab self-hosted source (v7.1.141): that token is an **`ApiSubscription`**
database row keyed by `(user_id, client_vendor, client_name)`; `issueToken` is
**idempotent** (returns the existing row for the same client triple) and writes **no
`expires_on`** — so the token is **long-lived and does not expire on a timer**. The
**only** token routes are `issue-token`, `issue-token-intent`, and `logout`
(terminate). **There is no `refresh-token` endpoint** — the OAuth2 `RefreshToken`
in the AC vendor tree belongs to third-party integrations (Xero/QuickBooks/Google),
not AC's own API.

Consequences for this CLI:

1. A refresh-token mechanism is **not applicable** — there is nothing to refresh. The
   token stays valid until it is **revoked** server-side (logout, password change,
   admin removing the subscription).
2. When the stored token *is* revoked, every authenticated request returns **HTTP
   401**. But the client/controller layer today **collapses all non-200 into empty
   data**: `fetch_open_tasks`/`fetch_user_map` return empty, `fetch_task` returns
   `(status, None)`, and `controller::load_task_data_from_path` returns
   `(Value::Null, vec![])`. So a 401 is **indistinguishable from a 404/500/network
   error** at the UI — the user sees an empty task or empty list with no explanation
   and no way to recover.
3. Recovery is **re-authentication**, not refresh: re-running `setup add` calls
   `exchange_token` and `InstanceRepository::save` (an `INSERT OR REPLACE`), which
   updates the stored token in place. The recovery path already exists; what is
   missing is **detecting the 401 and guiding the user to it**.

## Decision

### 1. A 401 is a distinct, typed condition — never collapsed into empty

A 401 from any authenticated request is detected and propagated as a distinct
condition, never silently turned into empty data. Two carriers, matching the two
shapes the existing methods already use (no sweeping signature rewrite):

- **Methods that already return the status** — `fetch_task` (`(u16, Option<Value>)`),
  `create_comment`/`update_comment` (`(u16, Option<Value>)`), `delete_comment`
  (`u16`): callers branch on `status == HTTP_UNAUTHORIZED` (a shared `401` constant).
- **Methods that collapse to a default** — `fetch_open_tasks` (and, where it matters,
  `fetch_user_map`/`resolve_user_id`): on 401 they raise a typed **`Unauthorized`**
  error (carried by the `anyhow::Result` they already return) instead of returning the
  empty default. Other non-200 stays the existing empty default (a missing task is not
  an auth failure). Existing `.unwrap_or_default()` callers keep their current behavior
  (the error is swallowed to the default) — so introducing the typed error is
  **non-breaking**; only auth-aware callers downcast and branch.

The invariant: **"a 401 is never silently rendered as empty data."** It is either
visible in a returned status the caller inspects, or raised as a typed `Unauthorized`.

### 2. Each surface translates a 401 into actionable re-auth guidance

One shared, translated message ("session expired / token invalid — re-authenticate
with `ac setup add`"), surfaced per context:

- **CLI** (`get`/`current` detail, `mine` list, `comment` write): print the actionable
  message and **exit non-zero** — instead of the current empty/"not found" output.
- **TUI** (detail load/refresh and comment mutation): set an auth-error status rendered
  in the existing **thin status line** ([ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md))
  pointing the user to `ac setup add`. **No in-app re-auth modal** — re-authentication
  happens by quitting and re-running setup (chosen as the minimal first step).

### 3. Re-auth reuses the existing setup path

No new re-auth command. `ac setup add` already re-issues the token (idempotent
`issue-token`) and `InstanceRepository::save` (`INSERT OR REPLACE`) updates it in
place. The guidance points there.

## Consequences

- A revoked token now produces a clear, actionable signal everywhere instead of a
  silent empty screen; the user always knows what happened and how to recover.
- The change is **incremental and non-breaking**: status-returning methods are
  unchanged; the typed `Unauthorized` error is swallowed by existing
  `.unwrap_or_default()` callers, so it can land before any surface consumes it.
- No refresh-token logic is added (the API has none); the project records *why* — so
  a future reader does not re-ask "should we implement refresh tokens?".
- The read data-flow gains an explicit 401 → re-auth branch (see `architecture.md`).
- Sliced for delivery (issues 0042 CLI detail+comment, 0043 CLI mine, 0044 TUI), each
  independently demoable.

## Alternatives considered

- **Implement OAuth2-style refresh tokens.** Rejected: ActiveCollab's API has no
  refresh-token endpoint and its token does not expire — there is nothing to refresh.
- **A central `ClientError` enum replacing every `(status, …)` tuple.** Rejected as
  over-large for the need: the status-returning methods already expose 401, so only the
  collapsing methods need the typed signal. A full rewrite would touch every call site
  for no added behavior.
- **In-app TUI re-auth modal (prompt password, re-issue, retry inline).** Deferred:
  more surface (masked input + retry) than the minimal "guide to setup" the user chose;
  can be a later slice without reworking this decision.
- **Treat all non-200 as auth errors.** Rejected: a 404 (missing task) or 5xx is not an
  auth failure; only 401 means the token is invalid. Conflating them would mislead.

## References

- [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md). Authenticated seam (host-gated token).
- [ADR 0038](/adr/0038-detail-footer-contextual-hint-and-status-line.md). The thin status line the TUI 401 message reuses.
- [ADR 0005](/adr/0005-i18n-catalog-as-embedded-json.md). i18n catalog for the re-auth message.
- [BDR 0029](/bdr/0029-token-invalidation-reauthentication.md). Observable behavior.
- ActiveCollab self-hosted source v7.1.141: `angie/src/Angie/Authentication/Repositories/TokensRepository.php` (`issueToken`, idempotent, no `expires_on`); `Foundation/Compile/CompiledUrlMatcher.php` (token routes: `issue-token`, `issue-token-intent`, `logout` — no refresh).
