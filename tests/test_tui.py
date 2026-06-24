"""Tests for tui.py: navigation helpers and BrowseController (no curses)."""

import contextlib
import curses
import io
import json
import types
import unittest
from dataclasses import dataclass
from typing import Callable
from unittest.mock import MagicMock, patch

from active_collab.assets import Asset
from active_collab.gitbranch import BranchResult, BranchStatus
from active_collab.models import Instance, MineTask
from active_collab.tui import (
    BrowseController,
    MineController,
    _artifacts_panel,
    _comment_box,
    _draw_frame,
    _hint_bar,
    _init_colors,
    _meta_rows,
    _meta_table,
    _open_asset_by_digit,
    _render_and_handle_detail,
    _render_list,
    _render_too_small,
    _resolve_browse_instance,
    _safe_addstr,
    _screen_mine_list,
    _scroll_offset,
    _truncate,
    _visible_window,
    build_detail_lines,
    clamp_index,
    move_selection,
    wrap_text,
)


@dataclass
class FakeRunResult:
    returncode: int
    stdout: str = ""
    stderr: str = ""


def _fake_run_factory(
    responses: dict[tuple, FakeRunResult],
) -> Callable:
    calls: list[list] = []

    def fake_run(argv: list, **_kwargs) -> FakeRunResult:
        calls.append(list(argv))
        key = tuple(argv)
        return responses.get(key, FakeRunResult(returncode=1, stderr="unexpected"))

    fake_run.calls = calls  # type: ignore[attr-defined]
    return fake_run


def _make_instance(base_url: str = "https://collab.example.com", token: str = "tok") -> Instance:
    return Instance(
        name="test",
        base_url=base_url,
        email="user@example.com",
        token=token,
        user_id=42,
    )


def _make_mine_task(
    task_id: int = 100,
    project_id: int = 10,
    name: str = "My Task",
    task_number: int | None = 5,
) -> MineTask:
    return MineTask(
        id=task_id,
        task_number=task_number,
        name=name,
        is_completed=False,
        is_trashed=False,
        project_id=project_id,
        instance_name="test",
    )


class FakeClient:
    """Test double for ActiveCollabClient."""

    def __init__(
        self,
        open_tasks: list[MineTask] | None = None,
        projects_status: int = 200,
        projects_body: bytes | None = None,
        task_status: int = 200,
        task_payload: dict | None = None,
    ) -> None:
        self._open_tasks = open_tasks or []
        self._projects_status = projects_status
        self._projects_body = projects_body or b"[]"
        self._task_status = task_status
        self._task_payload = task_payload or {}

    def fetch_open_tasks(self) -> list[MineTask]:
        return self._open_tasks

    def list_projects(self) -> tuple[int, bytes]:
        return self._projects_status, self._projects_body

    def fetch_task(self, project_id: int, task_id: int) -> tuple[int, dict | None]:
        if self._task_status != 200:
            return self._task_status, None
        return self._task_status, self._task_payload


class TestClampIndex(unittest.TestCase):
    def test_empty_list_returns_zero(self) -> None:
        self.assertEqual(clamp_index(0, 0), 0)

    def test_empty_list_any_index_returns_zero(self) -> None:
        self.assertEqual(clamp_index(5, 0), 0)

    def test_first_element_stays_at_zero(self) -> None:
        self.assertEqual(clamp_index(0, 5), 0)

    def test_last_element_stays_at_length_minus_one(self) -> None:
        self.assertEqual(clamp_index(4, 5), 4)

    def test_past_end_clamped_to_last(self) -> None:
        self.assertEqual(clamp_index(10, 5), 4)

    def test_negative_clamped_to_zero(self) -> None:
        self.assertEqual(clamp_index(-3, 5), 0)

    def test_mid_range_unchanged(self) -> None:
        self.assertEqual(clamp_index(2, 5), 2)

    def test_single_element_any_index_returns_zero(self) -> None:
        self.assertEqual(clamp_index(99, 1), 0)


class TestMoveSelection(unittest.TestCase):
    def test_move_down_increments(self) -> None:
        self.assertEqual(move_selection(0, 1, 5), 1)

    def test_move_up_decrements(self) -> None:
        self.assertEqual(move_selection(3, -1, 5), 2)

    def test_move_past_end_stays_at_last(self) -> None:
        self.assertEqual(move_selection(4, 1, 5), 4)

    def test_move_before_start_stays_at_zero(self) -> None:
        self.assertEqual(move_selection(0, -1, 5), 0)

    def test_empty_list_returns_zero(self) -> None:
        self.assertEqual(move_selection(0, 1, 0), 0)

    def test_large_delta_clamped(self) -> None:
        self.assertEqual(move_selection(2, 100, 5), 4)

    def test_large_negative_delta_clamped(self) -> None:
        self.assertEqual(move_selection(2, -100, 5), 0)


class TestTruncate(unittest.TestCase):
    def test_text_shorter_than_width_returned_unchanged(self) -> None:
        self.assertEqual(_truncate("hello", 10), "hello")

    def test_text_equal_to_width_returned_unchanged(self) -> None:
        self.assertEqual(_truncate("hello", 5), "hello")

    def test_text_longer_than_width_truncated_with_ellipsis(self) -> None:
        result = _truncate("hello world", 8)
        self.assertEqual(result, "hello w…")

    def test_overflow_result_length_equals_width(self) -> None:
        result = _truncate("abcdefghij", 6)
        self.assertEqual(len(result), 6)

    def test_overflow_ends_with_ellipsis_char(self) -> None:
        result = _truncate("abcdefghij", 6)
        self.assertTrue(result.endswith("…"))

    def test_width_zero_returns_empty_string(self) -> None:
        self.assertEqual(_truncate("hello", 0), "")

    def test_negative_width_returns_empty_string(self) -> None:
        self.assertEqual(_truncate("hello", -5), "")

    def test_empty_text_returned_unchanged(self) -> None:
        self.assertEqual(_truncate("", 10), "")

    def test_width_one_returns_ellipsis_on_overflow(self) -> None:
        result = _truncate("ab", 1)
        self.assertEqual(result, "…")
        self.assertEqual(len(result), 1)

    def test_width_one_returns_single_char_when_fits(self) -> None:
        self.assertEqual(_truncate("a", 1), "a")


class TestWrapText(unittest.TestCase):
    def test_width_zero_returns_empty(self) -> None:
        self.assertEqual(wrap_text("hello world", 0), [])

    def test_negative_width_returns_empty(self) -> None:
        self.assertEqual(wrap_text("hello world", -5), [])

    def test_short_text_fits_in_one_line(self) -> None:
        result = wrap_text("hello", 20)
        self.assertEqual(result, ["hello"])

    def test_no_line_exceeds_width(self) -> None:
        long_text = "word " * 30
        result = wrap_text(long_text, 20)
        for line in result:
            self.assertLessEqual(len(line), 20, f"Line too long: {line!r}")

    def test_preserves_blank_lines(self) -> None:
        text = "first\n\nsecond"
        result = wrap_text(text, 40)
        self.assertIn("", result)
        self.assertIn("first", result)
        self.assertIn("second", result)

    def test_blank_line_between_paragraphs_preserved(self) -> None:
        text = "para one\n\npara two"
        result = wrap_text(text, 40)
        blank_index = result.index("")
        self.assertGreater(blank_index, 0)
        self.assertLess(blank_index, len(result) - 1)

    def test_wraps_at_word_boundary(self) -> None:
        result = wrap_text("hello world foo bar", 10)
        for line in result:
            self.assertLessEqual(len(line), 10)
            self.assertNotIn("  ", line)

    def test_empty_string_returns_empty_list(self) -> None:
        result = wrap_text("", 20)
        self.assertEqual(result, [])

    def test_single_very_long_word_fits_width_constraint(self) -> None:
        result = wrap_text("abcdefghij", 5)
        for line in result:
            self.assertLessEqual(len(line), 5)

    def test_multiple_paragraphs_each_wrapped_independently(self) -> None:
        text = "short\n\n" + "word " * 20
        result = wrap_text(text, 15)
        for line in result:
            self.assertLessEqual(len(line), 15)


class TestVisibleWindow(unittest.TestCase):
    def test_count_fits_returns_zero(self) -> None:
        self.assertEqual(_visible_window(5, 0, 10), 0)

    def test_count_equals_height_returns_zero(self) -> None:
        self.assertEqual(_visible_window(10, 5, 10), 0)

    def test_sel_at_start_returns_zero(self) -> None:
        self.assertEqual(_visible_window(20, 0, 5), 0)

    def test_sel_near_end_scrolls_to_show_it(self) -> None:
        offset = _visible_window(20, 19, 5)
        self.assertGreaterEqual(19, offset)
        self.assertLess(19, offset + 5)

    def test_sel_in_middle_is_visible(self) -> None:
        offset = _visible_window(20, 10, 5)
        self.assertGreaterEqual(10, offset)
        self.assertLess(10, offset + 5)

    def test_offset_clamped_to_zero_minimum(self) -> None:
        offset = _visible_window(10, 0, 5)
        self.assertGreaterEqual(offset, 0)

    def test_offset_clamped_to_count_minus_height_maximum(self) -> None:
        offset = _visible_window(20, 19, 5)
        self.assertLessEqual(offset, 20 - 5)

    def test_sel_visible_for_all_positions(self) -> None:
        count, height = 15, 5
        for sel in range(count):
            offset = _visible_window(count, sel, height)
            self.assertGreaterEqual(sel, offset, f"sel={sel} not >= offset={offset}")
            self.assertLess(sel, offset + height, f"sel={sel} not < offset+height={offset+height}")

    def test_zero_count_returns_zero(self) -> None:
        self.assertEqual(_visible_window(0, 0, 5), 0)


class FakeWindow:
    """Fake curses window that records addstr calls; raises curses.error on demand."""

    def __init__(self, height: int = 24, width: int = 80, raise_on: set | None = None) -> None:
        self._height = height
        self._width = width
        self._raise_on: set = raise_on or set()
        self.calls: list[tuple] = []
        self._erased = False

    def erase(self) -> None:
        self._erased = True

    def getmaxyx(self) -> tuple[int, int]:
        return self._height, self._width

    def addstr(self, y: int, x: int, text: str, attr: int = 0) -> None:
        key = (y, x)
        if key in self._raise_on:
            raise curses.error("simulated write error")
        self.calls.append((y, x, text, attr))

    def refresh(self) -> None:
        pass

    def getch(self) -> int:
        return ord("q")

    def texts_at(self, row: int) -> list[str]:
        return [text for r, _c, text, _a in self.calls if r == row]

    def all_text(self) -> str:
        return " ".join(text for _r, _c, text, _a in self.calls)


class TestSafeAddstr(unittest.TestCase):
    def test_normal_write_is_recorded(self) -> None:
        win = FakeWindow()
        _safe_addstr(win, 0, 0, "hello")
        self.assertEqual(len(win.calls), 1)
        self.assertEqual(win.calls[0][2], "hello")

    def test_curses_error_is_silently_suppressed(self) -> None:
        win = FakeWindow(raise_on={(0, 0)})
        try:
            _safe_addstr(win, 0, 0, "boom")
        except curses.error:
            self.fail("_safe_addstr must not propagate curses.error")

    def test_attr_is_passed_through(self) -> None:
        win = FakeWindow()
        _safe_addstr(win, 1, 2, "txt", 99)
        self.assertEqual(win.calls[0][3], 99)

    def test_coordinates_are_correct(self) -> None:
        win = FakeWindow()
        _safe_addstr(win, 3, 7, "x")
        row, col, _text, _attr = win.calls[0]
        self.assertEqual(row, 3)
        self.assertEqual(col, 7)


class TestDrawFrame(unittest.TestCase):
    def _corners(self, win: FakeWindow) -> dict[str, str]:
        by_pos = {(r, c): text for r, c, text, _a in win.calls}
        h, w = win.getmaxyx()
        return {
            "tl": by_pos.get((0, 0), ""),
            "tr": by_pos.get((0, w - 1), ""),
            "bl": by_pos.get((h - 1, 0), ""),
            "br": by_pos.get((h - 1, w - 1), ""),
        }

    def test_top_left_corner_is_rounded(self) -> None:
        win = FakeWindow(height=10, width=30)
        _draw_frame(win, 0, 0, 10, 30, "Title")
        corners = self._corners(win)
        self.assertEqual(corners["tl"], "╭")

    def test_top_right_corner_is_rounded(self) -> None:
        win = FakeWindow(height=10, width=30)
        _draw_frame(win, 0, 0, 10, 30, "Title")
        corners = self._corners(win)
        self.assertEqual(corners["tr"], "╮")

    def test_bottom_left_corner_is_rounded(self) -> None:
        win = FakeWindow(height=10, width=30)
        _draw_frame(win, 0, 0, 10, 30, "Title")
        corners = self._corners(win)
        self.assertEqual(corners["bl"], "╰")

    def test_bottom_right_corner_is_rounded(self) -> None:
        win = FakeWindow(height=10, width=30)
        _draw_frame(win, 0, 0, 10, 30, "Title")
        corners = self._corners(win)
        self.assertEqual(corners["br"], "╯")

    def test_title_embedded_in_top_border(self) -> None:
        win = FakeWindow(height=10, width=40)
        _draw_frame(win, 0, 0, 10, 40, "My Title")
        top_texts = win.texts_at(0)
        combined = "".join(top_texts)
        self.assertIn("My Title", combined)

    def test_title_clipped_when_too_long(self) -> None:
        win = FakeWindow(height=10, width=12)
        _draw_frame(win, 0, 0, 10, 12, "Very Long Title That Does Not Fit")
        top_texts = win.texts_at(0)
        combined = "".join(top_texts)
        self.assertLessEqual(len(combined), 12 + 10)

    def test_curses_error_does_not_propagate(self) -> None:
        win = FakeWindow(height=10, width=30, raise_on={(0, 29)})
        try:
            _draw_frame(win, 0, 0, 10, 30, "Title")
        except curses.error:
            self.fail("_draw_frame must not propagate curses.error")

    def test_vertical_sides_drawn(self) -> None:
        win = FakeWindow(height=8, width=20)
        _draw_frame(win, 0, 0, 8, 20, "T")
        by_pos = {(r, c): text for r, c, text, _a in win.calls}
        for row in range(1, 7):
            self.assertEqual(by_pos.get((row, 0), ""), "│", f"missing left side at row {row}")
            self.assertEqual(by_pos.get((row, 19), ""), "│", f"missing right side at row {row}")


class TestRenderTooSmall(unittest.TestCase):
    def test_renders_message_without_raising(self) -> None:
        win = FakeWindow(height=2, width=10)
        try:
            _render_too_small(win)
        except Exception as exc:  # noqa: BLE001
            self.fail(f"_render_too_small raised unexpectedly: {exc}")

    def test_message_mentions_terminal(self) -> None:
        win = FakeWindow(height=2, width=20)
        _render_too_small(win)
        text = win.all_text().lower()
        self.assertIn("terminal", text)

    def test_works_on_tiny_window(self) -> None:
        win = FakeWindow(height=1, width=5)
        try:
            _render_too_small(win)
        except Exception as exc:  # noqa: BLE001
            self.fail(f"_render_too_small raised on tiny window: {exc}")

    def test_erase_called(self) -> None:
        win = FakeWindow(height=4, width=30)
        _render_too_small(win)
        self.assertTrue(win._erased)  # noqa: SLF001


class TestInitColors(unittest.TestCase):
    def test_no_start_color_when_has_colors_false(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with (
            patch("curses.has_colors", return_value=False),
            patch("curses.start_color") as mock_start,
        ):
            _init_colors()
        mock_start.assert_not_called()

    def test_no_exception_when_has_colors_false(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with patch("curses.has_colors", return_value=False):
            try:
                _init_colors()
            except Exception as exc:  # noqa: BLE001
                self.fail(f"_init_colors raised unexpectedly: {exc}")

    def test_attr_dict_empty_when_has_colors_false(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with patch("curses.has_colors", return_value=False):
            _init_colors()
        self.assertEqual(tui_mod._ATTR, {})

    def test_start_color_called_when_has_colors_true(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with (
            patch("curses.has_colors", return_value=True),
            patch("curses.start_color") as mock_start,
            patch("curses.use_default_colors"),
            patch("curses.init_pair"),
            patch("curses.color_pair", return_value=0),
        ):
            _init_colors()
        mock_start.assert_called_once()

    def test_use_default_colors_called_when_has_colors_true(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with (
            patch("curses.has_colors", return_value=True),
            patch("curses.start_color"),
            patch("curses.use_default_colors") as mock_udc,
            patch("curses.init_pair"),
            patch("curses.color_pair", return_value=0),
        ):
            _init_colors()
        mock_udc.assert_called_once()

    def test_attr_dict_populated_when_has_colors_true(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with (
            patch("curses.has_colors", return_value=True),
            patch("curses.start_color"),
            patch("curses.use_default_colors"),
            patch("curses.init_pair"),
            patch("curses.color_pair", return_value=42),
        ):
            _init_colors()
        self.assertIn("header", tui_mod._ATTR)
        self.assertIn("selected", tui_mod._ATTR)
        self.assertIn("status", tui_mod._ATTR)
        self.assertIn("badge", tui_mod._ATTR)

    def test_init_pair_called_four_times_when_has_colors_true(self) -> None:
        import active_collab.tui as tui_mod
        tui_mod._ATTR.clear()
        with (
            patch("curses.has_colors", return_value=True),
            patch("curses.start_color"),
            patch("curses.use_default_colors"),
            patch("curses.init_pair") as mock_init_pair,
            patch("curses.color_pair", return_value=0),
        ):
            _init_colors()
        self.assertEqual(mock_init_pair.call_count, 4)


class TestBrowseControllerTasksByProject(unittest.TestCase):
    def _controller(self, tasks: list, projects_body: bytes = b"[]") -> BrowseController:
        client = FakeClient(open_tasks=tasks, projects_body=projects_body)
        return BrowseController(
            client=client,  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
        )

    def test_tasks_grouped_by_project_id(self) -> None:
        tasks = [
            _make_mine_task(task_id=1, project_id=10, name="Task A"),
            _make_mine_task(task_id=2, project_id=20, name="Task B"),
            _make_mine_task(task_id=3, project_id=10, name="Task C"),
        ]
        ctrl = self._controller(tasks)
        groups = ctrl.tasks_by_project()
        pids_in_order = [
            t.project_id
            for _, task_list in groups
            for t in task_list
        ]
        self.assertIn(10, pids_in_order)
        self.assertIn(20, pids_in_order)
        tasks_in_proj_10 = [
            t for name, task_list in groups
            if any(t.project_id == 10 for t in task_list)
            for t in task_list
        ]
        self.assertEqual(len(tasks_in_proj_10), 2)

    def test_project_name_resolved_from_list_projects(self) -> None:
        projects = [{"id": 10, "name": "Alpha Project"}]
        body = json.dumps(projects).encode()
        tasks = [_make_mine_task(task_id=1, project_id=10)]
        ctrl = self._controller(tasks, projects_body=body)
        groups = ctrl.tasks_by_project()
        names = [name for name, _ in groups]
        self.assertIn("Alpha Project", names)

    def test_falls_back_to_id_when_name_missing(self) -> None:
        projects: list = []
        body = json.dumps(projects).encode()
        tasks = [_make_mine_task(task_id=1, project_id=99)]
        ctrl = self._controller(tasks, projects_body=body)
        groups = ctrl.tasks_by_project()
        names = [name for name, _ in groups]
        self.assertIn("99", names)

    def test_empty_tasks_returns_empty_list(self) -> None:
        ctrl = self._controller([])
        self.assertEqual(ctrl.tasks_by_project(), [])

    def test_returns_stable_ordered_structure(self) -> None:
        tasks = [
            _make_mine_task(task_id=1, project_id=10),
            _make_mine_task(task_id=2, project_id=20),
        ]
        ctrl = self._controller(tasks)
        groups1 = ctrl.tasks_by_project()
        groups2 = ctrl.tasks_by_project()
        self.assertEqual(
            [n for n, _ in groups1],
            [n for n, _ in groups2],
        )

    def test_project_with_null_name_falls_back_to_id(self) -> None:
        projects = [{"id": 55, "name": None}]
        body = json.dumps(projects).encode()
        tasks = [_make_mine_task(task_id=1, project_id=55)]
        ctrl = self._controller(tasks, projects_body=body)
        groups = ctrl.tasks_by_project()
        names = [name for name, _ in groups]
        self.assertIn("55", names)

    def test_projects_api_failure_falls_back_to_ids(self) -> None:
        client = FakeClient(
            open_tasks=[_make_mine_task(task_id=1, project_id=7)],
            projects_status=500,
            projects_body=b"Server Error",
        )
        ctrl = BrowseController(
            client=client,  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
        )
        groups = ctrl.tasks_by_project()
        names = [name for name, _ in groups]
        self.assertIn("7", names)


class TestBrowseControllerFetchOpenTasks(unittest.TestCase):
    def test_delegates_to_client(self) -> None:
        task = _make_mine_task()
        client = FakeClient(open_tasks=[task])
        ctrl = BrowseController(
            client=client,  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
        )
        result = ctrl.fetch_open_tasks()
        self.assertEqual(result, [task])

    def test_returns_empty_list_when_no_tasks(self) -> None:
        client = FakeClient(open_tasks=[])
        ctrl = BrowseController(
            client=client,  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
        )
        self.assertEqual(ctrl.fetch_open_tasks(), [])

    def test_returns_multiple_tasks(self) -> None:
        tasks = [_make_mine_task(task_id=i) for i in range(3)]
        client = FakeClient(open_tasks=tasks)
        ctrl = BrowseController(
            client=client,  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
        )
        self.assertEqual(ctrl.fetch_open_tasks(), tasks)


class TestBrowseControllerCreateTaskBranch(unittest.TestCase):
    _REV_PARSE_BRANCH = ("git", "rev-parse", "--verify", "feature/10-100")
    _REV_PARSE_MASTER = ("git", "rev-parse", "--verify", "master")
    _CHECKOUT = ("git", "checkout", "-b", "feature/10-100", "master")

    def _make_run(self, branch_exists: bool = False) -> Callable:
        responses: dict[tuple, FakeRunResult] = {
            self._REV_PARSE_BRANCH: FakeRunResult(returncode=0 if branch_exists else 1),
            self._REV_PARSE_MASTER: FakeRunResult(returncode=0),
            self._CHECKOUT: FakeRunResult(returncode=0),
        }
        return _fake_run_factory(responses)

    def _controller(self, run: Callable) -> BrowseController:
        client = FakeClient()
        return BrowseController(
            client=client,  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
            run=run,
        )

    def test_delegates_to_gitbranch_and_returns_branch_result(self) -> None:
        run = self._make_run()
        ctrl = self._controller(run)
        result = ctrl.create_task_branch("feature", 10, 100)
        self.assertIsInstance(result, BranchResult)

    def test_created_status_returned_on_success(self) -> None:
        run = self._make_run()
        ctrl = self._controller(run)
        result = ctrl.create_task_branch("feature", 10, 100)
        self.assertEqual(result.status, BranchStatus.created)

    def test_checkout_argv_contains_master_base_ref(self) -> None:
        run = self._make_run()
        ctrl = self._controller(run)
        ctrl.create_task_branch("feature", 10, 100)
        issued = [tuple(c) for c in run.calls]  # type: ignore[attr-defined]
        self.assertIn(self._CHECKOUT, issued)

    def test_never_uses_force_flag(self) -> None:
        run = self._make_run()
        ctrl = self._controller(run)
        ctrl.create_task_branch("feature", 10, 100)
        for call in run.calls:  # type: ignore[attr-defined]
            self.assertNotIn("-B", call)

    def test_returns_exists_when_branch_already_present(self) -> None:
        run = self._make_run(branch_exists=True)
        ctrl = self._controller(run)
        result = ctrl.create_task_branch("feature", 10, 100)
        self.assertEqual(result.status, BranchStatus.exists)

    def test_no_checkout_call_when_branch_already_exists(self) -> None:
        run = self._make_run(branch_exists=True)
        ctrl = self._controller(run)
        ctrl.create_task_branch("feature", 10, 100)
        for call in run.calls:  # type: ignore[attr-defined]
            self.assertNotIn("-b", call)

    def test_fix_type_produces_correct_branch_name(self) -> None:
        rev_parse_fix = ("git", "rev-parse", "--verify", "fix/10-100")
        checkout_fix = ("git", "checkout", "-b", "fix/10-100", "master")
        responses: dict[tuple, FakeRunResult] = {
            rev_parse_fix: FakeRunResult(returncode=1),
            self._REV_PARSE_MASTER: FakeRunResult(returncode=0),
            checkout_fix: FakeRunResult(returncode=0),
        }
        run = _fake_run_factory(responses)
        ctrl = self._controller(run)
        result = ctrl.create_task_branch("fix", 10, 100)
        self.assertEqual(result.name, "fix/10-100")

    def test_falls_back_to_main_when_master_absent(self) -> None:
        rev_parse_branch = ("git", "rev-parse", "--verify", "feature/10-100")
        rev_parse_master = ("git", "rev-parse", "--verify", "master")
        rev_parse_main = ("git", "rev-parse", "--verify", "main")
        checkout_main = ("git", "checkout", "-b", "feature/10-100", "main")
        responses: dict[tuple, FakeRunResult] = {
            rev_parse_branch: FakeRunResult(returncode=1),
            rev_parse_master: FakeRunResult(returncode=1),
            rev_parse_main: FakeRunResult(returncode=0),
            checkout_main: FakeRunResult(returncode=0),
        }
        run = _fake_run_factory(responses)
        ctrl = self._controller(run)
        result = ctrl.create_task_branch("feature", 10, 100)
        self.assertEqual(result.status, BranchStatus.created)
        issued = [tuple(c) for c in run.calls]  # type: ignore[attr-defined]
        self.assertIn(checkout_main, issued)


class TestBrowseControllerOpenAsset(unittest.TestCase):
    def _controller(self, opener: Callable) -> BrowseController:
        return BrowseController(
            client=FakeClient(),  # type: ignore[arg-type]
            http=MagicMock(),
            instance=_make_instance(),
            opener=opener,
        )

    def test_open_asset_calls_opener_with_url(self) -> None:
        opened: list[str] = []
        ctrl = self._controller(opened.append)
        asset = Asset(name="photo.jpg", url="https://collab.example.com/photo.jpg", kind="image")
        ctrl.open_asset(asset)
        self.assertEqual(opened, ["https://collab.example.com/photo.jpg"])

    def test_open_asset_does_not_modify_url(self) -> None:
        opened: list[str] = []
        ctrl = self._controller(opened.append)
        url = "https://collab.example.com/file?q=1"
        ctrl.open_asset(Asset(name="file", url=url, kind="link"))
        self.assertEqual(opened[0], url)

    def test_open_asset_foreign_host_url_passed_unchanged(self) -> None:
        opened: list[str] = []
        ctrl = self._controller(opened.append)
        url = "https://cdn.thirdparty.com/image.png"
        ctrl.open_asset(Asset(name="image.png", url=url, kind="image"))
        self.assertEqual(opened[0], url)


class TestBrowseControllerDownloadAsset(unittest.TestCase):
    _BASE_URL = "https://collab.example.com"
    _TOKEN = "secret123"

    def _http_mock(self, status: int = 200, body: bytes = b"data") -> MagicMock:
        http = MagicMock()
        http.get.return_value = (status, body)
        return http

    def _controller(self, http: MagicMock) -> BrowseController:
        return BrowseController(
            client=FakeClient(),  # type: ignore[arg-type]
            http=http,
            instance=_make_instance(base_url=self._BASE_URL, token=self._TOKEN),
        )

    def _asset(self, url: str) -> Asset:
        return Asset(name="file.jpg", url=url, kind="image")

    def test_token_attached_when_host_matches_instance(self) -> None:
        import tempfile
        http = self._http_mock()
        ctrl = self._controller(http)
        with tempfile.TemporaryDirectory() as tmp:
            ctrl.download_asset(
                self._asset(f"{self._BASE_URL}/path/file.jpg"), dest_dir=tmp
            )
        args = http.get.call_args[0]
        headers = args[1]
        self.assertIn("X-Angie-AuthApiToken", headers)
        self.assertEqual(headers["X-Angie-AuthApiToken"], self._TOKEN)

    def test_token_omitted_for_foreign_host(self) -> None:
        import tempfile
        http = self._http_mock()
        ctrl = self._controller(http)
        with tempfile.TemporaryDirectory() as tmp:
            ctrl.download_asset(
                self._asset("https://cdn.thirdparty.com/file.jpg"), dest_dir=tmp
            )
        args = http.get.call_args[0]
        headers = args[1]
        self.assertNotIn("X-Angie-AuthApiToken", headers)

    def test_download_writes_file_to_dest_dir(self) -> None:
        import os
        import tempfile
        http = self._http_mock(body=b"imgbytes")
        ctrl = self._controller(http)
        with tempfile.TemporaryDirectory() as tmp:
            path = ctrl.download_asset(
                self._asset(f"{self._BASE_URL}/file.jpg"), dest_dir=tmp
            )
            self.assertTrue(os.path.isfile(path))
            with open(path, "rb") as fh:
                self.assertEqual(fh.read(), b"imgbytes")

    def test_raises_runtime_error_on_http_failure(self) -> None:
        import tempfile
        http = self._http_mock(status=404)
        ctrl = self._controller(http)
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(RuntimeError):
                ctrl.download_asset(
                    self._asset(f"{self._BASE_URL}/missing.jpg"), dest_dir=tmp
                )


class TestRunNonTtyGuard(unittest.TestCase):
    """run() must exit early with code 2 when stdin or stdout is not a TTY."""

    def _args_stub(self) -> types.SimpleNamespace:
        return types.SimpleNamespace(instance=None)

    def _run_with_non_tty(
        self, stdin_is_tty: bool = False, stdout_is_tty: bool = False
    ) -> tuple[int, str]:
        """Invoke tui.run with patched isatty values; return (exit_code, stderr)."""
        from active_collab import tui

        stderr_buf = io.StringIO()
        with (
            patch.object(tui.sys.stdin, "isatty", return_value=stdin_is_tty),
            patch.object(tui.sys.stdout, "isatty", return_value=stdout_is_tty),
            contextlib.redirect_stderr(stderr_buf),
        ):
            exit_code = tui.run(self._args_stub())
        return exit_code, stderr_buf.getvalue()

    def test_returns_2_when_stdin_not_a_tty(self) -> None:
        exit_code, _ = self._run_with_non_tty(stdin_is_tty=False, stdout_is_tty=True)
        self.assertEqual(exit_code, 2)

    def test_returns_2_when_stdout_not_a_tty(self) -> None:
        exit_code, _ = self._run_with_non_tty(stdin_is_tty=True, stdout_is_tty=False)
        self.assertEqual(exit_code, 2)

    def test_returns_2_when_both_not_a_tty(self) -> None:
        exit_code, _ = self._run_with_non_tty(stdin_is_tty=False, stdout_is_tty=False)
        self.assertEqual(exit_code, 2)

    def test_no_exception_raised_in_non_tty_path(self) -> None:
        stderr_buf = io.StringIO()
        from active_collab import tui

        with (
            patch.object(tui.sys.stdin, "isatty", return_value=False),
            patch.object(tui.sys.stdout, "isatty", return_value=False),
            contextlib.redirect_stderr(stderr_buf),
        ):
            try:
                tui.run(self._args_stub())
            except Exception as exc:  # noqa: BLE001
                self.fail(f"run() raised an unexpected exception: {exc}")

    def test_error_message_mentions_interactive_terminal(self) -> None:
        _, stderr = self._run_with_non_tty(stdin_is_tty=False, stdout_is_tty=False)
        self.assertIn("interactive terminal", stderr.lower())

    def test_error_message_mentions_tty(self) -> None:
        _, stderr = self._run_with_non_tty(stdin_is_tty=False, stdout_is_tty=False)
        self.assertIn("TTY", stderr)

    def test_error_written_to_stderr_not_stdout(self) -> None:
        from active_collab import tui

        stdout_buf = io.StringIO()
        stderr_buf = io.StringIO()
        with (
            patch.object(tui.sys.stdin, "isatty", return_value=False),
            patch.object(tui.sys.stdout, "isatty", return_value=False),
            contextlib.redirect_stdout(stdout_buf),
            contextlib.redirect_stderr(stderr_buf),
        ):
            tui.run(self._args_stub())
        self.assertNotEqual(stderr_buf.getvalue(), "")
        self.assertEqual(stdout_buf.getvalue(), "")

    def test_config_load_not_called_in_non_tty_path(self) -> None:
        from active_collab import tui

        stderr_buf = io.StringIO()
        with (
            patch.object(tui.sys.stdin, "isatty", return_value=False),
            patch.object(tui.sys.stdout, "isatty", return_value=False),
            patch("active_collab.tui.Config") as mock_config,
            contextlib.redirect_stderr(stderr_buf),
        ):
            tui.run(self._args_stub())
        mock_config.load.assert_not_called()


class TestCliIntegrationBrowseSubcommand(unittest.TestCase):
    """Verify browse subcommand is wired into the dispatch dict."""

    def test_browse_in_known_commands(self) -> None:
        from active_collab.cli import _KNOWN_COMMANDS
        self.assertIn("browse", _KNOWN_COMMANDS)

    def test_browse_in_command_handlers(self) -> None:
        from active_collab.cli import _COMMAND_HANDLERS
        self.assertIn("browse", _COMMAND_HANDLERS)

    def test_browse_handler_is_callable(self) -> None:
        from active_collab.cli import _COMMAND_HANDLERS
        self.assertTrue(callable(_COMMAND_HANDLERS["browse"]))

    def test_argparse_accepts_browse_subcommand(self) -> None:
        from active_collab.cli import _build_parser
        parser = _build_parser()
        args = parser.parse_args(["browse"])
        self.assertEqual(args.command, "browse")

    def test_browse_accepts_instance_flag(self) -> None:
        from active_collab.cli import _build_parser
        parser = _build_parser()
        args = parser.parse_args(["browse", "--instance", "myinst"])
        self.assertEqual(args.instance, "myinst")


class FakeStdscr:
    """Minimal fake curses window for testing screen functions without a terminal."""

    def __init__(self, keys: list[int], height: int = 24, width: int = 80) -> None:
        self._keys = list(keys)
        self._key_idx = 0
        self._height = height
        self._width = width
        self.written: list[tuple] = []

    def erase(self) -> None:
        pass

    def getmaxyx(self) -> tuple[int, int]:
        return self._height, self._width

    def addstr(self, row: int, col: int, text: str, attr: int = 0) -> None:
        self.written.append((row, col, text, attr))

    def refresh(self) -> None:
        pass

    def getch(self) -> int:
        if self._key_idx < len(self._keys):
            key = self._keys[self._key_idx]
            self._key_idx += 1
            return key
        return ord("q")

    def text_written(self) -> str:
        return " ".join(t for _, _, t, *_ in self.written)


def _make_instance_named(name: str, base_url: str = "https://collab.example.com") -> Instance:
    return Instance(name=name, base_url=base_url, email="u@example.com", token="tok", user_id=1)


class FakeClientForMine:
    """Test double for ActiveCollabClient that returns a fixed list of MineTasks."""

    def __init__(self, tasks: list[MineTask]) -> None:
        self._tasks = tasks

    def fetch_open_tasks(self) -> list[MineTask]:
        return self._tasks

    def list_projects(self) -> tuple[int, bytes]:
        return 200, b"[]"

    def fetch_task(self, project_id: int, task_id: int) -> tuple[int, dict | None]:
        return 200, {}


class TestMineControllerMyTasks(unittest.TestCase):
    def _controller(
        self,
        inst_tasks: list[tuple[Instance, list[MineTask]]],
    ) -> MineController:
        instances = [inst for inst, _ in inst_tasks]
        ctrl = MineController(instances, MagicMock())

        for inst, tasks in inst_tasks:
            fake_client = FakeClientForMine(tasks)
            # Replace the underlying client on the browse controller so
            # fetch_open_tasks() is served by our fake.
            ctrl._controllers[inst.name]._client = fake_client  # noqa: SLF001

        return ctrl

    def test_aggregates_tasks_across_two_instances(self) -> None:
        inst_a = _make_instance_named("alpha")
        inst_b = _make_instance_named("beta", "https://beta.example.com")
        task_a = MineTask(id=1, name="Alpha task", instance_name="alpha", project_id=10)
        task_b = MineTask(id=2, name="Beta task", instance_name="beta", project_id=20)

        ctrl = self._controller([(inst_a, [task_a]), (inst_b, [task_b])])
        tasks = ctrl.my_tasks()

        self.assertEqual(len(tasks), 2)

    def test_each_task_retains_its_instance_name(self) -> None:
        inst_a = _make_instance_named("alpha")
        inst_b = _make_instance_named("beta", "https://beta.example.com")
        task_a = MineTask(id=1, name="Alpha task", instance_name="alpha", project_id=10)
        task_b = MineTask(id=2, name="Beta task", instance_name="beta", project_id=20)

        ctrl = self._controller([(inst_a, [task_a]), (inst_b, [task_b])])
        tasks = ctrl.my_tasks()

        instance_names = {t.instance_name for t in tasks}
        self.assertIn("alpha", instance_names)
        self.assertIn("beta", instance_names)

    def test_preserves_instance_order_then_task_order(self) -> None:
        inst_a = _make_instance_named("first")
        inst_b = _make_instance_named("second", "https://second.example.com")
        t1 = MineTask(id=1, name="T1", instance_name="first", project_id=1)
        t2 = MineTask(id=2, name="T2", instance_name="first", project_id=1)
        t3 = MineTask(id=3, name="T3", instance_name="second", project_id=2)

        ctrl = self._controller([(inst_a, [t1, t2]), (inst_b, [t3])])
        tasks = ctrl.my_tasks()

        self.assertEqual([t.id for t in tasks], [1, 2, 3])

    def test_empty_tasks_returns_empty_list(self) -> None:
        inst = _make_instance_named("solo")
        ctrl = self._controller([(inst, [])])
        self.assertEqual(ctrl.my_tasks(), [])

    def test_single_instance_with_multiple_tasks(self) -> None:
        inst = _make_instance_named("solo")
        tasks = [
            MineTask(id=i, name=f"Task {i}", instance_name="solo", project_id=5)
            for i in range(5)
        ]
        ctrl = self._controller([(inst, tasks)])
        self.assertEqual(len(ctrl.my_tasks()), 5)

    def test_my_tasks_uses_public_fetch_open_tasks_not_private_client(self) -> None:
        """MineController.my_tasks must route through ctrl.fetch_open_tasks()."""
        inst = _make_instance_named("solo")
        task = MineTask(id=1, name="T", instance_name="solo", project_id=1)
        ctrl = MineController([inst], MagicMock())

        fetch_calls: list[int] = []

        def tracking_fetch() -> list[MineTask]:
            fetch_calls.append(1)
            return [task]

        ctrl._controllers["solo"].fetch_open_tasks = tracking_fetch  # noqa: SLF001
        result = ctrl.my_tasks()
        self.assertEqual(len(fetch_calls), 1, "fetch_open_tasks must be called once")
        self.assertEqual(result, [task])


class TestMineControllerControllerFor(unittest.TestCase):
    def _controller(self, inst_a: Instance, inst_b: Instance) -> MineController:
        return MineController([inst_a, inst_b], MagicMock())

    def test_returns_controller_bound_to_correct_instance(self) -> None:
        inst_a = _make_instance_named("alpha")
        inst_b = _make_instance_named("beta", "https://beta.example.com")
        ctrl = self._controller(inst_a, inst_b)
        task = MineTask(id=1, name="Task", instance_name="beta", project_id=10)
        browse_ctrl = ctrl.controller_for(task)
        self.assertEqual(browse_ctrl._instance.name, "beta")  # noqa: SLF001

    def test_controller_for_alpha_task_points_to_alpha_instance(self) -> None:
        inst_a = _make_instance_named("alpha")
        inst_b = _make_instance_named("beta", "https://beta.example.com")
        ctrl = self._controller(inst_a, inst_b)
        task = MineTask(id=2, name="Task", instance_name="alpha", project_id=5)
        browse_ctrl = ctrl.controller_for(task)
        self.assertEqual(browse_ctrl._instance.name, "alpha")  # noqa: SLF001

    def test_controller_for_beta_is_different_from_alpha(self) -> None:
        inst_a = _make_instance_named("alpha")
        inst_b = _make_instance_named("beta", "https://beta.example.com")
        ctrl = self._controller(inst_a, inst_b)
        task_a = MineTask(id=1, name="A", instance_name="alpha", project_id=1)
        task_b = MineTask(id=2, name="B", instance_name="beta", project_id=2)
        self.assertIsNot(ctrl.controller_for(task_a), ctrl.controller_for(task_b))


class TestScreenMineList(unittest.TestCase):
    def _tasks(self) -> list[MineTask]:
        return [
            MineTask(id=1, task_number=5, name="Alpha task", instance_name="alpha", project_id=10),
            MineTask(id=2, task_number=None, name="Beta task", instance_name="beta", project_id=20),
        ]

    def test_label_contains_instance_name(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("q")])
        _screen_mine_list(stdscr, tasks, 0)
        text = stdscr.text_written()
        self.assertIn("alpha", text)
        self.assertIn("beta", text)

    def test_label_contains_task_number_when_present(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("q")])
        _screen_mine_list(stdscr, tasks, 0)
        text = stdscr.text_written()
        self.assertIn("#5", text)

    def test_label_falls_back_to_id_when_task_number_is_none(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("q")])
        _screen_mine_list(stdscr, tasks, 0)
        text = stdscr.text_written()
        self.assertIn("#2", text)

    def test_label_contains_task_name(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("q")])
        _screen_mine_list(stdscr, tasks, 0)
        text = stdscr.text_written()
        self.assertIn("Alpha task", text)
        self.assertIn("Beta task", text)

    def test_q_returns_quit_action(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("q")])
        _, action = _screen_mine_list(stdscr, tasks, 0)
        self.assertIsNone(action)

    def test_esc_returns_quit_action(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[27])
        _, action = _screen_mine_list(stdscr, tasks, 0)
        self.assertIsNone(action)

    def test_down_arrow_increments_selection(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[curses.KEY_DOWN])
        sel, action = _screen_mine_list(stdscr, tasks, 0)
        self.assertEqual(sel, 1)
        self.assertEqual(action, "list")

    def test_j_key_increments_selection(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("j")])
        sel, action = _screen_mine_list(stdscr, tasks, 0)
        self.assertEqual(sel, 1)
        self.assertEqual(action, "list")

    def test_up_arrow_decrements_selection(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[curses.KEY_UP])
        sel, action = _screen_mine_list(stdscr, tasks, 1)
        self.assertEqual(sel, 0)
        self.assertEqual(action, "list")

    def test_k_key_decrements_selection(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[ord("k")])
        sel, action = _screen_mine_list(stdscr, tasks, 1)
        self.assertEqual(sel, 0)
        self.assertEqual(action, "list")

    def test_enter_returns_detail_action(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[10])
        _, action = _screen_mine_list(stdscr, tasks, 0)
        self.assertEqual(action, "detail")

    def test_enter_on_empty_list_stays_on_list(self) -> None:
        stdscr = FakeStdscr(keys=[10, ord("q")])
        _, action = _screen_mine_list(stdscr, [], 0)
        self.assertEqual(action, "list")

    def test_selection_clamped_at_bottom(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[curses.KEY_DOWN])
        sel, _ = _screen_mine_list(stdscr, tasks, 1)
        self.assertEqual(sel, 1)

    def test_selection_clamped_at_top(self) -> None:
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[curses.KEY_UP])
        sel, _ = _screen_mine_list(stdscr, tasks, 0)
        self.assertEqual(sel, 0)

    def test_key_resize_returns_list_action_without_crashing(self) -> None:
        """KEY_RESIZE must be handled as a benign re-render trigger."""
        tasks = self._tasks()
        stdscr = FakeStdscr(keys=[curses.KEY_RESIZE, ord("q")])
        sel, action = _screen_mine_list(stdscr, tasks, 0)
        self.assertEqual(sel, 0)
        self.assertEqual(action, "list")

    def test_selected_row_shows_marker(self) -> None:
        """The ▸ marker must appear on the selected item row and not on others."""
        tasks = self._tasks()
        labels = [
            f"[{t.instance_name}] #{t.task_number or t.id}  {t.name}"
            for t in tasks
        ]
        stdscr = FakeStdscr(keys=[])
        _render_list(stdscr, labels, 0, "My Open Tasks")
        texts = [text for _, _, text, *_ in stdscr.written]
        self.assertTrue(any("▸" in t for t in texts), "▸ marker must appear somewhere")
        marker_rows = [
            (row, text) for row, _, text, *_ in stdscr.written if "▸" in text
        ]
        self.assertEqual(len(marker_rows), 1, "exactly one row should have the ▸ marker")

    def test_non_selected_row_has_no_marker(self) -> None:
        """Non-selected rows must not show the ▸ marker."""
        tasks = self._tasks()
        labels = [
            f"[{t.instance_name}] #{t.task_number or t.id}  {t.name}"
            for t in tasks
        ]
        stdscr = FakeStdscr(keys=[])
        _render_list(stdscr, labels, 0, "My Open Tasks")
        for _row, _, text, *_ in stdscr.written:
            if "beta" in text:
                msg = f"Non-selected 'beta' row must not have marker: {text!r}"
                self.assertNotIn("▸", text, msg)


class TestRenderListFrame(unittest.TestCase):
    """_render_list must draw rounded corners and embed the title in the top border."""

    def _render(
        self,
        items: list[str],
        sel: int = 0,
        title: str = "Test",
        height: int = 20,
        width: int = 60,
    ) -> FakeStdscr:
        stdscr = FakeStdscr(keys=[], height=height, width=width)
        _render_list(stdscr, items, sel, title)
        return stdscr

    def test_top_left_corner_is_rounded(self) -> None:
        stdscr = self._render(["item one", "item two"])
        by_pos = {(r, c): text for r, c, text, _a in stdscr.written}
        self.assertEqual(by_pos.get((0, 0), ""), "╭")

    def test_top_right_corner_is_rounded(self) -> None:
        stdscr = self._render(["item one", "item two"], width=40)
        by_pos = {(r, c): text for r, c, text, _a in stdscr.written}
        self.assertEqual(by_pos.get((0, 39), ""), "╮")

    def test_bottom_left_corner_is_rounded(self) -> None:
        stdscr = self._render(["item one", "item two"], height=15)
        by_pos = {(r, c): text for r, c, text, _a in stdscr.written}
        self.assertEqual(by_pos.get((14, 0), ""), "╰")

    def test_bottom_right_corner_is_rounded(self) -> None:
        stdscr = self._render(["item one"], height=10, width=30)
        by_pos = {(r, c): text for r, c, text, _a in stdscr.written}
        self.assertEqual(by_pos.get((9, 29), ""), "╯")

    def test_title_appears_in_rendered_output(self) -> None:
        stdscr = self._render(["item"], title="My Projects")
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertIn("My Projects", all_text)

    def test_selected_item_has_marker(self) -> None:
        items = ["Alpha", "Beta", "Gamma"]
        stdscr = self._render(items, sel=1)
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertIn("▸", all_text)
        marker_texts = [t for _, _, t, *_ in stdscr.written if "▸" in t]
        self.assertEqual(len(marker_texts), 1)
        self.assertIn("Beta", marker_texts[0])

    def test_non_selected_items_have_no_marker(self) -> None:
        items = ["Alpha", "Beta", "Gamma"]
        stdscr = self._render(items, sel=1)
        for _, _, text, *_ in stdscr.written:
            if "Alpha" in text or "Gamma" in text:
                self.assertNotIn("▸", text, f"Non-selected item has marker: {text!r}")

    def test_hint_bar_text_rendered(self) -> None:
        stdscr = self._render(["item"])
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertIn("[↑↓]", all_text)

    def test_too_small_terminal_shows_warning(self) -> None:
        stdscr = FakeStdscr(keys=[], height=3, width=10)
        _render_list(stdscr, ["item"], 0, "Title")
        all_text = " ".join(t for _, _, t, *_ in stdscr.written).lower()
        self.assertIn("terminal", all_text)

    def test_too_small_terminal_does_not_crash(self) -> None:
        stdscr = FakeStdscr(keys=[], height=2, width=8)
        try:
            _render_list(stdscr, ["item"], 0, "Title")
        except Exception as exc:  # noqa: BLE001
            self.fail(f"_render_list raised on tiny terminal: {exc}")

    def test_scrolling_list_shows_selected_item(self) -> None:
        items = [f"Item {i}" for i in range(30)]
        sel = 25
        stdscr = self._render(items, sel=sel, height=12, width=40)
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertIn(f"Item {sel}", all_text)

    def test_scrolled_list_hides_items_before_viewport(self) -> None:
        items = [f"Item {i}" for i in range(30)]
        stdscr = self._render(items, sel=29, height=12, width=40)
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertNotIn("Item 0", all_text)


class TestResizeSafeListScreens(unittest.TestCase):
    """KEY_RESIZE must not crash any list screen; it should re-render."""

    def test_screen_mine_list_handles_key_resize(self) -> None:
        tasks = [
            MineTask(id=1, name="Task", instance_name="inst", project_id=1),
        ]
        stdscr = FakeStdscr(keys=[curses.KEY_RESIZE, ord("q")])
        try:
            sel, action = _screen_mine_list(stdscr, tasks, 0)
        except Exception as exc:  # noqa: BLE001
            self.fail(f"_screen_mine_list crashed on KEY_RESIZE: {exc}")
        self.assertEqual(action, "list")

    def test_render_list_called_after_resize(self) -> None:
        tasks = [MineTask(id=1, name="T", instance_name="i", project_id=1)]
        stdscr = FakeStdscr(keys=[curses.KEY_RESIZE, ord("q")])
        _screen_mine_list(stdscr, tasks, 0)
        all_text = stdscr.text_written()
        self.assertIn("T", all_text)


class TestResolveBrowseInstance(unittest.TestCase):
    def _inst(self, name: str) -> Instance:
        return _make_instance_named(name)

    def test_single_instance_returned_when_one_configured(self) -> None:
        inst = self._inst("solo")
        result, err = _resolve_browse_instance([inst], None)
        self.assertIs(result, inst)
        self.assertIsNone(err)

    def test_named_instance_returned_when_name_matches(self) -> None:
        inst_a = self._inst("alpha")
        inst_b = self._inst("beta")
        result, err = _resolve_browse_instance([inst_a, inst_b], "beta")
        self.assertIs(result, inst_b)
        self.assertIsNone(err)

    def test_error_for_unknown_instance_name(self) -> None:
        inst = self._inst("alpha")
        result, err = _resolve_browse_instance([inst], "nonexistent")
        self.assertIsNone(result)
        self.assertIsNotNone(err)
        self.assertIn("nonexistent", err)  # type: ignore[arg-type]

    def test_error_message_lists_known_instances_on_unknown_name(self) -> None:
        inst_a = self._inst("alpha")
        inst_b = self._inst("beta")
        _, err = _resolve_browse_instance([inst_a, inst_b], "ghost")
        self.assertIn("alpha", err)  # type: ignore[arg-type]
        self.assertIn("beta", err)  # type: ignore[arg-type]

    def test_error_when_multiple_instances_and_no_name(self) -> None:
        inst_a = self._inst("alpha")
        inst_b = self._inst("beta")
        result, err = _resolve_browse_instance([inst_a, inst_b], None)
        self.assertIsNone(result)
        self.assertIsNotNone(err)

    def test_error_message_mentions_use_instance_flag(self) -> None:
        inst_a = self._inst("alpha")
        inst_b = self._inst("beta")
        _, err = _resolve_browse_instance([inst_a, inst_b], None)
        self.assertIn("--instance", err)  # type: ignore[arg-type]

    def test_error_when_no_instances_configured(self) -> None:
        result, err = _resolve_browse_instance([], None)
        self.assertIsNone(result)
        self.assertIsNotNone(err)

    def test_error_message_mentions_setup_add_when_no_instances(self) -> None:
        _, err = _resolve_browse_instance([], None)
        self.assertIn("setup add", err)  # type: ignore[arg-type]

    def test_first_named_match_returned_when_duplicates(self) -> None:
        inst_a = Instance(name="dupe", base_url="https://a.example.com",
                          email="a@example.com", token="t1", user_id=1)
        inst_b = Instance(name="dupe", base_url="https://b.example.com",
                          email="b@example.com", token="t2", user_id=2)
        result, err = _resolve_browse_instance([inst_a, inst_b], "dupe")
        self.assertIs(result, inst_a)
        self.assertIsNone(err)


class TestCommentBox(unittest.TestCase):
    """AC1: _comment_box renders a rounded box; no line exceeds width."""

    def _box(self, author: str = "Alice", when: str = "2026-01-01 10:00",
             body: str = "Hello world", width: int = 40) -> list[str]:
        return _comment_box(author, when, body, width)

    def test_first_line_starts_with_rounded_top_left(self) -> None:
        box = self._box()
        self.assertTrue(box[0].startswith("╭"), f"Expected ╭ at start, got: {box[0]!r}")

    def test_last_line_starts_with_rounded_bottom_left(self) -> None:
        box = self._box()
        self.assertTrue(box[-1].startswith("╰"), f"Expected ╰ at start, got: {box[-1]!r}")

    def test_top_border_contains_author(self) -> None:
        box = self._box(author="Bob")
        self.assertIn("Bob", box[0])

    def test_top_border_contains_date(self) -> None:
        box = self._box(when="2026-06-24 12:00")
        self.assertIn("2026-06-24 12:00", box[0])

    def test_top_border_contains_separator_dot(self) -> None:
        box = self._box()
        self.assertIn("·", box[0])

    def test_body_lines_are_wrapped_inside_box(self) -> None:
        long_body = "word " * 20
        box = _comment_box("Alice", "2026-01-01", long_body, width=30)
        for line in box[1:-1]:
            self.assertTrue(line.startswith("│"), f"Body line missing │: {line!r}")

    def test_no_line_exceeds_width(self) -> None:
        box = self._box(body="a longer body that may wrap or not", width=35)
        for line in box:
            self.assertLessEqual(len(line), 35, f"Line too long ({len(line)}): {line!r}")

    def test_box_is_empty_when_width_too_small(self) -> None:
        box = _comment_box("A", "2026-01-01", "body", width=3)
        self.assertEqual(box, [])

    def test_top_line_ends_with_rounded_top_right(self) -> None:
        box = self._box(width=40)
        self.assertTrue(box[0].endswith("╮"), f"Expected ╮ at end: {box[0]!r}")

    def test_bottom_line_ends_with_rounded_bottom_right(self) -> None:
        box = self._box(width=40)
        self.assertTrue(box[-1].endswith("╯"), f"Expected ╯ at end: {box[-1]!r}")

    def test_all_lines_same_length(self) -> None:
        box = self._box(width=40)
        lengths = [len(line) for line in box]
        self.assertEqual(len(set(lengths)), 1, f"Lines have different lengths: {lengths}")

    def test_author_clipped_when_header_too_long(self) -> None:
        long_author = "A" * 100
        box = _comment_box(long_author, "2026-01-01", "body", width=20)
        self.assertLessEqual(len(box[0]), 20)


class TestBuildDetailLines(unittest.TestCase):
    """AC2: build_detail_lines renders one box per comment; meta lines first."""

    def _comment(self, author: str = "Alice", body: str = "A comment body",
                 created_on: int = 0) -> dict:
        return {
            "created_by_name": author,
            "created_on": created_on,
            "body": body,
        }

    def test_returns_meta_lines_when_no_comments(self) -> None:
        lines = build_detail_lines("Task meta text", [], 60)
        self.assertTrue(len(lines) > 0)
        combined = "\n".join(lines)
        self.assertIn("Task meta text", combined)

    def test_no_boxes_when_no_comments(self) -> None:
        lines = build_detail_lines("meta", [], 60)
        self.assertFalse(any("╭" in line for line in lines))

    def test_one_box_for_one_comment(self) -> None:
        comments = [self._comment()]
        lines = build_detail_lines("meta", comments, 60)
        box_starts = [line for line in lines if line.startswith("╭")]
        self.assertEqual(len(box_starts), 1)

    def test_two_boxes_for_two_comments(self) -> None:
        comments = [self._comment("Alice"), self._comment("Bob")]
        lines = build_detail_lines("meta", comments, 60)
        box_starts = [line for line in lines if line.startswith("╭")]
        self.assertEqual(len(box_starts), 2)

    def test_n_boxes_for_n_comments(self) -> None:
        n = 5
        comments = [self._comment(f"Author{i}") for i in range(n)]
        lines = build_detail_lines("meta", comments, 60)
        box_starts = [line for line in lines if line.startswith("╭")]
        self.assertEqual(len(box_starts), n)

    def test_meta_lines_appear_before_comment_boxes(self) -> None:
        meta = "Task: #42 Name: My Task"
        comments = [self._comment()]
        lines = build_detail_lines(meta, comments, 60)
        first_box = next(i for i, line in enumerate(lines) if line.startswith("╭"))
        meta_lines = [line for line in lines[:first_box] if "Task" in line or "My Task" in line]
        self.assertTrue(len(meta_lines) > 0, "Meta text must appear before comment boxes")

    def test_no_line_exceeds_inner_width(self) -> None:
        comments = [self._comment("Alice", "Some body text")]
        lines = build_detail_lines("meta text here", comments, inner_width=50)
        for line in lines:
            self.assertLessEqual(len(line), 50, f"Line too long ({len(line)}): {line!r}")

    def test_comment_author_appears_in_output(self) -> None:
        comments = [self._comment("CharlieBrown")]
        lines = build_detail_lines("meta", comments, 60)
        combined = "\n".join(lines)
        self.assertIn("CharlieBrown", combined)

    def test_empty_meta_with_comments_still_produces_boxes(self) -> None:
        comments = [self._comment()]
        lines = build_detail_lines("", comments, 60)
        box_starts = [line for line in lines if line.startswith("╭")]
        self.assertEqual(len(box_starts), 1)


class FakeController:
    """Minimal BrowseController stand-in for detail-view tests."""

    def __init__(
        self,
        task_dict: dict | None = None,
        comments: list | None = None,
    ) -> None:
        self._task_dict = task_dict or {
            "id": 42,
            "task_number": 7,
            "name": "Test Task",
            "is_completed": False,
            "body": "Description here",
        }
        self._comments = comments or []

    def task_detail(
        self, project_id: int, task_id: int
    ) -> tuple[dict, list, list]:
        return self._task_dict, self._comments, []

    def create_task_branch(self, branch_type: str, project_id: int, task_id: int):  # type: ignore[no-untyped-def]
        from active_collab.gitbranch import BranchResult, BranchStatus
        return BranchResult(status=BranchStatus.created, name="feature/10-42")

    def open_asset(self, asset: object) -> None:
        pass

    def download_asset(self, asset: object, dest_dir: str | None = None) -> str:
        return "/tmp/file"


class FakeStdscrScrollable(FakeStdscr):
    """FakeStdscr variant that reports a stable size and supports multiple reads."""

    def __init__(self, keys: list[int], height: int = 24, width: int = 80) -> None:
        super().__init__(keys=keys, height=height, width=width)


def _make_scrollable_task() -> MineTask:
    return MineTask(
        id=42,
        task_number=7,
        name="Test Task",
        is_completed=False,
        is_trashed=False,
        project_id=10,
        instance_name="test",
    )


class TestDetailScrollOffset(unittest.TestCase):
    """AC3: scroll offset clamped to [0, max_offset]; no crash at boundaries."""

    def _run_detail(self, keys: list[int], height: int = 24, width: int = 80,
                    comments: list | None = None) -> FakeStdscrScrollable:
        stdscr = FakeStdscrScrollable(keys=keys, height=height, width=width)
        task = _make_scrollable_task()
        ctrl = FakeController(comments=comments or [])
        _render_and_handle_detail(stdscr, ctrl, 10, task)
        return stdscr

    def test_q_exits_without_crash(self) -> None:
        try:
            self._run_detail([ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"_render_and_handle_detail raised: {exc}")

    def test_scroll_down_then_quit_does_not_crash(self) -> None:
        try:
            self._run_detail([curses.KEY_DOWN, curses.KEY_DOWN, ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Scroll down then quit raised: {exc}")

    def test_scroll_up_from_zero_does_not_crash(self) -> None:
        try:
            self._run_detail([curses.KEY_UP, curses.KEY_UP, ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Scroll up from zero raised: {exc}")

    def test_page_down_then_quit_does_not_crash(self) -> None:
        try:
            self._run_detail([curses.KEY_NPAGE, ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"PgDn then quit raised: {exc}")

    def test_page_up_then_quit_does_not_crash(self) -> None:
        try:
            self._run_detail([curses.KEY_PPAGE, ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"PgUp then quit raised: {exc}")

    def test_many_downs_clamps_to_max_offset(self) -> None:
        many_downs = [curses.KEY_DOWN] * 200 + [ord("q")]
        try:
            self._run_detail(many_downs)
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Many downs raised: {exc}")

    def test_j_key_scrolls_down_without_crash(self) -> None:
        try:
            self._run_detail([ord("j"), ord("j"), ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"j key scrolling raised: {exc}")

    def test_k_key_scrolls_up_without_crash(self) -> None:
        try:
            self._run_detail([ord("k"), ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"k key scrolling raised: {exc}")

    def test_b_key_exits_without_crash(self) -> None:
        try:
            self._run_detail([ord("b")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"b key exit raised: {exc}")

    def test_esc_exits_without_crash(self) -> None:
        try:
            self._run_detail([27])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Esc exit raised: {exc}")

    def test_scroll_with_comments_does_not_crash(self) -> None:
        comments = [
            {"created_by_name": f"Author{i}", "created_on": 0, "body": f"Comment {i} body"}
            for i in range(10)
        ]
        try:
            self._run_detail(
                [curses.KEY_DOWN] * 5 + [curses.KEY_NPAGE, ord("q")],
                comments=comments,
            )
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Scroll with comments raised: {exc}")

    def test_written_output_contains_task_name(self) -> None:
        stdscr = self._run_detail([ord("q")])
        text = stdscr.text_written()
        self.assertIn("Test Task", text)


class TestDetailResizeAndTooSmall(unittest.TestCase):
    """AC4: KEY_RESIZE and too-small terminal do not crash the detail view."""

    def _run_detail(self, keys: list[int], height: int = 24, width: int = 80) -> None:
        stdscr = FakeStdscrScrollable(keys=keys, height=height, width=width)
        task = _make_scrollable_task()
        ctrl = FakeController()
        _render_and_handle_detail(stdscr, ctrl, 10, task)

    def test_key_resize_does_not_crash(self) -> None:
        try:
            self._run_detail([curses.KEY_RESIZE, ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"KEY_RESIZE raised: {exc}")

    def test_multiple_resize_events_do_not_crash(self) -> None:
        try:
            self._run_detail([curses.KEY_RESIZE, curses.KEY_RESIZE, ord("q")])
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Multiple KEY_RESIZE raised: {exc}")

    def test_too_small_terminal_does_not_crash(self) -> None:
        try:
            self._run_detail([ord("q")], height=3, width=10)
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Too-small terminal raised: {exc}")

    def test_very_narrow_terminal_does_not_crash(self) -> None:
        try:
            self._run_detail([ord("q")], height=24, width=5)
        except Exception as exc:  # noqa: BLE001
            self.fail(f"Very narrow terminal raised: {exc}")

    def test_minimal_terminal_shows_too_small_message(self) -> None:
        stdscr = FakeStdscrScrollable(keys=[ord("q")], height=3, width=10)
        task = _make_scrollable_task()
        ctrl = FakeController()
        _render_and_handle_detail(stdscr, ctrl, 10, task)
        text = stdscr.text_written().lower()
        self.assertIn("terminal", text)


class TestScrollOffset(unittest.TestCase):
    """AC FIX2: _scroll_offset clamps to [0, max_offset]; no-op for unrelated keys."""

    def test_up_at_zero_stays_zero(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_UP, 0, 10, 5), 0)

    def test_k_at_zero_stays_zero(self) -> None:
        self.assertEqual(_scroll_offset(ord("k"), 0, 10, 5), 0)

    def test_up_decrements_offset(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_UP, 5, 10, 5), 4)

    def test_k_decrements_offset(self) -> None:
        self.assertEqual(_scroll_offset(ord("k"), 3, 10, 5), 2)

    def test_down_increments_offset(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_DOWN, 3, 10, 5), 4)

    def test_j_increments_offset(self) -> None:
        self.assertEqual(_scroll_offset(ord("j"), 3, 10, 5), 4)

    def test_down_at_max_offset_stays_at_max(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_DOWN, 10, 10, 5), 10)

    def test_j_at_max_offset_stays_at_max(self) -> None:
        self.assertEqual(_scroll_offset(ord("j"), 10, 10, 5), 10)

    def test_pgup_moves_back_by_viewport(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_PPAGE, 8, 20, 5), 3)

    def test_pgup_at_start_clamps_to_zero(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_PPAGE, 2, 20, 5), 0)

    def test_pgdn_moves_forward_by_viewport(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_NPAGE, 3, 20, 5), 8)

    def test_pgdn_past_max_clamps_to_max(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_NPAGE, 18, 20, 5), 20)

    def test_unrelated_key_leaves_offset_unchanged(self) -> None:
        self.assertEqual(_scroll_offset(ord("x"), 7, 20, 5), 7)

    def test_enter_key_leaves_offset_unchanged(self) -> None:
        self.assertEqual(_scroll_offset(10, 4, 20, 5), 4)

    def test_zero_max_offset_down_stays_at_zero(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_DOWN, 0, 0, 5), 0)

    def test_zero_max_offset_pgdn_stays_at_zero(self) -> None:
        self.assertEqual(_scroll_offset(curses.KEY_NPAGE, 0, 0, 5), 0)


_DETAIL_HINT_PAIRS = [
    ("q", "back"), ("c", "branch"), ("a", "assets"), ("↑↓", "scroll"), ("⇞⇟", "page"),
]
_LIST_HINT_PAIRS = [("↑↓", "move"), ("Enter", "select"), ("q", "quit"), ("b", "back")]


class TestHintBar(unittest.TestCase):
    """S2-AC3: _hint_bar produces key-cap format and routes labels through __()."""

    def test_single_pair_produces_bracket_key_and_label(self) -> None:
        result = _hint_bar([("q", "back")])
        self.assertEqual(result, "[q] back")

    def test_multiple_pairs_joined_by_two_spaces(self) -> None:
        result = _hint_bar([("q", "back"), ("c", "branch")])
        self.assertEqual(result, "[q] back  [c] branch")

    def test_detail_hint_bar_contains_q_back(self) -> None:
        self.assertIn("[q] back", _hint_bar(_DETAIL_HINT_PAIRS))

    def test_detail_hint_bar_contains_c_branch(self) -> None:
        self.assertIn("[c] branch", _hint_bar(_DETAIL_HINT_PAIRS))

    def test_detail_hint_bar_contains_a_assets(self) -> None:
        self.assertIn("[a] assets", _hint_bar(_DETAIL_HINT_PAIRS))

    def test_detail_hint_bar_contains_scroll_with_arrows(self) -> None:
        self.assertIn("[↑↓] scroll", _hint_bar(_DETAIL_HINT_PAIRS))

    def test_detail_hint_bar_contains_page_with_arrows(self) -> None:
        self.assertIn("[⇞⇟] page", _hint_bar(_DETAIL_HINT_PAIRS))

    def test_list_hint_bar_contains_move(self) -> None:
        self.assertIn("[↑↓] move", _hint_bar(_LIST_HINT_PAIRS))

    def test_list_hint_bar_contains_select(self) -> None:
        self.assertIn("[Enter] select", _hint_bar(_LIST_HINT_PAIRS))

    def test_empty_pairs_returns_empty_string(self) -> None:
        self.assertEqual(_hint_bar([]), "")

    def test_labels_routed_through_translation_in_pt_br(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        try:
            result = _hint_bar([("q", "back")])
            self.assertIn("[q] voltar", result)
        finally:
            set_language("en")

    def test_labels_unchanged_in_en(self) -> None:
        result = _hint_bar([("q", "back"), ("↑↓", "scroll")])
        self.assertIn("[q] back", result)
        self.assertIn("[↑↓] scroll", result)


class TestTuiI18n(unittest.TestCase):
    """S2-AC2: tui.py strings are wrapped in __() with pt_BR catalog entries."""

    def setUp(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def tearDown(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def test_render_list_title_projects_translated_in_pt_br(self) -> None:
        from active_collab.i18n import __ as translate
        from active_collab.i18n import set_language
        set_language("pt_BR")
        stdscr = FakeStdscr(keys=[], height=20, width=60)
        _render_list(stdscr, ["item"], 0, translate("Projects"))
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertIn("Projetos", all_text)

    def test_render_list_title_tasks_translated_in_pt_br(self) -> None:
        from active_collab.i18n import __ as translate
        from active_collab.i18n import set_language
        set_language("pt_BR")
        stdscr = FakeStdscr(keys=[], height=20, width=60)
        _render_list(stdscr, ["item"], 0, translate("Tasks"))
        all_text = " ".join(t for _, _, t, *_ in stdscr.written)
        self.assertIn("Tarefas", all_text)

    def test_render_too_small_translated_in_pt_br(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        win = FakeWindow(height=4, width=30)
        _render_too_small(win)
        all_text = win.all_text()
        self.assertIn("pequeno", all_text)

    def test_render_too_small_en_unchanged(self) -> None:
        win = FakeWindow(height=4, width=30)
        _render_too_small(win)
        all_text = win.all_text()
        self.assertIn("Terminal too small", all_text)
        self.assertIn("Resize to continue", all_text)

    def test_hint_bar_detail_screen_format(self) -> None:
        result = _hint_bar(_DETAIL_HINT_PAIRS)
        expected = "[q] back  [c] branch  [a] assets  [↑↓] scroll  [⇞⇟] page"
        self.assertEqual(result, expected)

    def test_hint_bar_detail_screen_format_in_pt_br(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        try:
            result = _hint_bar(_DETAIL_HINT_PAIRS)
            self.assertIn("voltar", result)
            self.assertIn("rolar", result)
        finally:
            set_language("en")


class TestCliI18n(unittest.TestCase):
    """S2-AC1: cli.py user-facing output translated in pt_BR, unchanged in en."""

    def setUp(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def tearDown(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def test_no_open_tasks_message_in_en(self) -> None:
        from active_collab.i18n import __
        self.assertEqual(__("No open tasks assigned to you."), "No open tasks assigned to you.")

    def test_no_open_tasks_message_in_pt_br(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("No open tasks assigned to you.")
            self.assertIn("tarefa", result.lower())
        finally:
            set_language("en")

    def test_connectivity_ok_in_en(self) -> None:
        from active_collab.i18n import __
        self.assertEqual(__("Connectivity: OK"), "Connectivity: OK")

    def test_connectivity_ok_translated_in_pt_br(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Connectivity: OK")
            self.assertIn("Conectividade", result)
        finally:
            set_language("en")

    def test_no_instances_message_in_en(self) -> None:
        from active_collab.i18n import __
        msg = __("No instances configured. Run: active_collab.py setup add")
        self.assertIn("instances configured", msg)

    def test_no_instances_message_translated_in_pt_br(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("No instances configured. Run: active_collab.py setup add")
            self.assertIn("instância", result.lower())
        finally:
            set_language("en")

    def test_instance_saved_template_in_en(self) -> None:
        from active_collab.i18n import __
        result = __("Instance '{name}' saved.").format(name="myinst")
        self.assertEqual(result, "Instance 'myinst' saved.")

    def test_instance_saved_template_translated_in_pt_br(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Instance '{name}' saved.").format(name="myinst")
            self.assertIn("myinst", result)
            self.assertNotEqual(result, "Instance 'myinst' saved.")
        finally:
            set_language("en")

    # --- newly wrapped error messages (S2C completion pass) ---

    def test_error_no_instances_configured_en_unchanged(self) -> None:
        from active_collab.i18n import __
        msg = __("Error: no instances configured. Run: active_collab.py setup add")
        self.assertIn("no instances configured", msg)

    def test_error_no_instances_configured_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Error: no instances configured. Run: active_collab.py setup add")
            self.assertIn("instância", result.lower())
            self.assertNotIn("no instances", result)
        finally:
            set_language("en")

    def test_error_instance_not_found_template_en_unchanged(self) -> None:
        from active_collab.i18n import __
        result = __("Error: instance '{name}' not found. Known: {known}").format(
            name="prod", known="dev, staging"
        )
        self.assertEqual(result, "Error: instance 'prod' not found. Known: dev, staging")

    def test_error_instance_not_found_template_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Error: instance '{name}' not found. Known: {known}").format(
                name="prod", known="dev"
            )
            self.assertIn("prod", result)
            self.assertIn("dev", result)
            self.assertNotIn("not found", result)
        finally:
            set_language("en")

    def test_error_multiple_instances_template_en_unchanged(self) -> None:
        from active_collab.i18n import __
        result = __(
            "Error: multiple instances configured ({names}). Use --instance NAME."
        ).format(names="a, b")
        expected = "Error: multiple instances configured (a, b). Use --instance NAME."
        self.assertEqual(result, expected)

    def test_error_multiple_instances_template_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __(
                "Error: multiple instances configured ({names}). Use --instance NAME."
            ).format(names="a, b")
            self.assertIn("a, b", result)
            self.assertNotIn("multiple instances configured", result)
        finally:
            set_language("en")

    def test_error_cannot_parse_task_ref_en_unchanged(self) -> None:
        from active_collab.i18n import __
        key = (
            "Error: cannot parse task ref '{ref}'."
            " Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159)."
        )
        result = __(key).format(ref="bad-ref")
        self.assertIn("bad-ref", result)
        self.assertIn("cannot parse task ref", result)

    def test_error_cannot_parse_task_ref_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            key = (
                "Error: cannot parse task ref '{ref}'."
                " Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159)."
            )
            result = __(key).format(ref="bad-ref")
            self.assertIn("bad-ref", result)
            self.assertNotIn("cannot parse task ref", result)
        finally:
            set_language("en")

    def test_error_task_not_found_http_en_unchanged(self) -> None:
        from active_collab.i18n import __
        result = __("Error: task {p}/{t} not found (HTTP {status}).").format(
            p=10, t=42, status=404
        )
        self.assertEqual(result, "Error: task 10/42 not found (HTTP 404).")

    def test_error_task_not_found_http_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Error: task {p}/{t} not found (HTTP {status}).").format(
                p=10, t=42, status=404
            )
            self.assertIn("10", result)
            self.assertIn("42", result)
            self.assertIn("404", result)
            self.assertNotIn("not found", result)
        finally:
            set_language("en")

    def test_error_name_url_email_required_en_unchanged(self) -> None:
        from active_collab.i18n import __
        result = __("Error: --name, --url and --email are required.")
        self.assertEqual(result, "Error: --name, --url and --email are required.")

    def test_error_name_url_email_required_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Error: --name, --url and --email are required.")
            self.assertIn("--name", result)
            self.assertNotIn("are required", result)
        finally:
            set_language("en")

    def test_error_password_required_en_unchanged(self) -> None:
        from active_collab.i18n import __
        result = __("Error: password is required.")
        self.assertEqual(result, "Error: password is required.")

    def test_error_password_required_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Error: password is required.")
            self.assertNotIn("password is required", result)
        finally:
            set_language("en")

    def test_error_not_in_git_repo_en_unchanged(self) -> None:
        from active_collab.i18n import __
        result = __("Error: not in a git repository or HEAD is detached.")
        self.assertEqual(result, "Error: not in a git repository or HEAD is detached.")

    def test_error_not_in_git_repo_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            result = __("Error: not in a git repository or HEAD is detached.")
            self.assertNotIn("not in a git repository", result)
        finally:
            set_language("en")

    def test_error_branch_pattern_template_en_unchanged(self) -> None:
        from active_collab.i18n import __
        key = (
            "Error: branch '{branch}' does not match expected pattern"
            " (feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159)."
        )
        result = __(key).format(branch="main")
        self.assertIn("main", result)
        self.assertIn("does not match", result)

    def test_error_branch_pattern_template_pt_br_translated(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            key = (
                "Error: branch '{branch}' does not match expected pattern"
                " (feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159)."
            )
            result = __(key).format(branch="main")
            self.assertIn("main", result)
            self.assertNotIn("does not match", result)
        finally:
            set_language("en")


class TestTuiInstanceResolutionI18n(unittest.TestCase):
    """S2C-AC1: tui.py instance-resolution error strings translate under pt_BR."""

    def setUp(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def tearDown(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def _inst(self, name: str) -> Instance:
        return _make_instance_named(name)

    def test_no_instances_error_en_unchanged(self) -> None:
        _, err = _resolve_browse_instance([], None)
        self.assertIsNotNone(err)
        assert err is not None
        self.assertIn("no instances configured", err)

    def test_no_instances_error_pt_br_translated(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        try:
            _, err = _resolve_browse_instance([], None)
            self.assertIsNotNone(err)
            assert err is not None
            self.assertIn("instância", err.lower())
            self.assertNotIn("no instances", err)
        finally:
            set_language("en")

    def test_unknown_instance_error_en_contains_name(self) -> None:
        inst = self._inst("prod")
        _, err = _resolve_browse_instance([inst], "ghost")
        self.assertIsNotNone(err)
        assert err is not None
        self.assertIn("ghost", err)
        self.assertIn("not found", err)

    def test_unknown_instance_error_pt_br_translated(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        try:
            inst = self._inst("prod")
            _, err = _resolve_browse_instance([inst], "ghost")
            self.assertIsNotNone(err)
            assert err is not None
            self.assertIn("ghost", err)
            self.assertNotIn("not found", err)
        finally:
            set_language("en")

    def test_multiple_instances_error_en_contains_instance_flag(self) -> None:
        inst_a = self._inst("alpha")
        inst_b = self._inst("beta")
        _, err = _resolve_browse_instance([inst_a, inst_b], None)
        self.assertIsNotNone(err)
        assert err is not None
        self.assertIn("--instance", err)

    def test_multiple_instances_error_pt_br_translated(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        try:
            inst_a = self._inst("alpha")
            inst_b = self._inst("beta")
            _, err = _resolve_browse_instance([inst_a, inst_b], None)
            self.assertIsNotNone(err)
            assert err is not None
            self.assertIn("--instance", err)
            self.assertNotIn("multiple instances (", err)
        finally:
            set_language("en")


class TestMetaRows(unittest.TestCase):
    """S3-AC1: _meta_rows returns structured (label, value) tuples from task_dict."""

    def _task(self, **kwargs: object) -> dict:
        base = {
            "id": 42,
            "task_number": 7,
            "name": "My Task",
            "is_completed": False,
            "assignee_id": None,
            "start_on": None,
            "due_on": None,
            "estimate": None,
            "tracked_time": None,
        }
        base.update(kwargs)
        return base

    def test_returns_list_of_tuples(self) -> None:
        rows = _meta_rows(self._task())
        self.assertIsInstance(rows, list)
        for item in rows:
            self.assertIsInstance(item, tuple)
            self.assertEqual(len(item), 2)

    def test_task_label_and_number_present(self) -> None:
        rows = _meta_rows(self._task(task_number=7))
        labels_values = dict(rows)
        from active_collab.i18n import __
        task_label = __("Task")
        self.assertIn(task_label, labels_values)
        self.assertIn("#7", labels_values[task_label])

    def test_status_open_when_not_completed(self) -> None:
        rows = _meta_rows(self._task(is_completed=False))
        values = [v for _, v in rows]
        from active_collab.i18n import __
        self.assertIn(__("Open"), values)

    def test_status_completed_when_is_completed(self) -> None:
        rows = _meta_rows(self._task(is_completed=True))
        values = [v for _, v in rows]
        from active_collab.i18n import __
        self.assertIn(__("Completed"), values)

    def test_assignee_unassigned_when_none(self) -> None:
        rows = _meta_rows(self._task(assignee_id=None))
        values = [v for _, v in rows]
        from active_collab.i18n import __
        self.assertIn(__("(unassigned)"), values)

    def test_assignee_shows_id_when_set(self) -> None:
        rows = _meta_rows(self._task(assignee_id=99))
        values = [v for _, v in rows]
        self.assertTrue(any("99" in v for v in values))

    def test_start_date_included_when_set(self) -> None:
        rows = _meta_rows(self._task(start_on=1700000000))
        labels = [label for label, _ in rows]
        from active_collab.i18n import __
        self.assertIn(__("Start"), labels)

    def test_start_date_omitted_when_none(self) -> None:
        rows = _meta_rows(self._task(start_on=None))
        labels = [label for label, _ in rows]
        from active_collab.i18n import __
        self.assertNotIn(__("Start"), labels)

    def test_due_date_included_when_set(self) -> None:
        rows = _meta_rows(self._task(due_on=1700000000))
        labels = [label for label, _ in rows]
        from active_collab.i18n import __
        self.assertIn(__("Due"), labels)

    def test_estimate_and_logged_always_present(self) -> None:
        rows = _meta_rows(self._task())
        labels = [label for label, _ in rows]
        from active_collab.i18n import __
        self.assertIn(__("Estimate"), labels)
        self.assertIn(__("Logged"), labels)

    def test_name_not_in_rows(self) -> None:
        rows = _meta_rows(self._task(name="Should Not Appear In Grid"))
        values = [v for _, v in rows]
        self.assertFalse(
            any("Should Not Appear In Grid" in v for v in values),
            "Task name must stay in the frame title, not in meta rows",
        )

    def test_estimate_shows_hours_suffix(self) -> None:
        rows = _meta_rows(self._task(estimate=4))
        values = [v for _, v in rows]
        self.assertTrue(any("h" in v for v in values))


class TestMetaTable(unittest.TestCase):
    """S3-AC1: _meta_table renders a full-grid bordered table; no line exceeds width."""

    def _rows(self) -> list[tuple[str, str]]:
        return [
            ("Task", "#7"),
            ("Status", "Open"),
            ("Assignee", "(unassigned)"),
            ("Estimate", "0h"),
            ("Logged", "0h"),
        ]

    def test_returns_list_of_strings(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        self.assertIsInstance(result, list)
        self.assertTrue(all(isinstance(line, str) for line in result))

    def test_no_line_exceeds_width(self) -> None:
        width = 60
        result = _meta_table(self._rows(), width, "Details")
        for line in result:
            self.assertLessEqual(
                len(line), width, f"Line too long ({len(line)}): {line!r}"
            )

    def test_top_border_starts_with_rounded_corner(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        self.assertTrue(result[0].startswith("╭"), f"Expected ╭, got: {result[0]!r}")

    def test_top_border_ends_with_rounded_corner(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        self.assertTrue(result[0].endswith("╮"), f"Expected ╮, got: {result[0]!r}")

    def test_bottom_border_starts_with_rounded_corner(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        self.assertTrue(result[-1].startswith("╰"), f"Expected ╰, got: {result[-1]!r}")

    def test_bottom_border_ends_with_rounded_corner(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        self.assertTrue(result[-1].endswith("╯"), f"Expected ╯, got: {result[-1]!r}")

    def test_title_embedded_in_top_border(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        self.assertIn("Details", result[0])

    def test_separator_rows_between_fields(self) -> None:
        rows = self._rows()
        result = _meta_table(rows, 60, "Details")
        sep_lines = [line for line in result if "├" in line and "┼" in line and "┤" in line]
        self.assertEqual(
            len(sep_lines), len(rows) - 1,
            f"Expected {len(rows) - 1} separators, got {len(sep_lines)}: {result}",
        )

    def test_separator_uses_ltee_cross_rtee(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        sep_lines = [line for line in result if "├" in line]
        self.assertTrue(len(sep_lines) > 0, "No separator line found")
        for line in sep_lines:
            self.assertIn("┼", line, f"Missing cross in separator: {line!r}")
            self.assertTrue(line.endswith("┤"), f"Missing ┤ at end of separator: {line!r}")

    def test_value_columns_separated_by_pipe(self) -> None:
        result = _meta_table(self._rows(), 60, "Details")
        data_lines = [line for line in result if line.startswith("│")]
        self.assertTrue(len(data_lines) > 0, "No data lines found")
        for line in data_lines:
            self.assertIn("│", line[1:], f"Missing interior │ in data line: {line!r}")

    def test_returns_empty_for_too_narrow_width(self) -> None:
        result = _meta_table(self._rows(), 5, "Details")
        self.assertEqual(result, [])

    def test_values_truncated_to_fit(self) -> None:
        long_value = "x" * 200
        rows = [("Label", long_value)]
        result = _meta_table(rows, 40, "Details")
        for line in result:
            self.assertLessEqual(
                len(line), 40, f"Line not truncated to width: {line!r}"
            )

    def test_empty_rows_produces_minimal_table(self) -> None:
        result = _meta_table([], 40, "Details")
        self.assertIsInstance(result, list)

    def test_no_line_exceeds_width_narrow(self) -> None:
        result = _meta_table(self._rows(), 30, "Details")
        for line in result:
            self.assertLessEqual(len(line), 30, f"Line too long: {line!r}")

    def test_pt_br_title_used_when_set(self) -> None:
        from active_collab.i18n import __, set_language
        set_language("pt_BR")
        try:
            title = __("Details")
            result = _meta_table(self._rows(), 60, title)
            self.assertIn("Detalhes", result[0])
        finally:
            set_language("en")


class TestArtifactsPanel(unittest.TestCase):
    """S3-AC2: _artifacts_panel renders a bordered box; empty for no assets."""

    def _asset(self, name: str, url: str) -> Asset:
        return Asset(name=name, url=url, kind="link")

    def test_empty_list_returns_empty(self) -> None:
        result = _artifacts_panel([], 60)
        self.assertEqual(result, [])

    def test_too_narrow_returns_empty(self) -> None:
        assets = [self._asset("file.png", "https://example.com/file.png")]
        result = _artifacts_panel(assets, 5)
        self.assertEqual(result, [])

    def test_top_border_starts_with_rounded_corner(self) -> None:
        assets = [self._asset("photo.jpg", "https://example.com/photo.jpg")]
        result = _artifacts_panel(assets, 60)
        self.assertTrue(result[0].startswith("╭"), f"Expected ╭: {result[0]!r}")

    def test_bottom_border_ends_with_rounded_corner(self) -> None:
        assets = [self._asset("photo.jpg", "https://example.com/photo.jpg")]
        result = _artifacts_panel(assets, 60)
        self.assertTrue(result[-1].endswith("╯"), f"Expected ╯: {result[-1]!r}")

    def test_artifacts_title_in_top_border(self) -> None:
        from active_collab.i18n import __
        assets = [self._asset("file.png", "https://example.com/file.png")]
        result = _artifacts_panel(assets, 60)
        self.assertIn(__("Artifacts"), result[0])

    def test_first_asset_shows_n1_label(self) -> None:
        assets = [self._asset("report.pdf", "https://example.com/report.pdf")]
        result = _artifacts_panel(assets, 60)
        content_lines = "\n".join(result)
        self.assertIn("[1]", content_lines)
        self.assertIn("report.pdf", content_lines)

    def test_second_asset_shows_n2_label(self) -> None:
        assets = [
            self._asset("first.pdf", "https://example.com/first.pdf"),
            self._asset("second.pdf", "https://example.com/second.pdf"),
        ]
        result = _artifacts_panel(assets, 60)
        content_lines = "\n".join(result)
        self.assertIn("[2]", content_lines)

    def test_url_shown_on_indented_line(self) -> None:
        url = "https://example.com/image.png"
        assets = [self._asset("image.png", url)]
        result = _artifacts_panel(assets, 80)
        content_lines = "\n".join(result)
        self.assertIn(url, content_lines)

    def test_no_line_exceeds_width(self) -> None:
        assets = [
            self._asset("long-name-file.png", "https://example.com/very/long/path/image.png"),
        ]
        width = 50
        result = _artifacts_panel(assets, width)
        for line in result:
            self.assertLessEqual(len(line), width, f"Line too long: {line!r}")

    def test_multiple_assets_each_has_label_and_url(self) -> None:
        assets = [
            self._asset(f"file{i}.pdf", f"https://example.com/file{i}.pdf")
            for i in range(3)
        ]
        result = _artifacts_panel(assets, 80)
        content = "\n".join(result)
        for i in range(1, 4):
            self.assertIn(f"[{i}]", content)

    def test_pt_br_title_in_top_border(self) -> None:
        from active_collab.i18n import set_language
        set_language("pt_BR")
        try:
            assets = [self._asset("file.pdf", "https://example.com/file.pdf")]
            result = _artifacts_panel(assets, 60)
            self.assertIn("Anexos", result[0])
        finally:
            set_language("en")


class TestOpenAssetByDigit(unittest.TestCase):
    """S3-AC2: digit keys 1-9 open matching asset; out-of-range is a no-op."""

    def _asset(self, name: str) -> Asset:
        return Asset(name=name, url=f"https://example.com/{name}", kind="link")

    def _controller_with_tracking(self) -> tuple[object, list[Asset]]:
        opened: list[Asset] = []

        class TrackingController:
            def open_asset(self, asset: Asset) -> None:
                opened.append(asset)

        return TrackingController(), opened

    def test_key_1_opens_first_asset(self) -> None:
        assets = [self._asset("a.pdf"), self._asset("b.pdf")]
        ctrl, opened = self._controller_with_tracking()
        _open_asset_by_digit(ord("1"), assets, ctrl)  # type: ignore[arg-type]
        self.assertEqual(len(opened), 1)
        self.assertEqual(opened[0].name, "a.pdf")

    def test_key_2_opens_second_asset(self) -> None:
        assets = [self._asset("a.pdf"), self._asset("b.pdf"), self._asset("c.pdf")]
        ctrl, opened = self._controller_with_tracking()
        _open_asset_by_digit(ord("2"), assets, ctrl)  # type: ignore[arg-type]
        self.assertEqual(len(opened), 1)
        self.assertEqual(opened[0].name, "b.pdf")

    def test_key_9_opens_ninth_asset(self) -> None:
        assets = [self._asset(f"f{i}.pdf") for i in range(9)]
        ctrl, opened = self._controller_with_tracking()
        _open_asset_by_digit(ord("9"), assets, ctrl)  # type: ignore[arg-type]
        self.assertEqual(opened[0].name, "f8.pdf")

    def test_out_of_range_key_is_noop(self) -> None:
        assets = [self._asset("a.pdf")]
        ctrl, opened = self._controller_with_tracking()
        _open_asset_by_digit(ord("5"), assets, ctrl)  # type: ignore[arg-type]
        self.assertEqual(len(opened), 0)

    def test_key_1_with_empty_list_is_noop(self) -> None:
        ctrl, opened = self._controller_with_tracking()
        _open_asset_by_digit(ord("1"), [], ctrl)  # type: ignore[arg-type]
        self.assertEqual(len(opened), 0)


class TestDetailViewDigitKeyIntegration(unittest.TestCase):
    """S3-AC2: digit keys in _render_and_handle_detail open assets via controller."""

    def _asset(self, name: str, url: str) -> Asset:
        return Asset(name=name, url=url, kind="link")

    def test_digit_1_key_opens_first_asset(self) -> None:
        opened: list[Asset] = []
        asset = self._asset("doc.pdf", "https://example.com/doc.pdf")

        class TrackingController:
            def task_detail(self, pid: int, tid: int) -> tuple[dict, list, list]:
                return (
                    {"id": 1, "task_number": 1, "name": "T", "is_completed": False},
                    [],
                    [asset],
                )

            def open_asset(self, a: Asset) -> None:
                opened.append(a)

            def create_task_branch(self, *_args: object) -> object:
                from active_collab.gitbranch import BranchResult, BranchStatus
                return BranchResult(status=BranchStatus.created, name="feature/1-1")

        task = _make_scrollable_task()
        stdscr = FakeStdscrScrollable(keys=[ord("1"), ord("q")], height=24, width=80)
        _render_and_handle_detail(stdscr, TrackingController(), 1, task)  # type: ignore[arg-type]
        self.assertEqual(len(opened), 1)
        self.assertIs(opened[0], asset)

    def test_out_of_range_digit_does_not_open_any_asset(self) -> None:
        opened: list[Asset] = []
        asset = self._asset("doc.pdf", "https://example.com/doc.pdf")

        class TrackingController:
            def task_detail(self, pid: int, tid: int) -> tuple[dict, list, list]:
                return (
                    {"id": 1, "task_number": 1, "name": "T", "is_completed": False},
                    [],
                    [asset],
                )

            def open_asset(self, a: Asset) -> None:
                opened.append(a)

            def create_task_branch(self, *_args: object) -> object:
                from active_collab.gitbranch import BranchResult, BranchStatus
                return BranchResult(status=BranchStatus.created, name="feature/1-1")

        task = _make_scrollable_task()
        stdscr = FakeStdscrScrollable(keys=[ord("5"), ord("q")], height=24, width=80)
        _render_and_handle_detail(stdscr, TrackingController(), 1, task)  # type: ignore[arg-type]
        self.assertEqual(len(opened), 0)


class TestBuildDetailLinesNewLayout(unittest.TestCase):
    """S3-AC3: build_detail_lines composes meta table + description + artifacts + comments."""

    def _rows(self) -> list[tuple[str, str]]:
        return [("Task", "#1"), ("Status", "Open")]

    def _asset(self, name: str, url: str) -> Asset:
        return Asset(name=name, url=url, kind="link")

    def test_meta_table_appears_before_description(self) -> None:
        rows = self._rows()
        lines = build_detail_lines("Description body", [], 60, meta_rows=rows)
        first_box = next((i for i, line in enumerate(lines) if line.startswith("╭")), -1)
        desc_idx = next((i for i, line in enumerate(lines) if "Description" in line), -1)
        self.assertGreater(
            desc_idx, first_box, "Description heading must come after meta table"
        )

    def test_artifacts_panel_appears_in_output_when_assets_provided(self) -> None:
        rows = self._rows()
        assets = [self._asset("file.pdf", "https://example.com/file.pdf")]
        lines = build_detail_lines("body", [], 60, meta_rows=rows, asset_list=assets)
        content = "\n".join(lines)
        self.assertIn("[1]", content)

    def test_no_artifacts_panel_when_no_assets(self) -> None:
        rows = self._rows()
        lines = build_detail_lines("body", [], 60, meta_rows=rows, asset_list=[])
        content = "\n".join(lines)
        self.assertNotIn("Artifacts", content)

    def test_comment_boxes_appear_after_description(self) -> None:
        rows = self._rows()
        comment = {"created_by_name": "Alice", "created_on": 0, "body": "A comment"}
        lines = build_detail_lines("body", [comment], 60, meta_rows=rows)
        desc_idx = next((i for i, line in enumerate(lines) if "Description" in line), -1)
        box_indices = [i for i, line in enumerate(lines) if line.startswith("╭")]
        last_box = max(box_indices)
        self.assertGreater(
            last_box, desc_idx, "Comment box must appear after Description heading"
        )

    def test_backward_compat_no_meta_rows_uses_wrapped_text(self) -> None:
        lines = build_detail_lines("plain meta text", [], 60)
        combined = "\n".join(lines)
        self.assertIn("plain meta text", combined)

    def test_no_line_exceeds_inner_width_with_new_layout(self) -> None:
        rows = self._rows()
        assets = [self._asset("img.png", "https://example.com/img.png")]
        comment = {"created_by_name": "Bob", "created_on": 0, "body": "body"}
        lines = build_detail_lines("desc", [comment], 60, meta_rows=rows, asset_list=assets)
        for line in lines:
            self.assertLessEqual(len(line), 60, f"Line too long: {line!r}")


class TestDetailFooterHasDigitOpen(unittest.TestCase):
    """S3-AC3: detail footer includes '[1-9] open' cap when assets present."""

    def test_footer_contains_1_9_open_when_assets_present(self) -> None:
        opened: list = []
        asset = Asset(name="f.pdf", url="https://example.com/f.pdf", kind="link")

        class TrackingController:
            def task_detail(self, pid: int, tid: int) -> tuple[dict, list, list]:
                return (
                    {"id": 1, "task_number": 1, "name": "T", "is_completed": False},
                    [],
                    [asset],
                )

            def open_asset(self, a: object) -> None:
                opened.append(a)

            def create_task_branch(self, *_a: object) -> object:
                from active_collab.gitbranch import BranchResult, BranchStatus
                return BranchResult(status=BranchStatus.created, name="f/1-1")

        stdscr = FakeStdscrScrollable(keys=[ord("q")], height=24, width=80)
        task = _make_scrollable_task()
        _render_and_handle_detail(stdscr, TrackingController(), 1, task)  # type: ignore[arg-type]
        all_text = stdscr.text_written()
        self.assertIn("[1-9]", all_text)
        self.assertIn("open", all_text)

    def test_footer_omits_1_9_open_when_no_assets(self) -> None:
        class NoAssetsController:
            def task_detail(self, pid: int, tid: int) -> tuple[dict, list, list]:
                return (
                    {"id": 1, "task_number": 1, "name": "T", "is_completed": False},
                    [],
                    [],
                )

            def open_asset(self, a: object) -> None:
                pass

            def create_task_branch(self, *_a: object) -> object:
                from active_collab.gitbranch import BranchResult, BranchStatus
                return BranchResult(status=BranchStatus.created, name="f/1-1")

        stdscr = FakeStdscrScrollable(keys=[ord("q")], height=24, width=80)
        task = _make_scrollable_task()
        _render_and_handle_detail(stdscr, NoAssetsController(), 1, task)  # type: ignore[arg-type]
        all_text = stdscr.text_written()
        self.assertNotIn("[1-9]", all_text)


class TestTtyGuardI18n(unittest.TestCase):
    """S3-AC4: browse TTY-guard error string is wrapped through __() with pt_BR entry."""

    def setUp(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def tearDown(self) -> None:
        from active_collab.i18n import set_language
        set_language("en")

    def test_tty_guard_error_translated_in_pt_br(self) -> None:
        import contextlib
        import io
        import types

        from active_collab import tui
        from active_collab.i18n import set_language

        set_language("pt_BR")
        stderr_buf = io.StringIO()
        args = types.SimpleNamespace(instance=None)
        with (
            patch.object(tui.sys.stdin, "isatty", return_value=False),
            patch.object(tui.sys.stdout, "isatty", return_value=False),
            contextlib.redirect_stderr(stderr_buf),
        ):
            tui.run(args)
        output = stderr_buf.getvalue()
        self.assertIn("TTY", output)
        self.assertNotIn("'browse' requires an interactive terminal", output)

    def test_tty_guard_error_en_unchanged(self) -> None:
        import contextlib
        import io
        import types

        from active_collab import tui

        stderr_buf = io.StringIO()
        args = types.SimpleNamespace(instance=None)
        with (
            patch.object(tui.sys.stdin, "isatty", return_value=False),
            patch.object(tui.sys.stdout, "isatty", return_value=False),
            contextlib.redirect_stderr(stderr_buf),
        ):
            tui.run(args)
        output = stderr_buf.getvalue()
        self.assertIn("browse", output)
        self.assertIn("TTY", output)


if __name__ == "__main__":
    unittest.main()
