"""Tests for gitbranch.py — branch naming, existence check, creation."""

import unittest
from dataclasses import dataclass
from typing import Callable

from active_collab.gitbranch import (
    BranchStatus,
    build_branch_name,
    create_branch,
)

_MASTER_CHECK = ("git", "rev-parse", "--verify", "master")
_MAIN_CHECK = ("git", "rev-parse", "--verify", "main")


@dataclass
class FakeRunResult:
    returncode: int
    stdout: str = ""
    stderr: str = ""


def _fake_run_factory(responses: dict[tuple, FakeRunResult]) -> Callable:
    """Return a fake subprocess.run keyed by argv tuple."""
    calls: list[list] = []

    def fake_run(argv: list, **_kwargs) -> FakeRunResult:
        calls.append(argv)
        key = tuple(argv)
        if key in responses:
            return responses[key]
        return FakeRunResult(returncode=1, stderr="unexpected command")

    fake_run.calls = calls  # type: ignore[attr-defined]
    return fake_run


class TestBuildBranchName(unittest.TestCase):
    def test_feature_type_produces_correct_name(self) -> None:
        self.assertEqual(
            build_branch_name("feature", 10, 200), "feature/10-200"
        )

    def test_fix_type_produces_correct_name(self) -> None:
        self.assertEqual(build_branch_name("fix", 10, 200), "fix/10-200")

    def test_hotfix_type_produces_correct_name(self) -> None:
        self.assertEqual(build_branch_name("hotfix", 5, 99), "hotfix/5-99")

    def test_none_branch_type_defaults_to_feature(self) -> None:
        self.assertEqual(build_branch_name(None, 10, 200), "feature/10-200")

    def test_empty_string_defaults_to_feature(self) -> None:
        self.assertEqual(build_branch_name("", 10, 200), "feature/10-200")

    def test_whitespace_only_defaults_to_feature(self) -> None:
        self.assertEqual(build_branch_name("  ", 10, 200), "feature/10-200")

    def test_unknown_type_raises_value_error(self) -> None:
        with self.assertRaises(ValueError):
            build_branch_name("release", 10, 200)

    def test_unknown_type_error_message_names_the_bad_value(self) -> None:
        with self.assertRaises(ValueError) as ctx:
            build_branch_name("bogus", 10, 200)
        self.assertIn("bogus", str(ctx.exception))

    def test_project_and_task_ids_appear_in_output(self) -> None:
        name = build_branch_name("feature", 42, 99)
        self.assertIn("42", name)
        self.assertIn("99", name)


class TestCreateBranch(unittest.TestCase):
    def _rev_parse_argv(self, name: str) -> tuple:
        return ("git", "rev-parse", "--verify", name)

    def _checkout_argv(self, name: str, base: str = "master") -> tuple:
        return ("git", "checkout", "-b", name, base)

    def _responses_for_create(
        self, name: str, base: str = "master", checkout_rc: int = 0,
        checkout_err: str = "",
    ) -> dict:
        """Build the standard fake-run response dict for a create flow."""
        responses: dict[tuple, FakeRunResult] = {
            self._rev_parse_argv(name): FakeRunResult(returncode=1),
            _MASTER_CHECK: FakeRunResult(returncode=0),
            self._checkout_argv(name, base): FakeRunResult(
                returncode=checkout_rc,
                stderr=checkout_err,
            ),
        }
        return responses

    def test_creates_branch_when_it_does_not_exist(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(self._responses_for_create(name))
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.created)
        self.assertEqual(result.name, name)

    def test_checkout_argv_includes_master_base_ref(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(self._responses_for_create(name))
        create_branch(name, run=run)
        issued = [tuple(c) for c in run.calls]
        self.assertIn(self._checkout_argv(name, "master"), issued)

    def test_checkout_argv_does_not_branch_from_head(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(self._responses_for_create(name))
        create_branch(name, run=run)
        for call in run.calls:
            argv = tuple(call)
            if "-b" in argv:
                self.assertGreater(
                    len(argv), 4,
                    "checkout -b must include a base ref, not just the name",
                )

    def test_returns_exists_when_branch_already_exists(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=0),
            }
        )
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.exists)
        called = [tuple(c) for c in run.calls]
        self.assertNotIn(self._checkout_argv(name), called)

    def test_no_checkout_call_when_branch_exists(self) -> None:
        name = "hotfix/5-99"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=0),
            }
        )
        create_branch(name, run=run)
        all_calls = [tuple(c) for c in run.calls]
        for call in all_calls:
            self.assertNotIn("-b", call)
            self.assertNotIn("-B", call)

    def test_not_a_repo_detected_from_stderr(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(
                    returncode=128, stderr="fatal: not a git repository"
                ),
            }
        )
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.not_a_repo)

    def test_error_status_returned_on_checkout_failure(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            self._responses_for_create(
                name, checkout_rc=1, checkout_err="some git error"
            )
        )
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.error)
        self.assertIn("some git error", result.message)

    def test_never_calls_force_checkout(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(self._responses_for_create(name))
        create_branch(name, run=run)
        for call in run.calls:
            self.assertNotIn("-B", call)

    def test_checks_existence_before_creating(self) -> None:
        name = "fix/3-77"
        run = _fake_run_factory(self._responses_for_create(name))
        create_branch(name, run=run)
        self.assertGreater(len(run.calls), 0)
        first_call = tuple(run.calls[0])
        self.assertEqual(first_call, self._rev_parse_argv(name))

    def test_create_branch_result_contains_branch_name(self) -> None:
        name = "feature/99-1234"
        run = _fake_run_factory(self._responses_for_create(name))
        result = create_branch(name, run=run)
        self.assertEqual(result.name, name)

    def test_falls_back_to_main_when_master_absent(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=1),
                _MASTER_CHECK: FakeRunResult(returncode=1),
                _MAIN_CHECK: FakeRunResult(returncode=0),
                self._checkout_argv(name, "main"): FakeRunResult(
                    returncode=0
                ),
            }
        )
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.created)
        issued = [tuple(c) for c in run.calls]
        self.assertIn(self._checkout_argv(name, "main"), issued)

    def test_checkout_uses_main_not_master_on_fallback(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=1),
                _MASTER_CHECK: FakeRunResult(returncode=1),
                _MAIN_CHECK: FakeRunResult(returncode=0),
                self._checkout_argv(name, "main"): FakeRunResult(
                    returncode=0
                ),
            }
        )
        create_branch(name, run=run)
        issued = [tuple(c) for c in run.calls]
        self.assertNotIn(self._checkout_argv(name, "master"), issued)
        self.assertIn(self._checkout_argv(name, "main"), issued)

    def test_base_missing_when_neither_master_nor_main_exists(
        self,
    ) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=1),
                _MASTER_CHECK: FakeRunResult(returncode=1),
                _MAIN_CHECK: FakeRunResult(returncode=1),
            }
        )
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.base_missing)

    def test_base_missing_does_not_call_checkout(self) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=1),
                _MASTER_CHECK: FakeRunResult(returncode=1),
                _MAIN_CHECK: FakeRunResult(returncode=1),
            }
        )
        create_branch(name, run=run)
        for call in run.calls:
            self.assertNotIn("-b", tuple(call))

    def test_exists_still_returned_even_when_base_is_absent(
        self,
    ) -> None:
        name = "feature/10-200"
        run = _fake_run_factory(
            {
                self._rev_parse_argv(name): FakeRunResult(returncode=0),
            }
        )
        result = create_branch(name, run=run)
        self.assertEqual(result.status, BranchStatus.exists)


class TestRenderTaskToStr(unittest.TestCase):
    """Verify render_task_to_str matches the print-based render_task output."""

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

    def test_task_number_in_output(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertIn("42", out)

    def test_task_name_in_output(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertIn("Implement login flow", out)

    def test_status_open_in_output(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertIn("Open", out)

    def test_assignee_with_name_in_output(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertIn("Maiara Gutierre (486)", out)

    def test_due_date_in_output(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertIn("Due:       2026-06-09", out)

    def test_html_stripped_from_body(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertNotIn("<p>", out)
        self.assertIn("login page", out)

    def test_html_entities_unescaped(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, self.USERS)
        self.assertIn("& done", out)

    def test_comments_included_when_no_comments_false(self) -> None:
        from active_collab.render import render_task_to_str
        comments = [{
            "created_by_name": "Alice",
            "body_plain_text": "Visible comment",
        }]
        out = render_task_to_str(self.TASK, comments, False, self.USERS)
        self.assertIn("Visible comment", out)

    def test_comments_excluded_when_no_comments_true(self) -> None:
        from active_collab.render import render_task_to_str
        comments = [{
            "created_by_name": "Alice",
            "body_plain_text": "Hidden comment",
        }]
        out = render_task_to_str(self.TASK, comments, True, self.USERS)
        self.assertNotIn("Hidden comment", out)

    def test_returns_string_not_none(self) -> None:
        from active_collab.render import render_task_to_str
        out = render_task_to_str(self.TASK, [], False, {})
        self.assertIsInstance(out, str)
