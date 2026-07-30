---
type: Issue
title: Triage the remaining security leads and the pre-existing IaC misconfiguration from issue 0062
description: "Issue 0062's two confirmed findings (C1 terminal-escape injection, C2 cleartext token scheme downgrade) are fixed under ADR 0067/0068. Three leads were never worked into an exploit, and the completed deterministic sweep surfaced one pre-existing high/critical IaC misconfiguration with no captured scanner detail. Triage each one — confirm with a reproduction and route to a fix, or reject with the specific code reason."
status: Proposed # open | in-progress | closed | superseded
labels: [security, audit, needs-validation]
blocked_by: []              # issue numbers this depends on
tracker:                    # #NN once published to the tracker
timestamp: 2026-07-30T16:44:40Z
---

<!-- OKF frontmatter above carries the tracker metadata (number, labels, blocked-by,
     status) that previously lived only in the directory index. Everything BELOW the
     closing `---` is the issue body and MUST stay byte-identical to the published
     tracker body — strip the frontmatter when publishing. -->

## Triage the remaining security leads and the IaC misconfiguration from issue 0062

[Issue 0062](/issues/0062-security-audit-findings-pending-validation.md) confirmed and fixed
two findings — C1 terminal-escape injection
([ADR 0068](/adr/0068-control-characters-stripped-at-the-untrusted-text-render-boundary.md))
and C2 cleartext API-token scheme downgrade
([ADR 0067](/adr/0067-origin-gated-api-token-header-scheme-host-port.md)). Four items were
left open. This issue triages them; nothing here is a confirmed vulnerability yet.

### Scope

1. **IaC misconfiguration (new information).** The deterministic sweep now completes on this
   host: `deps` advisory-only, `secrets` pass, `sast` pass, `license` pass, `cargo-audit`
   pass, `policy` skipped (no `policy/*.rego`) — and **`iac` fails with 1 hard finding**
   (trivy, high/critical). The wrapper reports only the count, so the specific rule and file
   are still unknown. Run trivy's config scan directly to get the rule id and target
   (`Dockerfile` / `docker-compose.yml` are the only candidates), then decide: fix, or
   document the accepted risk. It is pre-existing — no change in issue 0062's fixes touched
   an IaC file.
2. **Argument injection via git branch name.** The branch name is fed into
   `cli::normalize_argv` before clap parses (`src/main.rs:33-34`). Determine whether a
   crafted branch name in a repo the victim clones can inject a CLI flag or argument when
   the victim runs `ac`.
3. **`ACTIVE_COLLAB_DB` arbitrary path** (`src/config.rs:18-27`) and the predictable
   attachment temp dir `${TMPDIR}/ac-attachments/{pid}-{tid}`
   (`src/controller.rs:442-446`). Local-only; assess symlink and denial-of-service exposure
   on shared hosts.
4. **`process_tag_rich` cognitive complexity 22** (`src/richtext.rs`, threshold 12). Not a
   security finding — the quality gate reports it on every run touching that file, so it is
   recorded here to be either refactored or ratcheted deliberately, not carried silently.

Explicitly KEPT out of scope: re-auditing the two fixed findings, and any change to the
sanitizer or the origin gate.

### Acceptance

- The `iac` finding has a named rule id, file, and a decision recorded in this issue: fixed
  (with the diff) or accepted (with the reason).
- Leads 2 and 3 each end as **confirmed** (with a reproduction and a routed fix issue) or
  **rejected** (with the specific code reason).
- Lead 4 ends as a refactor slice or an explicit, documented threshold decision.
- `~/.agent-tools/bin/security-gate-run.sh all` reports a verdict with no unexplained hard
  finding.

### Plan

Independent triage per item, cheapest first: lead 4 is a read; the IaC finding is one trivy
run; leads 2 and 3 each need a small reproduction attempt before any code is written.
Confirmed items get their own fix issue with a negative test bound to a deterministic
instrument — this issue stays a triage record.
