---
type: Issue
title: One npm package installs ac in both OpenCode and Pi
description: "active-collab-agent replaces opencode-active-collab and pi-active-collab: OpenCode loads its config hook, Pi loads only its skill."
status: open
timestamp: 2026-11-14T01:37:22Z
---

## 0092. One npm package installs ac in both OpenCode and Pi

### Problem

OpenCode and Pi install `ac` from two npm packages, `opencode-active-collab` and `pi-active-collab` (issue [0090](0090-install-ac-in-claude-code-codex-opencode-and-pi-as-a-plugin.md), published at 0.12.0). Users have to know which name their harness takes, and each package's skill copy is one more copy to keep equal.

### Scope

- One npm package, `active-collab-agent`, serves both harnesses. OpenCode loads its main export, the `config` hook that registers `ac mcp` and the skills path. Pi loads only the skill declared under the `pi` key of its `package.json`.
- `packages/opencode` and `packages/pi` become one package directory. The skill-copy check, its test, the proof script, CI, the README and ADR 0074 name the single package.
- Out of scope: the Claude Code and Codex plugins (git marketplace); deprecating the two published packages (`npm deprecate` runs with the owner's 2FA after the new package is published).

### Decision

- One package named `active-collab-agent` (owner, 2026-11-13). Not taken: the name `active-collab`; keeping the two packages.
- Pi reads the package through an explicit `pi` manifest listing only `skills`, so it never loads the OpenCode `index.ts` as an extension ([Pi packages doc](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/packages.md): "Filters narrow the package manifest"). Proved with Pi's own CLI in a temporary home.
- The two published packages get `npm deprecate` pointing at `active-collab-agent`; they are never unpublished.
- Released as 0.12.1 with the crate, so every manifest version stays equal to `Cargo.toml`.

### Acceptance

- AC1: `npm pack --dry-run` of the package lists only `package.json`, `index.ts` and `skills/active-collab/SKILL.md` (plus a README or LICENSE if added). (`verify_by: command`)
- AC2: OpenCode, given the package's local path in `plugin`, lists the `active-collab` MCP server and the `active-collab` skill; the existing `node --test` cases still pass against the moved `index.ts`. (`verify_by: command`)
- AC3: Pi installs the package from its local path, lists the `active-collab` skill, and loads no extension from it; removing it removes the skill. (`verify_by: command`)
- AC4: `scripts/check-skill-stubs.sh` checks the single package's skill copy, name (`active-collab-agent`) and version; its test plants a drift in each and fails. (`verify_by: test`)
- AC5: README, CI, ADR 0074 and `scripts/prove-plugins.sh` name only `active-collab-agent`; no tracked file outside the docs history names `opencode-active-collab` or `pi-active-collab` except the deprecation note. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `PI_BIN=/usr/bin/pi sh scripts/prove-plugins.sh`
- `sh scripts/test-check-skill-stubs.sh`
- `sh scripts/check-skill-stubs.sh`
- `node --test packages/active-collab-agent/`
- `npm pack --dry-run -w packages/active-collab-agent`
