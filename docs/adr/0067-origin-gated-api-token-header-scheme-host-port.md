---
type: ADR
title: Origin-gated API token header (scheme + host + port)
description: The x-angie-authapitoken header is attached only when the request URL's full origin — scheme, host, and port — equals the instance base URL's origin, instead of the host alone, closing the cleartext scheme-downgrade token disclosure confirmed as issue 0062 C2.
status: Proposed # Proposed | Accepted | Superseded | Deprecated
supersedes:                 # NNNN of the ADR this replaces, if any
superseded_by:              # NNNN, set when a later ADR replaces this one
tags: [security, http, token, credentials, origin, assets]
timestamp: 2026-07-30T14:35:37Z
---

# 0067. Origin-gated API token header (scheme + host + port)

## Context

`Http::host_gated_token_header` (`src/http.rs:30-44`) decides whether the
`x-angie-authapitoken` header is attached to an outgoing request. Its rule was:

```rust
let req_host = extract_host(url)?;          // parsed.host_str().to_lowercase()
let inst_host = extract_host(instance_base_url)?;
if req_host.eq_ignore_ascii_case(&inst_host) { /* attach token */ }
```

`extract_host` (`src/http.rs:159-162`) returns the **host only** — the scheme and the port
are discarded before the comparison.

That gate exists because request URLs are not all constructed by us. Asset URLs come
verbatim from untrusted payload:

- `<img src>` and `<a href>` scraped out of task and comment HTML
  (`src/controller.rs:300-308`, `image_assets_from_html`), and
- the attachment `url` / `download_url` fields (`src/controller.rs:311-333`).

Those URLs reach a real authenticated sink: `download_task_attachments` →
`fetch_and_write_asset` (`src/controller.rs:561`) → `ActiveCollabClient::fetch_asset_bytes`
(`src/client.rs:314`) → `Http::authed_get` (`src/http.rs:56`), triggered by
`ac get`/`ac current --download-attachments`
([ADR 0066](/adr/0066-agent-attachment-download-to-local-temp-dir.md)) and by the TUI
asset/image download.

**Abuser story (confirmed — [issue 0062](/issues/0062-security-audit-findings-pending-validation.md), C2).**
A collaborator who can comment on a task the victim opens posts
`<img src="http://<instance-host>/x.png">`. The host matches, so the token header is
attached and the request leaves the machine over **cleartext HTTP**. An on-path attacker
on the victim's network reads a full ActiveCollab API token out of the request headers.
Redirects are disabled (`redirect::Policy::none()`), so this is not a redirect leak a
policy change would catch — it is the first request itself.

What the old gate did get right, and what this decision must not regress: a
**foreign-host** asset URL gets no token at all. Cross-host exfiltration was never open.
The residual gap is strictly the scheme, and more weakly the port.

Rejected alternative — *hard-require `https`*: that would silently break self-hosted
instances legitimately configured with an `http://` base URL (LAN / on-prem), turning a
security fix into an outage. The property we actually want is **same origin as the
instance**: if the instance itself is cleartext, an asset request to it is no worse than
every other API call the CLI already makes.

## Decision

We will gate the token header on the **full origin** of the request URL. The header is
attached only when all three match the instance base URL:

1. **scheme** — ASCII case-insensitive equality, so `http` no longer matches `https`;
2. **host** — ASCII case-insensitive equality (unchanged behavior);
3. **port** — compared via `Url::port_or_known_default()`, so `https://h` and
   `https://h:443` are the same origin while `https://h:8443` is not.

`host_gated_token_header` is renamed `origin_gated_token_header`, and `extract_host` is
replaced by a private `origin_of(url) -> Option<(String, String, Option<u16>)>`. The
rename is deliberate: the old name is what made the weaker rule look correct at its four
call sites (`authed_get`, `authed_post`, `authed_put`, `authed_delete`).

Everything else about the seam is unchanged: same four call sites, same header name, same
`None` → no header behavior, same "foreign host gets nothing" guarantee.

## Consequences

**Easier / gained:**
- A same-host `http://` asset URL on an `https` instance now yields **no** token header,
  closing the cleartext credential-disclosure path end to end.
- The function name states the actual contract, so a future call site cannot adopt the
  weaker rule by reading the signature.

**Harder / accepted trade-offs:**
- An attacker-embedded cleartext asset still gets fetched (it may be public); it simply
  carries no credential. A 401/403 for such an asset is the correct, visible outcome.
- A non-default port must now match on both sides. A same-host different-port endpoint is
  treated as a different service — intentional, and a behavior change for anyone proxying
  assets on a second port.

**Follow-ups:**
- None required. Instances configured with an `http://` base URL are unaffected: their
  asset URLs share the instance origin and still receive the token.

## Verification

**Implementation impact:** `src/http.rs` (`origin_gated_token_header`, `origin_of`, the
four `authed_*` call sites), `tests/unit/http.rs`.

**Verification criteria:**
- `origin_gated_token_header("http://acme.example.com/x.png", "https://acme.example.com", tok)`
  returns `None` — the scheme-downgrade case from issue 0062 C2.
- Same host, different explicit port returns `None`.
- `https://h/...` against instance `https://h:443` returns `Some` (default-port
  normalization).
- A foreign host still returns `None` (no regression of the existing guarantee).
- An `authed_get` wiremock test asserts that no `x-angie-authapitoken` header reaches a
  cleartext request whose instance base URL is `https`.
