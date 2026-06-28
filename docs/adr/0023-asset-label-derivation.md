---
type: ADR
title: Derive the Anexos/Artefatos label from anchor text, then a real filename, then the host
description: Stop labeling a non-file link by its URL tail (e.g. "edit?tab=t.0"); derive an asset's display label as anchor text when present, else a real downloadable filename, else the URL host — never a query-string tail.
status: Accepted
supersedes:
superseded_by:
tags: [tui, detail, assets, ux]
timestamp: 2026-06-27T00:00:00Z
---

# 0023. Asset label: anchor text → real filename → host

## Context

The `Anexos/Artefatos` panel labels each extracted asset with a name derived by
`url_basename` (the rightmost URL path segment), shown as a filename when it passes
`looks_like_filename`, else the fallback `Open link`. For a Google Docs URL such as
`https://docs.google.com/document/d/…/edit?tab=t.0`, the rightmost segment is
`edit?tab=t.0`. Because `.0` reads as a 1-char extension, `looks_like_filename` accepts
it and the panel shows a meaningless `[1] ↗ edit?tab=t.0`. The operator reported the
artifact labels as "estranhos".

The basename heuristic is right for genuine file assets (`relatorio.pdf`,
`captura.png` — typically uploaded attachments) but wrong for web links, which are the
majority of pasted URLs and carry no meaningful trailing segment.

Force: **legibility of a read view** — the label must tell the operator *what* the link
is, not echo a query string. Presentation concern; extraction stays pure.

## Decision

Derive an asset's display label by the first rule that yields a meaningful string:

1. **Anchor text** — when the asset came from `<a href=…>text</a>` and `text` is
   non-empty and is not itself the bare URL, use `text` (e.g. `Especificação V1`).
2. **Real filename** — else, when the URL's last segment is a *genuine* downloadable
   filename, use it. Tighten `looks_like_filename`: reject any segment containing `?`,
   `=`, or `&` (query strings), require a dot with a **known-ish** extension (2–6
   alphabetic chars, not purely numeric like `.0`), and keep the existing length bound.
3. **Host** — else, use the URL host (`docs.google.com`, `drive.google.com`). A link
   with no parseable host falls back to the existing `Open link`.

The rule never emits a query-string tail. Asset **extraction** (which URLs become
assets) is unchanged ([ADR 0016](/adr/0016-refactor-render-decompose-relocate.md)); only
the **label** derivation changes.

## Alternatives considered

- **Always show the host for web links** (filename only for real files). Rejected: drops
  the most useful label we have — the anchor text a human wrote (`Especificação V1`) —
  in favor of a bare domain.
- **Middle-truncated URL** (`docs.google.com/…/edit`). Rejected: still noisy, and the
  ellipsis math competes with the panel's own wrapping; the host alone is cleaner and
  the full URL remains inline in the body (V5).

## Consequences

**Positive:** labels become meaningful — a human's link text when available, a true
filename for real attachments, a clean host otherwise; query tails never surface. The
`[N] ↗` affordance and click-to-open are unchanged.

**Accepted trade-offs:** two links to different pages on the same host show the same host
label; the operator disambiguates via the inline URL in the body (V5) or by opening.
Acceptable — the panel is an index, not the source of truth.

## Related

- ADR: [/adr/0016-refactor-render-decompose-relocate.md](/adr/0016-refactor-render-decompose-relocate.md) (asset extraction relocated; extraction unchanged here)
- ADR: [/adr/0020-body-links-inline-url-native-click.md](/adr/0020-body-links-inline-url-native-click.md) (full URL stays inline in the body)
- BDR: [/bdr/0017-asset-label-derivation.md](/bdr/0017-asset-label-derivation.md)
- Issue: [/issues/0022-detail-link-wrap-artifacts-project-title.md](/issues/0022-detail-link-wrap-artifacts-project-title.md)
