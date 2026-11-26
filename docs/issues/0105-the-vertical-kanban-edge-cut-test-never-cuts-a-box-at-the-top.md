---
type: Issue
title: The vertical kanban edge-cut test never cuts a box at the top
description: vertical_boxes_keep_the_selected_card_on_screen_and_cut_the_box_at_the_edge scrolls with four Down presses, which starts the screen on a box's own top border, so its check that a cut box hides its title cannot fail.
status: open
timestamp: 2026-11-26T02:45:18Z
---

## 0105. The vertical kanban edge-cut test never cuts a box at the top

### Problem

`vertical_boxes_keep_the_selected_card_on_screen_and_cut_the_box_at_the_edge` (`tests/unit/tui_render.rs`) presses Down four times; the first visible slot is then the IN REVIEW box's own top border, so no box is cut at the top and `!contains("TO DO")` repeats the "Alpha todo" check. A mutant that draws a title row over a top-cut box passes the test; with six Down presses the READY FOR RELEASE box is cut at the top and the production code correctly hides its title. Found on 2026-11-25 in the review of issue 0100 slice S4; the owner accepted closing the slice and tracking this.

### Scope

Included: the test of a box cut at the top of the vertical kanban viewport.

Excluded: production drawing, which is correct.

### Acceptance

- AC1: a test scrolls until a box is cut at the top and asserts its title is absent, the selected card is fully visible and the outer bottom border is intact; a mutant drawing the title over the cut box fails it. (`verify_by: test`)
