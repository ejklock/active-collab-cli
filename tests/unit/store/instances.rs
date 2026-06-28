use super::*;
use crate::config::Config;
use crate::store::Store;
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

fn sample_instance(name: &str) -> Instance {
    Instance {
        name: name.to_string(),
        base_url: format!("https://{name}.example.com"),
        email: format!("{name}@example.com"),
        token: format!("tok-{name}"),
        user_id: Some(42),
    }
}

#[test]
fn save_and_load_all_round_trip() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let inst = sample_instance("alpha");
    repo.save(&inst).unwrap();
    let all = repo.load_all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "alpha");
    assert_eq!(all[0].base_url, "https://alpha.example.com");
    assert_eq!(all[0].email, "alpha@example.com");
    assert_eq!(all[0].token, "tok-alpha");
    assert_eq!(all[0].user_id, Some(42));
}

#[test]
fn load_all_returns_instances_ordered_by_created_at_then_name() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    // Insert with a tiny sleep to get distinct timestamps; or force via SQL
    // Use direct insert to control created_at for ordering test
    store
        .conn()
        .execute(
            "INSERT INTO instances (name, base_url, email, token, user_id, created_at) \
             VALUES ('beta', 'https://b', 'b@b', 'tok-b', NULL, '2026-01-02T00:00:00Z')",
            [],
        )
        .unwrap();
    store
        .conn()
        .execute(
            "INSERT INTO instances (name, base_url, email, token, user_id, created_at) \
             VALUES ('alpha', 'https://a', 'a@a', 'tok-a', NULL, '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    let all = repo.load_all().unwrap();
    assert_eq!(all[0].name, "alpha");
    assert_eq!(all[1].name, "beta");
}

#[test]
fn delete_returns_rows_affected() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    repo.save(&sample_instance("to-delete")).unwrap();
    let affected = repo.delete("to-delete").unwrap();
    assert_eq!(affected, 1);
    let none = repo.delete("nonexistent").unwrap();
    assert_eq!(none, 0);
}

#[test]
fn delete_removes_the_row() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    repo.save(&sample_instance("gone")).unwrap();
    repo.delete("gone").unwrap();
    let all = repo.load_all().unwrap();
    assert!(all.is_empty());
}

#[test]
fn list_for_display_returns_correct_tuple_shape() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    repo.save(&sample_instance("disp")).unwrap();
    let rows = repo.list_for_display().unwrap();
    assert_eq!(rows.len(), 1);
    let (name, base_url, email, user_id) = &rows[0];
    assert_eq!(name, "disp");
    assert_eq!(base_url, "https://disp.example.com");
    assert_eq!(email, "disp@example.com");
    assert_eq!(*user_id, Some(42));
}

#[test]
fn list_connectivity_returns_correct_tuple_shape() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    repo.save(&sample_instance("conn")).unwrap();
    let rows = repo.list_connectivity().unwrap();
    assert_eq!(rows.len(), 1);
    let (name, base_url, token) = &rows[0];
    assert_eq!(name, "conn");
    assert_eq!(base_url, "https://conn.example.com");
    assert_eq!(token, "tok-conn");
}

#[test]
fn find_by_name_filters_correctly() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    repo.save(&sample_instance("one")).unwrap();
    repo.save(&sample_instance("two")).unwrap();
    let found = repo.find_by_name("one").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "one");
    let not_found = repo.find_by_name("three").unwrap();
    assert!(not_found.is_empty());
}

#[test]
fn save_null_user_id_round_trips() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let inst = Instance {
        name: "noid".to_string(),
        base_url: "https://noid.example.com".to_string(),
        email: "noid@example.com".to_string(),
        token: "tok-noid".to_string(),
        user_id: None,
    };
    repo.save(&inst).unwrap();
    let all = repo.load_all().unwrap();
    assert_eq!(all[0].user_id, None);
}
