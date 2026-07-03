---
type: ADR
title: The agent skill is served by an `ac skill` CLI command; per-harness integrations are thin pointers to it
description: A new `ac skill [name]` / `ac skill list` command prints the curated agent skill markdown embedded in the binary from the one canonical .claude/skills/ac-json/SKILL.md; every other harness (Codex, OpenCode, pi, Copilot, Cursor) integrates via a thin stub that defers to `ac skill ac-json` instead of copying the contract, and install-skill.sh writes those stubs at each harness's path.
status: Accepted
supersedes:
superseded_by:
tags: [cli, agent, llm, skill, distribution, structure]
timestamp: 2026-07-03T00:00:00Z
---

# 0057. The agent skill is served by an `ac skill` command; per-harness integrations are thin pointers

## Context

[ADR 0011](/adr/0011-agent-json-output-contract.md) gave agents a curated,
minified `--json` read contract, and a companion Claude Code skill documents it at
`.claude/skills/ac-json/SKILL.md`. That skill only reaches **one** harness (Claude
Code). Users drive this CLI from several agent harnesses — Claude Code, OpenAI
Codex, OpenCode, pi, GitHub Copilot, Cursor — and each loads agent skills /
custom instructions from a **different** path and, in some cases, a different file
format.

The naive fix — copy the full `SKILL.md` contract into all six locations — creates
the exact drift the living-docs "one home per fact" invariant forbids: a field
rename in the `--json` contract ([ADR 0011](/adr/0011-agent-json-output-contract.md),
locked by `tests/unit/agent_json.rs`) would have to be hand-propagated to six files,
and any missed copy silently teaches an agent a stale schema.

Two facts make a better design possible:

- The binary already **embeds** compile-time assets (`include_str!`), the pattern
  [ADR 0005](/adr/0005-i18n-catalog-as-embedded-json.md) established for the i18n
  catalog. The single-binary constraint ([ADR 0002](/adr/0002-rewrite-in-rust-with-ratatui.md))
  holds.
- Every one of the six harnesses runs its agent with a **shell tool**, and every
  one loads a markdown skill/instruction body the agent treats as instructions. So a
  short body that says *"run `ac skill ac-json` and follow its output"* is
  actionable in all six — the full contract need not live in the harness file at all.

## Decision

**The `ac` binary is the single source of truth for the agent skill, exposed
through a new `ac skill` command. Per-harness files are thin pointers to it, not
copies of the contract.**

1. **`ac skill` command (extensible registry).**
   - `ac skill list` — list available skills as `name<TAB>one-line description`.
   - `ac skill <name>` — print that skill's full markdown to stdout (exit 0); an
     unknown name prints an error to stderr and exits `2`.
   - `ac skill` (no argument) — print the single skill's markdown when exactly one
     is registered; otherwise behave as `ac skill list`.
   - `"skill"` joins `KNOWN_COMMANDS` so a bare `ac skill …` is **not** rewritten to
     `ac get skill` by `normalize_argv`.

2. **One home for the contract body.** The `ac-json` skill body is
   `include_str!("../.claude/skills/ac-json/SKILL.md")` — the *same* file Claude
   Code (and, natively, OpenCode and pi) already read. The contract text exists in
   exactly one file; the binary embeds it; `ac skill ac-json` prints it. A schema
   change edits that one file (and `tests/unit/agent_json.rs`), and every consumer —
   the CLI command and every harness pointer — tracks it for free.

3. **Pure, network-free command module.** All shaping lives in a pure
   `src/commands/skill.rs` — a registry `&[SkillEntry { name, description, body }]`
   and a `skill_output(args, &mut impl Write) -> i32` over it, no store/HTTP,
   unit-tested without network (mirroring the `agent_json.rs` purity discipline).

4. **Per-harness integration is a thin pointer.** Every harness other than the
   canonical file receives a small stub whose body defers to `ac skill ac-json`; it
   carries **no** contract fields, so a `--json` schema change never touches it.
   `install-skill.sh --harness <name>|all` writes the stub at each harness's path:

   | Harness | Path | Format |
   |---|---|---|
   | Claude Code | `.claude/skills/ac-json/SKILL.md` | full canonical `SKILL.md` (the source; also read natively by OpenCode & pi) |
   | Codex CLI | `.codex/skills/ac-json/SKILL.md` | thin `SKILL.md` (name+description frontmatter) |
   | OpenCode | `.opencode/skills/ac-json/SKILL.md` | thin `SKILL.md` |
   | pi | `.pi/skills/ac-json/SKILL.md` | thin `SKILL.md` |
   | GitHub Copilot | `.github/skills/ac-json/SKILL.md` | thin `SKILL.md` |
   | Cursor | `.cursor/rules/ac-json.mdc` | thin MDC rule (`description` set, `alwaysApply: false`) |

   The stub text is single-homed inside `install-skill.sh` (heredoc per format), so
   the pointer wording also has one home. The installer targets the current
   directory for repo-local install and honours a `--dir` override.

## Alternatives considered

- **Copy the full `SKILL.md` into all six harness paths.** Rejected: six copies of
  the `--json` contract is precisely the drift the one-home invariant forbids; a
  renamed field would silently rot five copies.
- **Ship only the files, no `ac skill` command.** Rejected: without an on-demand
  command the thin-pointer design has nothing to point at, and scripts/agents in a
  harness we did not anticipate have no way to fetch the contract.
- **A second source file for the embedded body (e.g. `src/skills/ac-json.md`),
  leaving `.claude/skills/ac-json/SKILL.md` a thin pointer too.** Rejected: it
  re-splits the one contract into two homes and denies Claude Code / OpenCode / pi
  the full skill they can read natively for free.
- **Generate each harness file from the canonical `SKILL.md` at install time (full
  copy, not a pointer).** Rejected: still N materialized copies to re-generate on
  every change; the thin pointer needs no regeneration because it holds no contract.

## Consequences

**Positive:**

- The `--json` contract has one home; the CLI command and all six harness pointers
  cannot drift from it.
- `ac skill` is a general, testable read surface: any agent or script — in any
  harness, or none — can fetch the contract on demand.
- Adding a second skill later is a new registry entry (+`include_str!`), not a new
  command; `ac skill list` already accommodates it.
- `src/commands/skill.rs` is pure and unit-tested; the registry shape is locked by
  tests.

**Accepted trade-offs:**

- `include_str!` binds the skill body at **compile time**: editing the contract
  needs a rebuild (acceptable — we ship a binary, per ADR 0005).
- Harnesses that cannot read `.claude/skills/` natively (Codex, Copilot, Cursor)
  need one `install-skill.sh` run to place their pointer; this is documented in the
  README and is a one-time step.
- The thin pointer adds one indirection (the agent runs `ac skill ac-json` before it
  has the contract). This is the deliberate cost that buys drift-freedom.

## Related

- ADR: [/adr/0011-agent-json-output-contract.md](/adr/0011-agent-json-output-contract.md) — the `--json` contract this skill documents.
- ADR: [/adr/0005-i18n-catalog-as-embedded-json.md](/adr/0005-i18n-catalog-as-embedded-json.md) — the `include_str!` compile-time-embed precedent.
- ADR: [/adr/0002-rewrite-in-rust-with-ratatui.md](/adr/0002-rewrite-in-rust-with-ratatui.md) — the single-binary constraint.
- ADR: [/adr/0055-commands-split-three-masters.md](/adr/0055-commands-split-three-masters.md) — the `commands/` module tree the new `skill.rs` joins.
- BDR: [/bdr/0031-ac-skill-command.md](/bdr/0031-ac-skill-command.md) — the observable `ac skill` behavior + test matrix.
</content>
</invoke>
