"""Tests for tui.py: navigation helpers and BrowseController (no curses)."""

import json
import unittest
from dataclasses import dataclass
from typing import Callable
from unittest.mock import MagicMock

from active_collab.assets import Asset
from active_collab.gitbranch import BranchResult, BranchStatus
from active_collab.models import Instance, MineTask
from active_collab.tui import (
    BrowseController,
    clamp_index,
    move_selection,
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


if __name__ == "__main__":
    unittest.main()
