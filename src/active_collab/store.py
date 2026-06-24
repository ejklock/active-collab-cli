import datetime
import json
import os
import sqlite3

from active_collab.config import Config
from active_collab.models import Instance


def _now_iso() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _is_mode_600(path: str) -> bool:
    mode = oct(os.stat(path).st_mode)[-3:]
    return mode == "600"


class Store:
    """Owns the SQLite connection and schema for the active-collab CLI.

    Opens (or creates) the DB file at the path from Config, enforcing
    directory 0700 and file 0600 permissions exactly as the legacy _open_db.
    """

    def __init__(self, config: Config) -> None:
        self._path = config.db_path
        self._conn = self._open()

    def _open(self) -> sqlite3.Connection:
        parent = os.path.dirname(self._path)
        if not os.path.isdir(parent):
            os.makedirs(parent, mode=0o700, exist_ok=True)
        conn = sqlite3.connect(self._path)
        if not _is_mode_600(self._path):
            os.chmod(self._path, 0o600)
        # DELETE journal avoids WAL sidecar files; single-process CLI needs no WAL.  # nosec
        conn.execute("PRAGMA journal_mode=DELETE")
        conn.execute("PRAGMA busy_timeout=5000")
        conn.execute("PRAGMA foreign_keys=ON")
        _init_schema(conn)
        return conn

    @property
    def conn(self) -> sqlite3.Connection:
        return self._conn

    def close(self) -> None:
        self._conn.close()

    def __enter__(self) -> "Store":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def _init_schema(conn: sqlite3.Connection) -> None:
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS instances (
            name       TEXT PRIMARY KEY,
            base_url   TEXT NOT NULL,
            email      TEXT NOT NULL,
            token      TEXT NOT NULL,
            user_id    INTEGER,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ticket_cache (
            instance   TEXT NOT NULL,
            project_id INTEGER NOT NULL,
            task_id    INTEGER NOT NULL,
            fields_json TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            PRIMARY KEY (instance, project_id, task_id)
        );
    """)
    conn.commit()


class InstanceRepository:
    """CRUD operations for the `instances` table."""

    def __init__(self, conn: sqlite3.Connection) -> None:
        self._conn = conn

    def save(self, instance: Instance) -> None:
        self._conn.execute(
            "INSERT OR REPLACE INTO instances"
            " (name, base_url, email, token, user_id, created_at)"
            " VALUES (?, ?, ?, ?, ?, ?)",
            (
                instance.name,
                instance.base_url,
                instance.email,
                instance.token,
                instance.user_id,
                _now_iso(),
            ),
        )
        self._conn.commit()

    def load_all(self) -> list[Instance]:
        rows = self._conn.execute(
            "SELECT name, base_url, email, token, user_id"
            " FROM instances ORDER BY created_at, name"
        ).fetchall()
        return [Instance.from_row(row) for row in rows]

    def delete(self, name: str) -> int:
        rowcount = self._conn.execute(
            "DELETE FROM instances WHERE name = ?", (name,)
        ).rowcount
        self._conn.commit()
        return rowcount

    def list_for_display(self) -> list[tuple]:
        """Return (name, base_url, email, user_id) rows ordered by created_at, name."""
        return self._conn.execute(
            "SELECT name, base_url, email, user_id FROM instances ORDER BY created_at, name"
        ).fetchall()

    def list_connectivity(self) -> list[tuple]:
        """Return (name, base_url, token) rows for connectivity checks."""
        return self._conn.execute(
            "SELECT name, base_url, token FROM instances ORDER BY created_at, name"
        ).fetchall()

    def find_by_name(self, name: str) -> list[tuple]:
        """Return (name, base_url, token) rows filtered by instance name."""
        return self._conn.execute(
            "SELECT name, base_url, token FROM instances WHERE name = ?", (name,)
        ).fetchall()


class TaskCache:
    """Read/write operations for the `ticket_cache` table."""

    def __init__(self, conn: sqlite3.Connection) -> None:
        self._conn = conn

    def read(
        self, instance: str, project_id: int, task_id: int
    ) -> dict | None:
        row = self._conn.execute(
            "SELECT fields_json, fetched_at FROM ticket_cache"
            " WHERE instance=? AND project_id=? AND task_id=?",
            (instance, project_id, task_id),
        ).fetchone()
        if not row:
            return None
        return {"fields": json.loads(row[0]), "fetched_at": row[1]}

    def write(
        self,
        instance: str,
        project_id: int,
        task_id: int,
        task: dict,
        comments: list,
    ) -> None:
        payload = {**task, "comments": comments}
        self._conn.execute(
            "INSERT OR REPLACE INTO ticket_cache"
            " (instance, project_id, task_id, fields_json, fetched_at)"
            " VALUES (?, ?, ?, ?, ?)",
            (instance, project_id, task_id, json.dumps(payload), _now_iso()),
        )
        self._conn.commit()

    def delete_for_instance(self, instance: str) -> None:
        self._conn.execute(
            "DELETE FROM ticket_cache WHERE instance = ?", (instance,)
        )
        self._conn.commit()
