---
type: Issue
title: "Dockerfile builder stage omits .claude/ from the build context — cold `docker compose run dev` fails on skill.rs include_str!"
description: The builder stage COPYs only src and locales, then runs cargo build --release, but src/commands/skill.rs embeds .claude/skills/active-collab/SKILL.md via include_str! (ADR 0057). .claude/ is never copied into the build context, so a cold image build fails ("couldn't read …/.claude/skills/active-collab/SKILL.md: No such file or directory"). It only works where a warm dev image already exists (the bind mount ./:/app supplies .claude at runtime). Add COPY .claude/skills ./.claude/skills to the builder stage before cargo build --release.
status: open
labels: [ci, docker, infra, bugfix]
blocked_by:
tracker:
timestamp: 2026-07-14T00:00:00Z
---

## Dockerfile builder stage omits `.claude/` from the build context

### Problem

`docker-compose.yml`'s `dev` service builds with `target: builder`. The builder stage
(`Dockerfile`) copies only `src` and `locales` before `RUN cargo build --release`
(Dockerfile:18). But `src/commands/skill.rs:13` embeds the agent skill at compile time:

```rust
body: include_str!("../../.claude/skills/active-collab/SKILL.md"),
```

`.claude/` is never `COPY`'d into the build context, so a **cold** image build fails:

```
couldn't read src/commands/../../.claude/skills/active-collab/SKILL.md: No such file or directory
```

It has been latent since the `ac skill` command landed ([ADR 0057](/adr/0057-agent-skill-served-by-ac-skill-command.md)):
it only "works" where a warm `dev` image already exists, because the `dev` service
bind-mounts `./:/app`, which supplies `.claude/` at **runtime** — but the image **build**
(before the mount) still needs the file. On any clean checkout / cold cache, every
Docker-first gate (`cargo build|test|clippy|fmt|comment_policy`) is blocked. Discovered
while verifying issue 0057 (agent-memory observation id 96).

### Scope

Included:

- `Dockerfile` — add `COPY .claude/skills ./.claude/skills` (narrow — only the embedded
  skill tree) to the builder stage **before** line 18's `RUN cargo build --release`.
- Confirm `.dockerignore` does not exclude `.claude` (adjust if it does).

Excluded: the Rust toolchain pin (issue 0056); any change to `skill.rs` or the embedded
contract (ADR 0057).

### Acceptance

- AC1 — a cold build (`docker compose run --rm dev cargo build`, no cached `dev` image)
  succeeds — `.claude/skills/active-collab/SKILL.md` is present at compile time.
  (`verify_by: command`)
- AC2 — the embedded skill still resolves: `docker compose run --rm dev cargo test`
  passes (no regression in the `ac skill` path). (`verify_by: command`)
- CC — no superfluous Dockerfile comments; the added COPY carries a why-comment only if
  non-obvious. (`verify_by: inspection`)

### Plan

1. Add `COPY .claude/skills ./.claude/skills` before `RUN cargo build --release` in the
   builder stage.
2. Verify `.dockerignore` allows it.
3. Cold-build to confirm the gate suite runs.

### Verification commands

- `docker compose run --rm dev cargo build`
- `docker compose run --rm dev cargo test -- --test-threads=1`
</content>
