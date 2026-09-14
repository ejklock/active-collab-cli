---
type: ADR
title: active-collab-cli becomes a limited write client
description: "active-collab-cli is a limited write client: beyond comments (PRD 0002), it may log time records and edit a bounded task field set (status, assignee, time estimate), each over the host-gated write seam (ADR 0033) with a typed outcome (ADR 0054) and server-truth refresh (ADR 0035). Task create/delete, bulk edits, and web-UI parity stay out of scope. Amends the constitution scope boundary."
status: Accepted
timestamp: 2026-09-14T15:01:30Z
---

# 0069. active-collab-cli becomes a limited write client

## Context

The [constitution](/constitution.md) ratified active-collab-cli as a read/browse tool and listed
"Writing to ActiveCollab (creating/editing/commenting on tasks)" as explicitly out of scope. That
boundary already drifted: comment authoring shipped via [PRD 0002](/prd/0002-task-comment-authoring.md)
and the write seam in [ADR 0033](/adr/0033-authenticated-write-seam-comment-client.md), without a
recorded amendment. The maintainer now wants two further write capabilities from the terminal:
logging time on a task ([issue 0067](/issues/0067-log-time-on-a-task-add-a-time-entry-from-cli-and-tui.md))
and editing a task's status, assignee, and time estimate
([issue 0068](/issues/0068-edit-task-fields-status-assignee-time-estimate-from-cli-and-tui.md)).

These widen the product from "read plus comment" to "mutate the task itself". This is an
expensive-to-reverse product-direction change — it grows the API surface, the failure modes, the
permission story, and user expectations — so it earns a decision record rather than an inline issue
choice. Alternatives considered: (a) keep the tool read+comment only and decline the writes — rejected
because it refuses the maintainer's stated need; (b) build an unbounded write client mirroring the web
UI — rejected as scope-unbounded and high-maintenance for a terminal companion.

## Decision

We will treat active-collab-cli as a **limited** write client: it may create comments (existing) and,
newly, log time records and edit a bounded set of task fields (status, assignee, time estimate). Every
write reuses the authenticated, host-gated write seam (ADR 0033) and follows the established write
patterns — a typed `...WriteOutcome` classification (ADR 0054) and a server-truth refresh after each
mutation (ADR 0035). Writes stay bounded: task/comment/time-record mutation only. Creating or deleting
tasks, bulk edits, and full web-UI parity remain out of scope until a later, separately recorded
decision. The constitution scope boundary is amended to reflect this (Amendment 1).

## Consequences

**Easier / gained:**
- The maintainer logs time and adjusts a task's status/assignee/estimate without leaving the terminal.
- A single, consistent write pattern (seam + typed outcome + server-truth refresh) governs every mutation.
- The scope record stops drifting: writes are now honestly in scope, with an explicit boundary.

**Harder / accepted trade-offs:**
- Larger write surface: more endpoints, more failure modes, and a stronger dependency on ActiveCollab
  permission semantics (a user may lack rights to a given field).
- "Local-first" and the SQLite cache apply to reads; writes require the network and are not offline-capable.
- Token host isolation (constitution non-negotiable) must hold on every new write path, not just reads.

**Follow-ups:**
- Constitution Amendment 1 records the scope change (this same change).
- Issues 0067 and 0068 clear their AC0 scope gate and become implementable.
- A future decision, if wanted, may extend or re-bound the write surface (task create/delete, bulk edits).

## Verification

**Implementation impact:** `src/client.rs` (new write methods over the host-gated seam),
`src/commands/` (new CLI write subcommands), `src/tui/` (write affordances), and the constitution
Amendment Log.

**Verification criteria:**
- Every new write method attaches the instance token only to the instance's own host (constitution
  token-host-isolation non-negotiable), proven by a unit test asserting no `Authorization` header on a
  foreign-host request — the same host-gate test pattern already covering reads and comment writes.
- Each write classifies its response into a typed outcome (Ok / Unauthorized / Failed(status)) mirroring
  `CommentWriteOutcome` (ADR 0054), and refreshes from server truth after success (ADR 0035).
