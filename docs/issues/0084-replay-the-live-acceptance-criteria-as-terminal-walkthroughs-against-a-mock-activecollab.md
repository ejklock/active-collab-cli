---
type: Issue
title: Replay the live acceptance criteria as terminal walkthroughs against a mock ActiveCollab
description: Replay 0073 AC6, 0074 AC5, 0076 AC4 and 0077 AC4 as qa-walkthrough terminal walkthroughs against a wiremock ActiveCollab, never the owner's real instance.
status: closed
labels: [qa, tests, tooling]
timestamp: 2026-11-12T13:13:20Z
---

## 0084. Replay the live acceptance criteria as terminal walkthroughs against a mock ActiveCollab

### Problem

Four acceptance criteria verify against the owner's real ActiveCollab at collab.base.digital: [0073](/issues/0073-ac-mcp-serve-the-cli-as-mcp-tools-over-stdio-for-parallel-agent-sessions.md) AC6, [0074](/issues/0074-show-estimate-and-logged-hours-this-week-and-total-per-task-in-ac-mine.md) AC5, [0076](/issues/0076-download-comment-attachments-in-the-cli-and-from-a-tui-key.md) AC4 and [0077](/issues/0077-move-a-task-between-task-lists-from-cli-tui-and-mcp.md) AC4. The owner's `ac` is connected to that instance, so running these checks reads and writes real work data; 0077 AC4 moves a real task. The owner asked on 2026-11-12 to replay them as client-ready walkthroughs against a mock instead, and never to touch the real instance.

### Scope

Included:

- `examples/mock_collab.rs`: a wiremock server that answers the routes the four flows call, with the JSON shapes `tests/mcp_stdio.rs` already mounts, and seeds an isolated store whose one instance points at it.
- A `setup.hooks` TypeScript module that starts the mock and seeds the store before the visible steps.
- Four terminal walkthroughs for `qa-walkthrough`, one per acceptance criterion, and one script that builds and replays them.
- The four acceptance criteria rewritten to run against the mock, each issue recording the real-API-shape risk.

Excluded: any production code change; any call to collab.base.digital or a read of the owner's store; a TUI flow (the CLI covers each criterion as written).

### Decision

- The mock is a Rust wiremock server in `examples/mock_collab.rs` on the existing `wiremock` dev-dependency, with the JSON shapes of `tests/mcp_stdio.rs` (owner, 2026-11-12). Not taken: the standalone WireMock Docker image, a second toolchain and a second copy of the shapes.
- The four live criteria become "a `qa-walkthrough` replay against the mock", and each issue records the risk that the real API diverges from the mocked shapes (owner, 2026-11-12). Not taken: the mock plus live read-only reads, which still reach the owner's instance.
- The 0073 walkthrough drives `ac mcp` through the MCP Inspector CLI, `@modelcontextprotocol/inspector --cli` (owner, 2026-11-12). Not taken: raw JSON-RPC typed into the terminal, unreadable for a client. The replay script installs one pinned version under `qa/.run/tools` before the first replay instead of `npx`, because the replayed shell's `HOME` is a scratch folder where an npx cache would not be found. The Inspector starts `ac mcp` with a reduced environment, so the walkthrough passes `ACTIVE_COLLAB_DB` with `-e`.
- Isolation comes from the terminal target's closed environment: each walkthrough declares `ACTIVE_COLLAB_DB`, `HOME`, `TMPDIR` and `ACTIVE_COLLAB_LANG` under its own run directory, so the replayed `ac` never opens `~/.config/active-collab`; the seeded instance's `base_url` is the mock's loopback URI.
- The engine is `host`, so `ac` and the mock run as native binaries built with `CARGO_TARGET_DIR=target/native`, beside the Docker build's Linux binaries in `target/debug` (the `make install-native` precedent). Not taken: the Docker engine, which would need the Linux binary to run inside the engine image.
- The mock exits when its stdin closes, so it dies with the engine process that started it. Not taken: a fixed lifetime, which outlives a short run and cuts a slow one.
- 0076 and 0077 replay through the CLI (`ac get` with attachment download, `ac task set --list`); a TUI replay adds key timing without covering more of their criteria.

### Acceptance

- AC1: `qa-walkthrough check --strict` passes for each of the four walkthroughs. (`verify_by: command`)
- AC2: the replay script runs the four walkthroughs green on the host engine, and each report lands under the `qa-walkthrough` reports root. (`verify_by: command`)
- AC3: a replay leaves the owner's store untouched (`~/.config/active-collab/active-collab.db` keeps its modification time) and the mock serves every request the flows make. (`verify_by: command`)
- AC4: no mock process is left listening after a replay ends. (`verify_by: command`)
- CC: no superfluous comments, banners or commented-out code. (`verify_by: inspection`)

### Verification commands

- `docker compose run --rm dev cargo clippy --all-targets -- -D warnings`
- `docker compose run --rm dev cargo fmt --check`
- `docker compose run --rm dev cargo test --test comment_policy`
- `qa-walkthrough check --strict qa/<flow>.walkthrough.json`
- `qa/run-walkthroughs.sh`

### Plan

- S1: `qa/mine-hours.walkthrough.json` replays 0074 AC5 against `examples/mock_collab.rs`, which a hook starts and seeds and the replay script builds natively | files: examples/mock_collab.rs, qa/hooks/mock-collab.ts, qa/hooks/mine-hours.ts, qa/hooks/package.json, qa/mine-hours.walkthrough.json, qa/run-walkthroughs.sh, .gitignore | after: none
- S2: three walkthroughs replay 0073 AC6, 0076 AC4 and 0077 AC4 against the mock, which gains their routes and the task-list move state; the README says how to run them | files: examples/mock_collab.rs, qa/hooks/mcp-read.ts, qa/hooks/download-attachment.ts, qa/hooks/move-task-list.ts, qa/mcp-read.walkthrough.json, qa/download-attachment.walkthrough.json, qa/move-task-list.walkthrough.json, qa/run-walkthroughs.sh, README.md | after: S1
- S3: the pinned MCP Inspector installs from a committed lockfile with `npm ci`, so a fresh install resolves the same dependency tree | files: qa/tools/package.json, qa/tools/package-lock.json, qa/run-walkthroughs.sh | after: S2
- S4: the mock's seeded comment carries its author name and creation date, so `ac get` against the mock prints `Ada Lovelace` and the date instead of `42 —` (owner, 2026-11-12) | files: examples/mock_collab.rs, the qa/*.walkthrough.json whose target reads that line, if any | after: S3
