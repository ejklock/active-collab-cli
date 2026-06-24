import json
import urllib.error
import urllib.request


class HttpClient:
    """Thin urllib wrapper: get/post return (status, body) even on HTTP error status.

    Only transport failures (URLError that is not HTTPError) raise ConnectionError,
    matching the legacy http_get/http_post behaviour exactly.
    """

    def __init__(
        self,
        timeout: int = 30,
        default_headers: dict | None = None,
    ) -> None:
        self._timeout = timeout
        self._default_headers = default_headers or {}

    def get(self, url: str, headers: dict | None = None) -> tuple[int, bytes]:
        merged = {**self._default_headers, **(headers or {})}
        req = urllib.request.Request(url, headers=merged)
        try:
            with urllib.request.urlopen(req, timeout=self._timeout) as resp:  # nosec: B310
                return resp.status, resp.read()
        except urllib.error.HTTPError as exc:
            return exc.code, exc.read()
        except urllib.error.URLError as exc:
            raise ConnectionError(str(exc.reason)) from exc

    def post(self, url: str, data: dict, headers: dict | None = None) -> tuple[int, bytes]:
        payload = json.dumps(data).encode("utf-8")
        merged = {
            "Content-Type": "application/json",
            **self._default_headers,
            **(headers or {}),
        }
        req = urllib.request.Request(url, data=payload, headers=merged)
        try:
            with urllib.request.urlopen(req, timeout=self._timeout) as resp:  # nosec: B310
                return resp.status, resp.read()
        except urllib.error.HTTPError as exc:
            return exc.code, exc.read()
        except urllib.error.URLError as exc:
            raise ConnectionError(str(exc.reason)) from exc
