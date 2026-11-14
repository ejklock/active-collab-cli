---
type: Issue
title: The comment gate applies every rule to tests and rejects every artifact citation
description: "Close the comment-gate gaps left after 0083: every citation form, block comments, the full rule set on tests/, and docs that describe the gate."
status: closed
labels: [tests, tooling, tech-debt]
blocked_by: []
timestamp: 2026-11-12T16:28:03Z
---

## 0086. The comment gate applies every rule to tests and rejects every artifact citation

### Problem

The comment gate (`tests/comment_policy.rs`) left four gaps after [0083](/issues/0083-follow-ups-from-the-0075-0077-reviews-readme-keys-fixture-tokens-untested-tui-routes-record-citations.md) S4, found by its reviews:

- `tests/` is scanned for record citations only: the full rule set there reports 148 banners and 5 commented-out-code lines, two of them false positives inside a multi-line string (`tests/comment_policy.rs:420`) and a raw string (`tests/unit/richtext.rs:212`).
- `cites_record` (`tests/comment_policy.rs:113`) misses `issue #45`, a lowercase `adr0012` and citations inside `/* */` block comments.
- Comments still name process artifacts the pattern does not see: criterion ids (`AC4`, 282 comment lines), scenario ids (`Sc.8`, 13), slice numbers (`slice 0059`), task ids (`ek-0061 S1`) and numberless records ("the ADR", "the BDR").
- `find_line_comment` (`tests/comment_policy.rs:37`) treats every `'` as a char-literal start, so lifetime ticks before `//` could hide a comment from the scan (review inference, not reproduced); `examples/` is not scanned at all; `src/tui/model.rs:2016` "Reserved for V6 text selection" reads like a phase code.
- `CLAUDE.md:27` and `docs/architecture.md:503` describe the gate as banners and commented-out code only, without the citation rule or which rules apply to `src/` and to `tests/`.

### Scope

Included:

- The citation rule catches every form above in `src/` and `tests/`, and the tree is cleaned of them.
- `tests/` gets the full rule set: its banners and commented-out code are removed, and the gate stops flagging comment-like text inside string literals.
- `examples/` joins the gate's roots with the full rule set (owner, 2026-11-12).
- `find_line_comment` (`tests/comment_policy.rs:37`) reads a lifetime tick as a lifetime, not as the start of a char literal, so a comment after an odd number of lifetimes on one line is still scanned (a review inference, to be reproduced first).
- The two docs describe the gate as it runs.

Excluded: `qa/` (TypeScript and JSON, not Rust); any production behavior change.

### Decision

- A comment that only named an artifact is deleted; one with a why keeps the why in the same words minus the artifact, as in 0083.
- The artifact forms the gate rejects in comments: a record (ADR, BDR, PRD, issue) followed by a number in any case and with `#`, `-` or a space between; a numberless "the ADR/BDR/PRD"; a criterion id `AC<n>`; a scenario id `Sc.<n>`; `slice <n>`; a task id `<two letters>-<four digits>`; a phase code such as `V6` is reworded by hand where found, not gated. Not taken: bare slice codes such as `S1`, which collide with ordinary text.
- Block comments are scanned with the same rules as line comments.
- A banner section divider in `tests/` is deleted, not rewritten; the test names already say what each group covers.
- S1 also rewrites the tail of `is_commented_out_code` (`tests/comment_policy.rs:101-106`, `if ... { return true; } false`) as one boolean expression, because clippy's `needless_bool` from Rust 1.100 rejects it under `-D warnings` and the slice already edits the file (owner, 2026-11-12). Pinning the toolchain is its own issue, [0089](/issues/0089-pin-the-rust-toolchain-and-raise-it-deliberately.md).
- The comment gate and the translation policy (`tests/i18n_policy.rs`) share one Rust source lexer under `tests/common/`, which already reads multi-line strings, raw strings, char literals and lifetime ticks; the comment gate takes its comment spans from it (owner, 2026-11-12, after the S1 review found the gate's own scanner duplicating the policy's and over the complexity ceiling). Not taken: splitting the gate's scanner locally, which keeps two lexers and makes S2 re-implement what the policy's already reads.
- The banner and commented-out-code rules read shape, not single tokens: a banner is a run of divider or box-drawing glyphs, or a comment made mostly of them, never prose that names one glyph such as the `│` border; a bare `//` between two comment lines is a paragraph break, not a banner; a comment starting with `return` is code only when it has code shape (session, 2026-11-12, from the S2 review, which found 25 comments reworded around these false positives). Not taken: rewording comments to satisfy the gate.

### Acceptance

- AC1: a planted comment of each artifact form, in a line comment and in a block comment, under `src/` and under `tests/`, fails `cargo test --test comment_policy` naming its file and line; the gate's own fixtures and text inside string literals do not fail it. (`verify_by: test`)
- AC2: the gate applies banners, commented-out code and citations to `src/`, `tests/` and `examples/`, and passes on the tree; a comment after a line with lifetimes is still scanned. (`verify_by: test`)
- AC3: `CLAUDE.md` and `docs/architecture.md` name the three rules and the three roots. (`verify_by: inspection`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test --test comment_policy`
- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`

### Plan

- S1: the citation rule rejects every artifact form, in line and block comments, and `src/` and `tests/` are cleaned of them | files: tests/comment_policy.rs, the src/ and tests/ files the new rule lists | after: 0082 (shares tests/unit/commands.rs)
- S2: `tests/` and `examples/` run the full rule set: banners and commented-out code removed, string-literal and lifetime-tick false readings fixed in the gate | files: tests/comment_policy.rs, the tests/ and examples/ files the full rule set lists | after: S1
- S3: `CLAUDE.md` and `docs/architecture.md` describe the gate's rules and roots | files: CLAUDE.md, docs/architecture.md | after: S2
