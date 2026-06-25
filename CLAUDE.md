# active-collab-cli — Project Guide

ActiveCollab task CLI + TUI. The Rust (ratatui + crossterm) single binary `ac`
is the shipped application, built and distributed via Docker. See the docs trail
below.

## Docs index

Living documentation lives in [`docs/`](docs/index.md). Start at:

- [Constitution](docs/constitution.md) — scope, data model, non-negotiables.
- [PRD 0001](docs/prd/0001-rust-tui-cli-parity.md) — the rewrite capability.
- [ADR 0002](docs/adr/0002-rewrite-in-rust-with-ratatui.md) — why Rust.
- [ADR 0006](docs/adr/0006-promote-crate-to-repo-root.md) — crate promoted to repo root.
- [Architecture](docs/architecture.md) — module + data-flow diagrams.
- [Issues](docs/issues/index.md) — slices R0–R8.

## Build & run commands — HARD RULES

There is **no local Rust toolchain**. The Cargo crate is at the repo root; the
compose file (`docker-compose.yml`) is also at the repo root, and its `dev`
service mounts `./` and sets `working_dir` to `/app`. Therefore:

1. **Run every build/test/lint command from the repo root, bare:**
   - `docker compose run --rm dev cargo build`
   - `docker compose run --rm dev cargo test`
   - `docker compose run --rm dev cargo test --test comment_policy` (comment-policy gate: no banners, no commented-out code; doc comments and non-obvious why-comments are allowed)
   - `docker compose run --rm dev cargo clippy -- -D warnings`
   - `docker compose run --rm dev cargo fmt --check`
   - `docker compose build` / `docker compose run --rm build` (release)
2. **NEVER prefix a command with `cd`.** The shell's working directory is already
   the repo root and persists between commands. `cd` (especially combined with a
   pipe or redirect) trips Claude Code's path-resolution guard and forces a manual
   approval prompt on every call — breaking unattended runs.
3. **NEVER use absolute paths** (no `docker compose -f /Volumes/.../docker-compose.yml`).
   Use the bare command above; compose auto-discovers the root `docker-compose.yml`.
   If you must point at a file, use the cwd-relative `./docker-compose.yml`.
4. **Do not append `2>/dev/null`, `| head`, `&& echo …`, or extra chaining** to a
   command unless required — these can also trip the bypass guard.
5. **Cargo fetches from crates.io / static.crates.io**, which are outside the
   default command sandbox allowlist. The FIRST build/test (and `hadolint` image
   pulls) need the network, so run those specific commands with the sandbox
   disabled (`dangerouslyDisableSandbox: true`) — a legitimate registry fetch.

## Maintenance rule

Any structural change updates its doc **and its Mermaid diagram** in the same
change ([architecture](docs/architecture.md), the relevant ADR/BDR, and the
directory `index.md`). No orphan docs, no stale diagrams.

## Conventions

- Docs language: English. User-facing chat: Brazilian Portuguese.
- No AI attribution in commits or docs.
- The TUI core (`src/app.rs` `update`) is pure (no terminal/async) and unit-tested.
