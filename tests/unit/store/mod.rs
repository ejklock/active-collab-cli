use super::*;
use tempfile::TempDir;

fn open_test_store() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("sub").join("test.db");
    let config = Config {
        db_path: db_path.clone(),
        task_cache_ttl_hours: 24,
    };
    let store = Store::open(&config).unwrap();
    (dir, store)
}

#[test]
fn open_creates_parent_dir_and_db_file() {
    let (_dir, store) = open_test_store();
    let count: i64 = store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='instances'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn schema_has_all_three_tables() {
    let (_dir, store) = open_test_store();
    for table in &["instances", "ticket_cache", "settings"] {
        let count: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![table],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "table {table} not found");
    }
}

#[test]
fn instances_table_has_correct_columns() {
    let (_dir, store) = open_test_store();
    let mut stmt = store
        .conn()
        .prepare("PRAGMA table_info(instances)")
        .unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        cols,
        [
            "name",
            "base_url",
            "email",
            "token",
            "user_id",
            "created_at"
        ]
    );
}

#[test]
fn ticket_cache_table_has_correct_columns() {
    let (_dir, store) = open_test_store();
    let mut stmt = store
        .conn()
        .prepare("PRAGMA table_info(ticket_cache)")
        .unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        cols,
        [
            "instance",
            "project_id",
            "task_id",
            "fields_json",
            "fetched_at"
        ]
    );
}

#[test]
fn settings_table_has_correct_columns() {
    let (_dir, store) = open_test_store();
    let mut stmt = store.conn().prepare("PRAGMA table_info(settings)").unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(cols, ["key", "value"]);
}

#[cfg(unix)]
#[test]
fn db_file_has_mode_600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = Config {
        db_path: db_path.clone(),
        task_cache_ttl_hours: 24,
    };
    Store::open(&config).unwrap();
    let meta = std::fs::metadata(&db_path).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "DB file mode should be 0600, got {mode:o}");
}

#[cfg(unix)]
#[test]
fn parent_dir_has_mode_700() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let parent = dir.path().join("newdir");
    let db_path = parent.join("test.db");
    let config = Config {
        db_path: db_path.clone(),
        task_cache_ttl_hours: 24,
    };
    Store::open(&config).unwrap();
    let meta = std::fs::metadata(&parent).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "parent dir mode should be 0700, got {mode:o}");
}

#[test]
fn now_iso_format_matches_python_strftime() {
    let ts = now_iso();
    assert_eq!(ts.len(), 20, "expected 20 chars, got: {ts}");
    assert!(ts.ends_with('Z'), "should end with Z: {ts}");
    assert_eq!(&ts[4..5], "-");
    assert_eq!(&ts[7..8], "-");
    assert_eq!(&ts[10..11], "T");
    assert_eq!(&ts[13..14], ":");
    assert_eq!(&ts[16..17], ":");
}

#[test]
fn foreign_keys_pragma_is_applied() {
    let (_dir, store) = open_test_store();
    let val: i64 = store
        .conn()
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(val, 1);
}
