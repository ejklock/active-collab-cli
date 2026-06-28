---
type: Issue
title: "R0 — Rust scaffold + Docker (dev/build) + ratatui mouse spike"
description: De-risking spike — prove ratatui+crossterm mouse/scroll with a Cargo+Docker scaffold.
status: closed
labels: [rust, spike, tui, docker]
blocked_by: []
tracker:
timestamp: 2026-06-25T00:00:00Z
---

## R0 — Rust scaffold + Docker (dev/build) + ratatui mouse spike

Stand up the new Rust binary crate and prove the load-bearing thesis of the
rewrite: a `ratatui`+`crossterm` TUI with working mouse and scroll where
over-scrolling clamps and a click selects — never quitting the app.
Implements [ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md); part of
[PRD 0001](/prd/0001-rust-tui-cli-parity.md); specifies
[BDR 0001](/bdr/0001-task-list-navigation.md). Slice R0 of plan `rust-rewrite`.

### Scope

Included: `rust/Cargo.toml`, a pure TEA core (`rust/src/app.rs`: Model/Msg/update
+ view), the imperative shell (`rust/src/main.rs`: tokio + crossterm terminal
setup/teardown + event loop), a multi-stage `Dockerfile`, a root
`docker-compose.yml` (a `dev` service with `cargo-watch` hot-reload and a release
`build` service), `rust/.dockerignore`, and `rust/.gitignore`. Mock data only —
no network, no SQLite. Kept: the Python app remains fully working.

### Acceptance

- `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo build` are clean
  (run in the dev container; no local toolchain).
- `update()` clamps selection at both edges and never panics on an empty list;
  over-scroll never sets `should_quit`.
- `Msg::Click(row)` selects the clicked row (clamped); `Msg::Quit` is the only
  message that exits.
- `docker compose build` succeeds and the release stage emits a single `ac`
  binary; `docker compose run --rm dev cargo test` passes.
- `docker compose up dev` provides working `cargo watch` hot-reload.
- hadolint on `rust/Dockerfile` is clean.

### Plan

1. Cargo bin crate `ac` with ratatui, crossterm, tokio, anyhow.
2. Pure `update`/`view` in `app.rs` with unit tests covering BDR 0001.
3. Terminal shell + event→Msg mapping in `main.rs`.
4. Multi-stage Dockerfile + compose (dev hot-reload + release build).
5. `.dockerignore` + `.gitignore` for `target/`.
