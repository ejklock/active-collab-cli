---
type: Issue
title: "B1 — multi-instance project-name isolation: key the in-memory name map by (instance, project_id)"
description: tasks_by_project flattens the per-instance project-name maps into one HashMap<i64,String> via extend(), so two instances sharing a numeric project_id with different names clobber each other and a group shows the wrong instance's name. Key the merged map by (instance_name, project_id) to honor the per-instance isolation ADR 0014 already decided and BDR 0008 Scenario 5 already specifies.
status: closed
labels: [controller, browse, cache, multi-instance, bugfix]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## B1 — multi-instance project-name isolation

Fixes a residual cross-instance collision in browse/mine list aggregation. Traces
to [BDR 0008](/bdr/0008-browse-list-refresh-cached-directory.md) Scenario 5
(per-instance isolation) and [ADR 0014](/adr/0014-browse-list-project-name-cache-swr.md)
(which mandates a per-instance cache, rejecting a global one). Recorded as
project memory obs 35 (refines obs 31).

### Problem

`ProjectNamesCache` is correctly per-instance, and `build_groups`
(`src/controller.rs:146`) already keys its groups on the composite
`(instance_name, project_id)`. But `tasks_by_project` (`src/controller.rs:28`)
merges every instance's `{project_id → name}` map into a single
`HashMap<i64, String>` via `project_names.extend(names)` — keyed by
`project_id` **alone**. When two instances both expose `project_id = N` with
**different** project names, `extend()` clobbers, so the last-joined instance's
name wins for **both** groups and `build_groups`' `project_names.get(&pid)`
resolves the wrong display name for the other instance's group.

Impact is **display-name only** — task grouping, click-routing, and asset
resolution are correct because they flow through the composite group key. The
existing per-instance-isolation unit test passes blind because it exercises the
cache read, not the end-to-end name resolution with colliding ids + differing
names.

### Decision

Key the in-memory name map by the composite `(instance_name, project_id)`,
mirroring `build_groups`' existing group key — completing the per-instance
isolation ADR 0014 already decided (no new architectural decision; the cache
already isolates, this makes the in-memory merge stop re-flattening it). No new
network calls, no schema change.

### Scope

Included: `tasks_by_project` builds `project_names: HashMap<(String, i64), String>`
(insert per `(inst_name, pid)` instead of `extend`); `build_groups` takes that map
and looks up `(instance_name, pid)`; the numeric-id fallback is unchanged.
Excluded: the cache shape (already per-instance), grouping/routing (already
composite), any UI/UX change.

### Acceptance

- Given two instances that both expose `project_id = N` with **different** names,
  When the browse list is built, Then each instance's group shows **its own**
  project name (no clobber) — a controller-level test with colliding ids +
  differing names that fails under the old `extend()` merge.
- A missing name still degrades to the numeric `project_id` (BDR 0008 Scenario 4
  preserved); single-instance behavior unchanged.
- Full suite green; clippy `-D warnings`, fmt, comment-policy clean; complexity
  within budget.

### Plan

Single slice (B1): `src/controller.rs` + `tests/unit/controller.rs`. Change the
`project_names` key type to `(String, i64)`, insert per instance in
`tasks_by_project`, update `build_groups`' signature + lookup, add the
colliding-ids/differing-names regression test.
