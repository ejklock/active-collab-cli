---
type: BDR
title: "`install-skill.sh --scope project|global` selects per-project vs user-level install paths"
description: install-skill.sh accepts --scope project|global (default project). Project scope writes the thin pointer under the --dir base (unchanged). Global scope writes each supported harness's user-level path (claude ~/.claude/skills, pi ~/.pi/agent/skills, codex ~/.codex/skills); opencode, copilot, and cursor are unsupported under global — a single named one exits 2, and --harness all skips them and installs the rest. --dir combined with --scope global errors. An invalid --scope value exits 2. A TTY-only prompt asks scope when neither --scope nor --dir is given; a non-TTY run defaults to project.
status: Accepted
superseded_by:
supersedes:
tags: [cli, agent, installer, skill, non-interactive]
timestamp: 2026-07-03T00:00:00Z
---

# 0032. `install-skill.sh --scope project|global`

## Context

[BDR 0031](/bdr/0031-ac-skill-command.md) specifies the `ac skill` command; the thin
pointer that defers to it is placed by `install-skill.sh` ([ADR
0057](/adr/0057-agent-skill-served-by-ac-skill-command.md)), which until now only wrote
into a project directory. [ADR
0058](/adr/0058-install-skill-scope-project-or-global.md) adds a `--scope` selector so the
pointer can be installed once at the user level. This record specifies the installer's
observable scope behavior.

## Textual Description

`install-skill.sh --harness <name>|all [--scope project|global] [--dir <path>] [--force]`:

- **`--scope project`** (the default) writes the stub under the `--dir` base (default
  `.`), exactly as before: `<dir>/.claude/skills/ac-json/SKILL.md`, etc.
- **`--scope global`** writes the stub at the harness's user-level path under `$HOME` for
  the **supported** harnesses:
  - claude → `$HOME/.claude/skills/ac-json/SKILL.md`
  - pi → `$HOME/.pi/agent/skills/ac-json/SKILL.md`
  - codex → `$HOME/.codex/skills/ac-json/SKILL.md`
- **Unsupported under global** — `opencode`, `copilot`, `cursor` have no standard
  user-level skills directory. `--scope global` for one of them, named explicitly, prints
  a message to **stderr** and exits `2`. `--harness all --scope global` **skips** those
  three (message to stderr), installs the three supported, and exits `0`.
- **`--dir` with `--scope global`** is contradictory (global paths are `$HOME`-absolute)
  and exits `2`.
- **An invalid `--scope` value** (neither `project` nor `global`) prints an error to
  stderr and exits `2`.
- **Scope prompt (TTY only).** When neither `--scope` nor `--dir` is passed **and** stdin
  is a TTY, the installer prompts for `project` vs `global` (default `project` on an empty
  answer). When stdin is **not** a TTY (the `curl … | sh` pipe, or a captured run), it
  defaults to `project` with no prompt.
- The skip-if-exists / `--force` behavior (ADR 0057) is unchanged and applies to both
  scopes.

## Scenarios

**Scenario 1: project scope unchanged (default, non-TTY)** — Given no `--scope`, When
`install-skill.sh --harness all --dir <tmp>` runs with a non-TTY stdin, Then the six stubs
are written under `<tmp>` exactly as in BDR 0031/ADR 0057, and exit is `0` — no prompt.

**Scenario 2: global scope writes user-level paths** — Given `HOME=<tmp>`, When
`install-skill.sh --harness all --scope global` runs, Then it writes
`<tmp>/.claude/skills/ac-json/SKILL.md`, `<tmp>/.pi/agent/skills/ac-json/SKILL.md`, and
`<tmp>/.codex/skills/ac-json/SKILL.md`, and exit is `0`.

**Scenario 3: unsupported harness under global, named explicitly** — When
`install-skill.sh --harness opencode --scope global` runs, Then stderr reports opencode is
unsupported under global scope, nothing is written, and exit is `2`. (Same for `copilot`
and `cursor`.)

**Scenario 4: `--harness all --scope global` skips the unsupported** — Given `HOME=<tmp>`,
When `install-skill.sh --harness all --scope global` runs, Then claude, pi, and codex
stubs are written under `<tmp>`, no `.opencode`/`.github`/`.cursor` file is written,
stderr notes the three skips, and exit is `0`.

**Scenario 5: `--dir` with `--scope global` errors** — When `install-skill.sh --harness
claude --scope global --dir <tmp>` runs, Then stderr reports the incompatibility and exit
is `2`.

**Scenario 6: invalid `--scope` value** — When `install-skill.sh --harness claude --scope
nope` runs, Then stderr reports the invalid scope and exit is `2`.

## Test Design

The installer is a POSIX `sh` script; `tests/install_skill.rs` runs it hermetically in
unique temp dirs and asserts on written files, stdout/stderr, and exit codes. Global-scope
cases set `HOME` to the temp dir (via the child process env) so no real user directory is
touched; all runs inherit a non-TTY stdin so the prompt never fires (its TTY branch is
verified by inspection, not by a pty test).

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Project default unchanged | integration | 1 | six stubs under `--dir`; exit 0; no prompt on non-TTY | back-compat, non-interactive default |
| Global supported paths | integration | 2 | claude/pi/codex stubs at `$HOME` user-level paths; exit 0 | correct per-harness global map |
| Global unsupported (single) | integration | 3 | stderr names the harness; no file; exit 2 | no guessed path, loud failure |
| Global all skips unsupported | integration | 4 | 3 written, 3 absent; stderr notes skips; exit 0 | `all` is resilient under global |
| `--dir` + global errors | integration | 5 | stderr incompatibility; exit 2 | contradictory flags rejected |
| Invalid `--scope` | integration | 6 | stderr invalid scope; exit 2 | argument validation |

## Related

- ADR: [/adr/0058-install-skill-scope-project-or-global.md](/adr/0058-install-skill-scope-project-or-global.md) — the scope-selector decision + path map.
- ADR: [/adr/0057-agent-skill-served-by-ac-skill-command.md](/adr/0057-agent-skill-served-by-ac-skill-command.md) — the installer + thin-pointer design.
- BDR: [/bdr/0031-ac-skill-command.md](/bdr/0031-ac-skill-command.md) — the `ac skill` command the stub points to.
</content>
