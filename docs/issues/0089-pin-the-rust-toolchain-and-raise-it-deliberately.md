---
type: Issue
title: Pin the Rust toolchain and raise it deliberately
description: Docker images and CI use one exact Rust version, raised in its own change, so a new stable release cannot break the lint gate.
status: closed
labels: [build, ci, tech-debt]
timestamp: 2026-11-13T01:02:09Z
---

## 0089. Pin the Rust toolchain and raise it deliberately

### Problem

The Rust toolchain floats in three places: `Dockerfile:1` builds on `rust:1-slim`, and `.github/workflows/ci.yml:14` and `.github/workflows/release.yml:35` install `dtolnay/rust-toolchain@stable`. Each new stable release can add lints that `cargo clippy --all-targets -- -D warnings` turns into errors, so code that passed yesterday fails today. On 2026-11-12 a fresh image with Rust 1.100.0 failed clippy with `needless_bool` at `tests/comment_policy.rs:101`; code that has not changed since `ed48ef4` passes with the project's current image (Rust 1.99.1).

### Scope

Included: one pinned Rust version used by the Docker dev and build images and by the CI and release workflows; a documented way to raise it.

Excluded: fixing new lints beyond what the pinned version reports; the runtime base image (`debian:trixie-slim`).

### Decision

- The toolchain is pinned to one exact version and raised deliberately, in its own change that also fixes what the new version's lints report (owner, 2026-11-12). Not taken: only fixing the lint that surfaced, which leaves the next release to break the build again; only recording the risk.
- The one place is a `rust-toolchain.toml` at the repo root, pinned to 1.99.1 (the version the project image runs today, which passes every gate) with `clippy` and `rustfmt`; the Docker builder stage installs that toolchain from the file, and both workflows read it instead of `@stable` (session, 2026-11-13). Not taken: an exact `rust:<version>-slim` tag plus a workflow input, which keeps the version in three places.
- The dev image gets coverage tooling: a separate `dev` Dockerfile stage built from `builder` adds `llvm-tools-preview` to the pinned toolchain and installs `cargo-llvm-cov --locked`, so the release image does not carry it; the compose `dev` service targets that stage, the coverage command is documented in CLAUDE.md, and `se-gates qg coverage-receipt` stamps the generated lcov (owner, 2026-11-13). Not taken: installing the tool on the host; accepting exit 3 on the crap check.
- The `needless_bool` finding itself is fixed in [issue 0086](/issues/0086-the-comment-gate-applies-every-rule-to-tests-and-rejects-every-artifact-citation.md) S1, which already edits that file.

### Acceptance

- AC1: the Docker images and both workflows build and lint with the same exact Rust version, read from one place. (`verify_by: inspection`)
- AC2: `docker compose build` and `docker compose run --rm dev cargo clippy --all-targets -- -D warnings` pass on a fresh image. (`verify_by: command`)
- AC3: the README or a doc says how to raise the version. (`verify_by: inspection`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose build`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
