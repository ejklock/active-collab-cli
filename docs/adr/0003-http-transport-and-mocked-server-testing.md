---
type: ADR
title: HTTP transport (reqwest + rustls, no auto-redirect, host-gated token) tested against a mocked server
description: How the Rust rewrite makes network calls and how the HTTP layer — including the token host-isolation NFR — is verified.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, http, security, testing, rust]
timestamp: 2026-06-25T00:00:00Z
---

# 0003. HTTP transport (reqwest + rustls, no auto-redirect, host-gated token) tested against a mocked server

<!-- Status lives in frontmatter. Realizes the network layer of ADR 0002 for
     slice R2; pairs with BDR 0002 (token host-isolation behavior). -->

## Context

Slice R2 ([Issue 0003](/issues/0003-r2-http-api-client.md)) re-implements the
network layer and the ActiveCollab REST client. The Python original
(`http.py` + `client.py`) is a thin `urllib` wrapper injected into the client so
tests can substitute a stub — no real socket. [ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md)
already fixed the stack (`reqwest` + `rustls-tls`, `serde`); two decisions it
left open must be settled before code:

1. **How to enforce the token host-isolation NFR.** The instance API token
   (`X-Angie-AuthApiToken`) must reach *only* its own instance host. `reqwest`'s
   built-in redirect handling strips *standard* sensitive headers
   (`Authorization`, `Cookie`) on a cross-host redirect, but it does **not** know
   our **custom** token header is sensitive — so a `30x` redirect to another host
   would carry the token by default. This is a present security force, not a
   hypothetical.

2. **How to verify the HTTP layer.** A Rust trait-injected fake (the literal
   Python design) makes the security negative test assert against the fake, not
   against the real `reqwest` redirect/host behavior — so it proves nothing about
   the actual transport. The Issue's acceptance explicitly asks for
   *mocked-server* tests and a *negative* token-leak test.

## Decision

**Transport (`rust/src/http.rs`):** a thin `Http` over a single configured
`reqwest::Client`:

- **TLS:** `rustls-tls` (no system OpenSSL), per ADR 0002.
- **Timeout:** 30s — parity with the Python `HttpClient` default.
- **Redirects:** `reqwest::redirect::Policy::none()`. The client never
  auto-follows a redirect, so the custom token header can never ride a cross-host
  hop. A `30x` is returned to the caller as a status (the ActiveCollab API
  answers `200` with JSON directly; no legitimate redirect exists). This is an
  intentional, safe divergence from Python `urllib`, which followed redirects.
- **Status semantics (parity):** an HTTP error *status* is data — returned as
  `Ok((status, body))`, never an error. Only a *transport* failure
  (connect/DNS/TLS/timeout) is an `Err`. This mirrors `http.py` exactly
  (`HTTPError` → returned; `URLError` → raised).

**Token host-gating (defense in depth):** the token header is attached to a
request **only when the request URL's host equals the instance host**. Combined
with `Policy::none()`, the token is doubly contained: it is never attached to a
foreign host, and a redirect to one is never followed.

**ActiveCollab client (`rust/src/client.rs`):** holds the instance + `Http` and
mirrors `client.py` method-for-method (`exchange_token`, `resolve_user_id`,
`fetch_user_map`, `fetch_task`, `fetch_open_tasks`, `list_projects` /
`test_connectivity`). `exchange_token` posts **without** a token (pre-auth), as
in Python. The `Instance` type is **reused from the R1 `store` module** — not
redefined — keeping one source of truth.

**Models (`rust/src/models.rs`):** `serde`-derived `Task`, `Comment`, `Project`,
`MineTask` with defaults such that a missing **or null** field deserializes to a
safe default (mirrors Python `data.get("x") or ""`); no parse panics on a
partial payload.

**Testing — mocked server (`wiremock`):** behavior is verified against a real
`wiremock` mock over the actual `reqwest` + `rustls` stack:

- Token exchange, task fetch, open-task fetch, user-map, connectivity assert on
  the requests the server actually receives and on parsed responses.
- The **negative security test** asserts a request whose host is not the instance
  host carries no `X-Angie-AuthApiToken`, and that a `30x` to a second mock yields
  **no** second request (the redirect is not followed).

### Alternatives considered

- **Trait-based transport injection (the literal Python port).** No new
  dependency, fastest tests — but the host-isolation negative test exercises the
  fake, not real `reqwest` redirect/host behavior, so it cannot actually prove the
  NFR. **Rejected** for the security-critical path.
- **Hybrid (trait seam + `wiremock` only for the security test).** Viable, but two
  test styles for one small layer with no payoff over a single mocked-server style.
  **Rejected** as needless surface.

## Consequences

- **Gained:** the security NFR is verified against the real transport; tests are
  deterministic and offline (mock binds to loopback); parity with the Python
  status/transport semantics is explicit.
- **Cost:** `wiremock` (+ a small async test surface) is added as a **dev**
  dependency only. The first `cargo test` fetches it from `crates.io`, which is
  outside the command sandbox — that run uses the relaxed sandbox, as for every
  R-slice (ADR 0002).
- **No auto-redirect** is now a transport invariant; if a future endpoint ever
  legitimately needs a redirect, it is a conscious, reviewed change to this policy.

**Fitness functions (these keep the decision true):**
- A `30x` response is **never** auto-followed (`wiremock`: second host unhit).
- A request to a non-instance host carries **no** token header (negative test).
- An HTTP error status returns `Ok((status, _))`; only transport failure is `Err`.
- The client uses `store::Instance` — there is no second `Instance` definition.

## Related

- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md)
- BDR: [/bdr/0002-token-host-isolation.md](/bdr/0002-token-host-isolation.md)
- PRD: [/prd/0001-rust-tui-cli-parity.md](/prd/0001-rust-tui-cli-parity.md)
- Issue: [/issues/0003-r2-http-api-client.md](/issues/0003-r2-http-api-client.md)
