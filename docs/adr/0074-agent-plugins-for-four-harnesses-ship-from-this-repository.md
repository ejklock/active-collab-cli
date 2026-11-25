---
type: ADR
title: Agent plugins for four harnesses ship from this repository
description: Claude Code and Codex get a git-marketplace plugin and OpenCode and Pi get npm packages, all inside the ac repository and released with the binary, each wiring ac mcp and the shared skill stub.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-13T18:22:21Z
---

# 0074. Agent plugins for four harnesses ship from this repository

## Context

The `ac` binary reaches agent harnesses through two manual steps: `claude mcp add active-collab -- ac mcp` (README) and `install-skill.sh`, which writes a thin-pointer skill stub per harness. Claude Code and Codex install plugins from git marketplaces; OpenCode loads npm or local plugin packages; Pi installs npm, git or local packages. The owner wants one install step per harness for all four.

## Decision

- The agent plugins live inside this repository, versioned and released with the binary: `plugins/active-collab/` is the Claude Code and Codex plugin (MCP server `ac mcp` plus the `active-collab` skill), listed by `.claude-plugin/marketplace.json` and `.agents/plugins/marketplace.json` at the root; `packages/active-collab-agent/` is the one npm package for OpenCode (MCP server and skill) and Pi (skill only), in this repository's npm workspace.
- Every plugin ships the same thin-pointer skill stub that `install-skill.sh` writes: two copies, the `install-skill.sh` heredoc and the plugin's `SKILL.md`, kept byte-equal by `scripts/check-skill-stubs.sh`, which CI runs.
- No plugin bundles the binary. Each requires an installed `ac` that has the `mcp` subcommand, installed by `install.sh`.
- Publishing to npm and pushing the marketplace are outward-facing and need the owner's word each time. (npm publishing superseded by [ADR 0075](0075-the-release-workflow-publishes-the-npm-package-on-each-release-tag.md): it follows each release tag.)

## Alternatives

- A separate plugins monorepo: independent releases, but a plugin could require an `ac` feature no release has yet, and two repositories must stay in step.
- Keep `install-skill.sh` plus `claude mcp add`: no new surface, but every harness keeps a manual MCP step.
- Claude Code and Codex only, OpenCode and Pi through `install-skill.sh`: no npm, but two harnesses keep a manual step.

## Consequences

- The Rust repository gains a Node workspace (`package.json`, lockfile); the gates and CI must cover or exclude it explicitly.
- Fitness function: `claude plugin validate .` passes, and each harness installs its plugin from this checkout into a temporary home in the plugin proof script.
