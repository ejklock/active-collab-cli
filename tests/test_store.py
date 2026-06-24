"""Tests for store.py — Store, InstanceRepository, TaskCache."""

import os
import tempfile
import unittest

from active_collab.config import Config
from active_collab.models import Instance
from active_collab.store import InstanceRepository, Store, TaskCache


def _tmp_config() -> tuple[Config, str]:
    """Return a Config pointing to a temp path + the path string (not yet created)."""
    db_file = tempfile.NamedTemporaryFile(suffix=".db", delete=False)
    db_file.close()
    path = db_file.name
    os.unlink(path)
    return Config(db_path=path, task_cache_ttl_hours=24), path


class TestStoreOpen(unittest.TestCase):
    def setUp(self) -> None:
        self._config, self._path = _tmp_config()
        os.environ["ACTIVE_COLLAB_DB"] = self._path

    def tearDown(self) -> None:
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        if os.path.exists(self._path):
            os.unlink(self._path)

    def test_tables_created_on_first_open(self) -> None:
        with Store(self._config) as store:
            tables = {
                row[0]
                for row in store.conn.execute(
                    "SELECT name FROM sqlite_master WHERE type='table'"
                ).fetchall()
            }
        self.assertIn("instances", tables)
        self.assertIn("ticket_cache", tables)

    def test_journal_mode_is_delete(self) -> None:
        with Store(self._config) as store:
            mode = store.conn.execute("PRAGMA journal_mode").fetchone()[0]
        self.assertEqual(mode, "delete")

    def test_schema_idempotent(self) -> None:
        Store(self._config).close()
        Store(self._config).close()

    def test_db_file_permission_is_0600(self) -> None:
        with Store(self._config):
            mode = oct(os.stat(self._path).st_mode)[-3:]
        self.assertEqual(mode, "600")

    def test_parent_dir_created_with_0700(self) -> None:
        nested = os.path.join(self._path + "_dir", "sub", "active-collab.db")
        config = Config(db_path=nested, task_cache_ttl_hours=24)
        try:
            with Store(config):
                parent = os.path.dirname(nested)
                mode = oct(os.stat(parent).st_mode)[-3:]
                self.assertEqual(mode, "700")
        finally:
            if os.path.exists(nested):
                os.unlink(nested)
            parent = os.path.dirname(nested)
            if os.path.exists(parent):
                os.rmdir(parent)
            grandparent = os.path.dirname(parent)
            if os.path.exists(grandparent):
                os.rmdir(grandparent)


class TestInstanceRepository(unittest.TestCase):
    def setUp(self) -> None:
        self._config, self._path = _tmp_config()

    def tearDown(self) -> None:
        if os.path.exists(self._path):
            os.unlink(self._path)

    def _store(self) -> Store:
        return Store(self._config)

    def test_save_and_load_all(self) -> None:
        inst = Instance(name="test", base_url="https://example.com", email="e@e.com", token="tok", user_id=7)
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            repo.save(inst)
            all_insts = repo.load_all()
        self.assertEqual(len(all_insts), 1)
        self.assertEqual(all_insts[0].name, "test")
        self.assertEqual(all_insts[0].token, "tok")
        self.assertEqual(all_insts[0].user_id, 7)

    def test_save_upserts_on_same_name(self) -> None:
        inst1 = Instance(name="dup", base_url="https://a.com", email="a@a.com", token="t1")
        inst2 = Instance(name="dup", base_url="https://b.com", email="b@b.com", token="t2")
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            repo.save(inst1)
            repo.save(inst2)
            rows = repo.load_all()
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0].token, "t2")

    def test_delete_removes_instance_returns_count(self) -> None:
        inst = Instance(name="del", base_url="https://x.com", email="x@x.com", token="t")
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            repo.save(inst)
            count = repo.delete("del")
            self.assertEqual(count, 1)
            self.assertEqual(len(repo.load_all()), 0)

    def test_delete_nonexistent_returns_0(self) -> None:
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            count = repo.delete("ghost")
        self.assertEqual(count, 0)

    def test_list_for_display_excludes_token(self) -> None:
        inst = Instance(name="x", base_url="https://x.com", email="x@x.com", token="SECRET")
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            repo.save(inst)
            rows = repo.list_for_display()
        self.assertEqual(len(rows), 1)
        row = rows[0]
        self.assertNotIn("SECRET", str(row))

    def test_list_connectivity_returns_name_url_token(self) -> None:
        inst = Instance(name="c", base_url="https://c.com", email="c@c.com", token="ctok")
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            repo.save(inst)
            rows = repo.list_connectivity()
        self.assertEqual(rows, [("c", "https://c.com", "ctok")])

    def test_find_by_name_returns_matching_row(self) -> None:
        inst = Instance(name="found", base_url="https://f.com", email="f@f.com", token="ftok")
        with self._store() as store:
            repo = InstanceRepository(store.conn)
            repo.save(inst)
            rows = repo.find_by_name("found")
            empty = repo.find_by_name("missing")
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][0], "found")
        self.assertEqual(empty, [])


class TestTaskCache(unittest.TestCase):
    def setUp(self) -> None:
        self._config, self._path = _tmp_config()

    def tearDown(self) -> None:
        if os.path.exists(self._path):
            os.unlink(self._path)

    def test_read_returns_none_when_empty(self) -> None:
        with Store(self._config) as store:
            cache = TaskCache(store.conn)
            result = cache.read("inst", 1, 2)
        self.assertIsNone(result)

    def test_write_then_read_returns_payload(self) -> None:
        task = {"id": 1, "name": "T", "tracked_time": 3.0}
        comments = [{"id": 10, "body": "hi"}]
        with Store(self._config) as store:
            cache = TaskCache(store.conn)
            cache.write("inst", 665, 75159, task, comments)
            result = cache.read("inst", 665, 75159)
        self.assertIsNotNone(result)
        fields = result["fields"]
        self.assertEqual(fields["name"], "T")
        self.assertEqual(fields["comments"], comments)

    def test_write_overwrites_existing_entry(self) -> None:
        with Store(self._config) as store:
            cache = TaskCache(store.conn)
            cache.write("inst", 1, 1, {"name": "old"}, [])
            cache.write("inst", 1, 1, {"name": "new"}, [])
            result = cache.read("inst", 1, 1)
        self.assertEqual(result["fields"]["name"], "new")

    def test_delete_for_instance_removes_entries(self) -> None:
        with Store(self._config) as store:
            cache = TaskCache(store.conn)
            cache.write("inst-a", 1, 1, {"x": 1}, [])
            cache.write("inst-b", 2, 2, {"y": 2}, [])
            cache.delete_for_instance("inst-a")
            self.assertIsNone(cache.read("inst-a", 1, 1))
            self.assertIsNotNone(cache.read("inst-b", 2, 2))

    def test_fetched_at_is_iso_format(self) -> None:
        with Store(self._config) as store:
            cache = TaskCache(store.conn)
            cache.write("inst", 1, 1, {}, [])
            result = cache.read("inst", 1, 1)
        fetched_at = result["fetched_at"]
        self.assertRegex(fetched_at, r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
