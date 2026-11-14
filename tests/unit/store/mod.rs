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
fn schema_has_all_four_tables() {
    let (_dir, store) = open_test_store();
    for table in &["instances", "ticket_cache", "settings", "user_map_cache"] {
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
fn user_map_cache_table_has_correct_columns() {
    let (_dir, store) = open_test_store();
    let mut stmt = store
        .conn()
        .prepare("PRAGMA table_info(user_map_cache)")
        .unwrap();
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(cols, ["instance", "users_json", "fetched_at"]);
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

fn journal_mode(store: &Store) -> String {
    store
        .conn()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn open_uses_wal_journal_mode() {
    let (_dir, store) = open_test_store();
    assert_eq!(journal_mode(&store), "wal");
}

#[test]
fn two_stores_write_the_task_cache_concurrently_without_lock_errors() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("shared.db");
    let handles: Vec<_> = (0..2)
        .map(|worker| {
            let db_path = db_path.clone();
            std::thread::spawn(move || -> Result<(), String> {
                let config = Config {
                    db_path,
                    task_cache_ttl_hours: 24,
                };
                let store = Store::open(&config).map_err(|e| e.to_string())?;
                let cache = cache::TaskCache::new(store.conn());
                for i in 0..50 {
                    cache
                        .write(
                            "inst",
                            worker,
                            i,
                            &serde_json::json!({ "name": "t" }),
                            &serde_json::json!([]),
                        )
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            })
        })
        .collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), Ok(()));
    }
    let store = Store::open(&Config {
        db_path,
        task_cache_ttl_hours: 24,
    })
    .unwrap();
    let rows: i64 = store
        .conn()
        .query_row("SELECT COUNT(*) FROM ticket_cache", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 100);
}

#[test]
fn existing_delete_mode_store_switches_to_wal_keeping_rows() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("legacy.db");
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "delete");
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);\
             INSERT INTO settings (key, value) VALUES ('theme', 'angie');",
        )
        .unwrap();
    }
    let store = Store::open(&Config {
        db_path,
        task_cache_ttl_hours: 24,
    })
    .unwrap();
    assert_eq!(journal_mode(&store), "wal");
    let value: String = store
        .conn()
        .query_row("SELECT value FROM settings WHERE key='theme'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, "angie");
}

const CONCURRENT_ROUNDS: usize = 40;
const WRITES_PER_THREAD: i64 = 5;

fn open_at(db_path: &std::path::Path) -> Result<Store, String> {
    Store::open(&Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    })
    .map_err(|e| e.to_string())
}

fn open_and_write_together(
    db_path: &std::path::Path,
    threads: usize,
    writes: i64,
) -> Vec<Result<(), String>> {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|worker| {
            let db_path = db_path.to_path_buf();
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || -> Result<(), String> {
                barrier.wait();
                let store = open_at(&db_path)?;
                let cache = cache::TaskCache::new(store.conn());
                for i in 0..writes {
                    cache
                        .write(
                            "inst",
                            worker as i64,
                            i,
                            &serde_json::json!({ "name": "t" }),
                            &serde_json::json!([]),
                        )
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            })
        })
        .collect();
    handles.into_iter().map(|h| h.join().unwrap()).collect()
}

fn count_cache_rows(db_path: &std::path::Path) -> i64 {
    open_at(db_path)
        .unwrap()
        .conn()
        .query_row("SELECT COUNT(*) FROM ticket_cache", [], |r| r.get(0))
        .unwrap()
}

fn assert_concurrent_opens_succeed(threads: usize) {
    for _ in 0..CONCURRENT_ROUNDS {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("shared.db");
        let results = open_and_write_together(&db_path, threads, WRITES_PER_THREAD);
        assert_eq!(results, vec![Ok(()); threads]);
        assert_eq!(
            count_cache_rows(&db_path),
            threads as i64 * WRITES_PER_THREAD
        );
    }
}

#[test]
fn concurrent_opens_by_two_threads_all_write() {
    assert_concurrent_opens_succeed(2);
}

#[test]
fn concurrent_opens_by_four_threads_all_write() {
    assert_concurrent_opens_succeed(4);
}

#[test]
fn concurrent_opens_of_a_delete_mode_store_end_in_wal_keeping_rows() {
    for _ in 0..CONCURRENT_ROUNDS {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("legacy.db");
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))
                .unwrap();
            assert_eq!(mode, "delete");
            conn.execute_batch(
                "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);\
                 INSERT INTO settings (key, value) VALUES ('theme', 'angie');",
            )
            .unwrap();
        }
        let results = open_and_write_together(&db_path, 4, 5);
        assert_eq!(results, vec![Ok(()); 4]);
        let store = open_at(&db_path).unwrap();
        assert_eq!(journal_mode(&store), "wal");
        let theme: String = store
            .conn()
            .query_row("SELECT value FROM settings WHERE key='theme'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(theme, "angie");
        assert_eq!(count_cache_rows(&db_path), 20);
    }
}

fn sqlite_failure(code: i32) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None)
}

#[test]
fn wal_switch_succeeding_lets_the_open_continue() {
    assert!(tolerate_busy_wal_switch(Ok("wal".to_string())).is_ok());
}

#[test]
fn wal_switch_busy_failure_lets_the_open_continue() {
    let busy = sqlite_failure(rusqlite::ffi::SQLITE_BUSY);
    assert!(tolerate_busy_wal_switch(Err(busy)).is_ok());
}

#[test]
fn wal_switch_failure_other_than_busy_fails_the_open_with_that_error() {
    for (code, expected) in [
        (
            rusqlite::ffi::SQLITE_READONLY,
            rusqlite::ErrorCode::ReadOnly,
        ),
        (
            rusqlite::ffi::SQLITE_CANTOPEN,
            rusqlite::ErrorCode::CannotOpen,
        ),
    ] {
        let err = tolerate_busy_wal_switch(Err(sqlite_failure(code))).unwrap_err();
        match err.downcast_ref::<rusqlite::Error>() {
            Some(rusqlite::Error::SqliteFailure(failure, _)) => {
                assert_eq!(failure.code, expected);
            }
            other => panic!("expected the sqlite failure to propagate, got {other:?}"),
        }
    }
}

#[test]
fn wal_switch_failure_that_is_not_a_sqlite_failure_fails_the_open() {
    let err = tolerate_busy_wal_switch(Err(rusqlite::Error::QueryReturnedNoRows)).unwrap_err();
    assert_eq!(err.to_string(), "Query returned no rows");
}

#[test]
fn fresh_store_opened_concurrently_has_wal_timeout_and_schema() {
    for _ in 0..CONCURRENT_ROUNDS {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("fresh.db");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let db_path = db_path.clone();
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    open_at(&db_path)
                })
            })
            .collect();
        let stores: Vec<Store> = handles
            .into_iter()
            .map(|h| h.join().unwrap().unwrap())
            .collect();
        for store in &stores {
            assert_eq!(journal_mode(store), "wal");
            let timeout: i64 = store
                .conn()
                .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
                .unwrap();
            assert_eq!(timeout, 5000);
            let tables: i64 = store
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN \
                     ('instances','ticket_cache','settings','user_map_cache','project_names_cache','task_list_cache')",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(tables, 6);
        }
    }
}
