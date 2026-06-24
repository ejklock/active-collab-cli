class ActiveCollabError(Exception):
    """Base error for all ActiveCollab CLI failures."""


class HttpError(ActiveCollabError):
    """Raised on transport failure (not on HTTP error status codes)."""

    def __init__(self, status: int, message: str = "") -> None:
        super().__init__(message or f"HTTP {status}")
        self.status = status


class ConfigError(ActiveCollabError):
    """Raised when required configuration is missing or invalid."""
