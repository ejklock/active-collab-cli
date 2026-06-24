#!/usr/bin/env python3
"""Offline tests for active_collab.py — no network, no real ~/.config writes."""

import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest
from unittest import mock

_SCRIPT = os.path.join(
    os.path.dirname(__file__), "..", "scripts", "active_collab.py"
)


def _load_module():
    spec = importlib.util.spec_from_file_location("active_collab", os.path.abspath(_SCRIPT))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


active_collab = _load_module()

# Real wrapped shapes as returned by ActiveCollab 7.2.25.
# Task endpoint: GET /api/v1/projects/{p}/tasks/{t} returns a dict with
# `single` (the task) and `comments` (list) at the top level.
# tracked_time is a float at the top level of the payload.

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
            "created_by_email": "alice@example.com",
            "created_on": 1736499600,
        }
    ],
    "subtasks": [],
    "task_list": {},
}

# Flat users list as returned by GET /api/v1/users.
USERS_PAYLOAD = [
    {"id": 486, "display_name": "Maiara Gutierre", "email": "maiara@example.com"},
    {"id": 69, "display_name": "Evaldo Klock", "email": "evaldo@example.com"},
]

COMPLETED_TASK = {
    "id": 75160,
    "task_number": 43,
    "name": "Old task",
    "is_completed": True,
    "is_trashed": False,
    "assignee_id": 7,
    "project_id": 665,
}

TRASHED_TASK = {
    "id": 75161,
    "task_number": 44,
    "name": "Trashed task",
    "is_completed": False,
    "is_trashed": True,
    "assignee_id": 7,
    "project_id": 665,
}

# Users/{id}/tasks endpoint returns a dict with a `tasks` list.
MINE_PAYLOAD = {
    "tasks": [
        {
            "id": 75159,
            "task_number": 42,
            "name": "Implement login flow",
            "is_completed": False,
            "is_trashed": False,
            "assignee_id": 7,
            "project_id": 665,
        },
        {**COMPLETED_TASK},
        {**TRASHED_TASK},
    ],
    "subtasks": [],
    "related": {},
}

TOKEN = "SUPER_SECRET_TOKEN_MUST_NOT_APPEAR"
USER_ID = 7


class ActiveCollabTestBase(unittest.TestCase):
    """Shared fixtures: isolated SQLite DB, fake HTTP, stdout/stderr capture."""

    def setUp(self):
        self._db_file = tempfile.NamedTemporaryFile(  # pylint: disable=consider-using-with
            suffix=".db", delete=False
        )
        self._db_file.close()
        os.environ["ACTIVE_COLLAB_DB"] = self._db_file.name
        os.unlink(self._db_file.name)
        self._captured_urls = []
        self._orig_stdin = active_collab._stdin_is_interactive  # pylint: disable=protected-access
        active_collab._stdin_is_interactive = lambda: False  # pylint: disable=protected-access

    def tearDown(self):
        if os.path.exists(self._db_file.name):
            os.unlink(self._db_file.name)
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        active_collab._stdin_is_interactive = self._orig_stdin  # pylint: disable=protected-access

    def _add_instance(self, name="default", base_url="https://collab.example.com",
                      email="user@example.com", user_id=USER_ID):
        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            conn.execute(
                "INSERT OR REPLACE INTO instances"
                " (name, base_url, email, token, user_id, created_at)"
                " VALUES (?, ?, ?, ?, ?, ?)",
                (name, base_url, email, TOKEN, user_id,
                 active_collab._now_iso()),  # pylint: disable=protected-access
            )
            conn.commit()

    def _make_fake_http_get(self, mapping: dict):
        """Build a fake http_get that maps url-substring -> (status, body_value)."""
        captured = self._captured_urls

        def fake_http_get(url, _headers):
            captured.append(url)
            for pattern, (status, body) in mapping.items():
                if pattern in url:
                    if body is None:
                        return status, b""
                    return status, json.dumps(body).encode()
            return 404, b'{"message":"Not Found"}'

        return fake_http_get

    def _run_main(self, argv: list) -> tuple:
        old_stdout, old_stderr = sys.stdout, sys.stderr
        sys.stdout = io.StringIO()
        sys.stderr = io.StringIO()
        try:
            code = active_collab.main(argv)
        except SystemExit as exc:
            code = exc.code if isinstance(exc.code, int) else 1
        finally:
            out = sys.stdout.getvalue()
            err = sys.stderr.getvalue()
            sys.stdout, sys.stderr = old_stdout, old_stderr
        return code, out, err

    def _assert_no_token(self, *strings: str):
        for s in strings:
            self.assertNotIn(TOKEN, s, "Token must never appear in output")


class TestBranchParsing(unittest.TestCase):
    """Tests for branch-name pattern matching."""

    def test_feature_branch_parses_project_and_task(self):
        result = active_collab._parse_branch_ref("feature/665-75159")  # pylint: disable=protected-access
        self.assertEqual(result, (665, 75159))

    def test_hotfix_branch_parses_project_and_task(self):
        result = active_collab._parse_branch_ref("hotfix/665-75159")  # pylint: disable=protected-access
        self.assertEqual(result, (665, 75159))

    def test_fix_branch_parses_project_and_task(self):
        result = active_collab._parse_branch_ref("fix/665-75159")  # pylint: disable=protected-access
        self.assertEqual(result, (665, 75159))

    def test_main_branch_returns_none(self):
        result = active_collab._parse_branch_ref("main")  # pylint: disable=protected-access
        self.assertIsNone(result)

    def test_non_matching_branch_returns_none(self):
        result = active_collab._parse_branch_ref("bugfix/some-fix")  # pylint: disable=protected-access
        self.assertIsNone(result)

    def test_feature_branch_without_task_returns_none(self):
        result = active_collab._parse_branch_ref("feature/665")  # pylint: disable=protected-access
        self.assertIsNone(result)

    def test_arbitrary_text_after_number_returns_none(self):
        result = active_collab._parse_branch_ref("feature/665-75159-extra")  # pylint: disable=protected-access
        self.assertIsNone(result)


class TestTaskRefParsing(unittest.TestCase):
    """Tests for URL and short-form task reference parsing."""

    def test_full_url_parses_project_and_task(self):
        ref = "https://collab.base.digital/projects/665/tasks/75159"
        result = active_collab._parse_task_ref(ref)  # pylint: disable=protected-access
        self.assertEqual(result, (665, 75159))

    def test_short_form_parses_project_and_task(self):
        result = active_collab._parse_task_ref("665/75159")  # pylint: disable=protected-access
        self.assertEqual(result, (665, 75159))

    def test_invalid_ref_exits_2(self):
        with self.assertRaises(SystemExit) as ctx:
            active_collab._parse_task_ref("not-a-ref")  # pylint: disable=protected-access
        self.assertEqual(ctx.exception.code, 2)


class TestHtmlToText(unittest.TestCase):
    """Unit tests for the HTML stripping helper."""

    def test_strips_paragraph_tags(self):
        result = active_collab._html_to_text("<p>Hello world</p>")  # pylint: disable=protected-access
        self.assertEqual(result, "Hello world")

    def test_unescapes_html_entities(self):
        result = active_collab._html_to_text("<p>A &amp; B &lt;here&gt;</p>")  # pylint: disable=protected-access
        self.assertIn("A & B", result)
        self.assertIn("<here>", result)

    def test_br_becomes_newline(self):
        result = active_collab._html_to_text("Line1<br>Line2")  # pylint: disable=protected-access
        self.assertIn("\n", result)

    def test_empty_string_returns_empty(self):
        result = active_collab._html_to_text("")  # pylint: disable=protected-access
        self.assertEqual(result, "")

    def test_none_handled_gracefully(self):
        result = active_collab._html_to_text(None)  # pylint: disable=protected-access
        self.assertEqual(result, "")

    def test_strips_inline_tags(self):
        result = active_collab._html_to_text("<strong>bold</strong> text")  # pylint: disable=protected-access
        self.assertEqual(result, "bold text")


class TestFmtTs(unittest.TestCase):
    """Unit tests for the timestamp formatter."""

    def test_unix_int_formats_as_utc_datetime(self):
        result = active_collab._fmt_ts(0)  # pylint: disable=protected-access
        self.assertEqual(result, "1970-01-01 00:00")

    def test_known_timestamp_produces_correct_date(self):
        result = active_collab._fmt_ts(1736499600)  # pylint: disable=protected-access
        self.assertTrue(result.startswith("2025-01-"))

    def test_none_returns_empty_string(self):
        result = active_collab._fmt_ts(None)  # pylint: disable=protected-access
        self.assertEqual(result, "")

    def test_string_passthrough(self):
        result = active_collab._fmt_ts("2026-01-10T09:00:00Z")  # pylint: disable=protected-access
        self.assertEqual(result, "2026-01-10T09:00:00Z")


class TestSetupAdd(ActiveCollabTestBase):
    """Tests for 'setup add' — instance registration and token exchange."""

    def _mock_token_exchange(self, token=TOKEN, is_ok=True):
        response = {"is_ok": is_ok, "token": token} if is_ok else {"is_ok": False, "message": "Bad credentials"}

        def fake_post(url, _data, _headers):
            return 200, json.dumps(response).encode()

        return fake_post

    def _mock_user_resolve(self, user_id=USER_ID, email="user@example.com"):
        users = [{"id": user_id, "email": email}]

        def fake_get(url, _headers):
            if "/users" in url:
                return 200, json.dumps(users).encode()
            return 404, b""

        return fake_get

    def test_add_stores_token_and_user_id(self):
        """setup add stores the returned token and resolved user_id in the DB."""
        active_collab.http_post = self._mock_token_exchange()
        active_collab.http_get = self._mock_user_resolve()

        with mock.patch.object(active_collab.getpass, "getpass", return_value="mypassword"):
            code, _out, _err = self._run_main([
                "setup", "add",
                "--name", "myinst",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])

        self.assertEqual(code, 0)
        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            row = conn.execute(
                "SELECT name, email, token, user_id FROM instances WHERE name='myinst'"
            ).fetchone()
        self.assertIsNotNone(row)
        self.assertEqual(row[0], "myinst")
        self.assertEqual(row[1], "user@example.com")
        self.assertEqual(row[2], TOKEN)
        self.assertEqual(row[3], USER_ID)

    def test_password_never_stored_in_db(self):
        """The plaintext password is never written to the SQLite database."""
        password = "PLAINTEXT_PASSWORD_MUST_NOT_APPEAR_IN_DB"
        active_collab.http_post = self._mock_token_exchange()
        active_collab.http_get = self._mock_user_resolve()

        with mock.patch.object(active_collab.getpass, "getpass", return_value=password):
            self._run_main([
                "setup", "add",
                "--name", "sectest",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])

        db_path = active_collab._db_path()  # pylint: disable=protected-access
        with open(db_path, "rb") as f:
            raw_bytes = f.read()
        self.assertNotIn(password.encode(), raw_bytes, "Password must never be written to the DB file")

    def test_password_not_in_any_column(self):
        """All DB columns are inspected and none contain the plaintext password."""
        password = "SECRET_PLAIN_PASSWORD_CHECK"
        active_collab.http_post = self._mock_token_exchange()
        active_collab.http_get = self._mock_user_resolve()

        with mock.patch.object(active_collab.getpass, "getpass", return_value=password):
            self._run_main([
                "setup", "add",
                "--name", "colcheck",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])

        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            row = conn.execute(
                "SELECT name, base_url, email, token, user_id FROM instances WHERE name='colcheck'"
            ).fetchone()
        for col_value in row:
            self.assertNotIn(password, str(col_value or ""), "Password must not appear in any column")

    def test_issue_token_post_sends_correct_body(self):
        """POST to issue-token sends username, password, client_name, client_vendor."""
        captured_body = {}

        def capturing_post(url, data, _headers):
            captured_body.update(data)
            return 200, json.dumps({"is_ok": True, "token": TOKEN}).encode()

        active_collab.http_post = capturing_post
        active_collab.http_get = self._mock_user_resolve()

        with mock.patch.object(active_collab.getpass, "getpass", return_value="pw123"):
            self._run_main([
                "setup", "add",
                "--name", "bodytest",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])

        self.assertEqual(captured_body.get("username"), "user@example.com")
        self.assertIn("password", captured_body)
        self.assertEqual(captured_body.get("client_name"), "active-collab-skill")
        self.assertEqual(captured_body.get("client_vendor"), "klock")

    def test_failed_token_exchange_exits_1(self):
        """A failed token exchange exits 1."""
        active_collab.http_post = self._mock_token_exchange(is_ok=False)

        with mock.patch.object(active_collab.getpass, "getpass", return_value="badpw"):
            code, _out, err = self._run_main([
                "setup", "add",
                "--name", "failinst",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])

        self.assertEqual(code, 1)
        self.assertIn("Error", err)

    def test_missing_name_noninteractive_exits_2(self):
        """Non-interactive mode with missing --name exits 2 without network calls."""
        code, _out, err = self._run_main([
            "setup", "add",
            "--url", "https://collab.example.com",
            "--email", "user@example.com",
        ])
        self.assertEqual(code, 2)
        self.assertIn("required", err.lower())

    def test_token_transmitted_via_header_only(self):
        """The token is sent via X-Angie-AuthApiToken header, not in the URL."""
        seen_urls = []
        seen_headers = {}

        active_collab.http_post = self._mock_token_exchange()

        def capturing_get(url, headers):
            seen_urls.append(url)
            seen_headers.update(headers)
            return 200, json.dumps([{"id": USER_ID, "email": "user@example.com"}]).encode()

        active_collab.http_get = capturing_get

        with mock.patch.object(active_collab.getpass, "getpass", return_value="pw"):
            self._run_main([
                "setup", "add",
                "--name", "headertest",
                "--url", "https://collab.example.com",
                "--email", "user@example.com",
            ])

        for url in seen_urls:
            self.assertNotIn(TOKEN, url, "Token must never appear in a URL")
        self.assertIn("X-Angie-AuthApiToken", seen_headers)
        self.assertEqual(seen_headers["X-Angie-AuthApiToken"], TOKEN)


class TestSetupList(ActiveCollabTestBase):
    """Tests for 'setup list'."""

    def test_list_never_shows_token(self):
        self._add_instance(name="inst1")
        code, out, err = self._run_main(["setup", "list"])
        self.assertEqual(code, 0)
        self._assert_no_token(out, err)
        self.assertIn("inst1", out)

    def test_list_shows_instance_name_url_email_user_id(self):
        self._add_instance(name="myinst", base_url="https://collab.example.com",
                           email="user@example.com", user_id=42)
        code, out, _err = self._run_main(["setup", "list"])
        self.assertEqual(code, 0)
        self.assertIn("myinst", out)
        self.assertIn("collab.example.com", out)
        self.assertIn("user@example.com", out)

    def test_list_empty_exits_0(self):
        code, _out, _err = self._run_main(["setup", "list"])
        self.assertEqual(code, 0)


class TestSetupRemove(ActiveCollabTestBase):
    """Tests for 'setup remove'."""

    def test_remove_deletes_instance_and_cache(self):
        self._add_instance(name="to-remove")
        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            conn.execute(
                "INSERT INTO ticket_cache (instance, project_id, task_id, fields_json, fetched_at)"
                " VALUES (?, ?, ?, ?, ?)",
                ("to-remove", 665, 1, "{}", active_collab._now_iso()),  # pylint: disable=protected-access
            )
            conn.commit()

        code, _out, _err = self._run_main(["setup", "remove", "--name", "to-remove"])
        self.assertEqual(code, 0)

        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            inst_count = conn.execute(
                "SELECT COUNT(*) FROM instances WHERE name='to-remove'"
            ).fetchone()[0]
            cache_count = conn.execute(
                "SELECT COUNT(*) FROM ticket_cache WHERE instance='to-remove'"
            ).fetchone()[0]
        self.assertEqual(inst_count, 0)
        self.assertEqual(cache_count, 0)

    def test_remove_unknown_exits_2(self):
        code, _out, _err = self._run_main(["setup", "remove", "--name", "nonexistent"])
        self.assertEqual(code, 2)


class TestDbInit(ActiveCollabTestBase):
    """Tests for DB initialisation."""

    def test_tables_created_on_first_open(self):
        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            tables = {
                row[0]
                for row in conn.execute(
                    "SELECT name FROM sqlite_master WHERE type='table'"
                ).fetchall()
            }
        self.assertIn("instances", tables)
        self.assertIn("ticket_cache", tables)

    def test_journal_mode_is_delete(self):
        with active_collab._open_db() as conn:  # pylint: disable=protected-access
            mode = conn.execute("PRAGMA journal_mode").fetchone()[0]
        self.assertEqual(mode, "delete")

    def test_schema_idempotent(self):
        active_collab._open_db().close()  # pylint: disable=protected-access
        active_collab._open_db().close()  # pylint: disable=protected-access


class TestGet(ActiveCollabTestBase):
    """Tests for 'get' subcommand — using real wrapped API shapes."""

    def test_get_unwraps_single_key_and_renders_name(self):
        """Task is unwrapped from `single`; the task name appears in output."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_get_renders_open_status(self):
        """is_completed=False produces 'Open' in the status line."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Open", out)

    def test_get_renders_completed_status(self):
        """is_completed=True produces 'Completed' in the status line."""
        self._add_instance()
        completed_payload = {
            "single": {**TASK_SINGLE, "is_completed": True},
            "comments": [],
        }
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, completed_payload),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Completed", out)

    def test_get_renders_assignee_resolved_name(self):
        """assignee_id is resolved to display_name via /api/v1/users."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, USERS_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Maiara Gutierre (486)", out)

    def test_get_strips_html_tags_from_body(self):
        """HTML tags in the body field are stripped before display."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertNotIn("<p>", out)
        self.assertIn("login page", out)

    def test_get_unescapes_html_entities_in_body(self):
        """HTML entities like &amp; are decoded in the displayed body."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("& done", out)

    def test_get_renders_comment_body_plain_text(self):
        """Comments use body_plain_text when available."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Great start on this feature", out)

    def test_get_renders_comment_author_name(self):
        """Comment author is shown using created_by_name."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Alice", out)

    def test_get_renders_comment_created_on_as_date(self):
        """created_on unix timestamp is formatted as YYYY-MM-DD HH:MM."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("2025-01-", out)

    def test_get_no_separate_comments_request(self):
        """Comments come from the task payload; no separate comments URL is called."""
        self._add_instance()
        seen_urls = []

        def tracking_get(url, _headers):
            seen_urls.append(url)
            if "/tasks/75159" in url:
                return 200, json.dumps(TASK_PAYLOAD).encode()
            return 500, b""

        active_collab.http_get = tracking_get
        code, _out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        for url in seen_urls:
            self.assertNotIn("/comments", url, "No separate comments endpoint should be called")

    def test_get_by_url_fetches_correct_task(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main([
            "get", "https://collab.example.com/projects/665/tasks/75159"
        ])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_get_short_flag_prints_only_ref_and_name(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159", "--short"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)
        self.assertNotIn("Description", out)
        self.assertNotIn("Status", out)

    def test_get_no_comments_omits_comments(self):
        """--no-comments suppresses the comments section."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159", "--no-comments"])
        self.assertEqual(code, 0)
        self.assertNotIn("Great start", out)

    def test_get_json_flag_returns_full_wrapped_api_payload(self):
        """--json prints the complete raw wrapped payload including the `single` key."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159", "--json"])
        self.assertEqual(code, 0)
        parsed = json.loads(out)
        self.assertIn("single", parsed)
        self.assertEqual(parsed["single"]["id"], 75159)
        self.assertEqual(parsed["single"]["name"], "Implement login flow")

    def test_get_not_found_exits_1(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({})
        code, _out, err = self._run_main(["get", "665/99999"])
        self.assertEqual(code, 1)
        self.assertIn("not found", err.lower())

    def test_get_no_instances_exits_2(self):
        code, _out, err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 2)
        self.assertIn("no instances", err.lower())

    def test_get_unknown_instance_exits_2(self):
        self._add_instance(name="real-instance")
        code, _out, err = self._run_main(["get", "665/75159", "--instance", "nonexistent"])
        self.assertEqual(code, 2)
        self.assertIn("not found", err.lower())

    def test_get_uses_cache_on_second_call(self):
        """Second get call reads from cache without calling http_get again."""
        self._add_instance()
        call_count = {"n": 0}

        def counting_get(url, _headers):
            call_count["n"] += 1
            return 200, json.dumps(TASK_PAYLOAD).encode()

        active_collab.http_get = counting_get
        self._run_main(["get", "665/75159"])
        first_count = call_count["n"]

        active_collab.http_get = lambda _url, _headers: (500, b"")
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertEqual(call_count["n"], first_count, "Second call must read from cache")
        self.assertIn("Implement login flow", out)

    def test_refresh_bypasses_cache(self):
        self._add_instance()
        call_count = {"n": 0}

        def counting_get(url, _headers):
            call_count["n"] += 1
            return 200, json.dumps(TASK_PAYLOAD).encode()

        active_collab.http_get = counting_get
        self._run_main(["get", "665/75159"])
        before = call_count["n"]
        self._run_main(["get", "665/75159", "--refresh"])
        self.assertGreater(call_count["n"], before, "--refresh must call http_get again")


class TestCurrent(ActiveCollabTestBase):
    """Tests for 'current' subcommand — git branch parsing."""

    def test_current_parses_feature_branch(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })

        def fake_branch():
            return "feature/665-75159"

        active_collab._current_branch = fake_branch  # pylint: disable=protected-access
        code, out, _err = self._run_main(["current"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)

    def test_current_non_matching_branch_exits_2(self):
        def fake_branch():
            return "main"

        active_collab._current_branch = fake_branch  # pylint: disable=protected-access
        code, _out, err = self._run_main(["current"])
        self.assertEqual(code, 2)
        self.assertIn("pattern", err.lower())

    def test_current_detached_head_exits_2(self):
        def fake_branch():
            return None

        active_collab._current_branch = fake_branch  # pylint: disable=protected-access
        code, _out, err = self._run_main(["current"])
        self.assertEqual(code, 2)

    def test_current_error_message_names_expected_pattern(self):
        """Error for non-matching branch names the expected format."""
        def fake_branch():
            return "fix-some-bug"

        active_collab._current_branch = fake_branch  # pylint: disable=protected-access
        code, _out, err = self._run_main(["current"])
        self.assertEqual(code, 2)
        self.assertIn("feature|hotfix|fix", err)


class TestMine(ActiveCollabTestBase):
    """Tests for 'mine' subcommand — uses GET /users/{id}/tasks with real wrapped shape."""

    def _make_mine_http(self, mine_payload: dict):
        """Return a fake http_get that serves the users/{id}/tasks endpoint."""
        def fake_get(url, _headers):
            if f"/users/{USER_ID}/tasks" in url:
                return 200, json.dumps(mine_payload).encode()
            return 404, b""
        return fake_get

    def test_mine_calls_user_tasks_endpoint(self):
        """mine calls GET /users/{user_id}/tasks (not a project fan-out)."""
        self._add_instance(user_id=USER_ID)
        seen_urls = []

        def tracking_get(url, _headers):
            seen_urls.append(url)
            if f"/users/{USER_ID}/tasks" in url:
                return 200, json.dumps(MINE_PAYLOAD).encode()
            return 404, b""

        active_collab.http_get = tracking_get
        self._run_main(["mine"])
        self.assertTrue(
            any(f"/users/{USER_ID}/tasks" in u for u in seen_urls),
            "mine must call /users/{user_id}/tasks",
        )

    def test_mine_lists_only_open_not_trashed_tasks(self):
        """Only tasks where not is_completed and not is_trashed are listed."""
        self._add_instance(user_id=USER_ID)
        active_collab.http_get = self._make_mine_http(MINE_PAYLOAD)
        code, out, _err = self._run_main(["mine"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)
        self.assertNotIn("Old task", out)
        self.assertNotIn("Trashed task", out)

    def test_mine_excludes_completed_tasks(self):
        payload = {"tasks": [COMPLETED_TASK], "subtasks": [], "related": {}}
        self._add_instance(user_id=USER_ID)
        active_collab.http_get = self._make_mine_http(payload)
        code, out, _err = self._run_main(["mine"])
        self.assertEqual(code, 0)
        self.assertNotIn("Old task", out)

    def test_mine_excludes_trashed_tasks(self):
        payload = {"tasks": [TRASHED_TASK], "subtasks": [], "related": {}}
        self._add_instance(user_id=USER_ID)
        active_collab.http_get = self._make_mine_http(payload)
        code, out, _err = self._run_main(["mine"])
        self.assertEqual(code, 0)
        self.assertNotIn("Trashed task", out)

    def test_mine_prints_project_id_task_number_and_name(self):
        """Output table contains project_id, task_number, and task name."""
        self._add_instance(user_id=USER_ID)
        active_collab.http_get = self._make_mine_http(MINE_PAYLOAD)
        code, out, _err = self._run_main(["mine"])
        self.assertEqual(code, 0)
        self.assertIn("665", out)
        self.assertIn("42", out)
        self.assertIn("Implement login flow", out)

    def test_mine_empty_case_exits_0_with_message(self):
        payload = {"tasks": [], "subtasks": [], "related": {}}
        self._add_instance(user_id=USER_ID)
        active_collab.http_get = self._make_mine_http(payload)
        code, out, _err = self._run_main(["mine"])
        self.assertEqual(code, 0)
        self.assertIn("no open tasks", out.lower())

    def test_mine_no_instances_exits_2(self):
        code, _out, err = self._run_main(["mine"])
        self.assertEqual(code, 2)
        self.assertIn("no instances", err.lower())

    def test_mine_unknown_instance_exits_2(self):
        self._add_instance(name="real")
        code, _out, _err = self._run_main(["mine", "--instance", "ghost"])
        self.assertEqual(code, 2)

    def test_list_alias_works_same_as_mine(self):
        """'list' is an alias for 'mine'."""
        self._add_instance(user_id=USER_ID)
        active_collab.http_get = self._make_mine_http(MINE_PAYLOAD)
        code, out, _err = self._run_main(["list"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)


class TestTaskMetaFields(ActiveCollabTestBase):
    """Tests for assignee name resolution, dates, estimate, and logged hours (AC1–AC4)."""

    def _make_task_http(self, task_payload=None, users_payload=None):
        tp = task_payload if task_payload is not None else TASK_PAYLOAD
        up = users_payload if users_payload is not None else USERS_PAYLOAD
        return self._make_fake_http_get({
            "/tasks/75159": (200, tp),
            "/api/v1/users": (200, up),
        })

    def test_assignee_renders_display_name_and_id(self):
        """Assignee line shows '<display_name> (<id>)' when resolved via /api/v1/users."""
        self._add_instance()
        active_collab.http_get = self._make_task_http()
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Assignee:  Maiara Gutierre (486)", out)

    def test_unresolved_assignee_renders_id_only(self):
        """An assignee id with no matching user renders '(<id>)'."""
        self._add_instance()
        payload_no_user = {
            **TASK_PAYLOAD,
            "single": {**TASK_SINGLE, "assignee_id": 999},
        }
        active_collab.http_get = self._make_task_http(task_payload=payload_no_user)
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("(999)", out)
        self.assertNotIn("Maiara", out)

    def test_no_assignee_renders_unassigned(self):
        """A task without assignee_id renders '(unassigned)'."""
        self._add_instance()
        payload_unassigned = {
            **TASK_PAYLOAD,
            "single": {k: v for k, v in TASK_SINGLE.items() if k != "assignee_id"},
        }
        active_collab.http_get = self._make_task_http(task_payload=payload_unassigned)
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("(unassigned)", out)

    def test_due_date_renders_as_yyyy_mm_dd(self):
        """due_on unix timestamp renders as 'YYYY-MM-DD' on the Due line."""
        self._add_instance()
        active_collab.http_get = self._make_task_http()
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Due:       2026-06-09", out)

    def test_start_date_renders_when_set(self):
        """start_on renders as 'YYYY-MM-DD' on the Start line."""
        self._add_instance()
        active_collab.http_get = self._make_task_http()
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Start:     2026-06-09", out)

    def test_start_date_omitted_when_not_set(self):
        """Start line is omitted when start_on is absent."""
        self._add_instance()
        payload_no_start = {
            **TASK_PAYLOAD,
            "single": {k: v for k, v in TASK_SINGLE.items() if k != "start_on"},
        }
        active_collab.http_get = self._make_task_http(task_payload=payload_no_start)
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertNotIn("Start:", out)

    def test_estimate_renders_without_decimal(self):
        """Estimate renders as '0h' (not '0.0h') when estimate is 0.0."""
        self._add_instance()
        active_collab.http_get = self._make_task_http()
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Estimate:  0h", out)

    def test_logged_hours_renders_without_decimal(self):
        """Logged renders as '3h' (not '3.0h') when tracked_time is 3.0."""
        self._add_instance()
        active_collab.http_get = self._make_task_http()
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Logged:    3h", out)

    def test_cached_render_still_shows_logged_hours(self):
        """tracked_time persists through the cache so a second (cached) render shows Logged."""
        self._add_instance()
        active_collab.http_get = self._make_task_http()
        self._run_main(["get", "665/75159"])

        active_collab.http_get = self._make_fake_http_get({
            "/api/v1/users": (200, USERS_PAYLOAD),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Logged:    3h", out)

    def test_json_flag_unchanged_and_no_users_lookup(self):
        """--json outputs unchanged raw payload; /api/v1/users is not called."""
        self._add_instance()
        seen_urls = []

        def tracking_get(url, _headers):
            seen_urls.append(url)
            if "/tasks/75159" in url:
                return 200, json.dumps(TASK_PAYLOAD).encode()
            return 404, b""

        active_collab.http_get = tracking_get
        code, out, _err = self._run_main(["get", "665/75159", "--json"])
        self.assertEqual(code, 0)
        parsed = json.loads(out)
        self.assertIn("single", parsed)
        self.assertEqual(parsed["tracked_time"], 3.0)
        users_calls = [u for u in seen_urls if "/api/v1/users" in u]
        self.assertEqual(users_calls, [], "--json must not call /api/v1/users")

    def test_short_flag_unchanged_and_no_users_lookup(self):
        """--short outputs PROJECT/TASK<TAB>name; /api/v1/users is not called."""
        self._add_instance()
        seen_urls = []

        def tracking_get(url, _headers):
            seen_urls.append(url)
            if "/tasks/75159" in url:
                return 200, json.dumps(TASK_PAYLOAD).encode()
            return 404, b""

        active_collab.http_get = tracking_get
        code, out, _err = self._run_main(["get", "665/75159", "--short"])
        self.assertEqual(code, 0)
        self.assertIn("Implement login flow", out)
        self.assertNotIn("Assignee", out)
        users_calls = [u for u in seen_urls if "/api/v1/users" in u]
        self.assertEqual(users_calls, [], "--short must not call /api/v1/users")

    def test_user_map_falls_back_to_first_last_name(self):
        """When display_name absent, first+last name is used for the user map."""
        self._add_instance()
        users_no_display = [
            {"id": 486, "first_name": "Maiara", "last_name": "Gutierre", "email": "m@example.com"},
        ]
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (200, users_no_display),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("Maiara Gutierre (486)", out)

    def test_users_api_failure_renders_id_only(self):
        """When /api/v1/users returns non-200, assignee_id renders without a name."""
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
            "/api/v1/users": (500, None),
        })
        code, out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)
        self.assertIn("(486)", out)


class TestFmtDate(unittest.TestCase):
    """Unit tests for the date-only formatter."""

    def test_unix_int_formats_as_date(self):
        result = active_collab._fmt_date(1780963200)  # pylint: disable=protected-access
        self.assertEqual(result, "2026-06-09")

    def test_none_returns_empty_string(self):
        result = active_collab._fmt_date(None)  # pylint: disable=protected-access
        self.assertEqual(result, "")

    def test_float_timestamp_formats_as_date(self):
        result = active_collab._fmt_date(1780963200.0)  # pylint: disable=protected-access
        self.assertEqual(result, "2026-06-09")

    def test_non_numeric_passthrough(self):
        result = active_collab._fmt_date("2026-06-09")  # pylint: disable=protected-access
        self.assertEqual(result, "2026-06-09")


class TestFmtHours(unittest.TestCase):
    """Unit tests for the hours formatter."""

    def test_whole_float_renders_as_integer(self):
        result = active_collab._fmt_hours(3.0)  # pylint: disable=protected-access
        self.assertEqual(result, "3")

    def test_zero_float_renders_as_zero(self):
        result = active_collab._fmt_hours(0.0)  # pylint: disable=protected-access
        self.assertEqual(result, "0")

    def test_fractional_renders_naturally(self):
        result = active_collab._fmt_hours(1.5)  # pylint: disable=protected-access
        self.assertEqual(result, "1.5")

    def test_none_returns_zero(self):
        result = active_collab._fmt_hours(None)  # pylint: disable=protected-access
        self.assertEqual(result, "0")

    def test_integer_input_renders_as_string(self):
        result = active_collab._fmt_hours(5)  # pylint: disable=protected-access
        self.assertEqual(result, "5")


class TestExitCodes(ActiveCollabTestBase):
    """Tests that exit codes follow the spec."""

    def test_exit_0_on_success(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        code, _out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 0)

    def test_exit_1_on_http_error(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({})  # all 404
        code, _out, _err = self._run_main(["get", "665/99999"])
        self.assertEqual(code, 1)

    def test_exit_2_on_usage_error_no_instances(self):
        code, _out, _err = self._run_main(["get", "665/75159"])
        self.assertEqual(code, 2)

    def test_exit_2_on_branch_mismatch(self):
        def fake_branch():
            return "main"

        active_collab._current_branch = fake_branch  # pylint: disable=protected-access
        code, _out, _err = self._run_main(["current"])
        self.assertEqual(code, 2)

    def test_exit_2_on_bad_task_ref(self):
        self._add_instance()
        with self.assertRaises(SystemExit) as ctx:
            active_collab._parse_task_ref("not-a-task-ref")  # pylint: disable=protected-access
        self.assertEqual(ctx.exception.code, 2)


class TestTokenNeverLeaks(ActiveCollabTestBase):
    """Tests that the token never appears in output or URLs."""

    def test_token_not_in_get_output(self):
        self._add_instance()
        active_collab.http_get = self._make_fake_http_get({
            "/tasks/75159": (200, TASK_PAYLOAD),
        })
        _code, out, err = self._run_main(["get", "665/75159"])
        self._assert_no_token(out, err)

    def test_token_not_in_url(self):
        self._add_instance()
        seen_urls = []

        def capturing_get(url, _headers):
            seen_urls.append(url)
            if "/tasks/75159" in url:
                return 200, json.dumps(TASK_PAYLOAD).encode()
            return 404, b""

        active_collab.http_get = capturing_get
        self._run_main(["get", "665/75159", "--refresh"])
        for url in seen_urls:
            self.assertNotIn(TOKEN, url, "Token must not appear in any URL")

    def test_token_not_in_setup_list_output(self):
        self._add_instance()
        _code, out, err = self._run_main(["setup", "list"])
        self._assert_no_token(out, err)


class TestSkillMd(unittest.TestCase):
    """Validates the SKILL.md file."""

    SKILL_MD = os.path.join(os.path.dirname(__file__), "..", "SKILL.md")

    def test_skill_md_exists(self):
        self.assertTrue(os.path.isfile(self.SKILL_MD), "SKILL.md must exist")

    def test_description_is_single_quoted(self):
        with open(self.SKILL_MD, encoding="utf-8") as fh:
            content = fh.read()
        self.assertIn("description: '", content, "SKILL.md description must use single quotes")

    def test_body_at_most_200_lines(self):
        with open(self.SKILL_MD, encoding="utf-8") as fh:
            lines = fh.readlines()
        fence_count = 0
        body_start = 0
        for i, line in enumerate(lines):
            if line.strip() == "---":
                fence_count += 1
                if fence_count == 2:
                    body_start = i + 1
                    break
        body_lines = len(lines) - body_start
        self.assertLessEqual(body_lines, 200, f"SKILL.md body is {body_lines} lines — must be <=200")

    def test_skill_md_references_user_tasks_endpoint(self):
        """SKILL.md must document that mine uses /users/{id}/tasks."""
        with open(self.SKILL_MD, encoding="utf-8") as fh:
            content = fh.read()
        self.assertIn("/users/", content, "SKILL.md must reference the users/{id}/tasks endpoint")

    def test_skill_md_notes_comments_from_task_payload(self):
        """SKILL.md must note that comments come inline with the task payload."""
        with open(self.SKILL_MD, encoding="utf-8") as fh:
            content = fh.read()
        self.assertIn("comment", content.lower(), "SKILL.md must mention comment sourcing")


if __name__ == "__main__":
    unittest.main(verbosity=2)
