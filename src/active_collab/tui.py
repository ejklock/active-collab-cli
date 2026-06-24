"""Interactive TUI browser for ActiveCollab tasks.

Provides:
- Pure navigation helpers (clamp_index, move_selection)
- Pure text helpers (_truncate, wrap_text)
- Color initialization (_init_colors, _attr)
- Frame drawing helpers (_safe_addstr, _draw_frame, _visible_window, _render_too_small)
- BrowseController — dependency-injected business logic
- MineController — aggregates tasks across multiple instances
- run_browser — thin curses view loop (not unit-tested; needs a terminal)
- run_mine_browser — flat-list TUI for mine/list subcommand
- run — entry point called by cli.py's `browse` subcommand
- run_mine — entry point called by cli.py's `mine` subcommand when TTY
"""

import curses
import json
import subprocess  # nosec B404
import sys
import tempfile
import textwrap
import webbrowser
from collections import OrderedDict
from typing import Callable

from active_collab import assets as assets_mod
from active_collab import gitbranch
from active_collab import render as _render
from active_collab.assets import Asset, extract_asset_urls
from active_collab.client import ActiveCollabClient
from active_collab.config import Config
from active_collab.http import HttpClient
from active_collab.i18n import __
from active_collab.models import Instance, MineTask
from active_collab.render import fmt_ts, html_to_text
from active_collab.store import InstanceRepository, Store

_BRANCH_TYPES = ("feature", "fix", "hotfix")
_DEFAULT_BRANCH_TYPE = "feature"

_BORDER = {
    "tl": "╭",
    "tr": "╮",
    "bl": "╰",
    "br": "╯",
    "h": "─",
    "v": "│",
    "ltee": "├",
    "rtee": "┤",
}

_MIN_HEIGHT = 6
_MIN_WIDTH = 24

# Populated by _init_colors() at TUI startup; keys are role names.
_ATTR: dict[str, int] = {}


def _truncate(text: str, width: int) -> str:
    """Return text clipped to width characters.

    When len(text) <= width, returns text unchanged.
    When len(text) > width, returns text[:width-1] + '…' (a single ellipsis char).
    When width <= 0, returns ''.
    """
    if width <= 0:
        return ""
    if len(text) <= width:
        return text
    return text[: width - 1] + "…"


def wrap_text(text: str, width: int) -> list[str]:
    """Wrap text to width, preserving blank lines and whole words.

    Returns [] when width <= 0. Splits on existing newlines and wraps each
    paragraph; blank lines yield an empty string in the result. No output
    line exceeds width characters.
    """
    if width <= 0:
        return []
    result: list[str] = []
    for paragraph in text.splitlines():
        if not paragraph.strip():
            result.append("")
        else:
            result.extend(textwrap.wrap(paragraph, width))
    return result


def _init_colors() -> None:
    """Initialize curses color pairs and cache attrs in _ATTR.

    Safe to call multiple times (idempotent). When the terminal does not
    support colors, returns without touching curses color state so callers
    fall back to the plain A_BOLD/A_REVERSE styling via _attr().
    """
    if not curses.has_colors():
        return
    curses.start_color()
    curses.use_default_colors()

    # Pair IDs: 1=header cyan, 2=selected (black on cyan), 3=status bar, 4=badge yellow
    curses.init_pair(1, curses.COLOR_CYAN, -1)
    curses.init_pair(2, curses.COLOR_BLACK, curses.COLOR_CYAN)
    curses.init_pair(3, curses.COLOR_WHITE, curses.COLOR_BLUE)
    curses.init_pair(4, curses.COLOR_YELLOW, -1)

    _ATTR["header"] = curses.color_pair(1) | curses.A_BOLD
    _ATTR["selected"] = curses.color_pair(2)
    _ATTR["status"] = curses.color_pair(3)
    _ATTR["badge"] = curses.color_pair(4)


def _attr(role: str, fallback: int) -> int:
    """Return the color attr for role, or fallback when colors are unavailable."""
    return _ATTR.get(role, fallback)


def clamp_index(index: int, length: int) -> int:
    """Return index clamped to [0, length-1]; 0 when length is 0."""
    if length == 0:
        return 0
    return max(0, min(index, length - 1))


def move_selection(index: int, delta: int, length: int) -> int:
    """Apply delta to index and clamp to [0, length-1]."""
    return clamp_index(index + delta, length)


def _safe_addstr(
    win: "curses._CursesWindow",
    y: int,
    x: int,
    text: str,
    attr: int = 0,
) -> None:
    """Write text to win at (y, x) with attr; silently ignore curses errors.

    Writes to the bottom-right cell or beyond the window boundary raise
    curses.error in most terminals — swallowing them is the standard pattern.
    """
    try:
        win.addstr(y, x, text, attr)
    except curses.error:
        pass


def _draw_frame(
    win: "curses._CursesWindow",
    top: int,
    left: int,
    height: int,
    width: int,
    title: str,
    attr: int = 0,
) -> None:
    """Draw a rounded box at (top, left) spanning height rows x width cols.

    Embeds ' {title} ' in the top border clipped to fit. Applies attr to all
    border characters.
    """
    bottom = top + height - 1
    right = left + width - 1

    _safe_addstr(win, top, left, _BORDER["tl"], attr)
    _safe_addstr(win, top, right, _BORDER["tr"], attr)
    _safe_addstr(win, bottom, left, _BORDER["bl"], attr)
    _safe_addstr(win, bottom, right, _BORDER["br"], attr)

    inner_width = width - 2
    top_fill = _BORDER["h"] * inner_width
    _safe_addstr(win, top, left + 1, top_fill, attr)
    _safe_addstr(win, bottom, left + 1, _BORDER["h"] * inner_width, attr)

    for row in range(top + 1, bottom):
        _safe_addstr(win, row, left, _BORDER["v"], attr)
        _safe_addstr(win, row, right, _BORDER["v"], attr)

    if inner_width >= 4:
        label = f" {title} "
        max_label = inner_width - 2
        label = _truncate(label, max_label)
        _safe_addstr(win, top, left + 2, label, attr)


def _visible_window(count: int, sel: int, height: int) -> int:
    """Return scroll offset so index sel is visible in a window of height rows.

    Returns 0 when count <= height (no scroll needed). Otherwise clamps the
    offset to [0, count-height] and guarantees sel is within [offset, offset+height-1].
    """
    if count <= height:
        return 0
    max_offset = count - height
    offset = sel - height + 1
    if offset < 0:
        offset = 0
    if offset > max_offset:
        offset = max_offset
    if sel < offset:
        offset = sel
    return offset


def _render_too_small(stdscr: "curses._CursesWindow") -> None:
    """Show a 'terminal too small' message; safe on any window size."""
    stdscr.erase()
    _safe_addstr(stdscr, 0, 0, __("Terminal too small"))
    _safe_addstr(stdscr, 1, 0, __("Resize to continue"))
    stdscr.refresh()


def _hint_bar(pairs: list[tuple[str, str]]) -> str:
    """Build a command hint bar string from (key, action_label) pairs.

    Each pair renders as '[key] label'; pairs are joined with two spaces.
    Action labels are routed through __() for i18n.
    """
    return "  ".join(f"[{key}] {__(label)}" for key, label in pairs)


def _comment_box(author: str, when: str, body: str, width: int) -> list[str]:
    """Return lines for a rounded sub-box of exactly `width` columns.

    Top border embeds ' {author} · {when} ' clipped to fit. Body lines are
    wrapped via wrap_text and each padded to fill the interior. No line
    exceeds `width` characters.
    """
    if width < 4:
        return []
    inner = width - 2
    header_text = f" {author} · {when} "
    max_header = inner - 2
    if len(header_text) > max_header:
        header_text = header_text[: max_header - 1] + "… "
    fill_left = _BORDER["h"] * 1
    fill_right = _BORDER["h"] * (inner - 1 - len(header_text))
    top = _BORDER["tl"] + fill_left + header_text + fill_right + _BORDER["tr"]

    body_lines = wrap_text(body, inner) or [""]
    middle = [
        _BORDER["v"] + line.ljust(inner) + _BORDER["v"]
        for line in body_lines
    ]
    bottom = _BORDER["bl"] + _BORDER["h"] * inner + _BORDER["br"]
    return [top, *middle, bottom]


def _detail_meta_section(
    meta_rows: list[tuple[str, str]],
    meta_text: str,
    inner_width: int,
) -> list[str]:
    """Return the meta table + blank + Description heading + wrapped body."""
    table = _meta_table(meta_rows, inner_width, __("Details"))
    body_lines = wrap_text(meta_text, inner_width) or [""]
    return [*table, "", __("Description") + ":", *body_lines]


def _detail_artifacts_section(asset_list: list, inner_width: int) -> list[str]:
    """Return a blank + artifacts panel when assets exist, otherwise []."""
    if not asset_list:
        return []
    panel = _artifacts_panel(asset_list, inner_width)
    return ["", *panel] if panel else []


def _detail_comment_section(comments: list, inner_width: int) -> list[str]:
    """Return a blank + comment boxes joined by blank separators."""
    if not comments:
        return []
    boxes: list[str] = []
    for idx, comment in enumerate(comments):
        author = str(
            comment.get("created_by_name") or comment.get("created_by_id") or "(unknown)"
        )
        when = fmt_ts(comment.get("created_on"))
        body_html = comment.get("body_plain_text") or html_to_text(comment.get("body") or "")
        box = _comment_box(author, when, body_html, inner_width)
        boxes.extend(box)
        if idx < len(comments) - 1:
            boxes.append("")
    return ["", *boxes]


def build_detail_lines(
    meta_text: str,
    comments: list,
    inner_width: int,
    meta_rows: list[tuple[str, str]] | None = None,
    asset_list: list | None = None,
) -> list[str]:
    """Build the full content lines for the detail view.

    Composes: meta table (when meta_rows provided), blank, Description heading +
    wrapped body, blank, artifacts panel (when asset_list provided), blank,
    comment boxes. Falls back to plain wrapped meta_text when meta_rows is None.
    """
    if meta_rows is not None:
        lines = _detail_meta_section(meta_rows, meta_text, inner_width)
    else:
        lines = list(wrap_text(meta_text, inner_width))

    lines.extend(_detail_artifacts_section(asset_list or [], inner_width))
    lines.extend(_detail_comment_section(comments, inner_width))
    return lines


def _render_list(
    stdscr: "curses._CursesWindow",
    items: list[str],
    sel: int,
    title: str,
) -> None:
    """Draw a rounded-frame list screen with title, items, and a hint bar.

    The outer frame uses _BORDER rounded corners. The title is embedded in
    the top border. A one-line hint bar sits just above the bottom border.
    The item list scrolls when it overflows the inner viewport. The selected
    item shows a ▸ marker with the 'selected' highlight; others get a plain
    two-space indent. All writes are routed through _safe_addstr.
    """
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    if h < _MIN_HEIGHT or w < _MIN_WIDTH:
        _render_too_small(stdscr)
        return

    border_attr = _attr("header", curses.A_BOLD)
    _draw_frame(stdscr, 0, 0, h, w, title, border_attr)

    # Inner content area: rows 1..(h-2), cols 1..(w-2)
    inner_top = 1
    inner_bottom = h - 2
    inner_left = 1
    inner_width = w - 2

    # Bottom hint bar sits at row (h-2), one row above the bottom border
    hint_row = inner_bottom
    # Item rows: inner_top .. hint_row-1
    item_area_height = hint_row - inner_top

    offset = _visible_window(len(items), sel, item_area_height)

    for slot in range(item_area_height):
        idx = offset + slot
        if idx >= len(items):
            break
        row = inner_top + slot
        if idx == sel:
            label = _truncate("▸ " + items[idx], inner_width)
            _safe_addstr(stdscr, row, inner_left, label, _attr("selected", curses.A_REVERSE))
        else:
            label = _truncate("  " + items[idx], inner_width)
            _safe_addstr(stdscr, row, inner_left, label)

    hint = _hint_bar([("↑↓", "move"), ("Enter", "select"), ("q", "quit"), ("b", "back")])
    status_attr = _attr("status", curses.A_NORMAL)
    _safe_addstr(stdscr, hint_row, inner_left, _truncate(hint, inner_width), status_attr)
    stdscr.refresh()


def _choose_branch_type(stdscr: "curses._CursesWindow") -> str | None:
    """Show a branch-type picker; return chosen type or None on cancel."""
    items = list(_BRANCH_TYPES)
    sel = 0
    while True:
        _render_list(
            stdscr, items, sel,
            __("Branch type (Enter to confirm, q cancel)"),
        )
        key = stdscr.getch()
        if key == curses.KEY_RESIZE:
            continue
        if key in (curses.KEY_UP, ord("k")):
            sel = move_selection(sel, -1, len(items))
        elif key in (curses.KEY_DOWN, ord("j")):
            sel = move_selection(sel, 1, len(items))
        elif key in (curses.KEY_ENTER, 10, 13):
            return items[sel]
        elif key in (ord("q"), ord("b"), 27):
            return None


def _asset_menu(
    stdscr: "curses._CursesWindow",
    assets: list[Asset],
    controller: "BrowseController",
) -> None:
    """Show asset list; handle open/download per item."""
    if not assets:
        return
    sel = 0
    while True:
        labels = [f"[{a.kind}] {a.name}" for a in assets]
        _render_list(stdscr, labels, sel, __("Assets"))
        key = stdscr.getch()
        if key == curses.KEY_RESIZE:
            continue
        if key in (curses.KEY_UP, ord("k")):
            sel = move_selection(sel, -1, len(assets))
        elif key in (curses.KEY_DOWN, ord("j")):
            sel = move_selection(sel, 1, len(assets))
        elif key == ord("o"):
            controller.open_asset(assets[sel])
        elif key == ord("d"):
            _handle_download(stdscr, controller, assets[sel])
        elif key in (ord("q"), ord("b"), 27):
            return


def _handle_download(
    stdscr: "curses._CursesWindow",
    controller: "BrowseController",
    asset: Asset,
) -> None:
    """Attempt to download asset and show the result or error message."""
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    try:
        path = controller.download_asset(asset)
        stdscr.addstr(0, 0, _truncate(f"{__('Downloaded:')} {path}", w - 1))
    except RuntimeError as exc:
        stdscr.addstr(0, 0, _truncate(f"{__('Error:')} {exc}", w - 1))
    stdscr.addstr(1, 0, __("Press any key..."))
    stdscr.refresh()
    stdscr.getch()


def _show_branch_result(
    stdscr: "curses._CursesWindow",
    result: gitbranch.BranchResult,
) -> None:
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    msg = f"{result.status.value}: {result.name}"
    if result.message:
        msg += f" — {result.message}"
    stdscr.addstr(0, 0, _truncate(msg, w - 1))
    stdscr.addstr(1, 0, __("Press any key..."))
    stdscr.refresh()
    stdscr.getch()


def _screen_projects(
    stdscr: "curses._CursesWindow",
    project_names: list[str],
    groups: list,
    proj_sel: int,
    task_sel: int,
) -> tuple[int, int, str | None]:
    """Render + handle one keypress on the projects screen.

    Returns (proj_sel, task_sel, next_screen) where next_screen is
    None to signal quit.
    """
    _render_list(stdscr, project_names, proj_sel, __("Projects"))
    key = stdscr.getch()
    if key == curses.KEY_RESIZE:
        return proj_sel, task_sel, "projects"
    if key in (curses.KEY_UP, ord("k")):
        proj_sel = move_selection(proj_sel, -1, len(project_names))
    elif key in (curses.KEY_DOWN, ord("j")):
        proj_sel = move_selection(proj_sel, 1, len(project_names))
    elif key in (curses.KEY_ENTER, 10, 13) and groups:
        task_sel = 0
        return proj_sel, task_sel, "tasks"
    elif key in (ord("q"), 27):
        return proj_sel, task_sel, None
    return proj_sel, task_sel, "projects"


def _screen_tasks(
    stdscr: "curses._CursesWindow",
    groups: list,
    proj_sel: int,
    task_sel: int,
) -> tuple[int, str | None]:
    """Render + handle one keypress on the tasks screen.

    Returns (task_sel, next_screen) where next_screen is None to
    signal quit.
    """
    _, task_list = groups[proj_sel]
    task_labels = [
        f"#{t.task_number or t.id}  {t.name}" for t in task_list
    ]
    _render_list(stdscr, task_labels, task_sel, __("Tasks"))
    key = stdscr.getch()
    if key == curses.KEY_RESIZE:
        return task_sel, "tasks"
    if key in (curses.KEY_UP, ord("k")):
        task_sel = move_selection(task_sel, -1, len(task_list))
    elif key in (curses.KEY_DOWN, ord("j")):
        task_sel = move_selection(task_sel, 1, len(task_list))
    elif key in (curses.KEY_ENTER, 10, 13) and task_list:
        return task_sel, "detail"
    elif key in (ord("b"), 27):
        return task_sel, "projects"
    elif key in (ord("q"),):
        return task_sel, None
    return task_sel, "tasks"


def _meta_rows(task_dict: dict) -> list[tuple[str, str]]:
    """Return (label, value) pairs for the meta grid; Name stays in the frame title."""
    assignee_id = task_dict.get("assignee_id")
    if assignee_id is None:
        assignee_label = __("(unassigned)")
    else:
        assignee_label = f"({assignee_id})"

    status_val = __("Completed") if task_dict.get("is_completed") else __("Open")
    num = task_dict.get("task_number") or task_dict.get("id", "")

    rows: list[tuple[str, str]] = [
        (__("Task"), f"#{num}"),
        (__("Status"), status_val),
        (__("Assignee"), assignee_label),
    ]

    start = _render.fmt_date(task_dict.get("start_on"))
    if start:
        rows.append((__("Start"), start))

    due = _render.fmt_date(task_dict.get("due_on"))
    if due:
        rows.append((__("Due"), due))

    rows.append((__("Estimate"), f"{_render.fmt_hours(task_dict.get('estimate'))}h"))
    rows.append((__("Logged"), f"{_render.fmt_hours(task_dict.get('tracked_time'))}h"))
    return rows


def _meta_table(rows: list[tuple[str, str]], width: int, title: str) -> list[str]:
    """Return a full-grid rounded bordered table with label and value columns.

    A ├──┼──┤ separator row appears between every field. The title is embedded
    in the top border. Values are truncated with … to fit. Returns [] when width
    is too narrow to be useful (< 10).
    """
    if width < 10:
        return []

    label_col = max((len(label) for label, _ in rows), default=0)
    label_col = min(label_col, width // 3)
    # Data row: │ {label:<label_col} │ {val:<val_col} │
    # = 1 + 1 + label_col + 1 + 1 + 1 + val_col + 1 + 1 = label_col + val_col + 7
    val_col = width - label_col - 7

    if val_col < 1:
        return []

    h = _BORDER["h"]
    v = _BORDER["v"]
    ltee = _BORDER["ltee"]
    rtee = _BORDER["rtee"]
    cross = "┼"

    label_fill = h * (label_col + 2)
    val_fill = h * (val_col + 2)

    top_inner = h * (width - 2)
    if len(title) + 4 <= width - 2:
        label_str = f" {title} "
        top_inner = h + label_str + h * (width - 4 - len(label_str))
    top = _BORDER["tl"] + top_inner + _BORDER["tr"]

    sep = ltee + label_fill + cross + val_fill + rtee

    result = [top]
    for idx, (label, value) in enumerate(rows):
        padded_label = label.ljust(label_col)
        truncated_val = _truncate(value, val_col)
        line = f"{v} {padded_label} {v} {truncated_val.ljust(val_col)} {v}"
        result.append(line)
        if idx < len(rows) - 1:
            result.append(sep)

    bottom = _BORDER["bl"] + h * (width - 2) + _BORDER["br"]
    result.append(bottom)
    return result


def _artifacts_panel(assets: list, width: int) -> list[str]:
    """Return a rounded 'Artifacts' box with '[n] name' and indented URL per asset.

    Returns [] when assets is empty or width is too narrow (< 8).
    No line exceeds width characters.
    """
    if not assets or width < 8:
        return []

    inner = width - 2
    title = __("Artifacts")
    title_str = f" {title} "
    if len(title_str) + 1 <= inner:
        right_fill = _BORDER["h"] * (inner - 1 - len(title_str))
        top_fill = _BORDER["h"] + title_str + right_fill
    else:
        top_fill = _BORDER["h"] * inner
    top = _BORDER["tl"] + top_fill + _BORDER["tr"]
    bottom = _BORDER["bl"] + _BORDER["h"] * inner + _BORDER["br"]

    lines = [top]
    for idx, asset in enumerate(assets, 1):
        label = _truncate(f"[{idx}] {asset.name}", inner)
        lines.append(_BORDER["v"] + label.ljust(inner) + _BORDER["v"])
        url_line = _truncate(f"  {asset.url}", inner)
        lines.append(_BORDER["v"] + url_line.ljust(inner) + _BORDER["v"])
    lines.append(bottom)
    return lines


def _detail_meta_text(task_dict: dict) -> str:
    """Return the description block only (meta is now rendered as a grid table)."""
    return html_to_text(task_dict.get("body") or "") or __("(no description)")


def _render_detail_frame(
    stdscr: "curses._CursesWindow",
    content: list[str],
    offset: int,
    title: str,
    has_assets: bool = False,
) -> None:
    """Draw the framed detail view with content scrolled to offset."""
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    if h < _MIN_HEIGHT or w < _MIN_WIDTH:
        _render_too_small(stdscr)
        return

    border_attr = _attr("header", curses.A_BOLD)
    _draw_frame(stdscr, 0, 0, h, w, title, border_attr)

    inner_top = 1
    inner_left = 1
    inner_width = w - 2
    hint_row = h - 2
    viewport = hint_row - inner_top

    for slot in range(viewport):
        line_idx = offset + slot
        if line_idx >= len(content):
            break
        _safe_addstr(
            stdscr,
            inner_top + slot,
            inner_left,
            content[line_idx][:inner_width].ljust(inner_width),
        )

    hint_pairs = [
        ("q", "back"), ("c", "branch"), ("a", "assets"), ("↑↓", "scroll"), ("⇞⇟", "page"),
    ]
    if has_assets:
        hint_pairs.insert(3, ("1-9", "open"))
    hint = _hint_bar(hint_pairs)
    status_attr = _attr("status", curses.A_NORMAL)
    _safe_addstr(stdscr, hint_row, inner_left, _truncate(hint, inner_width), status_attr)
    stdscr.refresh()


def _scroll_offset(key: int, offset: int, max_offset: int, viewport: int) -> int:
    """Return the new scroll offset for scroll-key presses, clamped to [0, max_offset].

    Handles KEY_UP/k (−1), KEY_DOWN/j (+1), KEY_PPAGE (−viewport), KEY_NPAGE
    (+viewport). Any other key leaves offset unchanged.
    """
    if key in (curses.KEY_UP, ord("k")):
        return max(0, offset - 1)
    if key in (curses.KEY_DOWN, ord("j")):
        return min(max_offset, offset + 1)
    if key == curses.KEY_PPAGE:
        return max(0, offset - viewport)
    if key == curses.KEY_NPAGE:
        return min(max_offset, offset + viewport)
    return offset


def _open_asset_by_digit(key: int, asset_list: list, controller: "BrowseController") -> None:
    """Open the asset at index (key - ord('1')) via controller.open_asset; no-op if out of range."""
    idx = key - ord("1")
    if 0 <= idx < len(asset_list):
        controller.open_asset(asset_list[idx])


def _handle_detail_key(
    key: int,
    stdscr: "curses._CursesWindow",
    controller: "BrowseController",
    project_id: int,
    task: MineTask,
    asset_list: list,
) -> str | None:
    """Dispatch non-scroll action keys in the detail loop.

    Returns 'quit' when the detail loop should exit, 'handled' when the key
    was consumed by a non-scroll action, and None when the key should be
    forwarded to _scroll_offset.
    """
    if key in (ord("q"), ord("b"), 27):
        return "quit"
    if key == ord("c"):
        chosen = _choose_branch_type(stdscr)
        if chosen:
            result = controller.create_task_branch(chosen, project_id, task.id)
            _show_branch_result(stdscr, result)
        return "handled"
    if key == ord("a"):
        _asset_menu(stdscr, asset_list, controller)
        return "handled"
    if ord("1") <= key <= ord("9"):
        _open_asset_by_digit(key, asset_list, controller)
        return "handled"
    return None


def _render_and_handle_detail(
    stdscr: "curses._CursesWindow",
    controller: "BrowseController",
    project_id: int,
    task: MineTask,
) -> None:
    """Render task detail with framed layout and vertical scroll; loop until exit.

    Keys: q/b/Esc return; ↑/k scroll up; ↓/j scroll down; PgUp/PgDn page;
    c branch picker + create; a asset menu; 1-9 open Nth asset; KEY_RESIZE re-renders.
    """
    task_dict, comments, asset_list = controller.task_detail(project_id, task.id)
    num = task_dict.get("task_number") or task_dict.get("id", "")
    name = task_dict.get("name", "")
    title = f"#{num} — {name}"
    desc_text = _detail_meta_text(task_dict)
    rows = _meta_rows(task_dict)

    offset = 0

    while True:
        h, w = stdscr.getmaxyx()
        inner_width = max(1, w - 2)
        content = build_detail_lines(desc_text, comments, inner_width, rows, asset_list)
        viewport = max(1, h - _MIN_HEIGHT + 2)
        max_offset = max(0, len(content) - viewport)

        _render_detail_frame(stdscr, content, offset, title, has_assets=bool(asset_list))
        key = stdscr.getch()

        action = _handle_detail_key(key, stdscr, controller, project_id, task, asset_list)
        if action == "quit":
            return
        if action is None:
            if key == curses.KEY_RESIZE:
                offset = min(offset, max_offset)
            else:
                offset = _scroll_offset(key, offset, max_offset, viewport)


def _screen_detail(
    stdscr: "curses._CursesWindow",
    groups: list,
    proj_sel: int,
    task_sel: int,
    controller: "BrowseController",
) -> str:
    """Render + handle detail screen for the project-browse flow.

    Returns the next screen name (always a string; no quit from detail).
    Delegates detail rendering and key handling to _render_and_handle_detail,
    then signals the browse loop to return to the tasks screen.
    """
    _, task_list = groups[proj_sel]
    task = task_list[task_sel]
    project_id = task.project_id or 0
    _render_and_handle_detail(stdscr, controller, project_id, task)
    return "tasks"


class BrowseController:
    """All TUI business logic; fully dependency-injected for unit testing."""

    def __init__(
        self,
        client: ActiveCollabClient,
        http: HttpClient,
        instance: Instance,
        run: Callable = subprocess.run,
        opener: Callable = webbrowser.open,
        download_dir: str | None = None,
    ) -> None:
        self._client = client
        self._http = http
        self._instance = instance
        self._run = run
        self._opener = opener
        self._download_dir = download_dir or tempfile.gettempdir()

    def tasks_by_project(
        self,
    ) -> list[tuple[str, list[MineTask]]]:
        """Return tasks grouped by project, ordered by project name.

        Calls fetch_open_tasks and list_projects; falls back to the
        project_id string when a project name is unavailable.
        """
        open_tasks = self._client.fetch_open_tasks()
        project_map = self._build_project_name_map()

        grouped: OrderedDict[int, list[MineTask]] = OrderedDict()
        for task in open_tasks:
            pid = task.project_id or 0
            grouped.setdefault(pid, []).append(task)

        result: list[tuple[str, list[MineTask]]] = []
        for pid, task_list in grouped.items():
            name = project_map.get(pid) or str(pid)
            result.append((name, task_list))

        return result

    def _build_project_name_map(self) -> dict[int, str]:
        status, body = self._client.list_projects()
        if status != 200:
            return {}
        try:
            data = json.loads(body)
        except (ValueError, TypeError):
            return {}
        projects = data if isinstance(data, list) else []
        return {
            p["id"]: p.get("name") or str(p["id"])
            for p in projects
            if isinstance(p, dict) and p.get("id")
        }

    def task_detail(
        self, project_id: int, task_id: int
    ) -> tuple[dict, list, list[Asset]]:
        """Return (task_dict, comments, assets) for the given task."""
        status, payload = self._client.fetch_task(project_id, task_id)
        if status != 200 or not payload:
            return {}, [], []
        task = payload.get("single") or {}
        task["tracked_time"] = payload.get("tracked_time")
        comments = payload.get("comments") or []
        asset_list = extract_asset_urls(task, comments)
        return task, comments, asset_list

    def fetch_open_tasks(self) -> list[MineTask]:
        """Return open tasks from the underlying client."""
        return self._client.fetch_open_tasks()

    def create_task_branch(
        self,
        branch_type: str,
        project_id: int,
        task_id: int,
    ) -> gitbranch.BranchResult:
        """Build and create a git branch; delegates entirely to gitbranch."""
        name = gitbranch.build_branch_name(branch_type, project_id, task_id)
        return gitbranch.create_branch(name, run=self._run, base="master")

    def open_asset(self, asset: Asset) -> None:
        """Open the asset URL in the browser; never attaches the auth token."""
        self._opener(asset.url)

    def download_asset(self, asset: Asset, dest_dir: str | None = None) -> str:
        """Download the asset; attaches auth token only on host match."""
        directory = dest_dir or self._download_dir
        return assets_mod.download_asset(
            self._http,
            asset,
            self._instance.base_url,
            self._instance.token,
            directory,
        )


class MineController:
    """Aggregates open tasks from multiple instances for the mine TUI."""

    def __init__(
        self,
        instances: list[Instance],
        http: HttpClient,
        run: Callable = subprocess.run,
        opener: Callable = webbrowser.open,
        download_dir: str | None = None,
    ) -> None:
        self._controllers: dict[str, BrowseController] = {
            inst.name: BrowseController(
                client=ActiveCollabClient(inst, http),
                http=http,
                instance=inst,
                run=run,
                opener=opener,
                download_dir=download_dir,
            )
            for inst in instances
        }

    def my_tasks(self) -> list[MineTask]:
        """Return open tasks aggregated across all instances.

        Preserves instance insertion order, then task order within each instance.
        """
        result: list[MineTask] = []
        for ctrl in self._controllers.values():
            result.extend(ctrl.fetch_open_tasks())
        return result

    def controller_for(self, task: MineTask) -> BrowseController:
        """Return the BrowseController bound to the task's instance."""
        return self._controllers[task.instance_name]


def run_browser(
    stdscr: "curses._CursesWindow",
    controller: BrowseController,
) -> None:
    """Thin curses view loop; all business logic lives in controller."""
    curses.curs_set(0)
    _init_colors()

    groups = controller.tasks_by_project()
    project_names = [name for name, _ in groups]

    proj_sel = 0
    task_sel = 0
    screen: str | None = "projects"

    while screen is not None:
        if screen == "projects":
            proj_sel, task_sel, screen = _screen_projects(
                stdscr, project_names, groups, proj_sel, task_sel
            )
        elif screen == "tasks":
            task_sel, screen = _screen_tasks(
                stdscr, groups, proj_sel, task_sel
            )
        elif screen == "detail":
            screen = _screen_detail(
                stdscr, groups, proj_sel, task_sel, controller
            )


def _screen_mine_list(
    stdscr: "curses._CursesWindow",
    tasks: list[MineTask],
    sel: int,
) -> tuple[int, str | None]:
    """Render the flat mine-task list and handle one keypress.

    Returns (sel, action) where action is 'detail', None (quit), or 'list' (stay).
    """
    labels = [
        f"[{t.instance_name}] #{t.task_number or t.id}  {t.name}"
        for t in tasks
    ]
    _render_list(stdscr, labels, sel, __("My Open Tasks"))
    key = stdscr.getch()
    if key == curses.KEY_RESIZE:
        return sel, "list"
    if key in (curses.KEY_UP, ord("k")):
        return move_selection(sel, -1, len(tasks)), "list"
    if key in (curses.KEY_DOWN, ord("j")):
        return move_selection(sel, 1, len(tasks)), "list"
    if key in (curses.KEY_ENTER, 10, 13) and tasks:
        return sel, "detail"
    if key in (ord("q"), 27):
        return sel, None
    return sel, "list"


def run_mine_browser(
    stdscr: "curses._CursesWindow",
    mine_controller: MineController,
) -> None:
    """Curses view loop for the mine flat-list TUI."""
    curses.curs_set(0)
    _init_colors()

    tasks = mine_controller.my_tasks()
    if not tasks:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        msg = __("No open tasks assigned to you.")
        stdscr.addstr(0, 0, _truncate(msg, w - 1))
        stdscr.addstr(1, 0, __("Press any key to exit..."))
        stdscr.refresh()
        stdscr.getch()
        return

    sel = 0
    screen: str | None = "list"
    while screen is not None:
        if screen == "list":
            sel, screen = _screen_mine_list(stdscr, tasks, sel)
        elif screen == "detail":
            task = tasks[sel]
            controller = mine_controller.controller_for(task)
            _render_and_handle_detail(stdscr, controller, task.project_id or 0, task)
            screen = "list"


def run_mine(
    instances: list[Instance],
    http: HttpClient,
    run: Callable = subprocess.run,
    opener: Callable = webbrowser.open,
    download_dir: str | None = None,
) -> int:
    """Launch the interactive mine TUI. Caller must have verified TTY."""
    controller = MineController(instances, http, run=run, opener=opener, download_dir=download_dir)
    curses.wrapper(run_mine_browser, controller)
    return 0


def _resolve_browse_instance(
    instances: list[Instance],
    instance_name: str | None,
) -> tuple[Instance | None, str | None]:
    """Resolve which instance to use for the browse command.

    Returns (instance, None) on success, or (None, error_message) on failure.
    Failure cases: no instances configured, unknown name, multiple instances
    with no name given.
    """
    if not instances:
        return None, __("Error: no instances configured. Run: active-collab setup add")
    if instance_name:
        return _resolve_named_instance(instances, instance_name)
    return _resolve_implicit_instance(instances)


def _resolve_named_instance(
    instances: list[Instance],
    name: str,
) -> tuple[Instance | None, str | None]:
    matches = [i for i in instances if i.name == name]
    if not matches:
        known = ", ".join(i.name for i in instances)
        return None, __("Error: instance '{name}' not found. Known: {known}").format(
            name=name, known=known
        )
    return matches[0], None


def _resolve_implicit_instance(
    instances: list[Instance],
) -> tuple[Instance | None, str | None]:
    if len(instances) == 1:
        return instances[0], None
    names = ", ".join(i.name for i in instances)
    return None, __("Error: multiple instances ({names}). Use --instance NAME.").format(
        names=names
    )


def run(args: object) -> int:
    """Entry point for the `browse` subcommand; builds deps and launches TUI."""
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        _render.print_error(
            __("Error: 'browse' requires an interactive terminal (TTY).")
        )
        return 2

    config = Config.load()
    store = Store(config)
    repo = InstanceRepository(store.conn)
    instances = repo.load_all()

    inst, error = _resolve_browse_instance(instances, getattr(args, "instance", None))
    if error:
        _render.print_error(error)
        return 2

    http = HttpClient()
    client = ActiveCollabClient(inst, http)
    controller = BrowseController(
        client=client,
        http=http,
        instance=inst,
    )
    curses.wrapper(run_browser, controller)
    return 0
