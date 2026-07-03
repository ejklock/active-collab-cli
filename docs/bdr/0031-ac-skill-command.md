---
type: BDR
title: "`ac skill` prints the embedded agent skill markdown; `ac skill list` enumerates the registry"
description: Running `ac skill <name>` prints that skill's full markdown to stdout and exits 0; `ac skill list` prints one `name<TAB>description` line per registered skill; a bare `ac skill` prints the single registered skill (or lists when more than one); an unknown name errors on stderr and exits 2. The output is the exact bytes embedded from the canonical .claude/skills/ac-json/SKILL.md, needs no configured instance or network, and `skill` is a known command so a bare invocation is not rewritten to `get`.
status: Accepted
superseded_by:
supersedes:
tags: [cli, agent, llm, skill, non-interactive]
timestamp: 2026-07-03T00:00:00Z
---

# 0031. `ac skill` serves the embedded agent skill

## Context

The `--json` read contract ([BDR 0010](/bdr/0010-agent-json-output-contract.md)) is
documented by a skill that, until now, reached only Claude Code. This BDR specifies
the observable behavior of a new **`ac skill` command** that prints the curated skill
markdown embedded in the binary, making the contract fetchable on demand from any
harness or script ([ADR 0057](/adr/0057-agent-skill-served-by-ac-skill-command.md)).
The per-harness thin-pointer files and `install-skill.sh` that consume this command are
covered by the ADR; this record specifies the command's own output.

## Textual Description

Running `ac skill`:

- **`ac skill <name>`** prints the named skill's **full markdown body** to stdout,
  byte-for-byte the embedded `.claude/skills/ac-json/SKILL.md`, followed by a trailing
  newline, and exits `0`.
- **`ac skill list`** prints one line per registered skill as
  `name<TAB>one-line description` (the description is the skill's frontmatter
  `description`, first sentence), and exits `0`.
- **`ac skill`** with no argument prints the single registered skill's markdown when
  exactly one is registered; when more than one is registered it behaves as
  `ac skill list`. Exit `0`.
- **`ac skill <unknown>`** (a name that is neither `list` nor a registered skill)
  prints an error to **stderr** naming the unknown skill and the known names, and exits
  `2` — nothing is written to stdout.
- The command needs **no configured instance, no database, and no network** — it is a
  pure read of embedded bytes. It never launches the TUI.
- `skill` is a **known command**, so `ac skill …` is parsed as the skill command and is
  **not** rewritten to `ac get skill` by the bare-invocation normalizer.

## Scenarios

**Scenario 1: print a named skill** — Given the binary embeds the `ac-json` skill, When
the user runs `ac skill ac-json`, Then stdout is exactly the embedded `SKILL.md` content
(the `# ac --json — agent read contract` document) and exit is `0`.

**Scenario 2: list skills** — When the user runs `ac skill list`, Then stdout contains a
line `ac-json<TAB>Read ActiveCollab task data as machine-readable JSON…` for the one
registered skill and exit is `0`.

**Scenario 3: bare skill with a single registered skill** — When the user runs `ac skill`
with exactly one skill registered, Then stdout is that skill's full markdown (same bytes
as Scenario 1) and exit is `0`.

**Scenario 4: unknown skill name** — When the user runs `ac skill nope`, Then stderr
reports `unknown skill 'nope'` and the known names, stdout is empty, and exit is `2`.

**Scenario 5: `skill` is not rewritten to `get`** — Given the bare-invocation normalizer
prepends `get` to an unknown first token, When the argv is `["skill","ac-json"]`, Then it
passes through unchanged (no `get` prepended) because `skill` is in `KNOWN_COMMANDS`.

**Scenario 6: no instance or network required** — Given no configured instance and no
database, When the user runs `ac skill ac-json`, Then it still prints the skill and exits
`0` (no "not logged in", no store access, no HTTP).

**Scenario 7: output is the single embedded source** — Given the skill body is embedded
from `.claude/skills/ac-json/SKILL.md`, When that file changes and the binary is rebuilt,
Then `ac skill ac-json` prints the new content (one home; no second copy to update).

## Test Design

`skill_output(args, &mut impl Write, &mut impl Write) -> i32` is a **pure** function over
the embedded registry with injected stdout/stderr writers — no store, no HTTP, no TTY.
Cases assert the observable stdout/stderr/exit. `normalize_argv` already has a unit-test
home ([BDR 0003](/bdr/0003-cli-command-output-parity.md)); Scenario 5 adds a `skill` case
there.

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Named skill | unit | 1 | stdout == embedded `ac-json` `SKILL.md` bytes (+`\n`); exit 0 | prints the contract verbatim |
| List | unit | 2 | stdout has `ac-json\t<description>`; exit 0 | registry enumeration |
| Bare, single skill | unit | 3 | stdout == the one skill's body; exit 0 | no-arg default |
| Unknown name | unit | 4 | stderr names `nope` + known names; stdout empty; exit 2 | safe failure, exit code |
| `skill` known command | unit | 5 | `normalize_argv(["skill","ac-json"], _)` == `["skill","ac-json"]` | no `get` rewrite |
| No instance/network | unit | 6 | `skill_output` writes the body with no store/HTTP dependency in its signature | pure, dependency-free |
| Single-source embed | unit | 7 | the registry body equals `include_str!("../../.claude/skills/ac-json/SKILL.md")` | one home, no second copy |

## Related

- ADR: [/adr/0057-agent-skill-served-by-ac-skill-command.md](/adr/0057-agent-skill-served-by-ac-skill-command.md)
- ADR: [/adr/0011-agent-json-output-contract.md](/adr/0011-agent-json-output-contract.md)
- BDR: [/bdr/0010-agent-json-output-contract.md](/bdr/0010-agent-json-output-contract.md) (the `--json` contract this skill documents), [/bdr/0003-cli-command-output-parity.md](/bdr/0003-cli-command-output-parity.md) (the `normalize_argv` known-command behavior)
</content>
