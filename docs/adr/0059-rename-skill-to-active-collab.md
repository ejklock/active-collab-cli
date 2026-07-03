---
type: ADR
title: Rename the agent skill from ac-json to active-collab and sharpen its frontmatter description
description: The single registered agent skill is renamed ac-json -> active-collab (the command stays `ac`; only the skill identifier changes) and its frontmatter description is rewritten to be trigger-rich so a harness activates it for the right requests. This updates the embedded registry name and include path, every per-harness install path (…/skills/active-collab/SKILL.md, .cursor/rules/active-collab.mdc), and the `ac skill active-collab` invocation. Amends the name/path parts of ADR 0057 and ADR 0058.
status: Accepted
supersedes:
superseded_by:
tags: [cli, agent, llm, skill, naming, distribution]
timestamp: 2026-07-03T00:00:00Z
---

# 0059. Rename the agent skill `ac-json` → `active-collab`; sharpen its description

## Context

[ADR 0057](/adr/0057-agent-skill-served-by-ac-skill-command.md) introduced a single
registered agent skill served by `ac skill <name>`, named **`ac-json`** — a name that
described the *mechanism* (the `ac --json` read contract). In practice the folder an agent
or a human sees when the skill is installed is `…/skills/ac-json/`, and that reads as an
implementation detail rather than "the ActiveCollab skill". The skill *is* the
ActiveCollab integration surface for agents; naming it after the product it reads
(`active-collab`) is what a user scanning `~/.claude/skills/` expects to find, and it
matches the repository and the historical Python skill's name.

The command is unaffected: the binary stays `ac` (`Cargo.toml [[bin]] name = "ac"`), and
every `ac get/current/mine/browse --json` invocation is unchanged. Only the **skill
identifier** changes — the registry `name`, the embedded file path, each harness install
path, and the `ac skill <name>` argument.

Separately, the skill's frontmatter **description** is the field a harness reads to decide
*whether to activate the skill for a given request*. The original description led with
"Read ActiveCollab task data as machine-readable JSON" but under-specified the triggers.
A sharper, trigger-rich description improves activation precision.

## Decision

**Rename the skill `ac-json` → `active-collab` everywhere, keep the command `ac`, and
rewrite the frontmatter description to be trigger-rich.**

1. **Name + invocation.** The registry entry's `name` becomes `active-collab`; the
   on-demand command is `ac skill active-collab`. `ac skill list` lists `active-collab`.
   `ac skill ac-json` is now an unknown skill (exit `2`) — this is a clean rename, not an
   alias; the old name is not kept.

2. **One home, new path.** The canonical embedded file moves
   `.claude/skills/ac-json/SKILL.md` → `.claude/skills/active-collab/SKILL.md` (its
   `name:` frontmatter and `include_str!` path follow). It remains the single home the
   binary embeds and Claude Code / OpenCode / pi read natively.

3. **Per-harness install paths (corrects the ADR 0057 / ADR 0058 path maps).**

   | Harness | project (`<dir>/…`) | global (`$HOME/…`) |
   |---|---|---|
   | Claude Code | `.claude/skills/active-collab/SKILL.md` | `~/.claude/skills/active-collab/SKILL.md` |
   | pi | `.pi/skills/active-collab/SKILL.md` | `~/.pi/agent/skills/active-collab/SKILL.md` |
   | Codex CLI | `.codex/skills/active-collab/SKILL.md` | `~/.codex/skills/active-collab/SKILL.md` |
   | OpenCode | `.opencode/skills/active-collab/SKILL.md` | *(unsupported under global — ADR 0058)* |
   | GitHub Copilot | `.github/skills/active-collab/SKILL.md` | *(unsupported)* |
   | Cursor | `.cursor/rules/active-collab.mdc` | *(unsupported)* |

   The `--scope project|global` behavior ([ADR 0058](/adr/0058-install-skill-scope-project-or-global.md))
   is unchanged; only the leaf name is `active-collab` instead of `ac-json`.

4. **Sharpened description (single wording, reused by the canonical file, each thin
   pointer, and the `ac skill list` registry entry).** The frontmatter `description` reads:

   > Read ActiveCollab task data — a task, your assignments, comments, or projects — as
   > machine-readable JSON from the `ac` CLI, non-interactively without the TUI. Use when
   > an agent or script needs to fetch a task by id or URL, list the logged-in user's open
   > tasks, read the task for the current git branch, or browse projects, and wants
   > structured JSON instead of the interactive terminal UI. Covers `ac get`, `ac
   > current`, `ac mine`, and `ac browse` with `--json` — the curated minified schemas,
   > the round-trippable `ref`, and the cache/`--no-comments` flags.

   Its **first sentence** is what `ac skill list` prints (BDR 0031), so it is
   self-contained. The registry `description` in `src/commands/skill.rs` is set to that
   first sentence.

## Alternatives considered

- **Keep `ac-json`.** Rejected on the user's call: the installed folder should read as the
  ActiveCollab skill, not as a `--json` implementation detail.
- **Alias — accept both `ac skill ac-json` and `ac skill active-collab`.** Rejected: the
  skill shipped in v0.2.0 only days ago with negligible adoption; a clean rename avoids
  carrying a second registry key and a stale folder name forever. (Local machines that
  installed the `ac-json` folder are cleaned up out-of-band, as with the retired Python
  skill.)
- **Parse the registry description from the embedded frontmatter instead of hard-coding
  it in `skill.rs`.** Deferred: it removes a second home for the description string but is
  a separate refactor; for now both are set to the same first sentence and the drift is
  covered by the byte-equality unit test on the body plus a description assertion.

## Consequences

**Positive:**

- The installed skill folder reads as `active-collab` — discoverable and matching the
  product and repo.
- A trigger-rich description improves when a harness activates the skill.
- The rename is mechanical and fully covered by the `skill` unit tests and the
  `install_skill` integration tests (both assert the new name/paths).

**Accepted trade-offs / breaking change:**

- `ac skill ac-json` no longer works (exit `2`) — a breaking change to that subcommand
  argument, warranting a minor-version bump at the next release.
- Machines that already installed the `ac-json` folder (per-harness) keep a stale
  `…/skills/ac-json/` until re-run with the new installer and the old folder is removed —
  a one-time local cleanup, not a code concern.
- The description wording now lives in three places (canonical frontmatter, each thin
  pointer, the `skill.rs` registry first sentence); the duplication is the same
  thin-pointer trade-off ADR 0057 already accepted, bounded by tests.

## Related

- ADR: [/adr/0057-agent-skill-served-by-ac-skill-command.md](/adr/0057-agent-skill-served-by-ac-skill-command.md) — amended here (the skill name + canonical path).
- ADR: [/adr/0058-install-skill-scope-project-or-global.md](/adr/0058-install-skill-scope-project-or-global.md) — amended here (the per-harness path leaf name).
- BDR: [/bdr/0031-ac-skill-command.md](/bdr/0031-ac-skill-command.md) — amended: `ac skill active-collab` is the invocation.
- BDR: [/bdr/0032-install-skill-scope.md](/bdr/0032-install-skill-scope.md) — amended: install paths use `active-collab`.
</content>
