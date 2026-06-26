use super::*;
use crate::config::Config;
use crate::store::cache::{ProjectNamesCache, UserMapCache};
use crate::store::Store;
use serde_json::json;
use std::collections::HashMap;
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
    assert_eq!(result.fields["id"], 7);
    assert_eq!(result.fields["status"], "open");
    assert_eq!(result.fields["comments"], comments);
}

// S5-A4: UserMapCache round-trip preserves i64 keys and string values
#[test]
fn user_map_cache_write_then_read_round_trips_i64_keys() {
    let (_dir, store) = make_store();
    let cache = UserMapCache::new(store.conn());
    let mut users: HashMap<i64, String> = HashMap::new();
    users.insert(1, "Alice".to_string());
    users.insert(9999999999i64, "Bob Large Id".to_string());
    cache.write("acme", &users).unwrap();
    let result = cache.read("acme").unwrap().unwrap();
    assert_eq!(result.users.get(&1).map(|s| s.as_str()), Some("Alice"));
    assert_eq!(
        result.users.get(&9999999999i64).map(|s| s.as_str()),
        Some("Bob Large Id")
    );
    assert_eq!(result.users.len(), 2);
}

#[test]
fn user_map_cache_read_returns_none_when_missing() {
    let (_dir, store) = make_store();
    let cache = UserMapCache::new(store.conn());
    let result = cache.read("nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn user_map_cache_write_overwrites_existing_entry() {
    let (_dir, store) = make_store();
    let cache = UserMapCache::new(store.conn());
    let v1: HashMap<i64, String> = [(1i64, "Old".to_string())].into_iter().collect();
    let v2: HashMap<i64, String> = [(1i64, "New".to_string()), (2i64, "Extra".to_string())]
        .into_iter()
        .collect();
    cache.write("acme", &v1).unwrap();
    cache.write("acme", &v2).unwrap();
    let result = cache.read("acme").unwrap().unwrap();
    assert_eq!(result.users.get(&1).map(|s| s.as_str()), Some("New"));
    assert_eq!(result.users.get(&2).map(|s| s.as_str()), Some("Extra"));
}

#[test]
fn user_map_cache_fetched_at_is_iso_utc() {
    let (_dir, store) = make_store();
    let cache = UserMapCache::new(store.conn());
    let users: HashMap<i64, String> = [(5i64, "Dave".to_string())].into_iter().collect();
    cache.write("acme", &users).unwrap();
    let result = cache.read("acme").unwrap().unwrap();
    assert_eq!(result.fetched_at.len(), 20);
    assert!(result.fetched_at.ends_with('Z'));
    assert_eq!(&result.fetched_at[10..11], "T");
}

#[test]
fn project_names_cache_write_then_read_round_trips_i64_keys() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let mut names: HashMap<i64, String> = HashMap::new();
    names.insert(1, "Alpha Project".to_string());
    names.insert(9999999999i64, "Large Id Project".to_string());
    cache.write("acme", &names).unwrap();
    let result = cache.read("acme").unwrap().unwrap();
    assert_eq!(
        result.names.get(&1).map(|s| s.as_str()),
        Some("Alpha Project")
    );
    assert_eq!(
        result.names.get(&9999999999i64).map(|s| s.as_str()),
        Some("Large Id Project")
    );
    assert_eq!(result.names.len(), 2);
}

#[test]
fn project_names_cache_read_returns_none_when_absent() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let result = cache.read("nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn project_names_cache_per_instance_isolation() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());

    let names_a: HashMap<i64, String> = [(100i64, "Project A".to_string())].into_iter().collect();
    let names_b: HashMap<i64, String> = [(100i64, "Project B".to_string())].into_iter().collect();

    cache.write("instance-a", &names_a).unwrap();
    cache.write("instance-b", &names_b).unwrap();

    let result_a = cache.read("instance-a").unwrap().unwrap();
    let result_b = cache.read("instance-b").unwrap().unwrap();

    assert_eq!(
        result_a.names.get(&100).map(|s| s.as_str()),
        Some("Project A"),
        "instance-a must read its own names"
    );
    assert_eq!(
        result_b.names.get(&100).map(|s| s.as_str()),
        Some("Project B"),
        "instance-b must read its own names, not instance-a's"
    );
}

#[test]
fn project_names_cache_fetched_at_is_recent() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let before = crate::store::now_epoch_secs();
    let names: HashMap<i64, String> = [(1i64, "Test".to_string())].into_iter().collect();
    cache.write("acme", &names).unwrap();
    let result = cache.read("acme").unwrap().unwrap();
    assert!(
        result.fetched_at >= before,
        "fetched_at must be >= timestamp captured before write"
    );
}

#[test]
fn project_names_cache_write_with_fetched_at_stamps_supplied_timestamp() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let names: HashMap<i64, String> = [(42i64, "Seeded Project".to_string())]
        .into_iter()
        .collect();
    let supplied_ts: i64 = 1_000_000;
    cache
        .write_with_fetched_at("acme", &names, supplied_ts)
        .unwrap();
    let result = cache.read("acme").unwrap().unwrap();
    assert_eq!(
        result.fetched_at, supplied_ts,
        "write_with_fetched_at must persist the exact fetched_at supplied, not now()"
    );
    assert_eq!(
        result.names.get(&42).map(|s| s.as_str()),
        Some("Seeded Project"),
        "names must be written alongside the custom timestamp"
    );
}
