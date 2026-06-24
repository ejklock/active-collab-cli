"""Tests for HttpClient — no real network; urllib.request.urlopen is always stubbed."""

import json
import unittest
import urllib.error
import urllib.request
from unittest import mock

from active_collab.http import HttpClient


def _make_mock_response(status: int, body: bytes) -> mock.MagicMock:
    resp = mock.MagicMock()
    resp.status = status
    resp.read.return_value = body
    resp.__enter__ = lambda s: s
    resp.__exit__ = mock.MagicMock(return_value=False)
    return resp


def _make_http_error(code: int, body: bytes) -> urllib.error.HTTPError:
    return urllib.error.HTTPError(
        url="http://example.com",
        code=code,
        msg="",
        hdrs={},  # type: ignore[arg-type]
        fp=mock.MagicMock(read=mock.MagicMock(return_value=body)),
    )


class TestHttpClientGet(unittest.TestCase):
    def setUp(self) -> None:
        self.client = HttpClient()

    def test_get_returns_status_and_body_on_200(self) -> None:
        body = b'{"ok": true}'
        mock_resp = _make_mock_response(200, body)
        with mock.patch("urllib.request.urlopen", return_value=mock_resp):
            status, result = self.client.get("http://example.com/api")
        self.assertEqual(status, 200)
        self.assertEqual(result, body)

    def test_get_returns_status_and_body_on_404(self) -> None:
        """HTTP 4xx comes back as (status, body) — not raised."""
        exc = _make_http_error(404, b"not found")
        with mock.patch("urllib.request.urlopen", side_effect=exc):
            status, result = self.client.get("http://example.com/missing")
        self.assertEqual(status, 404)
        self.assertEqual(result, b"not found")

    def test_get_returns_status_and_body_on_500(self) -> None:
        """HTTP 5xx comes back as (status, body) — not raised."""
        exc = _make_http_error(500, b"server error")
        with mock.patch("urllib.request.urlopen", side_effect=exc):
            status, result = self.client.get("http://example.com/broken")
        self.assertEqual(status, 500)
        self.assertEqual(result, b"server error")

    def test_get_raises_connection_error_on_url_error(self) -> None:
        """Transport failures (URLError) raise ConnectionError, matching legacy semantics."""
        url_err = urllib.error.URLError(reason="Name or service not known")
        with mock.patch("urllib.request.urlopen", side_effect=url_err):
            with self.assertRaises(ConnectionError):
                self.client.get("http://unreachable.invalid/")

    def test_get_merges_default_headers_with_per_call_headers(self) -> None:
        client = HttpClient(default_headers={"X-Default": "yes"})
        captured: list[urllib.request.Request] = []
        mock_resp = _make_mock_response(200, b"ok")

        def capturing_urlopen(req, timeout=None):  # noqa: ARG001
            captured.append(req)
            return mock_resp

        with mock.patch("urllib.request.urlopen", side_effect=capturing_urlopen):
            client.get("http://example.com", headers={"X-Extra": "extra"})

        req = captured[0]
        self.assertEqual(req.get_header("X-default"), "yes")
        self.assertEqual(req.get_header("X-extra"), "extra")

    def test_get_per_call_headers_override_defaults(self) -> None:
        client = HttpClient(default_headers={"X-Token": "old"})
        captured: list[urllib.request.Request] = []
        mock_resp = _make_mock_response(200, b"ok")

        def capturing_urlopen(req, timeout=None):  # noqa: ARG001
            captured.append(req)
            return mock_resp

        with mock.patch("urllib.request.urlopen", side_effect=capturing_urlopen):
            client.get("http://example.com", headers={"X-Token": "new"})

        req = captured[0]
        self.assertEqual(req.get_header("X-token"), "new")


class TestHttpClientPost(unittest.TestCase):
    def setUp(self) -> None:
        self.client = HttpClient()

    def test_post_returns_status_and_body_on_200(self) -> None:
        body = b'{"is_ok": true, "token": "abc"}'
        mock_resp = _make_mock_response(200, body)
        with mock.patch("urllib.request.urlopen", return_value=mock_resp):
            status, result = self.client.post("http://example.com/api/token", {"user": "x"})
        self.assertEqual(status, 200)
        self.assertEqual(result, body)

    def test_post_returns_status_and_body_on_401(self) -> None:
        exc = _make_http_error(401, b"unauthorized")
        with mock.patch("urllib.request.urlopen", side_effect=exc):
            status, result = self.client.post("http://example.com/api/token", {})
        self.assertEqual(status, 401)
        self.assertEqual(result, b"unauthorized")

    def test_post_raises_connection_error_on_url_error(self) -> None:
        url_err = urllib.error.URLError(reason="Connection refused")
        with mock.patch("urllib.request.urlopen", side_effect=url_err):
            with self.assertRaises(ConnectionError):
                self.client.post("http://unreachable.invalid/token", {})

    def test_post_sends_json_encoded_body(self) -> None:
        captured: list[urllib.request.Request] = []
        mock_resp = _make_mock_response(200, b"ok")

        def capturing_urlopen(req, timeout=None):  # noqa: ARG001
            captured.append(req)
            return mock_resp

        with mock.patch("urllib.request.urlopen", side_effect=capturing_urlopen):
            self.client.post("http://example.com/api", {"key": "value"})

        req = captured[0]
        sent = json.loads(req.data)
        self.assertEqual(sent, {"key": "value"})

    def test_post_sets_content_type_json(self) -> None:
        captured: list[urllib.request.Request] = []
        mock_resp = _make_mock_response(200, b"ok")

        def capturing_urlopen(req, timeout=None):  # noqa: ARG001
            captured.append(req)
            return mock_resp

        with mock.patch("urllib.request.urlopen", side_effect=capturing_urlopen):
            self.client.post("http://example.com/api", {})

        req = captured[0]
        self.assertEqual(req.get_header("Content-type"), "application/json")


if __name__ == "__main__":
    unittest.main(verbosity=2)
