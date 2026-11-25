---
type: ADR
title: The release workflow publishes the npm package on each release tag
description: Pushing a v* tag publishes active-collab-agent to npm from GitHub Actions through npm trusted publishing (OIDC), after the GitHub release, and fails when the package version differs from the tag.
owner: Evaldo Klock
status: Accepted
timestamp: 2026-11-25T12:00:00Z
---

# 0075. The release workflow publishes the npm package on each release tag

Supersedes the npm bullet of [ADR 0074](0074-agent-plugins-for-four-harnesses-ship-from-this-repository.md) ("Publishing to npm … need the owner's word each time"); the rest of 0074 stands.

## Context

`.github/workflows/release.yml` built the binaries and published the GitHub release, but nothing published `active-collab-agent`. On 2026-11-25 npm still served 0.12.2 while the latest GitHub release was v0.12.4, so OpenCode and Pi users received an older skill and server wiring than the binary. The owner asked for npm to follow each release (2026-11-25).

## Decision

- A `publish-npm` job in `release.yml` runs after the `release` job, only on a tag ref. It publishes `packages/active-collab-agent` with `npm publish --provenance --access public`.
- Authentication is npm trusted publishing: the job has `id-token: write`, and npm trusts this repository's `release.yml` workflow, configured once on npmjs.com by the package owner. No npm token is stored in the repository.
- The job fails before publishing when `package.json` `version` differs from the tag without its `v`, and when the package's `node --test` fails.
- The owner's word now attaches to the release tag, which already needs it; the npm publish follows the tag without a second step.

## Alternatives

- An `NPM_TOKEN` secret: works without setting anything on npmjs.com, but a long-lived token can leak and must be rotated.
- Keep publishing by hand: no CI change, but npm lags the release, as it did for 0.12.3 and 0.12.4.

## Consequences

- A tag whose version was not bumped in `packages/active-collab-agent/package.json` fails the `publish-npm` job after the GitHub release exists; `scripts/check-skill-stubs.sh` in CI already pins that version to `Cargo.toml`.
- Re-running the job for a version npm already has fails: npm refuses to republish a version.
- Until trusted publishing is configured on npmjs.com, the job fails at `npm publish` with an authentication error.
