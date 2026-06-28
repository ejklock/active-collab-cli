---
type: ADR
title: Unit tests live under rust/tests/unit/ and are included into their module via #[path]
description: Test code is kept out of the production source files and collected under a tests/ tree, without widening the crate's public API.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, testing, rust, structure]
timestamp: 2026-06-25T00:00:00Z
---

# 0004. Unit tests live under rust/tests/unit/ and are included into their module via #[path]

## Context

The Rust rewrite ([ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md)) grew
its unit tests inline as `#[cfg(test)] mod tests { ... }` at the bottom of each
`src/*.rs`. The maintainer wants the test code **out of the production source
files** and **collected under a `tests/` folder**, the way a Laravel/Rails app
keeps `tests/`/`spec/` separate from the code.

Rust offers two native test locations, and neither alone fits:

1. **Inline `#[cfg(test)] mod tests`** — can reach private items, but the test
   code sits *inside* the source file (the thing we want to avoid).
2. **Top-level `tests/*.rs` (integration tests)** — physically separate, but
   each file compiles as its own crate and can see **only the `pub` API**. Much
   of this codebase tests private helpers (`find_line_comment`,
   `classify_comment`, render/parse helpers). Honoring "everything in `tests/`"
   this way would force a `lib.rs` split and turn dozens of internal items
   `pub` **solely for testing** — polluting the public surface for no runtime
   benefit.

## Decision

Keep unit tests as a `#[cfg(test)]` submodule of their module (preserving
private-item access), but move the **test code into a file under a `tests/`
tree** and pull it in with `#[path]`:

```rust
// src/i18n.rs (production code only)
#[cfg(test)]
#[path = "../tests/unit/i18n.rs"]
mod tests;
```

```rust
// rust/tests/unit/i18n.rs
use super::*; // resolves to the host module (i18n), so private items are visible
// ... the tests ...
```

- Test files live in **`rust/tests/unit/<module>.rs`**. `unit/` is a
  **subdirectory**, so Cargo does *not* auto-compile these as standalone
  integration-test crates (it only auto-discovers `tests/*.rs` at the top
  level) — avoiding double compilation.
- The only thing left in each `src/*.rs` is the three-line `#[cfg(test)] #[path]
  mod tests;` declaration. **No production test code remains in the source
  files.**
- **No `lib.rs` split and no new `pub`** items: visibility is unchanged, so the
  public API stays as small as the binary needs.
- Genuinely black-box, public-only tests (e.g. the existing
  `rust/tests/comment_policy.rs`) stay as **top-level integration tests** in
  `rust/tests/`. The comment-policy gate already lives there; this decision is
  consistent with it.

## Consequences

- **Positive:** production files contain only production code; all test code is
  under `rust/tests/`; private-helper tests keep working; the API surface does
  not grow; the change is mechanical (cut the `mod tests` body, paste into
  `tests/unit/<module>.rs`, leave the `#[path]` stub) and low-risk.
- **Negative / trade-offs:** the `#[path]` include is a less common idiom than
  plain inline tests, so a newcomer must know the convention (captured here).
  `tests/unit/` is intentionally a subdirectory to dodge Cargo's integration
  auto-discovery — moving a file to the `tests/` top level would change its
  compilation model.
- **Migration:** applied incrementally, slice by slice (≈2 modules per slice),
  starting with `i18n` alongside the JSON-catalog work ([ADR 0005](/adr/0005-i18n-catalog-as-embedded-json.md)).
  Each slice goes through the normal Coder → gates → Reviewer pipeline; the test
  count must not drop.

## Related

- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md),
  [/adr/0005-i18n-catalog-as-embedded-json.md](/adr/0005-i18n-catalog-as-embedded-json.md)
- The comment-policy gate (`rust/tests/comment_policy.rs`) is the precedent for a
  test living under `tests/`.
