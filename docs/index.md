---
okf_version: "0.1"
---

# active-collab-cli — Docs

Living documentation bundle. Every structural decision and behavior has one home
here and is reachable from this index.

## Root of trace

- [Constitution](/constitution.md) — product scope, data model, non-negotiables.

## Context

- [Context index](/context/index.md) — domain & module vocabulary.
- [Glossary](/context/glossary.md) — terms and acronyms, defined once.
- [Architecture](/architecture.md) — Rust module structure and data-flow diagrams.

## Product Requirements (PRD)

See [prd/](/prd/index.md).

- [0001](/prd/0001-rust-tui-cli-parity.md) — ActiveCollab task CLI + TUI in Rust (parity rewrite) *(Accepted)*

## Architecture Decision Records (ADR)

See [adr/](/adr/index.md).

- [0001](/adr/0001-replace-curses-tui-with-textual.md) — Replace the curses TUI with Textual *(Superseded by 0002)*
- [0002](/adr/0002-rewrite-in-rust-with-ratatui.md) — Rewrite the application in Rust (ratatui + crossterm), built and shipped via Docker *(Accepted)*

## Behavior Decision Records (BDR)

See [bdr/](/bdr/index.md).

- [0001](/bdr/0001-task-list-navigation.md) — Task list navigation: mouse, scroll, and bounded selection *(Accepted)*

## Issues

See [issues/](/issues/index.md) — slices R0–R8 of the Rust rewrite.
