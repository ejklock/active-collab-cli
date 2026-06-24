"""Tests for ActiveCollabClient — stubs HttpClient, no real HTTP."""

import json
import unittest

from active_collab.client import ActiveCollabClient
from active_collab.http import HttpClient
from active_collab.models import Instance

TOKEN = "SUPER_SECRET_TOKEN_MUST_NOT_APPEAR"
USER_ID = 7

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

TASK_PAYLOAD = {
    "single": {
        "id": 75159, "task_number": 42, "name": "Implement login flow",
        "is_completed": False, "is_trashed": False,
        "assignee_id": 486, "project_id": 665,
        "body": "<p>Description</p>",
    },
    "tracked_time": 3.0,
    "comments": [{"id": 1, "body": "nice"}],
}


def _make_instance(user_id: int | None = USER_ID) -> Instance:
    return Instance(
        name="default",
        base_url="https://collab.example.com",
        email="user@example.com",
        token=TOKEN,
        user_id=user_id,
    )


class StubHttp(HttpClient):
    """Minimal HttpClient stub — routes by URL substring."""

    def __init__(self, routes: dict[str, tuple[int, object]]) -> None:
        self._routes = routes
        self._seen_urls: list[str] = []
        self._seen_headers: list[dict] = []

    def get(self, url: str, headers: dict | None = None) -> tuple[int, bytes]:
        self._seen_urls.append(url)
        self._seen_headers.append(headers or {})
        for pattern, (status, body) in self._routes.items():
            if pattern in url:
                raw = json.dumps(body).encode() if body is not None else b""
                return status, raw
        return 404, b'{"message":"Not Found"}'

    def post(self, url: str, data: dict, headers: dict | None = None) -> tuple[int, bytes]:
        self._seen_urls.append(url)
        for pattern, (status, body) in self._routes.items():
            if pattern in url:
                raw = json.dumps(body).encode() if body is not None else b""
                return status, raw
        return 404, b""


class TestExchangeToken(unittest.TestCase):
    def _client(self, routes: dict) -> ActiveCollabClient:
        return ActiveCollabClient(_make_instance(), StubHttp(routes))

    def test_successful_exchange_returns_token(self) -> None:
        client = self._client({
            "/issue-token": (200, {"is_ok": True, "token": TOKEN}),
        })
        token, response = client.exchange_token("https://collab.example.com", "u@e.com", "pw")
        self.assertEqual(token, TOKEN)
        self.assertTrue(response.get("is_ok"))

    def test_failed_is_ok_returns_none_token(self) -> None:
        client = self._client({
            "/issue-token": (200, {"is_ok": False, "message": "Bad credentials"}),
        })
        token, response = client.exchange_token("https://collab.example.com", "u@e.com", "bad")
        self.assertIsNone(token)
        self.assertIn("message", response)

    def test_non_200_returns_none_token(self) -> None:
        client = self._client({"/issue-token": (401, None)})
        token, _ = client.exchange_token("https://collab.example.com", "u@e.com", "pw")
        self.assertIsNone(token)

    def test_post_body_captures_fields(self) -> None:
        captured: list[dict] = []

        class CapturingHttp(HttpClient):
            def post(self, url, data, headers=None):  # type: ignore[override]
                captured.append(data)
                return 200, json.dumps({"is_ok": True, "token": TOKEN}).encode()

        client = ActiveCollabClient(_make_instance(), CapturingHttp())
        client.exchange_token("https://collab.example.com", "user@e.com", "secret")
        self.assertEqual(len(captured), 1)
        body = captured[0]
        self.assertEqual(body["username"], "user@e.com")
        self.assertIn("password", body)
        self.assertEqual(body["client_name"], "active-collab-skill")
        self.assertEqual(body["client_vendor"], "klock")

    def test_token_not_in_url(self) -> None:
        seen_urls: list[str] = []

        class UrlCapture(HttpClient):
            def post(self, url, data, headers=None):  # type: ignore[override]
                seen_urls.append(url)
                return 200, json.dumps({"is_ok": True, "token": TOKEN}).encode()

            def get(self, url, headers=None):  # type: ignore[override]
                seen_urls.append(url)
                return 200, json.dumps([{"id": USER_ID, "email": "u@e.com"}]).encode()

        client = ActiveCollabClient(_make_instance(), UrlCapture())
        client.exchange_token("https://collab.example.com", "u@e.com", "pw")
        client.resolve_user_id("https://collab.example.com", TOKEN, "u@e.com")
        for url in seen_urls:
            self.assertNotIn(TOKEN, url, "Token must never appear in a URL")


class TestResolveUserId(unittest.TestCase):
    def _client(self, routes: dict) -> ActiveCollabClient:
        return ActiveCollabClient(_make_instance(), StubHttp(routes))

    def test_matches_email_case_insensitive(self) -> None:
        client = self._client({
            "/api/v1/users": (200, [{"id": USER_ID, "email": "User@Example.COM"}]),
        })
        uid = client.resolve_user_id("https://collab.example.com", TOKEN, "user@example.com")
        self.assertEqual(uid, USER_ID)

    def test_returns_none_when_no_match(self) -> None:
        client = self._client({
            "/api/v1/users": (200, [{"id": 99, "email": "other@e.com"}]),
        })
        uid = client.resolve_user_id("https://collab.example.com", TOKEN, "me@e.com")
        self.assertIsNone(uid)

    def test_returns_none_on_api_error(self) -> None:
        client = self._client({"/api/v1/users": (500, None)})
        uid = client.resolve_user_id("https://collab.example.com", TOKEN, "u@e.com")
        self.assertIsNone(uid)


class TestFetchUserMap(unittest.TestCase):
    def test_returns_display_name_map(self) -> None:
        stub = StubHttp({"/api/v1/users": (200, USERS_PAYLOAD)})
        client = ActiveCollabClient(_make_instance(), stub)
        user_map = client.fetch_user_map()
        self.assertEqual(user_map[486], "Maiara Gutierre")
        self.assertEqual(user_map[69], "Evaldo Klock")

    def test_falls_back_to_first_last_name(self) -> None:
        users = [{"id": 1, "first_name": "Ada", "last_name": "Lovelace", "email": "a@e.com"}]
        stub = StubHttp({"/api/v1/users": (200, users)})
        client = ActiveCollabClient(_make_instance(), stub)
        user_map = client.fetch_user_map()
        self.assertEqual(user_map[1], "Ada Lovelace")

    def test_falls_back_to_email_when_no_name(self) -> None:
        users = [{"id": 2, "email": "fallback@e.com"}]
        stub = StubHttp({"/api/v1/users": (200, users)})
        client = ActiveCollabClient(_make_instance(), stub)
        user_map = client.fetch_user_map()
        self.assertEqual(user_map[2], "fallback@e.com")

    def test_returns_empty_on_api_failure(self) -> None:
        stub = StubHttp({"/api/v1/users": (500, None)})
        client = ActiveCollabClient(_make_instance(), stub)
        self.assertEqual(client.fetch_user_map(), {})

    def test_token_sent_via_header_not_url(self) -> None:
        stub = StubHttp({"/api/v1/users": (200, USERS_PAYLOAD)})
        client = ActiveCollabClient(_make_instance(), stub)
        client.fetch_user_map()
        for url in stub._seen_urls:  # noqa: SLF001
            self.assertNotIn(TOKEN, url)
        auth_header = stub._seen_headers[0].get("X-Angie-AuthApiToken")  # noqa: SLF001
        self.assertEqual(auth_header, TOKEN)


class TestFetchTask(unittest.TestCase):
    def test_returns_status_and_payload_on_200(self) -> None:
        stub = StubHttp({"/tasks/75159": (200, TASK_PAYLOAD)})
        client = ActiveCollabClient(_make_instance(), stub)
        status, payload = client.fetch_task(665, 75159)
        self.assertEqual(status, 200)
        self.assertIsNotNone(payload)
        self.assertIn("single", payload)

    def test_returns_none_payload_on_404(self) -> None:
        stub = StubHttp({})
        client = ActiveCollabClient(_make_instance(), stub)
        status, payload = client.fetch_task(665, 99999)
        self.assertEqual(status, 404)
        self.assertIsNone(payload)

    def test_token_sent_via_header_not_url(self) -> None:
        stub = StubHttp({"/tasks/75159": (200, TASK_PAYLOAD)})
        client = ActiveCollabClient(_make_instance(), stub)
        client.fetch_task(665, 75159)
        for url in stub._seen_urls:  # noqa: SLF001
            self.assertNotIn(TOKEN, url)
        auth_header = stub._seen_headers[0].get("X-Angie-AuthApiToken")  # noqa: SLF001
        self.assertEqual(auth_header, TOKEN)


class TestFetchOpenTasks(unittest.TestCase):
    def test_returns_only_open_non_trashed_tasks(self) -> None:
        stub = StubHttp({f"/users/{USER_ID}/tasks": (200, MINE_PAYLOAD)})
        client = ActiveCollabClient(_make_instance(user_id=USER_ID), stub)
        tasks = client.fetch_open_tasks()
        names = [t.name for t in tasks]
        self.assertIn("Implement login flow", names)
        self.assertNotIn("Old task", names)
        self.assertNotIn("Trashed task", names)

    def test_returns_empty_when_no_user_id(self) -> None:
        stub = StubHttp({})
        client = ActiveCollabClient(_make_instance(user_id=None), stub)
        self.assertEqual(client.fetch_open_tasks(), [])

    def test_returns_empty_on_api_failure(self) -> None:
        stub = StubHttp({f"/users/{USER_ID}/tasks": (500, None)})
        client = ActiveCollabClient(_make_instance(user_id=USER_ID), stub)
        self.assertEqual(client.fetch_open_tasks(), [])

    def test_tasks_carry_instance_name(self) -> None:
        stub = StubHttp({f"/users/{USER_ID}/tasks": (200, MINE_PAYLOAD)})
        client = ActiveCollabClient(_make_instance(user_id=USER_ID), stub)
        tasks = client.fetch_open_tasks()
        self.assertTrue(all(t.instance_name == "default" for t in tasks))
