---
type: ADR
title: The i18n message catalog is a per-locale JSON file embedded at compile time
description: Translations live in rust/locales/<locale>.json (Laravel-style flat map), embedded into the single binary via include_str! and parsed once.
status: Accepted
supersedes:
superseded_by:
tags: [architecture, i18n, rust, structure]
timestamp: 2026-06-25T00:00:00Z
---

# 0005. The i18n message catalog is a per-locale JSON file embedded at compile time

## Context

Slice R7 ([Issue 0008](/issues/0008-r7-i18n-assets.md)) completes localization.
The Python oracle (`src/active_collab/i18n.py`) holds an in-code `_CATALOG`
dict (`{locale: {english_key: translation}}`) and a magic `__()` that does
`_CATALOG.get(lang, {}).get(s, s)`. The first Rust attempt mirrored this as an
in-code `HashMap` literal — but an 80-entry table embedded in `i18n.rs` is hard
to read and review, and (as the R7a review found) easy to under-populate without
a faithful completeness instrument.

The maintainer asked for the catalog to be a **separate `.json` file, the way
Laravel keeps `lang/<locale>.json`**.

Constraint from [ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md): the
product ships as a **single binary** (built in Docker). Reading translation
files from disk at runtime would break that portability (the files would have to
travel with the binary and resolve a path).

## Decision

Store each non-default locale as a flat JSON map and **embed it at compile time**:

- File: **`rust/locales/pt_BR.json`** — a flat object `{"English source string":
  "tradução em pt-BR", ...}`, exactly Laravel's `lang/pt_BR.json` shape, where
  the **English source string is the key** (so `t("...")` call sites pass the
  English literal).
- `en` is the identity locale and needs **no file** (a missing key returns the
  input, same as Python `.get(s, s)`).
- The catalog is embedded with `include_str!("../locales/pt_BR.json")` and parsed
  **once** into a `OnceLock<HashMap<String, String>>` via `serde_json`. No
  runtime file I/O; the single-binary guarantee holds.
- `t(s)`: under `pt_BR`, return `catalog.get(s).cloned().unwrap_or_else(|| s
  .to_owned())`; under `en`, identity. Semantics identical to Python
  `_CATALOG.get(lang, {}).get(s, s)`.

**Completeness is verified against the Python oracle, not against itself.** The
catalog-completeness test transcribes the full expected key set **from
`src/active_collab/i18n.py` `_CATALOG`** and asserts every key is present in the
JSON — so dropping a key fails the test (closing the self-referential gap the
first R7a attempt had).

## Consequences

- **Positive:** translations are data in a reviewable `locales/*.json` file, not
  a code literal; adding a locale is a new file plus one `include_str!`; the
  diff for translation changes is clean; the single-binary constraint is kept;
  parsing once behind `OnceLock` keeps `t()` cheap.
- **Negative / trade-offs:** `include_str!` binds the catalog at **compile
  time**, so changing a translation requires a rebuild (acceptable — we ship a
  binary, not a deployable lang directory). A malformed JSON is a parse error at
  first use; the parse is covered by a test so it cannot ship silently.
- New dependency surface: none — `serde_json` is already a dependency.

## Related

- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md),
  [/adr/0004-tests-in-tests-dir-via-path-include.md](/adr/0004-tests-in-tests-dir-via-path-include.md)
- Issue: [/issues/0008-r7-i18n-assets.md](/issues/0008-r7-i18n-assets.md)
- Oracle: `src/active_collab/i18n.py` (`_CATALOG`, `__()`)
