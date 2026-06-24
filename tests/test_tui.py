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
    _draw_frame,
    _init_colors,
    _render_list,
    _render_too_small,
    _resolve_browse_instance,
    _safe_addstr,
    _screen_mine_list,
    _truncate,
    _visible_window,
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
        self.assertIn("↑/↓", all_text)

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


if __name__ == "__main__":
    unittest.main()
