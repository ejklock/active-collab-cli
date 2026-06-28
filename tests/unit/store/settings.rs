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

#[test]
fn set_then_get_returns_stored_value() {
    let (_dir, store) = make_store();
    let repo = SettingsRepository::new(store.conn());
    repo.set("lang", "pt-BR").unwrap();
    let val = repo.get("lang", None).unwrap();
    assert_eq!(val, Some("pt-BR".to_string()));
}

#[test]
fn get_missing_key_with_default_returns_default() {
    let (_dir, store) = make_store();
    let repo = SettingsRepository::new(store.conn());
    let val = repo.get("missing", Some("fallback")).unwrap();
    assert_eq!(val, Some("fallback".to_string()));
}

#[test]
fn get_missing_key_without_default_returns_none() {
    let (_dir, store) = make_store();
    let repo = SettingsRepository::new(store.conn());
    let val = repo.get("absent", None).unwrap();
    assert_eq!(val, None);
}

#[test]
fn set_overwrites_existing_value() {
    let (_dir, store) = make_store();
    let repo = SettingsRepository::new(store.conn());
    repo.set("key", "old").unwrap();
    repo.set("key", "new").unwrap();
    let val = repo.get("key", None).unwrap();
    assert_eq!(val, Some("new".to_string()));
}
