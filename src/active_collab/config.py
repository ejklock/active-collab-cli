import os
from dataclasses import dataclass

_DEFAULT_DB_PATH = os.path.join(
    os.path.expanduser("~"), ".config", "active-collab", "active-collab.db"
)
_TASK_CACHE_TTL_HOURS = 24


@dataclass(frozen=True)
class Config:
    db_path: str
    task_cache_ttl_hours: int

    @classmethod
    def load(cls) -> "Config":
        db_path = os.environ.get("ACTIVE_COLLAB_DB", _DEFAULT_DB_PATH)
        return cls(db_path=db_path, task_cache_ttl_hours=_TASK_CACHE_TTL_HOURS)
