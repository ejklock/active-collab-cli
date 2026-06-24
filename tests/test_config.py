"""Tests for Config.load() — env override and legacy default path."""

import os
import unittest

from active_collab.config import Config


class TestConfigLoad(unittest.TestCase):
    def tearDown(self) -> None:
        os.environ.pop("ACTIVE_COLLAB_DB", None)

    def test_load_uses_env_override_when_set(self) -> None:
        os.environ["ACTIVE_COLLAB_DB"] = "/tmp/override.db"
        cfg = Config.load()
        self.assertEqual(cfg.db_path, "/tmp/override.db")

    def test_load_falls_back_to_default_path_when_env_absent(self) -> None:
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        cfg = Config.load()
        self.assertIn(".config", cfg.db_path)
        self.assertIn("active-collab", cfg.db_path)
        self.assertTrue(cfg.db_path.endswith(".db"))

    def test_default_path_is_under_home_directory(self) -> None:
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        cfg = Config.load()
        home = os.path.expanduser("~")
        self.assertTrue(cfg.db_path.startswith(home))

    def test_task_cache_ttl_is_24_hours(self) -> None:
        cfg = Config.load()
        self.assertEqual(cfg.task_cache_ttl_hours, 24)

    def test_config_is_frozen(self) -> None:
        cfg = Config.load()
        with self.assertRaises(Exception):
            cfg.db_path = "/new/path.db"  # type: ignore[misc]

    def test_env_override_empty_falls_back_to_default(self) -> None:
        """An empty ACTIVE_COLLAB_DB string means env var is absent — falls back to default."""
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        cfg = Config.load()
        self.assertIn("active-collab", cfg.db_path)

    def test_legacy_default_db_name_matches(self) -> None:
        os.environ.pop("ACTIVE_COLLAB_DB", None)
        cfg = Config.load()
        self.assertTrue(cfg.db_path.endswith("active-collab.db"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
