---
type: Issue
title: "Reconcile CHANGELOG.md with the Rust crate tag line — retire/segregate the legacy Python history"
description: CHANGELOG.md still carries the pre-cutover Python history ([0.1.0]–[1.0.0], curses / __version__ entries) and was never updated for the Rust crate's own version line (tags v0.1.0→v0.4.0). Its top entry is [1.0.0] (the Python→Rust cutover note) while Cargo.toml ships 0.4.0, so the changelog's latest version is both ahead of and unrelated to the shipped crate version, and the numbered entries collide with the Rust tags (a legacy Python [0.4.0] curses entry vs the Rust v0.4.0 release). Because of this, the v0.4.0 release notes were placed on the GitHub Release only, leaving CHANGELOG.md silently stale. Decide and apply a reconciliation so the changelog tracks the Rust crate again.
status: closed
labels: [documentation, release, tech-debt, chore]
blocked_by:
tracker:
timestamp: 2026-07-09T00:00:00Z
---

## Reconcile CHANGELOG.md with the Rust crate tag line

### Problem

`CHANGELOG.md` is the pre-cutover **Python** changelog. Its headers run `[0.1.0]`
… `[0.5.0]` … `[1.0.0]` (all curses / `__version__` era), with `[1.0.0]` at the
top describing the Rust cutover. Meanwhile the shipped crate is versioned on a
**separate** line: `Cargo.toml` is `0.4.0` and the git tags are `v0.1.0`, `v0.2.0`,
`v0.3.0`, `v0.4.0`. The two lines reuse the same numbers but mean different
things — e.g. the legacy Python `[0.4.0]` entry (rounded frame + `↑/↓`/`k`/`j`
scrolling, curses `KEY_RESIZE`) is unrelated to the Rust `v0.4.0` design-system
release. As a result:

- The changelog's newest header (`[1.0.0]`) is ahead of the crate's actual
  version (`0.4.0`) and describes work that predates every Rust tag.
- Appending a Rust `[0.4.0]` entry would create a **duplicate header** colliding
  with the Python one — which is why the v0.4.0 notes were put on the GitHub
  Release instead, leaving the file stale.

### Decision (to be finalized in this issue)

Pick one reconciliation strategy and apply it (no code change — docs only):

- **Option A — Segregate:** move the entire existing body under a single
  `## Pre-Rust (Python) history` section (kept verbatim for provenance) and start
  a fresh Keep-a-Changelog list for the Rust crate at `## [0.4.0]` with the notes
  currently on the GitHub Release, backfilling `[0.3.0]`/`[0.2.0]`/`[0.1.0]` from
  their tag annotations where available.
- **Option B — Restart:** archive the legacy content to
  `docs/CHANGELOG-python.md`, link it from a short note at the top of
  `CHANGELOG.md`, and keep `CHANGELOG.md` as the Rust-only line from `[0.4.0]`.

Recommended: **Option A** (one file, provenance preserved, no dangling doc). The
chosen option is recorded here before editing.

### Scope

- `CHANGELOG.md` reconciliation per the chosen option.
- Backfill the Rust `[0.1.0]`–`[0.4.0]` entries from their annotated tags / PRs.
- If Option B, add `docs/CHANGELOG-python.md` and its index/link so the living-docs
  ratchet stays green.

### Out of scope

- Any source/behavior change.
- Changing the versioning scheme itself (SemVer on the Rust crate stays).
- Retroactively re-tagging or editing published GitHub Releases.

### Acceptance criteria

- **AC1** (constraint, inspection): `CHANGELOG.md`'s newest entry is the current
  crate version (`0.4.0`) and there is exactly one header per version — no header
  collides with a legacy Python version of the same number.
- **AC2** (constraint, inspection): the Python-era history is preserved (segregated
  or archived+linked), not deleted, and any new file is linked so no orphan doc
  exists (living-docs strict).
- **AC3** (behavior, command): the v0.4.0 section matches the published GitHub
  Release notes for `v0.4.0`.
- **CC** (constraint, inspection): clean docs — no orphan links, index updated if a
  new file is added.

### Verification

- Manual review of `CHANGELOG.md` against the tag list (`git tag`) and the
  `v0.4.0` GitHub Release body.
- Living-docs enforcer green (no orphan doc / broken link).

### Traces

- Surfaced during the `v0.4.0` release (PR #25): the release notes had to go on the
  GitHub Release because a Rust `[0.4.0]` header would collide with the legacy
  Python `[0.4.0]` entry.

### Resolution — Option A (segregate), applied 2026-07-17

Applied **Option A** during the `v0.5.0` release. `CHANGELOG.md` now opens with a
fresh Rust-crate line — `[0.5.0]`, `[0.4.0]`, `[0.3.0]`, `[0.2.0]`, `[0.1.0]` at
their tag dates (2026-07-03 … 2026-07-17), backfilled from the `v0.1.0` GitHub
Release body and the annotated tag messages. The pre-cutover Python history is
preserved verbatim under a single `## Pre-Rust (Python) history` section, with its
version headers demoted to `###` (and inner sections to `####`) so no `##` header
collides with a Rust version of the same number (AC1). Nothing was deleted (AC2);
no source or behavior changed; the SemVer scheme and published GitHub Releases are
untouched (out of scope).
