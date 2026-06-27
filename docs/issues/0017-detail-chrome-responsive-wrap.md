---
type: Issue
title: "Detail chrome responsiveness — wrap header, task title, footer, and artifacts on narrow widths"
description: Make the Detail user-header bar, task-name header, status/hint footer, and Anexos/Artefatos rows word-wrap (with dynamic region heights) instead of truncating; relocate the task name off the un-wrappable frame title.
status: closed
labels: [tui, responsiveness, detail, wrap, view]
blocked_by:
tracker:
timestamp: 2026-06-26T00:00:00Z
---

## Detail chrome responsiveness

Reflow the four Detail chrome regions on narrow widths. Implements
[ADR 0018](/adr/0018-detail-chrome-dynamic-height-wrap.md); pins
[BDR 0012](/bdr/0012-detail-chrome-responsive-wrap.md). The body already wraps
(R3/U10); only the chrome is changed. Colors/data unchanged.

### Scope

Included: word-wrap + dynamic Layout heights for the user header bar and footer
(view.rs top-level layout) and for the task-name header and Anexos/Artefatos rows
(detail.rs), reusing `render::wrap_text`/`display_width`; relocating the task name
from the ratatui Block title into a wrapped bold header line. Excluded: the body
wrap (already done); list/browse screens; any color or data change.

### Acceptance

- Each chrome element wraps to the next line at narrow widths (no ellipsis/clip);
  the region height grows to fit (BDR 0012 S1–S4, S6).
- A wide terminal renders exactly as today: header/footer height 1, no element wraps
  (S5).
- The full task name is shown wrapped (no ellipsis) when it exceeds the inner width;
  a single-line name stays on one line (S2).
- Rendered-buffer (TestBackend) tests cover narrow + wide for each element; full
  suite green; clippy/fmt/comment-policy clean; complexity within budget.

### Plan (slices, persisted plan `detail-chrome-wrap`)

- **W1** — view-level chrome: wrap the **user header bar** and the **footer**
  (hint + Updated-at timestamp) with dynamic `Constraint::Length` from the wrapped
  line count (`src/tui/view.rs`, reusing `render::wrap_text`). Render tests.
- **W2** — detail-level chrome: relocate the **task name** from the Block title to a
  wrapped bold header line, and wrap the **Anexos/Artefatos** rows with a hanging
  indent + `asset_panel_height` counting wrapped rows (`src/tui/screens/detail.rs`).
  Render tests.
