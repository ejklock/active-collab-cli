use super::*;
use crate::config::Config;
use crate::store::cache::{instances_key, ProjectNamesCache, TaskListCache, UserMapCache};
use crate::store::instances::Instance;
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

// ADR 0052: read_fresh returns Some for a recently-written entry when
// max_age_secs is large enough to cover its age.
#[test]
fn project_names_cache_read_fresh_returns_some_for_recent_entry() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let recent_ts = crate::store::now_epoch_secs() - 60;
    let names: HashMap<i64, String> = [(1i64, "Fresh Project".to_string())].into_iter().collect();
    cache
        .write_with_fetched_at("acme", &names, recent_ts)
        .unwrap();
    let result = cache.read_fresh("acme", 3600).unwrap();
    assert_eq!(
        result.unwrap().names.get(&1).map(|s| s.as_str()),
        Some("Fresh Project"),
        "entry 60s old with max_age=3600 must be a hit"
    );
}

// ADR 0052: read_fresh returns None for a stale entry when max_age_secs is
// smaller than the entry's age.
#[test]
fn project_names_cache_read_fresh_returns_none_for_stale_entry() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let old_ts = crate::store::now_epoch_secs() - 7200;
    let names: HashMap<i64, String> = [(1i64, "Stale Project".to_string())].into_iter().collect();
    cache.write_with_fetched_at("acme", &names, old_ts).unwrap();
    let result = cache.read_fresh("acme", 3600).unwrap();
    assert!(
        result.is_none(),
        "entry older than max_age_secs must be treated as a miss"
    );
}

// ADR 0052: read_fresh returns None when no entry has been written.
#[test]
fn project_names_cache_read_fresh_returns_none_when_absent() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let result = cache.read_fresh("nonexistent", 3600).unwrap();
    assert!(result.is_none());
}

// ADR 0052: entry exactly at the max_age boundary reads Some (age ==
// max_age_secs), mirroring the TaskListCache boundary test below — the
// read_fresh comparison must be inclusive, not strict.
#[test]
fn project_names_cache_read_fresh_at_exact_max_age_returns_some() {
    let (_dir, store) = make_store();
    let cache = ProjectNamesCache::new(store.conn());
    let max_age_secs = 3600;
    let boundary_ts = crate::store::now_epoch_secs() - max_age_secs;
    let names: HashMap<i64, String> = [(1i64, "Boundary Project".to_string())]
        .into_iter()
        .collect();
    cache
        .write_with_fetched_at("acme", &names, boundary_ts)
        .unwrap();
    let result = cache.read_fresh("acme", max_age_secs).unwrap();
    assert_eq!(
        result.unwrap().names.get(&1).map(|s| s.as_str()),
        Some("Boundary Project"),
        "entry with age == max_age_secs must be a hit (inclusive boundary)"
    );
}

fn make_instance(name: &str) -> Instance {
    Instance {
        name: name.to_string(),
        base_url: "https://example.com".to_string(),
        email: "test@example.com".to_string(),
        token: "tok".to_string(),
        user_id: None,
    }
}

// S8a-A1: write then read within max-age returns the stored list_json
#[test]
fn task_list_cache_write_then_read_within_max_age_returns_json() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    cache.write("browse", "acme|beta", r#"["task1"]"#).unwrap();
    let result = cache.read("browse", "acme|beta", 3600).unwrap();
    assert_eq!(result.as_deref(), Some(r#"["task1"]"#));
}

// S8a-A1: write overwrites and round-trips latest value
#[test]
fn task_list_cache_write_overwrites_previous_entry() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    cache.write("browse", "acme", r#"["old"]"#).unwrap();
    cache.write("browse", "acme", r#"["new"]"#).unwrap();
    let result = cache.read("browse", "acme", 3600).unwrap();
    assert_eq!(result.as_deref(), Some(r#"["new"]"#));
}

// S8a-A1: read when no row written returns None
#[test]
fn task_list_cache_read_when_absent_returns_none() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    let result = cache.read("browse", "acme", 3600).unwrap();
    assert!(result.is_none());
}

// S8a-A2: different instances_key does not cross-read
#[test]
fn task_list_cache_different_instances_key_returns_none() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    cache.write("browse", "acme", r#"["task1"]"#).unwrap();
    let result = cache.read("browse", "other", 3600).unwrap();
    assert!(
        result.is_none(),
        "different instances_key must not cross-read"
    );
}

// S8a-A2: different scope (browse vs mine) does not cross-read
#[test]
fn task_list_cache_different_scope_returns_none() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    cache.write("browse", "acme", r#"["browse-task"]"#).unwrap();
    let result = cache.read("mine", "acme", 3600).unwrap();
    assert!(result.is_none(), "mine scope must not read browse snapshot");
}

// S8a-A2: instances_key helper is order-independent
#[test]
fn instances_key_is_order_independent() {
    let a = make_instance("acme");
    let b = make_instance("beta");
    let key_ab = instances_key(&[a.clone(), b.clone()]);
    let key_ba = instances_key(&[b, a]);
    assert_eq!(key_ab, key_ba, "instances_key must be order-independent");
}

// S8a-A2: instances_key with single instance equals its name
#[test]
fn instances_key_single_instance_equals_name() {
    let inst = make_instance("acme");
    assert_eq!(instances_key(&[inst]), "acme");
}

// S8a-A3: row exactly at max_age boundary reads Some (age == max_age_secs)
#[test]
fn task_list_cache_row_exactly_at_max_age_reads_some() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    let now = crate::store::now_epoch_secs();
    cache
        .write_with_fetched_at("browse", "acme", r#"["data"]"#, now)
        .unwrap();
    // age is 0, which is <= max_age_secs=0, so it should be a hit
    let result = cache.read("browse", "acme", 0).unwrap();
    assert!(
        result.is_some(),
        "row with age=0 and max_age=0 must be a hit"
    );
}

// S8a-A3: row older than max_age_secs reads None
#[test]
fn task_list_cache_row_older_than_max_age_returns_none() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    let old_ts = crate::store::now_epoch_secs() - 7200;
    cache
        .write_with_fetched_at("browse", "acme", r#"["stale"]"#, old_ts)
        .unwrap();
    let result = cache.read("browse", "acme", 3600).unwrap();
    assert!(
        result.is_none(),
        "row older than max_age_secs must be treated as a miss"
    );
}

// S8a-A3: row within max_age but not too old reads Some
#[test]
fn task_list_cache_row_within_max_age_reads_some() {
    let (_dir, store) = make_store();
    let cache = TaskListCache::new(store.conn());
    let recent_ts = crate::store::now_epoch_secs() - 60;
    cache
        .write_with_fetched_at("browse", "acme", r#"["fresh"]"#, recent_ts)
        .unwrap();
    let result = cache.read("browse", "acme", 3600).unwrap();
    assert_eq!(
        result.as_deref(),
        Some(r#"["fresh"]"#),
        "row 60s old with max_age=3600 must be a hit"
    );
}
