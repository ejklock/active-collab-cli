"""Tests for cli.py — end-to-end behavior through main(); no real network or DB in ~/.config."""

import io
import json
import os
import sys
import tempfile
import unittest
from unittest import mock

from active_collab import cli
from active_collab.config import Config
from active_collab.http import HttpClient
from active_collab.models import Instance
from active_collab.store import InstanceRepository, Store

TOKEN = "SUPER_SECRET_TOKEN_MUST_NOT_APPEAR"
USER_ID = 7

TASK_SINGLE = {
    "id": 75159,
    "task_number": 42,
    "name": "Implement login flow",
    "is_completed": False,
    "is_trashed": False,
    "assignee_id": 486,
    "project_id": 665,
    "body": "<p>The login page needs to be implemented. &amp; done.</p>",
    "start_on": 1780963200,
    "due_on": 1780963200,
    "estimate": 0.0,
}

TASK_PAYLOAD = {
    "single": TASK_SINGLE,
    "tracked_time": 3.0,
    "comments": [
        {
            "id": 1,
            "body": "<p>Great start on this feature.</p>",
            "body_plain_text": "Great start on this feature.",
            "created_by_name": "Alice",
            "created_by_id": 5,
            "created_on": 1736499600,
        }
    ],
}

USERS_PAYLOAD = [
    {"id": 486, "display_name": "Maiara Gutierre", "email": "maiara@example.com"},
    {"id": 69, "display_name": "Evaldo Klock", "email": "evaldo@example.com"},
]

MINE_PAYLOAD = {
    "tasks": [
        {"id": 75159, "task_number": 42, "name": "Implement login flow",
         "is_completed": False, "is_trashed": False, "project_id": 665},
        {"id": 75160, "task_number": 43, "name": "Old task",
         "is_completed": True, "is_trashed": False, "project_id": 665},
        {"id": 75161, "task_number": 44, "name": "Trashed task",
         "is_completed": False, "is_trashed": True, "project_id": 665},
    ],
    "subtasks": [],
}


class CliTestBase(unittest.TestCase):
    """Fixtures: isolated SQLite DB, HttpClient stub, stdout/stderr capture."""

    def setUp(self) -> None:
        self._db_file = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
        self._db_file.close()
        self._db_path = self._db_file.name
        os.unlink(self._db_path)
        os.environ["ACTIVE_COLLAB_DB"] = self._db_path

        self._orig_current_branch = cli._current_branch  # noqa: SLF001
        cli._current_branch = lambda: None  # noqa: SLF001

        self._orig_stdin_check = cli._stdin_is_interactive  # noqa: SLF001
        cli._stdin_is_interactive = lambda: False  # noqa: SLF001

    def tearDown(self) -> None:
        cli._current_branch = self._orig_current_branch  # noqa: SLF001
        cli._stdin_is_interactive = self._orig_stdin_check  # noqa: SLF001
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        if os.path.exists(self._db_path):
            os.unlink(self._db_path)

    def _add_instance(
        self,
        name: str = "default",
        base_url: str = "https://collab.example.com",
        email: str = "user@example.com",
        user_id: int | None = USER_ID,
    ) -> None:
        config = Config.load()
        with Store(config) as store:
            repo = InstanceRepository(store.conn)
            repo.save(Instance(name=name, base_url=base_url, email=email, token=TOKEN, user_id=user_id))

    def _make_stub_http(self, routes: dict) -> HttpClient:
        """Build an HttpClient stub mapping url-substring -> (status, body)."""
        captured_urls: list[str] = []
        captured_headers: list[dict] = []

        class StubHttp(HttpClient):
            def get(self, url, headers=None):  # type: ignore[override]
                captured_urls.append(url)
                captured_headers.append(headers or {})
                for pattern, (status, body) in routes.items():
                    if pattern in url:
                        raw = json.dumps(body).encode() if body is not None else b""
                        return status, raw
                return 404, b'{"message":"Not Found"}'

            def post(self, url, data, headers=None):  # type: ignore[override]
                captured_urls.append(url)
                for pattern, (status, body) in routes.items():
                    if pattern in url:
                        raw = json.dumps(body).encode() if body is not None else b""
                        return status, raw
                return 404, b""

        stub = StubHttp()
        stub.captured_urls = captured_urls  # type: ignore[attr-defined]
        stub.captured_headers = captured_headers  # type: ignore[attr-defined]
        return stub

    def _run(self, argv: list) -> tuple[int, str, str]:
        old_out, old_err = sys.stdout, sys.stderr
        sys.stdout = io.StringIO()
        sys.stderr = io.StringIO()
        try:
            with mock.patch("active_collab.cli.HttpClient", return_value=self._http):
                code = cli.main(argv)
        except SystemExit as exc:
            code = exc.code if isinstance(exc.code, int) else 1
        finally:
            out = sys.stdout.getvalue()
            err = sys.stderr.getvalue()
            sys.stdout, sys.stderr = old_out, old_err
        return code, out, err

    def _assert_no_token(self, *strings: str) -> None:
        for s in strings:
            self.assertNotIn(TOKEN, s, "Token must never appear in output")


class TestBranchParsing(unittest.TestCase):
    def test_feature_branch_parses_project_and_task(self) -> None:
        self.assertEqual(cli._parse_branch_ref("feature/665-75159"), (665, 75159))  # noqa: SLF001

    def test_hotfix_branch_parses_project_and_task(self) -> None:
        self.assertEqual(cli._parse_branch_ref("hotfix/665-75159"), (665, 75159))  # noqa: SLF001

    def test_fix_branch_parses_project_and_task(self) -> None:
        self.assertEqual(cli._parse_branch_ref("fix/665-75159"), (665, 75159))  # noqa: SLF001

    def test_main_branch_returns_none(self) -> None:
        self.assertIsNone(cli._parse_branch_ref("main"))  # noqa: SLF001

    def test_non_matching_branch_returns_none(self) -> None:
        self.assertIsNone(cli._parse_branch_ref("bugfix/some-fix"))  # noqa: SLF001

    def test_feature_branch_without_task_returns_none(self) -> None:
        self.assertIsNone(cli._parse_branch_ref("feature/665"))  # noqa: SLF001

    def test_arbitrary_text_after_number_returns_none(self) -> None:
        self.assertIsNone(cli._parse_branch_ref("feature/665-75159-extra"))  # noqa: SLF001


class TestTaskRefParsing(unittest.TestCase):
    def test_full_url_parses_project_and_task(self) -> None:
        result = cli._parse_task_ref("https://collab.base.digital/projects/665/tasks/75159")  # noqa: SLF001
        self.assertEqual(result, (665, 75159))

    def test_short_form_parses_project_and_task(self) -> None:
        self.assertEqual(cli._parse_task_ref("665/75159"), (665, 75159))  # noqa: SLF001

    def test_invalid_ref_exits_2(self) -> None:
        with self.assertRaises(SystemExit) as ctx:
            cli._parse_task_ref("not-a-ref")  # noqa: SLF001
        self.assertEqual(ctx.exception.code, 2)


class TestGet(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })

    def test_get_unwraps_single_key_and_renders_name(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_get_renders_open_status(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Open", out)

    def test_get_renders_completed_status(self) -> None:
        payload = {"single": {**TASK_SINGLE, "is_completed": True}, "tracked_time": 0, "comments": []}
        self._http = self._make_stub_http({"/tasks/75159": (200, payload)})
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Completed", out)

    def test_get_renders_assignee_resolved_name(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Maiara Gutierre (486)", out)

    def test_get_strips_html_tags_from_body(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertNotIn("<p>", out)
        self.assertIn("login page", out)

    def test_get_unescapes_html_entities_in_body(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("& done", out)

    def test_get_renders_comment_body_plain_text(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Great start on this feature", out)

    def test_get_renders_comment_author_name(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Alice", out)

    def test_get_renders_comment_created_on_as_date(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("2025-01-", out)

    def test_get_no_separate_comments_request(self) -> None:
        self._add_instance()
        _, _, _ = self._run(["get", "665/75159"])
        for url in self._http.captured_urls:  # type: ignore[attr-defined]
            self.assertNotIn("/comments", url)

    def test_get_by_url_fetches_correct_task(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "https://collab.example.com/projects/665/tasks/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_get_short_flag_prints_only_ref_and_name(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159", "--short"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)
        self.assertNotIn("Description", out)
        self.assertNotIn("Status", out)

    def test_get_no_comments_omits_comments(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159", "--no-comments"])
        self.assertEqual(code, 0)
        self.assertNotIn("Great start", out)

    def test_get_json_flag_returns_full_wrapped_api_payload(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159", "--json"])
        self.assertEqual(code, 0)
        parsed = json.loads(out)
        self.assertIn("single", parsed)
        self.assertEqual(parsed["single"]["id"], 75159)

    def test_get_not_found_exits_1(self) -> None:
        self._http = self._make_stub_http({})
        self._add_instance()
        code, _, err = self._run(["get", "665/99999"])
        self.assertEqual(code, 1)
        self.assertIn("not found", err.lower())

    def test_get_no_instances_exits_2(self) -> None:
        code, _, err = self._run(["get", "665/75159"])
        self.assertEqual(code, 2)
        self.assertIn("no instances", err.lower())

    def test_get_unknown_instance_exits_2(self) -> None:
        self._add_instance(name="real-instance")
        code, _, err = self._run(["get", "665/75159", "--instance", "nonexistent"])
        self.assertEqual(code, 2)
        self.assertIn("not found", err.lower())

    def test_get_uses_cache_on_second_call(self) -> None:
        self._add_instance()
        self._run(["get", "665/75159"])
        self._http = self._make_stub_http({"/api/v1/users": (200, USERS_PAYLOAD)})
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)
        task_calls = [u for u in self._http.captured_urls if "/tasks/" in u]  # type: ignore[attr-defined]
        self.assertEqual(task_calls, [])

    def test_refresh_bypasses_cache(self) -> None:
        self._add_instance()
        self._run(["get", "665/75159"])
        before = len(self._http.captured_urls)  # type: ignore[attr-defined]
        self._run(["get", "665/75159", "--refresh"])
        after = len(self._http.captured_urls)  # type: ignore[attr-defined]
        self.assertGreater(after, before, "--refresh must call the API again")


class TestCurrent(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })

    def test_current_parses_feature_branch(self) -> None:
        self._add_instance()
        cli._current_branch = lambda: "feature/665-75159"  # noqa: SLF001
        code, out, _ = self._run(["current"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_current_non_matching_branch_exits_2(self) -> None:
        cli._current_branch = lambda: "main"  # noqa: SLF001
        code, _, err = self._run(["current"])
        self.assertEqual(code, 2)
        self.assertIn("pattern", err.lower())

    def test_current_detached_head_exits_2(self) -> None:
        cli._current_branch = lambda: None  # noqa: SLF001
        code, _, err = self._run(["current"])
        self.assertEqual(code, 2)

    def test_current_error_message_names_expected_pattern(self) -> None:
        cli._current_branch = lambda: "fix-some-bug"  # noqa: SLF001
        code, _, err = self._run(["current"])
        self.assertEqual(code, 2)
        self.assertIn("feature|hotfix|fix", err)


class TestBareInvocation(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })

    def test_project_task_ref_routes_to_get(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_empty_argv_with_matching_branch_routes_to_current(self) -> None:
        self._add_instance()
        cli._current_branch = lambda: "feature/665-75159"  # noqa: SLF001
        code, out, _ = self._run([])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_empty_argv_non_matching_branch_shows_help(self) -> None:
        cli._current_branch = lambda: "main"  # noqa: SLF001
        code, out, _ = self._run([])
        self.assertIn("usage", out.lower() + _.lower())


class TestMine(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            f"/users/{USER_ID}/tasks": (200, MINE_PAYLOAD),
        })

    def test_mine_calls_user_tasks_endpoint(self) -> None:
        self._add_instance(user_id=USER_ID)
        self._run(["mine"])
        self.assertTrue(
            any(f"/users/{USER_ID}/tasks" in u for u in self._http.captured_urls),  # type: ignore[attr-defined]
        )

    def test_mine_lists_only_open_not_trashed_tasks(self) -> None:
        self._add_instance(user_id=USER_ID)
        code, out, _ = self._run(["mine"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)
        self.assertNotIn("Old task", out)
        self.assertNotIn("Trashed task", out)

    def test_mine_excludes_completed_tasks(self) -> None:
        payload = {"tasks": [{"id": 75160, "task_number": 43, "name": "Old task",
                              "is_completed": True, "is_trashed": False, "project_id": 665}]}
        self._http = self._make_stub_http({f"/users/{USER_ID}/tasks": (200, payload)})
        self._add_instance(user_id=USER_ID)
        code, out, _ = self._run(["mine"])
        self.assertEqual(code, 0)
        self.assertNotIn("Old task", out)

    def test_mine_excludes_trashed_tasks(self) -> None:
        payload = {"tasks": [{"id": 75161, "task_number": 44, "name": "Trashed task",
                              "is_completed": False, "is_trashed": True, "project_id": 665}]}
        self._http = self._make_stub_http({f"/users/{USER_ID}/tasks": (200, payload)})
        self._add_instance(user_id=USER_ID)
        code, out, _ = self._run(["mine"])
        self.assertEqual(code, 0)
        self.assertNotIn("Trashed task", out)

    def test_mine_prints_project_id_task_number_and_name(self) -> None:
        self._add_instance(user_id=USER_ID)
        code, out, _ = self._run(["mine"])
        self.assertEqual(code, 0)
        self.assertIn("665", out)
        self.assertIn("42", out)
        self.assertIn("Implement login flow", out)

    def test_mine_empty_case_exits_0_with_message(self) -> None:
        self._http = self._make_stub_http({f"/users/{USER_ID}/tasks": (200, {"tasks": []})})
        self._add_instance(user_id=USER_ID)
        code, out, _ = self._run(["mine"])
        self.assertEqual(code, 0)
        self.assertIn("no open tasks", out.lower())

    def test_mine_no_instances_exits_2(self) -> None:
        code, _, err = self._run(["mine"])
        self.assertEqual(code, 2)
        self.assertIn("no instances", err.lower())

    def test_mine_unknown_instance_exits_2(self) -> None:
        self._add_instance(name="real")
        code, _, _ = self._run(["mine", "--instance", "ghost"])
        self.assertEqual(code, 2)

    def test_list_alias_works_same_as_mine(self) -> None:
        self._add_instance(user_id=USER_ID)
        code, out, _ = self._run(["list"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)


class TestSetupList(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({})

    def test_list_never_shows_token(self) -> None:
        self._add_instance(name="inst1")
        code, out, err = self._run(["setup", "list"])
        self.assertEqual(code, 0)
        self._assert_no_token(out, err)
        self.assertIn("inst1", out)

    def test_list_shows_instance_name_url_email(self) -> None:
        self._add_instance(name="myinst", base_url="https://collab.example.com",
                           email="user@example.com", user_id=42)
        code, out, _ = self._run(["setup", "list"])
        self.assertEqual(code, 0)
        self.assertIn("myinst", out)
        self.assertIn("collab.example.com", out)
        self.assertIn("user@example.com", out)

    def test_list_empty_exits_0(self) -> None:
        code, _, _ = self._run(["setup", "list"])
        self.assertEqual(code, 0)


class TestSetupRemove(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({})

    def test_remove_deletes_instance_and_cache(self) -> None:
        self._add_instance(name="to-remove")
        from active_collab.store import Store, TaskCache
        config = Config.load()
        with Store(config) as store:
            TaskCache(store.conn).write("to-remove", 665, 1, {}, [])

        code, _, _ = self._run(["setup", "remove", "--name", "to-remove"])
        self.assertEqual(code, 0)

        with Store(config) as store:
            inst_count = store.conn.execute(
                "SELECT COUNT(*) FROM instances WHERE name='to-remove'"
            ).fetchone()[0]
            cache_count = store.conn.execute(
                "SELECT COUNT(*) FROM ticket_cache WHERE instance='to-remove'"
            ).fetchone()[0]
        self.assertEqual(inst_count, 0)
        self.assertEqual(cache_count, 0)

    def test_remove_unknown_exits_2(self) -> None:
        code, _, _ = self._run(["setup", "remove", "--name", "nonexistent"])
        self.assertEqual(code, 2)


class TestSetupAdd(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({})

    def _stub_token_and_users(self, token: str = TOKEN, is_ok: bool = True) -> None:
        if is_ok:
            self._http = self._make_stub_http({
                "/issue-token": (200, {"is_ok": True, "token": token}),
                "/api/v1/users": (200, [{"id": USER_ID, "email": "user@example.com"}]),
            })
        else:
            self._http = self._make_stub_http({
                "/issue-token": (200, {"is_ok": False, "message": "Bad credentials"}),
            })

    def test_add_stores_token_and_user_id(self) -> None:
        self._stub_token_and_users()
        with mock.patch("active_collab.cli.getpass.getpass", return_value="mypassword"):
            code, _, _ = self._run([
                "setup", "add", "--name", "myinst",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])
        self.assertEqual(code, 0)
        config = Config.load()
        with Store(config) as store:
            row = store.conn.execute(
                "SELECT name, email, token, user_id FROM instances WHERE name='myinst'"
            ).fetchone()
        self.assertIsNotNone(row)
        self.assertEqual(row[0], "myinst")
        self.assertEqual(row[1], "user@example.com")
        self.assertEqual(row[2], TOKEN)
        self.assertEqual(row[3], USER_ID)

    def test_password_never_stored_in_db(self) -> None:
        password = "PLAINTEXT_PASSWORD_MUST_NOT_APPEAR_IN_DB"
        self._stub_token_and_users()
        with mock.patch("active_collab.cli.getpass.getpass", return_value=password):
            self._run([
                "setup", "add", "--name", "sectest",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])
        with open(self._db_path, "rb") as f:
            raw = f.read()
        self.assertNotIn(password.encode(), raw, "Password must never be written to the DB file")

    def test_password_not_in_any_column(self) -> None:
        password = "SECRET_PLAIN_PASSWORD_CHECK"
        self._stub_token_and_users()
        with mock.patch("active_collab.cli.getpass.getpass", return_value=password):
            self._run([
                "setup", "add", "--name", "colcheck",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])
        config = Config.load()
        with Store(config) as store:
            row = store.conn.execute(
                "SELECT name, base_url, email, token, user_id FROM instances WHERE name='colcheck'"
            ).fetchone()
        for col_value in row:
            self.assertNotIn(password, str(col_value or ""))

    def test_failed_token_exchange_exits_1(self) -> None:
        self._stub_token_and_users(is_ok=False)
        with mock.patch("active_collab.cli.getpass.getpass", return_value="badpw"):
            code, _, err = self._run([
                "setup", "add", "--name", "failinst",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])
        self.assertEqual(code, 1)
        self.assertIn("Error", err)

    def test_missing_name_noninteractive_exits_2(self) -> None:
        code, _, err = self._run([
            "setup", "add",
            "--url", "https://collab.example.com",
            "--email", "user@example.com",
        ])
        self.assertEqual(code, 2)
        self.assertIn("required", err.lower())

    def test_token_transmitted_via_header_only(self) -> None:
        self._stub_token_and_users()
        with mock.patch("active_collab.cli.getpass.getpass", return_value="pw"):
            self._run([
                "setup", "add", "--name", "headertest",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])
        for url in self._http.captured_urls:  # type: ignore[attr-defined]
            self.assertNotIn(TOKEN, url, "Token must never appear in a URL")
        auth_headers = [
            h.get("X-Angie-AuthApiToken")
            for h in self._http.captured_headers  # type: ignore[attr-defined]
            if h.get("X-Angie-AuthApiToken")
        ]
        self.assertTrue(len(auth_headers) > 0, "Token must be sent via header")
        self.assertEqual(auth_headers[0], TOKEN)


class TestTokenNeverLeaks(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })

    def test_token_not_in_get_output(self) -> None:
        self._add_instance()
        _, out, err = self._run(["get", "665/75159"])
        self._assert_no_token(out, err)

    def test_token_not_in_url(self) -> None:
        self._add_instance()
        self._run(["get", "665/75159", "--refresh"])
        for url in self._http.captured_urls:  # type: ignore[attr-defined]
            self.assertNotIn(TOKEN, url)

    def test_token_not_in_setup_list_output(self) -> None:
        self._add_instance()
        _, out, err = self._run(["setup", "list"])
        self._assert_no_token(out, err)


class TestFlagBehavior(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._seen_urls: list[str] = []
        seen = self._seen_urls

        class TrackingHttp(HttpClient):
            def get(self, url, headers=None):  # type: ignore[override]
                seen.append(url)
                if "/tasks/75159" in url:
                    return 200, json.dumps(TASK_PAYLOAD).encode()
                if "/api/v1/users" in url:
                    return 200, json.dumps(USERS_PAYLOAD).encode()
                return 404, b""

        self._http = TrackingHttp()

    def test_json_flag_does_not_call_users_endpoint(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159", "--json"])
        self.assertEqual(code, 0)
        users_calls = [u for u in self._seen_urls if "/api/v1/users" in u]
        self.assertEqual(users_calls, [], "--json must not call /api/v1/users")
        parsed = json.loads(out)
        self.assertIn("single", parsed)
        self.assertEqual(parsed["tracked_time"], 3.0)

    def test_short_flag_does_not_call_users_endpoint(self) -> None:
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159", "--short"])
        self.assertEqual(code, 0)
        users_calls = [u for u in self._seen_urls if "/api/v1/users" in u]
        self.assertEqual(users_calls, [], "--short must not call /api/v1/users")
        self.assertIn("Implement login flow", out)
        self.assertNotIn("Assignee", out)

    def test_json_bypasses_cache(self) -> None:
        self._add_instance()
        with mock.patch("active_collab.cli.HttpClient", return_value=self._http):
            cli.main(["get", "665/75159"])
        before = len(self._seen_urls)
        with mock.patch("active_collab.cli.HttpClient", return_value=self._http):
            cli.main(["get", "665/75159", "--json"])
        self.assertGreater(len(self._seen_urls), before)


class TestExitCodes(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })

    def test_exit_0_on_success(self) -> None:
        self._add_instance()
        code, _, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)

    def test_exit_1_on_http_error(self) -> None:
        self._http = self._make_stub_http({})
        self._add_instance()
        code, _, _ = self._run(["get", "665/99999"])
        self.assertEqual(code, 1)

    def test_exit_2_on_no_instances(self) -> None:
        code, _, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 2)

    def test_exit_2_on_branch_mismatch(self) -> None:
        cli._current_branch = lambda: "main"  # noqa: SLF001
        code, _, _ = self._run(["current"])
        self.assertEqual(code, 2)

    def test_exit_2_on_bad_task_ref(self) -> None:
        self._add_instance()
        with self.assertRaises(SystemExit) as ctx:
            cli._parse_task_ref("not-a-task-ref")  # noqa: SLF001
        self.assertEqual(ctx.exception.code, 2)


class TestMetaFields(CliTestBase):
    def setUp(self) -> None:
        super().setUp()
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })

    def test_assignee_renders_display_name_and_id(self) -> None:
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("Assignee:  Maiara Gutierre (486)", out)

    def test_unresolved_assignee_renders_id_only(self) -> None:
        payload = {**TASK_PAYLOAD, "single": {**TASK_SINGLE, "assignee_id": 999}}
        self._http = self._make_stub_http({
            "/tasks/75159": (200, payload),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("(999)", out)
        self.assertNotIn("Maiara", out)

    def test_no_assignee_renders_unassigned(self) -> None:
        single_no_assignee = {k: v for k, v in TASK_SINGLE.items() if k != "assignee_id"}
        payload = {**TASK_PAYLOAD, "single": single_no_assignee}
        self._http = self._make_stub_http({
            "/tasks/75159": (200, payload),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("(unassigned)", out)

    def test_due_date_renders_as_yyyy_mm_dd(self) -> None:
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("Due:       2026-06-09", out)

    def test_start_date_renders_when_set(self) -> None:
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("Start:     2026-06-09", out)

    def test_start_date_omitted_when_not_set(self) -> None:
        single_no_start = {k: v for k, v in TASK_SINGLE.items() if k != "start_on"}
        payload = {**TASK_PAYLOAD, "single": single_no_start}
        self._http = self._make_stub_http({
            "/tasks/75159": (200, payload),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertNotIn("Start:", out)

    def test_estimate_renders_without_decimal(self) -> None:
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("Estimate:  0h", out)

    def test_logged_hours_renders_without_decimal(self) -> None:
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("Logged:    3h", out)

    def test_cached_render_still_shows_logged_hours(self) -> None:
        self._add_instance()
        self._run(["get", "665/75159"])
        self._http = self._make_stub_http({"/api/v1/users": (200, USERS_PAYLOAD)})
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Logged:    3h", out)

    def test_user_map_falls_back_to_first_last_name(self) -> None:
        users_no_display = [
            {"id": 486, "first_name": "Maiara", "last_name": "Gutierre", "email": "m@example.com"},
        ]
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, users_no_display),
        })
        self._add_instance()
        _, out, _ = self._run(["get", "665/75159"])
        self.assertIn("Maiara Gutierre (486)", out)

    def test_users_api_failure_renders_id_only(self) -> None:
        self._http = self._make_stub_http({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (500, None),
        })
        self._add_instance()
        code, out, _ = self._run(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("(486)", out)
