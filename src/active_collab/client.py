import json

from active_collab.http import HttpClient
from active_collab.models import Instance, MineTask


def _auth_headers(instance: Instance) -> dict:
    return {
        "Accept": "application/json",
        "X-Angie-AuthApiToken": instance.token,
    }


class ActiveCollabClient:
    """ActiveCollab REST API client.

    All HTTP calls are delegated to the injected HttpClient so that
    callers can substitute a stub for tests — no real network required.
    """

    def __init__(self, instance: Instance, http: HttpClient) -> None:
        self._inst = instance
        self._http = http

    def exchange_token(self, base_url: str, email: str, password: str) -> tuple[str | None, dict]:
        """POST to issue-token endpoint. Return (token, raw_response)."""
        url = f"{base_url}/api/v1/issue-token"
        body = {
            "username": email,
            "password": password,
            "client_name": "active-collab-skill",
            "client_vendor": "klock",
        }
        status, raw = self._http.post(url, body, {})
        if status != 200:
            return None, {}
        data = json.loads(raw)
        if not data.get("is_ok"):
            return None, data
        return data.get("token"), data

    def resolve_user_id(self, base_url: str, token: str, email: str) -> int | None:
        """Fetch /api/v1/users and return the user_id matching email (case-insensitive)."""
        headers = {"Accept": "application/json", "X-Angie-AuthApiToken": token}
        status, body = self._http.get(f"{base_url}/api/v1/users", headers)
        if status != 200:
            return None
        data = json.loads(body)
        users = data if isinstance(data, list) else []
        email_lower = email.lower()
        for user in users:
            if (user.get("email") or "").lower() == email_lower:
                return user.get("id")
        return None

    def fetch_user_map(self) -> dict:
        """Return {user_id: display_name}. Returns {} on any failure."""
        base = self._inst.base_url.rstrip("/")
        status, body = self._http.get(f"{base}/api/v1/users", _auth_headers(self._inst))
        if status != 200:
            return {}
        data = json.loads(body)
        if not isinstance(data, list):
            return {}
        result = {}
        for user in data:
            uid = user.get("id")
            if uid is None:
                continue
            name = (
                user.get("display_name")
                or " ".join(
                    filter(
                        None,
                        [
                            (user.get("first_name") or "").strip(),
                            (user.get("last_name") or "").strip(),
                        ],
                    )
                )
                or user.get("email")
                or ""
            )
            result[uid] = name
        return result

    def fetch_task(self, project_id: int, task_id: int) -> tuple[int, dict | None]:
        """Return (status_code, full_payload_dict_or_None) from the API."""
        base = self._inst.base_url.rstrip("/")
        url = f"{base}/api/v1/projects/{project_id}/tasks/{task_id}"
        status, body = self._http.get(url, _auth_headers(self._inst))
        if status == 200:
            return status, json.loads(body)
        return status, None

    def fetch_open_tasks(self) -> list[MineTask]:
        """Fetch open tasks assigned to this user via GET /api/v1/users/{user_id}/tasks."""
        user_id = self._inst.user_id
        if not user_id:
            return []
        base = self._inst.base_url.rstrip("/")
        status, body = self._http.get(
            f"{base}/api/v1/users/{user_id}/tasks", _auth_headers(self._inst)
        )
        if status != 200:
            return []
        data = json.loads(body)
        raw_tasks = data.get("tasks", []) if isinstance(data, dict) else []
        return [
            MineTask.from_api(t, instance_name=self._inst.name)
            for t in raw_tasks
            if not t.get("is_completed") and not t.get("is_trashed")
        ]

    def list_projects(self) -> tuple[int, bytes]:
        """GET /api/v1/projects — used by connectivity checks."""
        base = self._inst.base_url.rstrip("/")
        return self._http.get(f"{base}/api/v1/projects", _auth_headers(self._inst))

    def test_connectivity(self) -> tuple[int, bytes]:
        """Alias for list_projects — used by setup test / setup add."""
        return self.list_projects()
