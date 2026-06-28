---
type: ADR
title: Authenticated write seam — host-gated POST/PUT/DELETE on Http, comment-mutation methods on the client
description: The app has only authed_get (reads) and an unauthenticated post_json (the pre-auth issue-token call). Comment create/edit/delete need authenticated writes. Add Http::authed_post / authed_put / authed_delete that reuse the existing host_gated_token_header (so the token never crosses to a foreign host) and return (status, bytes) like authed_get. Expose create_comment / update_comment / delete_comment on ActiveCollabClient mapping to POST /api/v1/comments/{parent_type}/{parent_id}, PUT /api/v1/comments/{comment_id}, DELETE /api/v1/comments/{comment_id}. The client keeps URL/parse logic; Http stays a thin transport.
status: Accepted
supersedes:
superseded_by:
tags: [http, client, write, comments, security, api]
timestamp: 2026-06-28T00:00:00Z
---

# 0033. Authenticated write seam — host-gated POST/PUT/DELETE + comment client methods

## Context

The HTTP layer ([`src/http.rs`](/architecture.md)) today exposes exactly two verbs:

- `authed_get(url, instance_base_url, token)` — attaches the `X-Angie-AuthApiToken`
  header **only when the request host matches the instance host** (via
  `host_gated_token_header`), and returns `(status, bytes)` for any HTTP response
  (4xx/5xx included; only transport failures are `Err`).
- `post_json(url, body)` — an **unauthenticated** POST used solely by
  `exchange_token` (the pre-auth `issue-token` call). It attaches **no** token.

[PRD 0002](/prd/0002-task-comment-authoring.md) introduces comment create/edit/delete,
which are **authenticated writes**. The verified server contract (ActiveCollab
v7.1.141 routes + `CommentsController`) is:

- **Create:** `POST /api/v1/comments/{parent_type}/{parent_id}` with body
  `{ "body": "<text>" }`; `parent_type` is the slug `task`, `parent_id` is the task
  id. Returns the created `Comment`.
- **Edit:** `PUT /api/v1/comments/{comment_id}` with body `{ "body": "<text>" }`.
- **Delete:** `DELETE /api/v1/comments/{comment_id}`.

All three are served by `AuthRequiredController` — they require the
`X-Angie-AuthApiToken` header, and enforce `canComment` / `canEdit` / `canDelete`
server-side.

There is no authenticated write verb on `Http`, and `post_json` is the wrong tool
(it never attaches the token, by design). Using `post_json` for a comment would
either fail auth or — worse, if "fixed" by attaching the token unconditionally —
**leak the token to whatever host the URL points at**, breaking the token-isolation
guarantee (PRD 0001 req. 7).

## Decision

Add three authenticated write verbs to `Http`, each a near-mirror of `authed_get`:

```
authed_post(url, instance_base_url, token, body: &serde_json::Value) -> Result<(u16, Bytes)>
authed_put (url, instance_base_url, token, body: &serde_json::Value) -> Result<(u16, Bytes)>
authed_delete(url, instance_base_url, token) -> Result<(u16, Bytes)>
```

Each:

1. Builds the request for its verb with `ACCEPT: application/json` (and
   `CONTENT_TYPE: application/json` for post/put).
2. Attaches the token **only** via the existing `host_gated_token_header(url,
   instance_base_url, token)` — the *same* gate `authed_get` uses, so the
   host-isolation property is inherited, not re-implemented.
3. Returns `(status, bytes)` for any HTTP response; only transport failures are `Err`.

Expose the comment mutations on `ActiveCollabClient`, which owns URL construction and
response parsing (keeping `Http` a thin transport):

```
create_comment(task_id, body) -> Result<(u16, Option<Value>)>   // POST comments/task/{task_id}
update_comment(comment_id, body) -> Result<(u16, Option<Value>)> // PUT comments/{comment_id}
delete_comment(comment_id) -> Result<u16>                        // DELETE comments/{comment_id}
```

`create_comment` builds `{base}/api/v1/comments/task/{task_id}` with
`{ "body": body }`; `update_comment` builds `{base}/api/v1/comments/{comment_id}`
with `{ "body": body }`; `delete_comment` builds the same `comment_id` URL. Each
passes `self.instance.base_url` + `self.instance.token` to the host-gated verb, so a
comment whose URL somehow pointed off-host would carry no token. Non-200/2xx returns
the status with `None`/no parse, matching the `fetch_task` shape callers already
handle.

### Guard / fitness function

- **Token isolation (security):** a unit test asserts `authed_post`/`authed_put`/
  `authed_delete` attach the token when the request host equals the instance host and
  attach **no** token header when it differs — the same negative-test bar as
  `authed_get` (PRD 0002 NFR). This is the load-bearing security gate for the new
  write path.
- **Contract shape:** client tests (wiremock) assert `create_comment` POSTs to
  `comments/task/{id}` with `{"body": ...}` and returns `(200, Some(comment))`;
  `update_comment` PUTs to `comments/{id}`; `delete_comment` DELETEs and returns the
  status. A non-2xx response yields the status without a parsed body.
- **No regression:** `authed_get` / `post_json` / `exchange_token` tests stay green
  unchanged.

## Alternatives considered

- **Reuse `post_json` and attach the token unconditionally.** Rejected: it removes
  the host gate, leaking the token to any host the URL resolves to — the exact attack
  PRD 0001 req. 7 forbids. The gate is the whole point of the `instance_base_url`
  parameter.
- **One generic `authed_request(method, …)` instead of three verbs.** Rejected for
  now: three tiny explicit methods read better at the call site, mirror the existing
  `authed_get` shape, and keep the body-vs-no-body typing honest (delete has no body).
  A generic helper can be extracted later if a fourth verb appears.
- **Put URL building in `Http`.** Rejected: `Http` is deliberately a thin transport
  (no `/api/v1` knowledge); `ActiveCollabClient` already owns every endpoint path and
  parse. Keeping comments there preserves one home for the API surface.

## Consequences

**Positive:** the app can perform authenticated writes with the same host-isolation
guarantee as reads, reusing the audited `host_gated_token_header` gate. The comment
endpoints live next to the existing `fetch_task` in the client, so the API surface
stays in one module. The `(status, bytes)`/`(status, Option<Value>)` return shape is
already familiar to callers.

**Accepted trade-offs:** three new transport methods (small duplication of the
builder/await/return shape with `authed_get`); a future refactor may unify them
behind one helper. `delete_comment` returns only the status (no body to parse), so
its signature differs slightly from create/update — intentional, the server returns
no useful object on delete.

## Related

- PRD: [/prd/0002-task-comment-authoring.md](/prd/0002-task-comment-authoring.md)
- ADR: [/adr/0034-comment-compose-mode-multiline.md](/adr/0034-comment-compose-mode-multiline.md) (the UI that drives these writes)
- ADR: [/adr/0035-server-truth-refresh-after-comment-mutation.md](/adr/0035-server-truth-refresh-after-comment-mutation.md) (what happens after a write returns 2xx)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md)
- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md) (the client/http boundary this extends)
