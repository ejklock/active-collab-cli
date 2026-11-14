---
type: Issue
title: Messages filled with format! before t() are never translated
description: 34 call sites pass already-filled text to t(), so pt_BR never matches the catalog template; translate the template first, then fill it, and forbid the pattern with a policy test.
status: closed
labels: [i18n, cli, tech-debt]
timestamp: 2026-11-12T11:51:34Z
---

## 0082. Messages filled with format! before t() are never translated

### Problem

`src/i18n.rs` `t()` looks a message up in the pt_BR catalog by its exact text. 34 call sites build the message first and translate it after, `t(&format!("... {x} ...", x = v))`, so the lookup gets the already-filled text, never matches the catalog's template, and the message stays in English when the language is pt_BR. The catalog already holds those templates with their placeholders (for example `locales/pt_BR.json:36-40`). The call sites: `src/commands/setup.rs` (17), `src/commands/task.rs` (8), `src/commands/resolve.rs` (4), `src/commands/presenter.rs` (3), `src/commands/mine.rs` (1), `src/main.rs` (1).

### Scope

Included:

- Each of the 34 call sites translates its template first, then fills the placeholders: `t("... {x} ...").replace("{x}", &v)`.
- A template missing from `locales/pt_BR.json` gains its pt_BR entry.
- A policy test fails when `t(&format!` appears in `src/`, so the defect cannot return.

Excluded: the English text of any message; the `t()` signature and the catalog format; messages that never pass through `t()`.

### Decision

- Fix at the call sites, not in `t()`: a `t()` that guessed the template back from filled text would need the values, and the catalog already stores templates.
- Placeholder names stay the ones the catalog already uses, so no catalog key changes meaning.
- The policy test lives beside `tests/comment_policy.rs` as a source scan, the same instrument the project already runs.
- A message with two or more values a user controls (an instance name and the list of known names, a task list name and the lists of a project) fills them in one pass with `i18n::fill(template, &[(placeholder, value)])`, so a value holding placeholder text is never substituted again; a message with one user value keeps the chained `.replace` the tree already uses (session, 2026-11-12, from the S1 review). Not taken: chained `.replace` everywhere, which re-substitutes when both values are user text; a formatting crate, a new dependency for one function.

### Acceptance

- AC1: with the language set to pt_BR, each of the 34 messages renders in Portuguese with its values filled. (`verify_by: test`)
- AC2: with the language set to en, each message is byte-identical to before. (`verify_by: test`)
- AC3: the policy test fails on a `t(&format!` added to `src/` and passes on the tree. (`verify_by: test`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo test -- --test-threads=1`
- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`

### Plan

- S1: the 17 `ac setup` messages translate their template first and then fill it, so pt_BR renders them in Portuguese and en stays byte-identical | files: src/commands/setup.rs, locales/pt_BR.json, tests/unit/commands.rs | after: none
- S2: the other 17 call sites (task, resolve, presenter, mine, main) translate first, and a source-scan policy test beside tests/comment_policy.rs fails on any `t(&format!` in src/ | files: src/i18n.rs, src/commands/task.rs, src/commands/resolve.rs, src/commands/presenter.rs, src/commands/mine.rs, src/main.rs, locales/pt_BR.json, tests/unit/i18n.rs, tests/unit/commands.rs, tests/i18n_policy.rs | after: S1 (shares locales/pt_BR.json and tests/unit/commands.rs)
