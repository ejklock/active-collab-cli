---
type: ADR
title: The task title renders as a "Título" row inside the Detalhes panel, not a loose header
description: Move the detail-view task title from a loose line floating above the bordered Detalhes box into a labeled "Título" meta row immediately after "Tarefa", so the title sits inside a frame and the Projeto value is shown alongside it.
status: Accepted
supersedes:
superseded_by:
tags: [tui, detail, layout, ux]
timestamp: 2026-06-27T00:00:00Z
---

# 0022. Detail task title as a Detalhes meta row

## Context

[ADR 0018](/adr/0018-detail-chrome-dynamic-height-wrap.md) moved the task name **off**
the un-wrappable ratatui frame title and rendered it as a separate header line so it
could wrap on narrow terminals. In practice that header now **floats loose above** the
bordered `Detalhes` box — it reads as an orphaned, unframed string, and the operator
reported it looks broken ("título solto lá em cima"). At the same time the `Projeto`
meta row renders **empty** (a separate defect, [BDR 0016](/bdr/0016-detail-title-row-project-name.md)),
so the top of the detail screen shows a stray title and a blank project.

The operator asked for the title to live **inside** the details, right after `Tarefa`,
and floated wrapping the whole thing in a card whose header carries the project.

Force: **usability/legibility of a read view** — the title is a primary field and
should sit in the same framed, labeled structure as the other fields. Presentation
concern; the pure TEA core stays pure.

## Decision

Render the task title as a labeled **`Título`** meta row **inside** the `Detalhes`
panel, immediately **after** the `Tarefa` row, and drop the loose header line above the
box. The `Projeto` row (populated per BDR 0016) follows. A meta row wraps naturally
within the panel, so ADR 0018's wrap-friendliness concern is preserved — the title is
no longer pinned to an un-wrappable frame title **nor** left unframed; it is an ordinary
wrapping row.

```
┌ Detalhes ───────────────────┐
│  Tarefa     725-71583       │
│  Título     OSV-Scanner     │
│  Projeto    Base · Sustent. │
│  Status     Aberto          │
│  Responsável Evaldo Klock   │
└─────────────────────────────┘
```

## Alternatives considered

- **Outer card wrapping the detail, header = "Projeto / Título".** Rejected: a second
  nested border around the existing `Detalhes` box adds chrome and horizontal indent on
  already-narrow terminals, and the card's frame title is the same un-wrappable element
  ADR 0018 deliberately moved away from.
- **In-card title bar above a divider (single box, title bar + meta rows).** Rejected:
  introduces a second text style/region to maintain and a custom divider; the labeled
  meta row reuses the existing row renderer with zero new layout primitives.
- **Keep the loose header, only fix the empty project.** Rejected: leaves the title
  unframed — the actual complaint.

## Consequences

**Positive:** the title sits in the same framed, labeled, wrapping structure as every
other field; the top of the screen no longer shows an orphaned string; reuses the
existing meta-row renderer (no new layout primitive). Refines ADR 0018's placement
without reintroducing the un-wrappable frame title.

**Accepted trade-offs:** the title loses its visual prominence as a standalone heading —
it is now one row among several. Acceptable: the task is identified by `Tarefa` + `Título`
together, and the panel label `Detalhes` frames the whole.

## Related

- ADR: [/adr/0018-detail-chrome-dynamic-height-wrap.md](/adr/0018-detail-chrome-dynamic-height-wrap.md) (title moved off frame title — placement refined here)
- BDR: [/bdr/0016-detail-title-row-project-name.md](/bdr/0016-detail-title-row-project-name.md)
- BDR: [/bdr/0012-detail-chrome-responsive-wrap.md](/bdr/0012-detail-chrome-responsive-wrap.md) (title-region wrapping)
- Issue: [/issues/0022-detail-link-wrap-artifacts-project-title.md](/issues/0022-detail-link-wrap-artifacts-project-title.md)
- Architecture: [/architecture.md](/architecture.md)
