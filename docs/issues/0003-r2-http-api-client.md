---
type: Issue
title: "R2 — HTTP + ActiveCollab API client parity"
description: reqwest+rustls client for token exchange, connectivity, and task/user fetches.
status: closed
labels: [rust, http, api, parity, security]
blocked_by: [2]
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R2 — HTTP + ActiveCollab API client parity

Re-implement the network layer and the ActiveCollab client. Part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md) (requirements 3, 7); implements
[ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md). Slice R2 of plan
`rust-rewrite`.

### Scope

Included: `reqwest` with `rustls-tls`; token exchange (login), `test_connectivity`,
`fetch_task`, `fetch_open_tasks`, `fetch_user_map`; `serde` models. Kept: the
**token host-isolation** non-negotiable — the instance token is attached only to
that instance's host.

### Acceptance

- Mocked-server tests for token exchange and task/open-task fetches.
- A request to a non-instance host carries no `Authorization`/token (negative
  test) — the PRD security NFR.
- Models deserialize the live payload shape used by the renderer.

### Plan

Re-planned after R0/R1 (provisional in plan `rust-rewrite`). Mirror `http.py` +
`client.py`; a BDR for token isolation is authored with this slice.
