"""Tests for render.py — pure formatting functions; no network, no DB."""

import io
import sys
import unittest

from active_collab import render
from active_collab.i18n import set_language


class TestHtmlToText(unittest.TestCase):
    def test_strips_paragraph_tags(self) -> None:
        result = render.html_to_text("<p>Hello world</p>")
        self.assertEqual(result, "Hello world")

    def test_unescapes_html_entities(self) -> None:
        result = render.html_to_text("<p>A &amp; B &lt;here&gt;</p>")
        self.assertIn("A & B", result)
        self.assertIn("<here>", result)

    def test_br_becomes_newline(self) -> None:
        result = render.html_to_text("Line1<br>Line2")
        self.assertIn("\n", result)

    def test_empty_string_returns_empty(self) -> None:
        self.assertEqual(render.html_to_text(""), "")

    def test_none_handled_gracefully(self) -> None:
        self.assertEqual(render.html_to_text(None), "")

    def test_strips_inline_tags(self) -> None:
        result = render.html_to_text("<strong>bold</strong> text")
        self.assertEqual(result, "bold text")

    def test_div_becomes_newline(self) -> None:
        result = render.html_to_text("<div>A</div><div>B</div>")
        self.assertIn("\n", result)


class TestFmtTs(unittest.TestCase):
    def test_unix_int_formats_as_utc_datetime(self) -> None:
        self.assertEqual(render.fmt_ts(0), "1970-01-01 00:00")

    def test_known_timestamp_produces_correct_date(self) -> None:
        result = render.fmt_ts(1736499600)
        self.assertTrue(result.startswith("2025-01-"))

    def test_none_returns_empty_string(self) -> None:
        self.assertEqual(render.fmt_ts(None), "")

    def test_string_passthrough(self) -> None:
        self.assertEqual(render.fmt_ts("2026-01-10T09:00:00Z"), "2026-01-10T09:00:00Z")


class TestFmtDate(unittest.TestCase):
    def test_unix_int_formats_as_date(self) -> None:
        self.assertEqual(render.fmt_date(1780963200), "2026-06-09")

    def test_none_returns_empty_string(self) -> None:
        self.assertEqual(render.fmt_date(None), "")

    def test_float_timestamp_formats_as_date(self) -> None:
        self.assertEqual(render.fmt_date(1780963200.0), "2026-06-09")

    def test_non_numeric_passthrough(self) -> None:
        self.assertEqual(render.fmt_date("2026-06-09"), "2026-06-09")


class TestFmtHours(unittest.TestCase):
    def test_whole_float_renders_as_integer(self) -> None:
        self.assertEqual(render.fmt_hours(3.0), "3")

    def test_zero_float_renders_as_zero(self) -> None:
        self.assertEqual(render.fmt_hours(0.0), "0")

    def test_fractional_renders_naturally(self) -> None:
        self.assertEqual(render.fmt_hours(1.5), "1.5")

    def test_none_returns_zero(self) -> None:
        self.assertEqual(render.fmt_hours(None), "0")

    def test_integer_input_renders_as_string(self) -> None:
        self.assertEqual(render.fmt_hours(5), "5")


class TestRenderComments(unittest.TestCase):
    def _capture(self, comments: list) -> str:
        buf = io.StringIO()
        old = sys.stdout
        sys.stdout = buf
        try:
            render.render_comments(comments)
        finally:
            sys.stdout = old
        return buf.getvalue()

    def test_empty_comments_produces_no_output(self) -> None:
        self.assertEqual(self._capture([]), "")

    def test_shows_author_and_body(self) -> None:
        comment = {
            "created_by_name": "Alice",
            "created_on": 1736499600,
            "body_plain_text": "Great work",
            "created_by_id": 5,
        }
        out = self._capture([comment])
        self.assertIn("Alice", out)
        self.assertIn("Great work", out)

    def test_falls_back_to_html_body_when_no_plain_text(self) -> None:
        comment = {
            "created_by_name": "Bob",
            "body": "<p>HTML body</p>",
            "created_by_id": 2,
        }
        out = self._capture([comment])
        self.assertIn("HTML body", out)
        self.assertNotIn("<p>", out)

    def test_shows_count_in_header(self) -> None:
        comments = [
            {"created_by_name": "A", "body_plain_text": "c1"},
            {"created_by_name": "B", "body_plain_text": "c2"},
        ]
        out = self._capture(comments)
        self.assertIn("Comments (2)", out)


class TestRenderTask(unittest.TestCase):
    TASK = {
        "id": 75159,
        "task_number": 42,
        "name": "Implement login flow",
        "is_completed": False,
        "assignee_id": 486,
        "project_id": 665,
        "body": "<p>The login page. &amp; done.</p>",
        "start_on": 1780963200,
        "due_on": 1780963200,
        "estimate": 0.0,
        "tracked_time": 3.0,
    }
    USERS = {486: "Maiara Gutierre"}

    def _capture_render(
        self,
        task: dict,
        comments: list = (),
        no_comments: bool = False,
        user_map: dict | None = None,
    ) -> str:
        buf = io.StringIO()
        old = sys.stdout
        sys.stdout = buf
        try:
            render.render_task(task, list(comments), no_comments, user_map or {})
        finally:
            sys.stdout = old
        return buf.getvalue()

    def test_renders_task_number(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("42", out)

    def test_renders_task_name(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("Implement login flow", out)

    def test_renders_open_status(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("Open", out)

    def test_renders_completed_status(self) -> None:
        task = {**self.TASK, "is_completed": True}
        out = self._capture_render(task)
        self.assertIn("Completed", out)

    def test_renders_assignee_with_name(self) -> None:
        out = self._capture_render(self.TASK, user_map=self.USERS)
        self.assertIn("Maiara Gutierre (486)", out)

    def test_renders_unassigned_when_no_assignee(self) -> None:
        task = {k: v for k, v in self.TASK.items() if k != "assignee_id"}
        out = self._capture_render(task)
        self.assertIn("(unassigned)", out)

    def test_renders_due_date(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("Due:       2026-06-09", out)

    def test_renders_start_date(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("Start:     2026-06-09", out)

    def test_omits_start_when_not_set(self) -> None:
        task = {k: v for k, v in self.TASK.items() if k != "start_on"}
        out = self._capture_render(task)
        self.assertNotIn("Start:", out)

    def test_renders_estimate_without_decimal(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("Estimate:  0h", out)

    def test_renders_logged_hours_without_decimal(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("Logged:    3h", out)

    def test_strips_html_from_body(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertNotIn("<p>", out)
        self.assertIn("login page", out)

    def test_unescapes_html_entities_in_body(self) -> None:
        out = self._capture_render(self.TASK)
        self.assertIn("& done", out)

    def test_no_comments_flag_suppresses_comments(self) -> None:
        comments = [{"created_by_name": "Alice", "body_plain_text": "Secret"}]
        out = self._capture_render(self.TASK, comments=comments, no_comments=True)
        self.assertNotIn("Secret", out)

    def test_comments_shown_when_flag_not_set(self) -> None:
        comments = [{"created_by_name": "Alice", "body_plain_text": "Visible"}]
        out = self._capture_render(self.TASK, comments=comments, no_comments=False)
        self.assertIn("Visible", out)


class TestRenderMineTable(unittest.TestCase):
    def _capture(self, tasks: list) -> str:
        buf = io.StringIO()
        old = sys.stdout
        sys.stdout = buf
        try:
            render.render_mine_table(tasks)
        finally:
            sys.stdout = old
        return buf.getvalue()

    def test_shows_instance_project_task_name(self) -> None:
        tasks = [
            {
                "instance": "myinst",
                "project_id": 665,
                "task_number": 42,
                "task_id": 75159,
                "name": "Do it",
            },
        ]
        out = self._capture(tasks)
        self.assertIn("myinst", out)
        self.assertIn("665", out)
        self.assertIn("42", out)
        self.assertIn("Do it", out)

    def test_shows_header_row(self) -> None:
        out = self._capture([])
        self.assertIn("INSTANCE", out)
        self.assertIn("PROJECT", out)
        self.assertIn("NAME", out)


class TestRenderPtBrTranslation(unittest.TestCase):
    """Verify that render output changes to pt_BR when language is set."""

    TASK = {
        "id": 75159,
        "task_number": 42,
        "name": "Implement login flow",
        "is_completed": False,
        "assignee_id": None,
        "project_id": 665,
        "body": "",
        "estimate": 0.0,
        "tracked_time": 0.0,
    }

    def setUp(self) -> None:
        set_language("pt_BR")

    def tearDown(self) -> None:
        set_language("en")

    def _render_task_str(self, task: dict) -> str:
        return render.render_task_to_str(task, [], True, {})

    def _capture_mine_table(self, tasks: list) -> str:
        buf = io.StringIO()
        old = sys.stdout
        sys.stdout = buf
        try:
            render.render_mine_table(tasks)
        finally:
            sys.stdout = old
        return buf.getvalue()

    def test_task_label_is_translated(self) -> None:
        out = self._render_task_str(self.TASK)
        self.assertIn("Tarefa:", out)
        self.assertNotIn("Task:", out)

    def test_status_open_is_translated(self) -> None:
        out = self._render_task_str(self.TASK)
        self.assertIn("Aberto", out)
        self.assertNotIn("Open", out)

    def test_status_completed_is_translated(self) -> None:
        task = {**self.TASK, "is_completed": True}
        out = self._render_task_str(task)
        self.assertIn("Concluído", out)
        self.assertNotIn("Completed", out)

    def test_unassigned_is_translated(self) -> None:
        out = self._render_task_str(self.TASK)
        self.assertIn("(não atribuído)", out)
        self.assertNotIn("(unassigned)", out)

    def test_no_description_is_translated(self) -> None:
        out = self._render_task_str(self.TASK)
        self.assertIn("(sem descrição)", out)
        self.assertNotIn("(no description)", out)

    def test_mine_table_header_is_translated(self) -> None:
        out = self._capture_mine_table([])
        self.assertIn("INSTÂNCIA", out)
        self.assertNotIn("INSTANCE", out)
