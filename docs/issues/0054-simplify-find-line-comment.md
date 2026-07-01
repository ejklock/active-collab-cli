---
type: Issue
title: "Bring the comment-policy harness find_line_comment within the complexity budget — extract the duplicated quoted-literal skip (obs 34)"
description: tests/comment_policy.rs find_line_comment (the gate harness that locates a line-comment start while skipping over string and char literals) carries cognitive 45 / cyclomatic 13 — above the ≤12 / ≤10 budget the gate itself enforces on the rest of the tree (obs 34). The complexity is driven by two near-identical inner loops that skip a double-quoted and a single-quoted literal (each handling backslash escapes). Extract one skip_quoted(bytes, from, quote) -> usize helper and reduce the main scan to a small match; behavior byte-for-byte preserved (all seven find_line_comment specs + the classify/scan specs stay green).
status: closed
labels: [tests, tooling, complexity, refactor, locality, slice]
blocked_by:
tracker:
timestamp: 2026-06-30T00:00:00Z
---

## Simplify find_line_comment to within the complexity budget (obs 34)

### Problem

`tests/comment_policy.rs::find_line_comment` (`comment_policy.rs:18-55`) is the harness that
finds where a line-comment (`//`) starts on a source line while **skipping over string and char
literals** (so a `//` inside `"a // b"` or a `'/'` char literal is not mistaken for a comment).
The arborist complexity check (which runs full-tree in the quality-gate image) reports it at
**cognitive 45 / cyclomatic 13** — above the ≤ 12 / ≤ 10 budget the comment-policy gate itself
enforces on `src/`. The harness violates the discipline it exists to protect (obs 34).

The complexity is duplication: the double-quote branch (`:24-35`) and the single-quote branch
(`:36-47`) are the **same inner loop** — advance past a `\\`-escaped pair, stop just past the
matching closing quote, else step one — differing only in the quote byte. Two copies of a
three-branch inner `while` inside a four-branch outer `while` is what drives the cognitive score.

### Decision

Extract the shared quoted-literal skip into one helper and reduce the main scan to a `match`
(no new architectural decision — this is a behavior-preserving decomposition under the standing
complexity-budget constraint, so it traces to that discipline, not a new ADR):

```rust
/// Advance past a quoted literal opened at `from - 1`, returning the index just
/// past its closing `quote` (or `len` if unterminated). Handles `\\` escapes.
fn skip_quoted(bytes: &[u8], from: usize, quote: u8) -> usize { … }

fn find_line_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = skip_quoted(bytes, i + 1, b'"'),
            b'\'' => i = skip_quoted(bytes, i + 1, b'\''),
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => return Some(i),
            _ => i += 1,
        }
    }
    None
}
```

### Scope

- `tests/comment_policy.rs`: add `skip_quoted`; rewrite `find_line_comment` to the `match` form
  above, deleting the two duplicated inner loops. No change to `is_banner`,
  `is_commented_out_code`, `classify_comment`, `scan_source`, or `walk_rs_files`.

### Out of scope

- Any change to what the gate flags (banners / commented-out code) — only the comment-start
  scan is decomposed.
- The gate's `src/`-scanning entry point (`rust_src_has_no_comment_policy_violations`).

### Acceptance criteria

- **AC1** (constraint, inspection): a single `skip_quoted(bytes, from, quote) -> usize` helper
  exists; `find_line_comment` calls it for both quote kinds and no longer contains two
  duplicated inner quote-skip loops.
- **AC2** (behavior, test): the scan is byte-for-byte unchanged — the seven `find_line_comment_*`
  specs (string-literal URL, slashes in a double-quoted string, real trailing comment, string +
  comment on one line, no comment, escaped quote in string, char-literal slash) and the
  `classify_*` / `scan_source_*` specs all stay green.
- **CX** (constraint, command): `find_line_comment` and `skip_quoted` are each within budget —
  cyclomatic ≤ 10 (≤ 8 for the new `skip_quoted`), cognitive ≤ 12 — verified by the quality-gate
  arborist check (the obs-34 debt is cleared).
- **CC** (constraint, inspection): clean code — no banners/commented-out code; only non-obvious
  why-comments; comment-policy gate green.
- **TE** (constraint, command): tests assert observable scan results and survive the mutation
  floor — dropping the `\\`-escape advance, the closing-quote stop, or a quote branch must fail a
  spec (the escaped-quote and slash-in-string specs already pin these).

### Verification

`docker compose run --rm dev cargo test --test comment_policy` (all green),
`docker compose run --rm dev cargo test -- --test-threads=1` (full suite green),
`docker compose run --rm dev cargo clippy --all-targets -- -D warnings`,
`docker compose run --rm dev cargo fmt --check`.

### Traces

- Observation: obs 34 (pre-existing complexity debt in the comment-policy harness).
- The standing complexity-budget constraint the gate enforces on `src/`.
