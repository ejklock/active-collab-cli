---
type: Issue
title: "Pin the Docker dev image's Rust toolchain to match CI stable so local clippy stops emitting false-negatives"
description: The local gate `docker compose run --rm dev cargo clippy --all-targets -- -D warnings` runs an older Rust than CI's dtolnay/rust-toolchain@stable (rust 1.97.0 at v0.4.0), so newer/expanded lints do not fire locally. On PR #25 this produced a green-local / red-CI failure — clippy::useless_borrows_in_formatting denied a redundant & at tests/unit/app.rs:915 that local clippy never flagged (fixed in c0816d2). Because the pinned dev image lags CI, the local clippy gate is an unreliable oracle and every Rust PR risks an extra CI round-trip. Pin/bump the Dockerfile's Rust toolchain to track CI stable (or add a scheduled bump) so the local gate matches the authoritative CI gate.
status: open
labels: [tooling, ci, docker, tech-debt, chore]
blocked_by:
tracker:
timestamp: 2026-07-09T00:00:00Z
---

## Pin the Docker dev Rust toolchain to match CI stable

### Problem

The build/test/lint gate runs inside the pinned `dev` service image, but CI
(`.github/workflows/ci.yml`) uses `dtolnay/rust-toolchain@stable` — rust **1.97.0**
at the time of `v0.4.0`. The dev image's Rust is **older**, so lints that newer
clippy promotes or expands never fire locally. On PR #25 the local gate reported
clippy clean while CI failed:

```
error: redundant reference in `assert!` argument
  --> tests/unit/app.rs:915:9
  = note: `-D clippy::useless_borrows_in_formatting` implied by `-D warnings`
```

The Coder even ran `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
and saw "no warnings" — still red on CI (fixed in commit `c0816d2`). This is the
same theme as two known caveats:

- clippy in the quality-gate image failing on `linker cc not found` (never a real
  code signal here);
- the parallel-only i18n `LANG_MUTEX` race in `tests/unit/commands.rs`, which CI
  dodges because `ci.yml` runs `cargo test -- --test-threads=1`.

Net: **CI is the authoritative clippy/test oracle; a clean local run does not
guarantee CI-green.** That costs an extra push+CI cycle on lint-only failures.

### Decision (to be finalized in this issue)

Align the local toolchain with CI so the local gate stops emitting false-negatives:

- Pin the `Dockerfile`'s Rust base to a specific recent stable that matches (or
  leads) CI, and bump it on a cadence; **or**
- Add a `rust-toolchain.toml` at the repo root pinning `channel = "stable"` (or an
  explicit version) so both the dev image and CI resolve the same toolchain; **or**
- Keep the image but add a scheduled workflow that bumps the pinned Rust and opens
  a PR when a new stable ships.

Recommended: a repo-root `rust-toolchain.toml` pinning an explicit stable version,
bumped deliberately — it makes local and CI resolve identically and keeps the pin
reviewable in one place. The chosen option is recorded here before editing.

### Scope

- `Dockerfile` (and/or a new `rust-toolchain.toml`) so `docker compose run --rm dev
  cargo clippy --all-targets -- -D warnings` runs the same Rust CI runs.
- A one-time sweep: run the aligned clippy across the tree and clear any lints it
  newly surfaces (so the pin lands green).
- Document the bump cadence in the build HARD RULES / CONTRIBUTING note.

### Out of scope

- The quality-gate image's `linker cc not found` issue (separate infra caveat).
- Making the local suite parallel-safe (the `LANG_MUTEX` race is tracked separately;
  CI already uses `--test-threads=1`).

### Acceptance criteria

- **AC1** (behavior, command): after the change, `docker compose run --rm dev cargo
  clippy --all-targets -- -D warnings` uses the same Rust version as CI (verified via
  `cargo --version` / `rustc --version` parity between the dev image and the CI log).
- **AC2** (behavior, command): the lint that escaped on PR #25
  (`useless_borrows_in_formatting`) and any siblings are reproducible locally; the
  tree is clean under the aligned toolchain.
- **AC3** (constraint, inspection): the pinned version is single-homed (one place to
  bump) and the bump cadence is documented.
- **CC** (constraint, inspection): clean diff, docs/build-rules updated, no orphan
  docs.

### Verification

- `rustc --version` inside the dev image equals the CI job's `rustc --version`.
- Full local gate green under the aligned toolchain: `cargo fmt --check`, `cargo
  clippy --all-targets -- -D warnings`, `cargo test -- --test-threads=1`, `cargo test
  --test comment_policy`, `cargo build --release`.

### Traces

- PR #25 / commit `c0816d2` (the escaped clippy lint at `tests/unit/app.rs:915`).
- Memory lesson: "Local Docker `dev` clippy is NOT the clippy oracle — CI's stable
  toolchain lints stricter (version skew)".
- Related caveats: quality-gate clippy `linker cc not found`; the parallel-only
  i18n `LANG_MUTEX` test race (CI uses `--test-threads=1`).
