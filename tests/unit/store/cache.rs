use super::*;
use crate::config::Config;
use crate::store::Store;
use serde_json::json;
use tempfile::TempDir;

fn make_store() -> (TempDir, Store) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = Config {
        db_path,
        task_cache_ttl_hours: 24,
    };
    let store = Store::open(&config).unwrap();
    (dir, store)
}

#[test]
fn write_then_read_returns_stored_fields_with_comments() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let task = json!({ "id": 1, "title": "Fix bug" });
    let comments = json!([{ "body": "great catch" }]);
    cache.write("acme", 10, 99, &task, &comments).unwrap();
    let result = cache.read("acme", 10, 99).unwrap().unwrap();
    assert_eq!(result.fields["id"], 1);
    assert_eq!(result.fields["title"], "Fix bug");
    assert_eq!(result.fields["comments"], comments);
}

#[test]
fn read_returns_none_when_not_present() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let result = cache.read("acme", 10, 99).unwrap();
    assert!(result.is_none());
}

#[test]
fn fetched_at_format_is_iso_utc() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    cache.write("acme", 1, 1, &json!({}), &json!([])).unwrap();
    let result = cache.read("acme", 1, 1).unwrap().unwrap();
    assert_eq!(result.fetched_at.len(), 20);
    assert!(result.fetched_at.ends_with('Z'));
    assert_eq!(&result.fetched_at[10..11], "T");
}

#[test]
fn delete_for_instance_removes_only_that_instance() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    cache
        .write("acme", 1, 1, &json!({"x": 1}), &json!([]))
        .unwrap();
    cache
        .write("other", 1, 1, &json!({"x": 2}), &json!([]))
        .unwrap();
    cache.delete_for_instance("acme").unwrap();
    assert!(cache.read("acme", 1, 1).unwrap().is_none());
    assert!(cache.read("other", 1, 1).unwrap().is_some());
}

#[test]
fn write_overwrites_existing_entry() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    cache
        .write("acme", 5, 5, &json!({"v": 1}), &json!([]))
        .unwrap();
    cache
        .write("acme", 5, 5, &json!({"v": 2}), &json!([{"new": true}]))
        .unwrap();
    let result = cache.read("acme", 5, 5).unwrap().unwrap();
    assert_eq!(result.fields["v"], 2);
    assert_eq!(result.fields["comments"][0]["new"], true);
}

#[test]
fn payload_merge_matches_python_semantics() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let task = json!({ "id": 7, "status": "open" });
    let comments = json!([{ "id": 1, "body": "hello" }]);
    cache.write("inst", 3, 7, &task, &comments).unwrap();
    let result = cache.read("inst", 3, 7).unwrap().unwrap();
    // all task fields present
    assert_eq!(result.fields["id"], 7);
    assert_eq!(result.fields["status"], "open");
    // comments merged in
    assert_eq!(result.fields["comments"], comments);
}
