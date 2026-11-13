---
type: Issue
title: Install ac in Claude Code, Codex, OpenCode and Pi as a plugin
description: "One install step per agent harness: a git-marketplace plugin for Claude Code and Codex and npm packages for OpenCode and Pi, each wiring ac mcp and the shared skill stub."
status: in-progress
timestamp: 2026-11-13T18:22:35Z
---

## 0090. Install ac in Claude Code, Codex, OpenCode and Pi as a plugin

### Problem

Installing `ac` for an agent takes manual steps per harness: `claude mcp add active-collab -- ac mcp` for the MCP server and `install-skill.sh` for the skill stub. Claude Code, Codex, OpenCode and Pi each have a plugin or package mechanism that could do both in one step. See [ADR 0074](/adr/0074-agent-plugins-for-four-harnesses-ship-from-this-repository.md).

### Scope

Included: a Claude Code and Codex plugin with its two marketplace files; an OpenCode npm plugin package; a Pi npm package; README install steps per harness; a local proof that installs each into a temporary home.

Excluded: `npm publish`, reserving package names, pushing, and a release; bundling the `ac` binary; Copilot and Cursor.

### Decision

- Plugins live in this repository in an npm workspace (owner, 2026-11-13). Not taken: a separate plugins monorepo; keeping `install-skill.sh` plus `claude mcp add`; Claude Code and Codex only.
- All four harnesses, OpenCode and Pi as npm packages (owner, 2026-11-13). Not taken: OpenCode and Pi through `install-skill.sh` without npm.
- Pi has no native MCP: the Pi package ships the skill, which drives the `ac` CLI with `--json`; an MCP bridge extension is left out unless the owner asks.
- The skill stub exists twice, the `install-skill.sh` heredoc (which runs as `curl | sh` and cannot read a repo file) and the plugin's `SKILL.md`; `scripts/check-skill-stubs.sh` keeps them byte-equal, and also checks the plugin and marketplace names and versions against `Cargo.toml`.
- The plugin keeps two MCP declarations only if one cannot serve both harnesses: Codex 0.167 reads only a root `mcp.json` with a transport `type`, never `.mcp.json`; Claude Code reads the file its manifest's `mcpServers` names. One `mcp.json`, named by the Claude manifest, serves both (session, 2026-11-13).
- The plugin keeps a root `plugin.json` for Codex: without it Codex installs the plugin but shows no display name, description, developer or website (session, 2026-11-13).
- Manifests name the author Evaldo Klock and license MIT, as `LICENSE` does (session default, 2026-11-13). Not taken: Klock Tecnologia as author.

### Acceptance

- AC1: `claude plugin validate .` passes, and `claude plugin marketplace add` plus `claude plugin install active-collab@<marketplace>` from this checkout, in a temporary `CLAUDE_CONFIG_DIR`, lists the `active-collab` MCP server and skill. (`verify_by: command`)
- AC2: `codex plugin marketplace add` from this checkout, in a temporary `CODEX_HOME`, installs the plugin with the MCP server and skill. (`verify_by: command`)
- AC3: OpenCode 1.19 loads `packages/opencode` from a local path in a temporary config and registers the `ac mcp` server and the skill. (`verify_by: command`)
- AC4: `pi install ./packages/pi` in a temporary home lists the package and its skill. (`verify_by: command`)
- AC5: `npm pack` of each npm package contains only its intended files. (`verify_by: command`)
- AC6: every plugin's skill stub equals the one `install-skill.sh` writes. (`verify_by: command`)
- AC7: the README gives one install step per harness and says `ac` must have the `mcp` subcommand. (`verify_by: inspection`)
- CC: no superfluous comments, banners or commented-out code; the owner's real configs are never touched. (`verify_by: inspection`)

### Plan

- S1: Claude Code and Codex plugin, both marketplaces, shared skill stub and its test | files: plugins/active-collab/**, .claude-plugin/marketplace.json, .agents/plugins/marketplace.json, install-skill.sh, a stub check script | after: none
- S2: OpenCode npm plugin package and npm workspace root | files: package.json, package-lock.json, packages/opencode/** | after: S1
- S3: Pi npm package | files: packages/pi/**, package.json | after: S2
- S4: README install section, proof script, gate and CI wiring for the Node workspace and for `scripts/check-skill-stubs.sh` with its test | files: README.md, scripts/prove-plugins.sh, .jscpd.json, .github/workflows/ci.yml | after: S3
