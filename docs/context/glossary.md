---
type: Context
title: Glossary
description: Terms and acronyms used across the active-collab-cli docs, defined once.
tags: [glossary]
timestamp: 2026-06-25T00:00:00Z
---

# Glossary

One home per term. Acronym headwords are kept as-is; only the explanation is in
the doc language (English).

## Acronyms

- **ADR** — Architecture Decision Record. A record of one architectural/
  implementation decision, its context, and consequences. See [adr/](/adr/index.md).
- **API** — Application Programming Interface. Here, the ActiveCollab HTTP API the
  client talks to.
- **ATAM** — Architecture Tradeoff Analysis Method (SEI). Source of the six-part
  quality-attribute scenario form used for NFRs in the PRD.
- **BDR** — Behavior Decision Record. A record of observable behavior as
  Given/When/Then scenarios plus a Test Design matrix. See [bdr/](/bdr/index.md).
- **CLI** — Command-Line Interface. The non-interactive command surface
  (`setup`, `get`, `current`, `mine`, `browse`).
- **MSRV** — Minimum Supported Rust Version. The oldest rustc a crate compiles on;
  relevant when pinning the builder image.
- **NFR** — Non-Functional Requirement. A quality attribute (deployability,
  responsiveness, security…) written as a quality-attribute scenario bound to an
  instrument.
- **OKF** — Open Knowledge Format. The markdown + YAML-frontmatter format these
  docs conform to.
- **PRD** — Product Requirements Document. What a capability must do and why. See
  [prd/](/prd/index.md).
- **SGR** — Select Graphic Rendition. The terminal escape-sequence family whose
  mouse-mode (`?1006h`) the old curses code hand-parsed (and mis-parsed).
- **TEA** — The Elm Architecture. The Model / Msg / `update` / `view` pattern the
  Rust TUI uses; the `update` core is pure and unit-tested.
- **TUI** — Text/Terminal User Interface. The interactive full-screen browser
  (`browse`).

## Terms

- **Instance** — one configured ActiveCollab deployment (name, base URL, email,
  token, user id). Tasks are scoped per instance.
- **Slice** — one issue-sized unit of the rewrite (R0–R8) that is independently
  reviewable; tracked in [issues/](/issues/index.md) and plan `rust-rewrite`.
- **Parity** — feature/output equivalence: a Rust command produces the same
  observable result as the Python command it replaces.
- **Single-flight refresh** — at most one in-flight fetch per group; a refresh
  requested while one is running does not enqueue a second.
- **Token host isolation** — an instance's API token is attached only to requests
  to that instance's own host (a non-negotiable).
