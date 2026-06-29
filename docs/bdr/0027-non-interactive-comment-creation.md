---
type: BDR
title: "A non-interactive `comment` command posts a comment to a task as the logged-in user, from a -m flag or stdin, with a --json write result"
description: Running `ac comment [TASK_REF] [-m TEXT] [--json] [--instance NAME]` posts a comment to the resolved task as the logged-in user (the instance token owner). The task is the explicit ref (URL or PROJECT_ID/TASK_ID) or, when omitted, the current git branch's task. The body comes from -m/--message or, absent that, from piped stdin (multi-line preserved); an empty body is a usage error (exit 2) with no write. Success prints a human confirmation, or with --json a minified {"ok":true,"comment_id":N,"task_id":N,"project_id":N}; failures (no body, no task, no instance, HTTP error) exit non-zero with no false success. The token reaches only the instance host.
status: Accepted
superseded_by:
supersedes:
tags: [cli, comments, write, agent, json, non-interactive]
timestamp: 2026-06-29T00:00:00Z
---

# 0027. Non-interactive comment creation as the logged-in user

## Context

Comment authoring exists only in the interactive TUI
([BDR 0024](/bdr/0024-comment-authoring-create-edit-delete.md)). This BDR specifies the
observable behavior of a **non-interactive `comment` command** for LLM/agent/script use
([ADR 0040](/adr/0040-non-interactive-comment-write-command.md)), reusing the authenticated
write seam ([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)) and extending
the agent `--json` read contract ([ADR 0011](/adr/0011-agent-json-output-contract.md)) to a
write result.

## Textual Description

Running `ac comment`:

- The **task** is the explicit `TASK_REF` (a task URL or `PROJECT_ID/TASK_ID`); when
  omitted, it is resolved from the **current git branch** (as the `current` command does).
  If neither resolves a task, the command errors and exits non-zero **without writing**.
- The **body** is `-m/--message <TEXT>` when given; otherwise it is read in full from
  **stdin** (piped), preserving multi-line content. An empty/absent body is a **usage
  error** (exit `2`) — nothing is posted.
- The comment is posted as the **logged-in user** — created with the selected instance's
  host-gated token, so ActiveCollab attributes it to the token owner. A **configured
  instance is required** (`--instance <NAME>` selects among several); with none configured
  the command errors ("not logged in"). There is **no way to post as another user**.
- On **success**: without `--json`, a human one-line confirmation; with `--json`, exactly
  one minified line `{"ok":true,"comment_id":N,"task_id":N,"project_id":N}`. Exit `0`.
- On **failure** (no body, no resolvable task, no instance/auth, HTTP `4xx`/`5xx`): a
  non-zero exit and an error on stderr — or, with `--json`, a minified
  `{"ok":false,"error":"<reason>"}`. **No false success.**
- The token is sent **only to the instance host** (host isolation), as for every other
  authenticated call.

## Scenarios

**Scenario 1: post via the -m flag on an explicit ref** — Given a configured instance,
When the user runs `ac comment 524/75346 -m "Deploy em homolog."`, Then the comment is
created on task 75346 as the logged-in user and a confirmation prints, exit 0.

**Scenario 2: post via stdin pipe** — Given a configured instance, When the user runs
`printf 'Linha 1\nLinha 2' | ac comment 524/75346`, Then the two-line body is posted
verbatim, exit 0.

**Scenario 3: --json write result** — Given a configured instance, When the user runs
`ac comment 524/75346 -m "ok" --json`, Then exactly one minified line
`{"ok":true,"comment_id":<id>,"task_id":75346,"project_id":524}` is printed and exit 0.

**Scenario 4: empty body is a usage error** — Given no `-m` and empty/closed stdin, When
the user runs `ac comment 524/75346`, Then the command prints `no comment body`, exits `2`,
and **no** `create_comment` call is made.

**Scenario 5: task from the current branch** — Given the working directory is on a git
branch that maps to a task and no `TASK_REF` is given, When the user runs
`ac comment -m "..."`, Then the comment is posted to that branch's task, exit 0.

**Scenario 6: no task resolvable** — Given no `TASK_REF` and a branch that maps to no task,
When the user runs `ac comment -m "..."`, Then the command errors and exits non-zero with
no write.

**Scenario 7: not logged in** — Given no configured instance, When the user runs
`ac comment 524/75346 -m "..."`, Then the command errors ("not logged in" / no instance)
and exits non-zero with no write.

**Scenario 8: HTTP failure is not a false success** — Given the server returns `4xx`/`5xx`,
When the user posts a comment, Then the command exits non-zero and reports the failure
(human stderr or `--json` `{"ok":false,...}`); it never prints a success line.

**Scenario 9: posted as the logged-in user, token host-isolated** — Given a configured
instance, When the comment is posted, Then `create_comment` attaches the token only to the
instance host and the comment is attributed to the token owner (the logged-in user).

## Test Design

`comment_core` is unit-tested against a **mocked client** with injected stdout/stderr
writers and a body source (flag vs stdin); the task-ref/branch resolution and `--json`
shaping are asserted on the observable output and the (mock) `create_comment` call. The
token-host-isolation negative test already covers `authed_post`
([ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md)).

| Case | Level | Scenario | Asserts (observable) | Proves |
|---|---|---|---|---|
| Flag body, explicit ref | unit | 1 | `create_comment(task_id=75346, body="Deploy…")` called once; confirmation written; exit 0 | happy path, flag body |
| Stdin body (multi-line) | unit | 2 | body read from stdin passed verbatim (incl. `\n`); exit 0 | stdin channel, multi-line |
| `--json` result | unit | 3 | stdout is one minified `{"ok":true,"comment_id":…,"task_id":75346,"project_id":524}`; no extra lines | write-result contract |
| Empty body | unit | 4 | exit 2; `no comment body`; `create_comment` **not** called | usage guard, no accidental write |
| Branch-resolved task | unit | 5 | with no ref, the branch resolver's `(project_id, task_id)` is used | current-branch fallback |
| No task resolvable | unit | 6 | non-zero exit; no `create_comment` | resolution failure is safe |
| No instance | unit | 7 | non-zero exit; "not logged in"; no `create_comment` | requires logged-in user |
| HTTP failure | unit | 8 | non-zero exit; error reported; **no** success line / `{"ok":false,…}` with `--json` | no false success |
| Token host-isolation | unit (existing) | 9 | token attached only to the instance host | write stays host-gated |

## Related

- ADR: [/adr/0040-non-interactive-comment-write-command.md](/adr/0040-non-interactive-comment-write-command.md)
- ADR: [/adr/0033-authenticated-write-seam-comment-client.md](/adr/0033-authenticated-write-seam-comment-client.md), [/adr/0011-agent-json-output-contract.md](/adr/0011-agent-json-output-contract.md)
- BDR: [/bdr/0024-comment-authoring-create-edit-delete.md](/bdr/0024-comment-authoring-create-edit-delete.md) (the interactive authoring this complements), [/bdr/0010-agent-json-output-contract.md](/bdr/0010-agent-json-output-contract.md) (the read `--json` contract)
- Issue: [/issues/0039-non-tty-comment-command.md](/issues/0039-non-tty-comment-command.md)
