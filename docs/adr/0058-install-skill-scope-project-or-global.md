---
type: ADR
title: install-skill.sh gains a --scope project|global selector; global writes each harness's user-level path
description: The skill installer adds a --scope selector (default project, unchanged) so a stub can be installed once at the user level instead of per-project. Global scope maps each harness to its real user-level skills directory (claude ~/.claude/skills, pi ~/.pi/agent/skills, codex ~/.codex/skills); harnesses with no standard user-level skills dir (opencode, copilot, cursor) are explicitly unsupported under global and the installer says so. An interactive prompt asks project-vs-global only on a TTY, so curl | sh stays non-interactive.
status: Accepted
supersedes:
superseded_by:
tags: [cli, agent, llm, skill, distribution, installer]
timestamp: 2026-07-03T00:00:00Z
---

# 0058. `install-skill.sh` gains a `--scope project|global` selector

## Context

[ADR 0057](/adr/0057-agent-skill-served-by-ac-skill-command.md) made `install-skill.sh`
write a thin `ac skill ac-json` pointer at each harness's path, targeting the **current
project directory** (`--dir` override, default `.`). That is the right default for a
repo-local install, but agents are usually configured **once at the user level** — a
developer wants the `ac-json` skill available in every project without re-running the
installer in each one.

The naive way to reach the user level with the current script is `--dir "$HOME"`. It is
wrong, because the user-level skills path is **not** the project path re-rooted at
`$HOME` for every harness:

- pi reads user-level skills from `~/.pi/agent/skills/` (and per-profile
  `~/.pi/agent-profiles/<name>/skills/`), **not** `~/.pi/skills/` — so `--dir "$HOME"`
  would write a file pi never loads. (This is exactly the mismatch that forced a
  by-hand global install of the pi pointer.)
- Claude Code's user-level dir *is* `~/.claude/skills/`, so `--dir "$HOME"` happens to
  work there — but relying on that coincidence per harness is fragile.

So "install at the user level" needs its own per-harness path map, not a base-dir hack.

A second fact bounds the feature: not every harness has a **stable, documented
user-level skills directory**. Claude Code (`~/.claude/skills`), pi
(`~/.pi/agent/skills`), and Codex (`~/.codex/skills`, under the established `~/.codex`
home) do. OpenCode, GitHub Copilot, and Cursor do not have a single well-known
user-level *skills* directory we are willing to hard-code (Cursor user rules are managed
in-app, not as a file). Writing a guessed path there would silently drop a stub where
the agent never reads it — the same failure mode as the `--dir "$HOME"` hack.

## Decision

**`install-skill.sh` gains `--scope project|global` (default `project`). Project scope is
unchanged. Global scope writes each supported harness's real user-level path; harnesses
with no standard user-level skills dir are explicitly unsupported under global and the
installer reports that rather than guessing.**

1. **`--scope project` (default).** Exactly the ADR 0057 behavior: writes under the
   `--dir` base (default `.`). No change for existing callers or the `curl | sh` README
   one-liner.

2. **`--scope global`.** Writes the stub at the harness's user-level path, rooted at
   `$HOME` (not `--dir`):

   | Harness | project (`<dir>/…`) | global (`$HOME/…`) |
   |---|---|---|
   | Claude Code | `.claude/skills/ac-json/SKILL.md` | `~/.claude/skills/ac-json/SKILL.md` |
   | pi | `.pi/skills/ac-json/SKILL.md` | `~/.pi/agent/skills/ac-json/SKILL.md` |
   | Codex CLI | `.codex/skills/ac-json/SKILL.md` | `~/.codex/skills/ac-json/SKILL.md` |
   | OpenCode | `.opencode/skills/ac-json/SKILL.md` | *(unsupported — see 3)* |
   | GitHub Copilot | `.github/skills/ac-json/SKILL.md` | *(unsupported)* |
   | Cursor | `.cursor/rules/ac-json.mdc` | *(unsupported)* |

3. **Unsupported-under-global policy.** For OpenCode, Copilot, and Cursor, `--scope
   global` prints to stderr that global scope is not supported for that harness (no
   standard user-level skills directory) and to install per-project instead. A **single**
   explicitly-named unsupported harness exits `2`; `--harness all --scope global`
   **skips** those three, installs the three supported, and exits `0`, reporting the
   processed count. No path is ever guessed for an unsupported harness.

4. **`--dir` is incompatible with `--scope global`.** Global paths are `$HOME`-absolute,
   so combining them with a base-dir override is contradictory; the installer errors
   (exit `2`). `--dir` stays valid for `--scope project`.

5. **Interactive scope prompt (TTY only).** When neither `--scope` nor `--dir` is given
   **and** stdin is a TTY, the installer asks `project` vs `global` (default `project`).
   When stdin is not a TTY — the `curl … | sh` pipe — it silently defaults to `project`,
   so the documented one-liner stays non-interactive and deterministic. The prompt is an
   affordance for a human running the script directly, never a blocker.

6. **One home for the paths.** The per-harness × per-scope path map lives once in
   `install-skill.sh` (extending the existing per-harness `case`), and the stub bodies
   stay single-homed in their heredocs (ADR 0057). Global adds a column to the map, not a
   second copy of anything.

## Alternatives considered

- **Document `--dir "$HOME"` as the "global" install.** Rejected: the user-level path is
  not the project path re-rooted at `$HOME` for pi (and is only a coincidence for Claude
  Code); it would write stubs the agent never reads.
- **Guess a user-level path for OpenCode/Copilot/Cursor too.** Rejected: same
  silently-dropped-stub failure. An explicit "unsupported under global, install
  per-project" message is honest and actionable; a wrong path is neither.
- **Always prompt for scope.** Rejected: it would break the `curl … | sh` one-liner
  (no TTY on the pipe). Prompt only on a TTY; default `project` otherwise.
- **A separate `install-skill-global.sh`.** Rejected: two scripts is two homes for the
  stub bodies and the harness list; a `--scope` flag keeps one installer.

## Consequences

**Positive:**

- A developer installs the `ac-json` pointer **once** at the user level for the harnesses
  that support it, and it is available in every project.
- The user-level paths are correct per harness (notably pi's `~/.pi/agent/skills`), so no
  by-hand placement is needed.
- Unsupported harnesses fail **loudly and specifically** under global, never silently.
- The `curl … | sh` README flow is unchanged (still project, still non-interactive).

**Accepted trade-offs:**

- Global scope covers three of the six harnesses today (claude, pi, codex). Extending it
  to OpenCode/Copilot/Cursor is a future change once each has a verified user-level skills
  path — a new row in the map, not a redesign.
- The pi profile directories (`~/.pi/agent-profiles/<name>/skills`) are **not** targeted
  by `--scope global`; installing into a specific profile stays a `--scope project
  --dir ~/.pi/agent-profiles/<name>`-style explicit call (documented in the README).

## Related

- ADR: [/adr/0057-agent-skill-served-by-ac-skill-command.md](/adr/0057-agent-skill-served-by-ac-skill-command.md) — the installer + thin-pointer design this extends.
- BDR: [/bdr/0032-install-skill-scope.md](/bdr/0032-install-skill-scope.md) — the observable `--scope` behavior + test matrix.
</content>
</invoke>
