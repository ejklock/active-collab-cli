use super::*;
use crate::config::Config;
use crate::i18n::set_language;
use crate::render;
use crate::store::Store;
use std::sync::Mutex;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mine_outcome_code(outcome: MineOutcome) -> i32 {
    match outcome {
        MineOutcome::Done(code) => code,
        MineOutcome::TuiLaunch { .. } => panic!("expected Done, got TuiLaunch"),
    }
}

static LANG_MUTEX: Mutex<()> = Mutex::new(());

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

fn make_http() -> Http {
    Http::new().unwrap()
}

fn sample_instance(store: &Store, name: &str) -> Instance {
    let inst = Instance {
        name: name.to_owned(),
        base_url: format!("https://{name}.example.com"),
        email: format!("{name}@example.com"),
        token: format!("tok-{name}"),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();
    inst
}

fn output_str(buf: &[u8]) -> &str {
    std::str::from_utf8(buf).unwrap()
}

#[test]
fn setup_list_empty_prints_no_instances_message() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let code = setup_list(&repo, &mut out);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("No instances configured"),
        "got: {}",
        output_str(&out)
    );
    assert!(output_str(&out).contains("active_collab.py setup add"));
}

#[test]
fn setup_list_nonempty_prints_header_separator_and_rows() {
    let (_dir, store) = make_store();
    sample_instance(&store, "work");
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let code = setup_list(&repo, &mut out);
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("NAME"), "header missing NAME: {s}");
    assert!(s.contains("URL"), "header missing URL: {s}");
    assert!(s.contains("EMAIL"), "header missing EMAIL: {s}");
    assert!(s.contains("USER_ID"), "header missing USER_ID: {s}");
    assert!(
        s.contains(&"-".repeat(100)),
        "100-char separator missing: {s}"
    );
    assert!(s.contains("work"), "instance row missing: {s}");
}

#[test]
fn setup_list_row_formatting_matches_widths() {
    let (_dir, store) = make_store();
    sample_instance(&store, "myinst");
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    setup_list(&repo, &mut out);
    let s = output_str(&out);
    let lines: Vec<&str> = s.lines().collect();
    // header line is the first non-empty line
    let header = lines[0];
    assert_eq!(&header[..4], "NAME");
    // NAME column is 20 wide; URL starts at position 21
    assert!(header.len() >= 21, "header too short: {header}");
}

#[test]
fn setup_remove_existing_instance_returns_0_and_prints_removed() {
    let (_dir, store) = make_store();
    sample_instance(&store, "gone");
    let repo = InstanceRepository::new(store.conn());
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_remove(&repo, &cache, "gone", &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("removed"),
        "got: {}",
        output_str(&out)
    );
    // verify it's actually gone
    let all = repo.load_all().unwrap();
    assert!(all.is_empty());
}

#[test]
fn setup_remove_nonexistent_returns_exit2_and_error_message() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let cache = TaskCache::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_remove(&repo, &cache, "nosuchname", &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("not found"),
        "err: {}",
        output_str(&err)
    );
    assert!(
        output_str(&err).contains("nosuchname"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn setup_remove_deletes_task_cache_for_instance() {
    let (_dir, store) = make_store();
    sample_instance(&store, "cached");
    // write a cache entry
    let cache = TaskCache::new(store.conn());
    cache
        .write(
            "cached",
            1,
            1,
            &serde_json::json!({"id": 1}),
            &serde_json::json!([]),
        )
        .unwrap();
    assert!(cache.read("cached", 1, 1).unwrap().is_some());

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    setup_remove(&repo, &cache, "cached", &mut out, &mut err);
    assert!(cache.read("cached", 1, 1).unwrap().is_none());
}

#[test]
fn setup_language_none_shows_default_en() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_language(&settings, None, &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("Current language: en"),
        "got: {}",
        output_str(&out)
    );
}

#[test]
fn setup_language_none_shows_previously_set_language() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    settings.set("language", "pt_BR").unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_language(&settings, None, &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("pt_BR"),
        "got: {}",
        output_str(&out)
    );
}

#[test]
fn setup_language_set_supported_persists_and_returns_0() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_language(&settings, Some("pt_BR"), &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("pt_BR"),
        "got: {}",
        output_str(&out)
    );
    // verify persisted
    let stored = settings.get("language", None).unwrap();
    assert_eq!(stored, Some("pt_BR".to_owned()));
}

#[test]
fn setup_language_unsupported_returns_exit2_with_error() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_language(&settings, Some("zz_ZZ"), &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("unsupported language"),
        "err: {}",
        output_str(&err)
    );
    assert!(
        output_str(&err).contains("zz_ZZ"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn setup_language_unsupported_shows_supported_list() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    setup_language(&settings, Some("de"), &mut out, &mut err);
    let e = output_str(&err);
    for lang in SUPPORTED {
        assert!(e.contains(lang), "missing {lang} in: {e}");
    }
}

#[test]
fn setup_theme_none_shows_default_angie() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_theme(&settings, None, &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("Current theme: angie"),
        "got: {}",
        output_str(&out)
    );
}

#[test]
fn setup_theme_none_shows_previously_set_theme() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    settings.set("theme", "nord").unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_theme(&settings, None, &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("nord"),
        "got: {}",
        output_str(&out)
    );
}

#[test]
fn setup_theme_set_valid_persists_and_returns_0() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_theme(&settings, Some("slate"), &mut out, &mut err);
    assert_eq!(code, 0);
    assert!(
        output_str(&out).contains("slate"),
        "got: {}",
        output_str(&out)
    );
    let stored = settings.get("theme", None).unwrap();
    assert_eq!(stored, Some("slate".to_owned()));
}

#[test]
fn setup_theme_set_uppercase_is_lowercased_and_persisted() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_theme(&settings, Some("NORD"), &mut out, &mut err);
    assert_eq!(code, 0);
    let stored = settings.get("theme", None).unwrap();
    assert_eq!(stored, Some("nord".to_owned()));
}

#[test]
fn setup_theme_unsupported_returns_exit2_with_error_and_does_not_persist() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_theme(&settings, Some("neon"), &mut out, &mut err);
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("unsupported theme"),
        "err: {}",
        output_str(&err)
    );
    assert!(
        output_str(&err).contains("neon"),
        "err: {}",
        output_str(&err)
    );
    let stored = settings.get("theme", None).unwrap();
    assert_eq!(stored, None, "unsupported theme must not be persisted");
}

#[test]
fn setup_theme_unsupported_shows_supported_list() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    setup_theme(&settings, Some("bogus"), &mut out, &mut err);
    let e = output_str(&err);
    for theme in ["angie", "slate", "nord"] {
        assert!(e.contains(theme), "missing {theme} in: {e}");
    }
}

#[test]
fn setup_theme_uses_theme_settings_key() {
    let (_dir, store) = make_store();
    let settings = SettingsRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    setup_theme(&settings, Some("slate"), &mut out, &mut err);
    assert_eq!(
        settings.get("theme", None).unwrap(),
        Some("slate".to_owned()),
        "setup_theme must persist under the 'theme' settings key"
    );
}

#[tokio::test]
async fn setup_test_named_missing_returns_exit2() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_test(&repo, Some("nosuch"), make_http(), &mut out, &mut err).await;
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("not found"),
        "err: {}",
        output_str(&err)
    );
}

#[tokio::test]
async fn setup_test_200_prints_ok_exit0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    // insert a real instance pointing to the mock server
    let inst = Instance {
        name: "testinst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_test(&repo, None, make_http(), &mut out, &mut err).await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("testinst"), "missing instance name: {s}");
    assert!(s.contains("OK"), "missing OK: {s}");
    assert!(s.contains("200"), "missing status: {s}");
}

#[tokio::test]
async fn setup_test_non_200_prints_failed_exit1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "badauth".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_test(&repo, None, make_http(), &mut out, &mut err).await;
    assert_eq!(code, 1);
    let s = output_str(&out);
    assert!(s.contains("FAILED"), "missing FAILED: {s}");
    assert!(s.contains("403"), "missing status: {s}");
}

#[tokio::test]
async fn setup_test_transport_error_prints_failed_exc_exit1() {
    // Point to a port that refuses connections (transport error).
    let (_dir, store) = make_store();
    let inst = Instance {
        name: "unreachable".to_owned(),
        base_url: "http://127.0.0.1:1".to_owned(),
        email: String::new(),
        token: "tok".to_owned(),
        user_id: None,
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_test(&repo, None, make_http(), &mut out, &mut err).await;
    assert_eq!(code, 1);
    let s = output_str(&out);
    assert!(s.contains("FAILED"), "missing FAILED: {s}");
}

#[test]
fn setup_add_missing_name_returns_exit2() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = rt.block_on(setup_add(
        SetupAddFields {
            name: None,
            url: Some("https://x.com".to_owned()),
            email: Some("a@b.com".to_owned()),
        },
        Some("pass".to_owned()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    ));
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("required"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn setup_add_missing_url_returns_exit2() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = rt.block_on(setup_add(
        SetupAddFields {
            name: Some("work".to_owned()),
            url: None,
            email: Some("a@b.com".to_owned()),
        },
        Some("pass".to_owned()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    ));
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("required"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn setup_add_missing_email_returns_exit2() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = rt.block_on(setup_add(
        SetupAddFields {
            name: Some("work".to_owned()),
            url: Some("https://x.com".to_owned()),
            email: None,
        },
        Some("pass".to_owned()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    ));
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("required"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn setup_add_empty_password_returns_exit2() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = rt.block_on(setup_add(
        SetupAddFields {
            name: Some("work".to_owned()),
            url: Some("https://x.com".to_owned()),
            email: Some("a@b.com".to_owned()),
        },
        Some(String::new()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    ));
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("password is required"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn setup_add_none_password_returns_exit2() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = rt.block_on(setup_add(
        SetupAddFields {
            name: Some("work".to_owned()),
            url: Some("https://x.com".to_owned()),
            email: Some("a@b.com".to_owned()),
        },
        None,
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    ));
    assert_eq!(code, 2);
    assert!(
        output_str(&err).contains("password is required"),
        "err: {}",
        output_str(&err)
    );
}

#[tokio::test]
async fn setup_add_happy_path_saves_instance_and_prints_saved() {
    let server = MockServer::start().await;

    // Token exchange endpoint
    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .and(body_json(serde_json::json!({
            "username": "user@example.com",
            "password": "secret123",
            "client_name": "active-collab-skill",
            "client_vendor": "klock"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": true,
            "token": "test-tok-abc"
        })))
        .mount(&server)
        .await;

    // Resolve user_id endpoint
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 99, "email": "user@example.com" }
        ])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_add(
        SetupAddFields {
            name: Some("mywork".to_owned()),
            url: Some(format!("{}/", server.uri())), // trailing slash — should be trimmed
            email: Some("user@example.com".to_owned()),
        },
        Some("secret123".to_owned()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "err: {}", output_str(&err));
    let s = output_str(&out);
    assert!(s.contains("saved"), "missing 'saved': {s}");
    assert!(s.contains("mywork"), "missing name: {s}");

    // verify persisted instance
    let all = repo.load_all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "mywork");
    assert_eq!(all[0].email, "user@example.com");
    assert_eq!(all[0].token, "test-tok-abc");
    assert_eq!(all[0].user_id, Some(99));
    // trailing slash trimmed
    assert!(
        !all[0].base_url.ends_with('/'),
        "trailing slash not trimmed: {}",
        all[0].base_url
    );
}

#[tokio::test]
async fn setup_add_with_check_connectivity_prints_connectivity_line() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": true,
            "token": "tok-xyz"
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    // Connectivity check hits /api/v1/projects
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_add(
        SetupAddFields {
            name: Some("conn-inst".to_owned()),
            url: Some(server.uri()),
            email: Some("u@u.com".to_owned()),
        },
        Some("pw".to_owned()),
        &repo,
        make_http(),
        true, // check_connectivity
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "err: {}", output_str(&err));
    let s = output_str(&out);
    assert!(
        s.contains("Connectivity:"),
        "missing connectivity line: {s}"
    );
}

#[tokio::test]
async fn setup_add_token_failure_returns_exit1_with_detail() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": false,
            "message": "Invalid credentials"
        })))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_add(
        SetupAddFields {
            name: Some("fail-inst".to_owned()),
            url: Some(server.uri()),
            email: Some("u@u.com".to_owned()),
        },
        Some("wrongpass".to_owned()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1);
    let e = output_str(&err);
    assert!(e.contains("Invalid credentials"), "err: {e}");
}

#[tokio::test]
async fn setup_add_token_failure_no_message_shows_generic_detail() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "is_ok": false })),
        )
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = setup_add(
        SetupAddFields {
            name: Some("fail2".to_owned()),
            url: Some(server.uri()),
            email: Some("u@u.com".to_owned()),
        },
        Some("pw".to_owned()),
        &repo,
        make_http(),
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 1);
    let e = output_str(&err);
    assert!(e.contains("token exchange failed"), "err: {e}");
}

#[test]
fn pick_instance_empty_list_returns_exit2() {
    let mut err = Vec::new();
    let result = pick_instance(&[], None, &mut err);
    assert_eq!(result, Err(2));
    assert!(
        output_str(&err).contains("no instances configured"),
        "err: {}",
        output_str(&err)
    );
}

#[test]
fn pick_instance_single_no_name_returns_it() {
    let instances = vec![Instance {
        name: "solo".to_owned(),
        base_url: "https://x.com".to_owned(),
        email: "a@b.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    }];
    let mut err = Vec::new();
    let result = pick_instance(&instances, None, &mut err);
    assert_eq!(result, Ok(0));
}

#[test]
fn pick_instance_multiple_no_name_returns_exit2() {
    let instances = vec![
        Instance {
            name: "a".to_owned(),
            base_url: "https://a.com".to_owned(),
            email: "a@a.com".to_owned(),
            token: "tok".to_owned(),
            user_id: None,
        },
        Instance {
            name: "b".to_owned(),
            base_url: "https://b.com".to_owned(),
            email: "b@b.com".to_owned(),
            token: "tok".to_owned(),
            user_id: None,
        },
    ];
    let mut err = Vec::new();
    let result = pick_instance(&instances, None, &mut err);
    assert_eq!(result, Err(2));
    let e = output_str(&err);
    assert!(e.contains("multiple instances"), "err: {e}");
    assert!(e.contains("--instance NAME"), "err: {e}");
}

#[test]
fn pick_instance_named_found_returns_index() {
    let instances = vec![
        Instance {
            name: "alpha".to_owned(),
            base_url: "https://a.com".to_owned(),
            email: "a@a.com".to_owned(),
            token: "tok".to_owned(),
            user_id: None,
        },
        Instance {
            name: "beta".to_owned(),
            base_url: "https://b.com".to_owned(),
            email: "b@b.com".to_owned(),
            token: "tok".to_owned(),
            user_id: None,
        },
    ];
    let mut err = Vec::new();
    let result = pick_instance(&instances, Some("beta"), &mut err);
    assert_eq!(result, Ok(1));
}

#[test]
fn pick_instance_named_not_found_returns_exit2_with_known() {
    let instances = vec![Instance {
        name: "alpha".to_owned(),
        base_url: "https://a.com".to_owned(),
        email: "a@a.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    }];
    let mut err = Vec::new();
    let result = pick_instance(&instances, Some("missing"), &mut err);
    assert_eq!(result, Err(2));
    let e = output_str(&err);
    assert!(e.contains("not found"), "err: {e}");
    assert!(e.contains("alpha"), "known list missing: {e}");
}

#[test]
fn parse_task_ref_url_form() {
    let mut err = Vec::new();
    let result = parse_task_ref("https://example.com/projects/665/tasks/75159", &mut err);
    assert_eq!(result, Ok((665, 75159)));
    assert!(output_str(&err).is_empty());
}

#[test]
fn parse_task_ref_pt_form() {
    let mut err = Vec::new();
    let result = parse_task_ref("665/75159", &mut err);
    assert_eq!(result, Ok((665, 75159)));
}

#[test]
fn parse_task_ref_bad_input_returns_exit2_with_message() {
    let mut err = Vec::new();
    let result = parse_task_ref("not-a-ref", &mut err);
    assert_eq!(result, Err(2));
    let e = output_str(&err);
    assert!(e.contains("cannot parse task ref"), "err: {e}");
    assert!(e.contains("not-a-ref"), "err: {e}");
    assert!(e.contains("665/75159"), "err: {e}");
}

#[test]
fn parse_task_ref_text_slash_text_returns_exit2() {
    let mut err = Vec::new();
    let result = parse_task_ref("abc/def", &mut err);
    assert_eq!(result, Err(2));
}

#[test]
fn parse_task_ref_url_embedded_in_longer_text() {
    let mut err = Vec::new();
    let result = parse_task_ref(
        "see https://app.example.com/projects/10/tasks/20 for details",
        &mut err,
    );
    assert_eq!(result, Ok((10, 20)));
}

#[test]
fn parse_branch_ref_feature_matches() {
    assert_eq!(parse_branch_ref("feature/665-75159"), Some((665, 75159)));
}

#[test]
fn parse_branch_ref_hotfix_matches() {
    assert_eq!(parse_branch_ref("hotfix/1-2"), Some((1, 2)));
}

#[test]
fn parse_branch_ref_fix_matches() {
    assert_eq!(parse_branch_ref("fix/99-1000"), Some((99, 1000)));
}

#[test]
fn parse_branch_ref_wrong_prefix_returns_none() {
    assert_eq!(parse_branch_ref("chore/665-75159"), None);
    assert_eq!(parse_branch_ref("main"), None);
}

#[test]
fn parse_branch_ref_non_digit_ids_returns_none() {
    assert_eq!(parse_branch_ref("feature/abc-def"), None);
}

#[tokio::test]
async fn load_task_cache_hit_returns_data_without_network() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());

    let task = serde_json::json!({ "id": 1, "name": "Cached task" });
    let comments = serde_json::json!([{ "body_plain_text": "A comment" }]);
    cache.write("inst", 10, 99, &task, &comments).unwrap();

    // Client points to a non-existent server — if network is hit, it will error
    let inst = Instance {
        name: "inst".to_owned(),
        base_url: "http://127.0.0.1:1".to_owned(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let flags_refresh = false;

    let result = load_task(
        &cache,
        &client,
        "inst",
        10,
        99,
        flags_refresh,
        false,
        &mut Vec::new(),
    )
    .await;
    assert!(result.is_some(), "expected cache hit");
    let (returned_task, returned_comments) = result.unwrap();
    assert_eq!(returned_task["name"], "Cached task");
    assert_eq!(returned_comments.len(), 1);
    // comments must NOT appear inside task anymore
    assert!(returned_task.get("comments").is_none());
}

#[tokio::test]
async fn load_task_cache_miss_fetches_and_writes_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 99, "name": "Fetched task" },
            "tracked_time": 3.5,
            "comments": [{ "body_plain_text": "hi" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(inst.clone(), make_http());

    let result = load_task(
        &cache,
        &client,
        "inst",
        10,
        99,
        false,
        false,
        &mut Vec::new(),
    )
    .await;
    server.verify().await;
    assert!(result.is_some());
    let (task, comments) = result.unwrap();
    assert_eq!(task["name"], "Fetched task");
    assert_eq!(task["tracked_time"], 3.5);
    assert_eq!(comments.len(), 1);

    // verify written to cache
    let cached = cache.read("inst", 10, 99).unwrap();
    assert!(cached.is_some(), "should be in cache after fetch");
}

#[tokio::test]
async fn load_task_non_200_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(inst, make_http());

    let result = load_task(
        &cache,
        &client,
        "inst",
        10,
        99,
        false,
        false,
        &mut Vec::new(),
    )
    .await;
    assert!(result.is_none());
}

#[tokio::test]
async fn load_task_refresh_bypasses_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 99, "name": "Fresh task" },
            "tracked_time": null,
            "comments": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    // Put stale data in cache
    cache
        .write(
            "inst",
            10,
            99,
            &serde_json::json!({ "id": 99, "name": "Stale" }),
            &serde_json::json!([]),
        )
        .unwrap();

    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(inst, make_http());

    let result = load_task(
        &cache,
        &client,
        "inst",
        10,
        99,
        true,
        false,
        &mut Vec::new(),
    )
    .await;
    server.verify().await;
    let (task, _) = result.unwrap();
    assert_eq!(task["name"], "Fresh task");
}

#[tokio::test]
async fn load_task_no_comments_flag_returns_empty_comments() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 99, "name": "Task" },
            "tracked_time": null,
            "comments": [{ "body_plain_text": "should be skipped" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(inst, make_http());

    let result = load_task(
        &cache,
        &client,
        "inst",
        10,
        99,
        false,
        true,
        &mut Vec::new(),
    )
    .await;
    let (_, comments) = result.unwrap();
    assert!(comments.is_empty(), "no_comments should suppress comments");
}

fn make_inst(base_url: &str) -> Instance {
    Instance {
        name: "testinst".to_owned(),
        base_url: base_url.to_owned(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(1),
    }
}

fn default_flags() -> DisplayFlags {
    DisplayFlags {
        json: false,
        short: false,
        refresh: false,
        no_comments: false,
        download_attachments: false,
        attachments_dir: None,
    }
}

#[tokio::test]
async fn do_get_task_json_mode_prints_minified_curated_contract_and_returns_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 10,
                "project_id": 5,
                "project_name": "My Project",
                "name": "Test task",
                "is_completed": false,
                "assignee_id": serde_json::Value::Null,
                "estimate": 0,
                "tracked_time": 0,
                "body": ""
            },
            "comments": [],
            "tracked_time": 0
        })))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let flags = DisplayFlags {
        json: true,
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0, "exit code must be 0");
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "output must be a single minified line (no embedded newlines): {s:?}"
    );
    assert!(
        !trimmed.contains("  "),
        "output must not have 2-space indent (must be minified): {s:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(trimmed).expect("output must be valid JSON");
    assert_eq!(obj["ref"], "5/10", "ref must be project_id/task_id");
    assert_eq!(obj["status"], "open", "status must be literal 'open'");
    assert_eq!(obj["name"], "Test task", "name must match");
    assert!(
        obj.get("assignee").is_some(),
        "assignee key must be present"
    );
    assert!(
        obj.get("project_id").is_some(),
        "project_id key must be present"
    );
}

#[tokio::test]
async fn do_get_task_json_mode_http_error_returns_1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let flags = DisplayFlags {
        json: true,
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 1, "HTTP 404 on task fetch must return exit code 1");
}

#[tokio::test]
async fn do_get_task_short_mode_prints_pid_tid_tab_name_and_returns_0() {
    let server = MockServer::start().await;
    // For short mode: load_task is called (not json path)
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 10, "name": "My Task" },
            "tracked_time": null,
            "comments": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let flags = DisplayFlags {
        short: true,
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("5/10\t"), "got: {s}");
    assert!(s.contains("My Task"), "got: {s}");
}

#[tokio::test]
async fn do_get_task_normal_mode_renders_task_and_returns_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 10, "task_number": 3, "name": "Full Task", "is_completed": false },
            "tracked_time": null,
            "comments": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Users endpoint for user_map
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = do_get_task(
        &inst,
        &cache,
        &client,
        5,
        10,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("Task:"), "got: {s}");
    assert!(s.contains("Full Task"), "got: {s}");
    assert!(s.contains("Open"), "got: {s}");
}

#[tokio::test]
async fn do_get_task_load_failure_returns_1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = do_get_task(
        &inst,
        &cache,
        &client,
        5,
        10,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 1);
}

fn task_with_attachment(tid: i64, name: &str, attachment_url: &str) -> serde_json::Value {
    serde_json::json!({
        "id": tid,
        "name": name,
        "is_completed": false,
        "assignee_id": serde_json::Value::Null,
        "estimate": 0,
        "tracked_time": 0,
        "body": "",
        "attachments": [{"name": "note.txt", "url": attachment_url}],
    })
}

#[tokio::test]
async fn do_get_task_json_mode_includes_downloaded_attachments_when_flag_set() {
    let server = MockServer::start().await;
    let attachment_url = format!("{}/note.txt", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": task_with_attachment(10, "Attachment task", &attachment_url),
            "tracked_time": 0,
            "comments": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/note.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let dest = TempDir::new().unwrap();
    let flags = DisplayFlags {
        json: true,
        download_attachments: true,
        attachments_dir: Some(dest.path().to_string_lossy().to_string()),
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let obj: serde_json::Value = serde_json::from_str(output_str(&out).trim_end()).unwrap();
    let downloaded = obj["downloaded_attachments"]
        .as_array()
        .expect("downloaded_attachments must be an array when the flag is set");
    assert_eq!(downloaded.len(), 1);
    assert_eq!(downloaded[0]["name"], "note.txt");
    assert_eq!(downloaded[0]["url"], attachment_url);
    assert!(
        downloaded[0]["path"].as_str().is_some(),
        "successful download must carry a path: {downloaded:?}"
    );
    assert!(downloaded[0]["error"].is_null());
}

#[tokio::test]
async fn do_get_task_json_mode_omits_downloaded_attachments_when_flag_absent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 10, "name": "No download", "is_completed": false, "estimate": 0, "tracked_time": 0, "body": "" },
            "tracked_time": 0,
            "comments": []
        })))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let flags = DisplayFlags {
        json: true,
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let obj: serde_json::Value = serde_json::from_str(output_str(&out).trim_end()).unwrap();
    assert!(
        obj.get("downloaded_attachments").is_none(),
        "downloaded_attachments must be absent when --download-attachments was not passed: {obj}"
    );
}

#[tokio::test]
async fn do_get_task_normal_mode_prints_download_summary_line() {
    let server = MockServer::start().await;
    let attachment_url = format!("{}/note.txt", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": task_with_attachment(10, "Attachment task", &attachment_url),
            "tracked_time": 0,
            "comments": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/note.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let dest = TempDir::new().unwrap();
    let dest_path = dest.path().to_string_lossy().to_string();
    let flags = DisplayFlags {
        download_attachments: true,
        attachments_dir: Some(dest_path.clone()),
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(
        s.contains("Downloaded 1 of 1 attachment(s)"),
        "missing summary counts: {s}"
    );
    assert!(s.contains(&dest_path), "missing destination dir: {s}");
}

#[tokio::test]
async fn do_get_task_normal_mode_download_summary_reports_failures() {
    let server = MockServer::start().await;
    let attachment_url = format!("{}/missing.txt", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": task_with_attachment(10, "Attachment task", &attachment_url),
            "tracked_time": 0,
            "comments": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/missing.txt"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let dest = TempDir::new().unwrap();
    let flags = DisplayFlags {
        download_attachments: true,
        attachments_dir: Some(dest.path().to_string_lossy().to_string()),
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(
        s.contains("Downloaded 0 of 1 attachment(s)"),
        "missing failure counts: {s}"
    );
    assert!(s.contains("failed:"), "missing failure reason: {s}");
    assert!(s.contains("note.txt"), "missing failed asset name: {s}");
}

#[tokio::test]
async fn do_get_task_short_mode_runs_download_but_prints_only_the_short_line() {
    let server = MockServer::start().await;
    let attachment_url = format!("{}/note.txt", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": task_with_attachment(10, "Short mode task", &attachment_url),
            "tracked_time": 0,
            "comments": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/note.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let dest = TempDir::new().unwrap();
    let flags = DisplayFlags {
        short: true,
        download_attachments: true,
        attachments_dir: Some(dest.path().to_string_lossy().to_string()),
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert_eq!(
        s, "5/10\tShort mode task\n",
        "short output must stay byte-for-byte unchanged"
    );
    assert!(
        dest.path().join("note.txt").exists(),
        "download must still run as a side effect in short mode"
    );
}

#[tokio::test]
async fn do_get_task_download_attachments_defaults_to_controller_default_dir() {
    let server = MockServer::start().await;
    let pid = 900_555_i64;
    let tid = 900_666_i64;
    let attachment_url = format!("{}/note.txt", server.uri());
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/projects/{pid}/tasks/{tid}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": task_with_attachment(tid, "Default dir task", &attachment_url),
            "tracked_time": 0,
            "comments": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/note.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let flags = DisplayFlags {
        json: true,
        download_attachments: true,
        ..default_flags()
    };

    let expected_dir = crate::controller::default_attachments_dir(pid, tid);
    std::fs::remove_dir_all(&expected_dir).ok();

    let code = do_get_task(&inst, &cache, &client, pid, tid, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    assert!(
        expected_dir.join("note.txt").exists(),
        "file must be written under controller::default_attachments_dir when --attachments-dir is omitted"
    );

    std::fs::remove_dir_all(&expected_dir).ok();
}

#[tokio::test]
async fn current_core_none_branch_returns_exit2_with_message() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        None,
        &inst,
        &cache,
        &client,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 2);
    let e = output_str(&err);
    assert!(e.contains("not in a git repository"), "err: {e}");
}

#[tokio::test]
async fn current_core_non_matching_branch_returns_exit2() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        Some("main"),
        &inst,
        &cache,
        &client,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 2);
    let e = output_str(&err);
    assert!(e.contains("does not match expected pattern"), "err: {e}");
    assert!(e.contains("main"), "err: {e}");
}

#[tokio::test]
async fn current_core_matching_branch_renders_task() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/665/tasks/75159"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 75159, "task_number": 100, "name": "Branch Task", "is_completed": false },
            "tracked_time": null,
            "comments": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = current_core(
        Some("feature/665-75159"),
        &inst,
        &cache,
        &client,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("Branch Task"), "got: {s}");
    assert!(s.contains("Open"), "got: {s}");
}

#[tokio::test]
async fn get_core_bad_ref_returns_exit2() {
    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "not-a-ref",
        &inst,
        &cache,
        &client,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 2);
    let e = output_str(&err);
    assert!(e.contains("cannot parse task ref"), "err: {e}");
}

#[tokio::test]
async fn get_core_valid_ref_renders_task() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 10, "task_number": 5, "name": "Get Task" },
            "tracked_time": null,
            "comments": []
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = get_core(
        "5/10",
        &inst,
        &cache,
        &client,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("Get Task"), "got: {s}");
}

fn tasks_response(tasks: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "tasks": tasks })
}

#[tokio::test]
async fn collect_mine_rows_aggregates_across_two_instances() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 10, "task_number": 5, "name": "Alpha task", "is_completed": false, "is_trashed": false, "project_id": 1 }
        ]))))
        .mount(&server_a)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 20, "task_number": 8, "name": "Beta task", "is_completed": false, "is_trashed": false, "project_id": 2 }
        ]))))
        .mount(&server_b)
        .await;

    let inst_a = Instance {
        name: "alpha".to_owned(),
        base_url: server_a.uri(),
        email: "a@a.com".to_owned(),
        token: "tok-a".to_owned(),
        user_id: Some(42),
    };
    let inst_b = Instance {
        name: "beta".to_owned(),
        base_url: server_b.uri(),
        email: "b@b.com".to_owned(),
        token: "tok-b".to_owned(),
        user_id: Some(42),
    };

    let http = make_http();
    let rows = collect_mine_rows(&[inst_a, inst_b], &http).await;

    assert_eq!(rows.len(), 2, "should aggregate both instances");
    assert_eq!(rows[0].instance, "alpha");
    assert_eq!(rows[0].task_number, 5);
    assert_eq!(rows[0].task_id, 10);
    assert_eq!(rows[0].name, "Alpha task");
    assert_eq!(rows[1].instance, "beta");
    assert_eq!(rows[1].task_number, 8);
    assert_eq!(rows[1].task_id, 20);
    assert_eq!(rows[1].name, "Beta task");
}

// S5.3 tests: concurrent collect_mine_rows

#[tokio::test]
async fn collect_mine_rows_concurrent_aggregates_all_instances() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 1, "task_number": 10, "name": "Concurrent A", "is_completed": false, "is_trashed": false, "project_id": 1 }
        ]))))
        .mount(&server_a)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 2, "task_number": 20, "name": "Concurrent B", "is_completed": false, "is_trashed": false, "project_id": 2 }
        ]))))
        .mount(&server_b)
        .await;

    let inst_a = Instance {
        name: "conc-a".to_owned(),
        base_url: server_a.uri(),
        email: "a@a.com".to_owned(),
        token: "tok-a".to_owned(),
        user_id: Some(1),
    };
    let inst_b = Instance {
        name: "conc-b".to_owned(),
        base_url: server_b.uri(),
        email: "b@b.com".to_owned(),
        token: "tok-b".to_owned(),
        user_id: Some(2),
    };

    let rows = collect_mine_rows(&[inst_a, inst_b], &make_http()).await;

    assert_eq!(rows.len(), 2, "must aggregate from both instances");
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"Concurrent A"),
        "missing Concurrent A: {names:?}"
    );
    assert!(
        names.contains(&"Concurrent B"),
        "missing Concurrent B: {names:?}"
    );
}

#[tokio::test]
async fn collect_mine_rows_preserves_instance_order() {
    let server_first = MockServer::start().await;
    let server_second = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 100, "task_number": 1, "name": "First Instance Task", "is_completed": false, "is_trashed": false, "project_id": 1 }
        ]))))
        .mount(&server_first)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 200, "task_number": 2, "name": "Second Instance Task", "is_completed": false, "is_trashed": false, "project_id": 2 }
        ]))))
        .mount(&server_second)
        .await;

    let inst_first = Instance {
        name: "first".to_owned(),
        base_url: server_first.uri(),
        email: "f@f.com".to_owned(),
        token: "tok-f".to_owned(),
        user_id: Some(1),
    };
    let inst_second = Instance {
        name: "second".to_owned(),
        base_url: server_second.uri(),
        email: "s@s.com".to_owned(),
        token: "tok-s".to_owned(),
        user_id: Some(2),
    };

    let rows = collect_mine_rows(&[inst_first, inst_second], &make_http()).await;

    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].instance, "first",
        "instance[0] rows must come first"
    );
    assert_eq!(rows[0].name, "First Instance Task");
    assert_eq!(rows[1].instance, "second", "instance[1] rows must follow");
    assert_eq!(rows[1].name, "Second Instance Task");
}

#[tokio::test]
async fn collect_mine_rows_failing_instance_yields_no_rows_for_it() {
    let server_ok = MockServer::start().await;
    let server_err = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 77, "task_number": 3, "name": "Healthy Task", "is_completed": false, "is_trashed": false, "project_id": 5 }
        ]))))
        .mount(&server_ok)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&server_err)
        .await;

    let inst_ok = Instance {
        name: "healthy".to_owned(),
        base_url: server_ok.uri(),
        email: "h@h.com".to_owned(),
        token: "tok-h".to_owned(),
        user_id: Some(1),
    };
    let inst_err = Instance {
        name: "broken".to_owned(),
        base_url: server_err.uri(),
        email: "b@b.com".to_owned(),
        token: "tok-b".to_owned(),
        user_id: Some(2),
    };

    let rows = collect_mine_rows(&[inst_ok, inst_err], &make_http()).await;

    assert_eq!(rows.len(), 1, "failing instance must yield no rows");
    assert_eq!(rows[0].name, "Healthy Task");
    assert_eq!(rows[0].instance, "healthy");
}

#[tokio::test]
async fn collect_mine_rows_falls_back_task_number_to_id_when_absent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 55, "task_number": null, "name": "No number task", "is_completed": false, "is_trashed": false, "project_id": 3 },
            { "id": 66, "task_number": 0,    "name": "Zero number",    "is_completed": false, "is_trashed": false, "project_id": 3 }
        ]))))
        .mount(&server)
        .await;

    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    let rows = collect_mine_rows(&[inst], &make_http()).await;
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].task_number, 55,
        "null task_number should fall back to id"
    );
    assert_eq!(
        rows[1].task_number, 66,
        "zero task_number should fall back to id"
    );
}

#[tokio::test]
async fn mine_core_empty_instances_returns_exit2_with_message() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, false, false, &mut out, &mut err).await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 2);
    let e = output_str(&err);
    assert!(e.contains("no instances configured"), "err: {e}");
    assert!(e.contains("active_collab.py setup add"), "err: {e}");
}

#[tokio::test]
async fn mine_core_instance_filter_not_found_returns_exit2_with_known() {
    let (_dir, store) = make_store();
    sample_instance(&store, "work");
    sample_instance(&store, "personal");
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(
        &repo,
        &make_http(),
        Some("nosuch"),
        false,
        false,
        &mut out,
        &mut err,
    )
    .await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 2);
    let e = output_str(&err);
    assert!(e.contains("not found"), "err: {e}");
    assert!(e.contains("nosuch"), "err: {e}");
    assert!(e.contains("work"), "known list missing 'work': {e}");
    assert!(e.contains("personal"), "known list missing 'personal': {e}");
}

#[tokio::test]
async fn mine_core_instance_filter_limits_to_matching_instance() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 1, "task_number": 1, "name": "Only from work", "is_completed": false, "is_trashed": false, "project_id": 1 }
        ]))))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "work".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();
    sample_instance(&store, "personal");

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(
        &repo,
        &make_http(),
        Some("work"),
        false,
        false,
        &mut out,
        &mut err,
    )
    .await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("Only from work"), "missing task: {s}");
    assert!(
        !s.contains("personal"),
        "personal instance should be excluded: {s}"
    );
}

#[tokio::test]
async fn mine_core_non_tty_with_rows_writes_table_and_returns_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 99, "task_number": 3, "name": "My task", "is_completed": false, "is_trashed": false, "project_id": 7 }
        ]))))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "myinst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, false, false, &mut out, &mut err).await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(s.contains("INSTANCE"), "table header missing: {s}");
    assert!(s.contains("My task"), "task name missing: {s}");
    assert!(s.contains("myinst"), "instance missing: {s}");
    assert!(s.contains(&"-".repeat(80)), "separator missing: {s}");
}

#[tokio::test]
async fn mine_core_non_tty_no_rows_writes_no_tasks_message_and_returns_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([]))),
        )
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "empty".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, false, false, &mut out, &mut err).await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(
        s.contains("No open tasks assigned to you"),
        "missing message: {s}"
    );
}

// S8c-A4: TTY interactive path returns TuiLaunch (no row pre-fetch, no table output).
// The network is NOT hit — rows are fetched lazily inside the TUI via Cmd::LoadMineTasks.
#[tokio::test]
async fn mine_core_tty_returns_tui_launch_without_fetching_rows() {
    let (_dir, store) = make_store();
    let inst = Instance {
        name: "tui-inst".to_owned(),
        base_url: "http://127.0.0.1:1".to_owned(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, false, true, &mut out, &mut err).await;

    match outcome {
        MineOutcome::TuiLaunch { targets } => {
            assert_eq!(targets.len(), 1, "must carry the resolved instance");
            assert_eq!(targets[0].name, "tui-inst");
        }
        MineOutcome::Done(code) => {
            panic!("expected TuiLaunch for TTY path, got Done({code})");
        }
    }
    let s = output_str(&out);
    assert!(
        !s.contains("INSTANCE"),
        "table must NOT be printed on TTY path: {s}"
    );
}

// S3-A3: Non-TTY mine path writes render_mine_table byte-for-byte; returns Done (no TuiLaunch).
#[tokio::test]
async fn mine_core_non_tty_writes_render_mine_table_exactly_and_returns_done() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 77, "task_number": 4, "name": "Table row task", "is_completed": false, "is_trashed": false, "project_id": 9 }
        ]))))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "s3inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, false, false, &mut out, &mut err).await;

    match &outcome {
        MineOutcome::TuiLaunch { .. } => panic!("non-TTY path must NOT return TuiLaunch"),
        MineOutcome::Done(_) => {}
    }
    let code = mine_outcome_code(outcome);
    assert_eq!(code, 0);

    let expected = render::render_mine_table(&[render::MineTableRow {
        instance: "s3inst".to_owned(),
        project_id: 9,
        task_number: 4,
        task_id: 77,
        name: "Table row task".to_owned(),
        due_on: None,
        project_name: None,
    }]);
    let s = output_str(&out);
    assert!(
        s.trim() == expected.trim(),
        "non-TTY output must match render_mine_table exactly.\nexpected:\n{expected}\ngot:\n{s}"
    );
}

#[test]
fn setup_list_empty_emits_pt_br_message_when_language_is_pt_br() {
    let (_dir, store) = make_store();
    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();

    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let code = setup_list(&repo, &mut out);
    set_language("en");
    drop(_guard);

    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(
        s.contains("Nenhuma instância configurada"),
        "expected pt-BR message, got: {s}"
    );
    assert!(
        s.contains("Execute: active_collab.py setup add"),
        "expected pt-BR command hint, got: {s}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn browse_async_inside_active_runtime_returns_i32_without_panic() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let http = make_http();
    let code = crate::tui::browse(vec![], http, db_path).await;
    assert_eq!(
        code, 1,
        "no-TTY path must return exit code 1 (terminal setup fails)"
    );
}

// --- J2-A2: mine --json prints minified mine line, never launches TUI, exit 0 ---

#[tokio::test]
async fn mine_core_json_mode_prints_minified_line_and_never_calls_launch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 55, "task_number": 7, "name": "JSON task", "is_completed": false, "is_trashed": false, "project_id": 99 }
        ]))))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "jsoninst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(
        &repo,
        &make_http(),
        None,
        true,
        true, // is_tty=true: json must still suppress TUI
        &mut out,
        &mut err,
    )
    .await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 0, "json mode must always return 0");

    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "output must be a single minified line: {s:?}"
    );
    assert!(
        !trimmed.contains("  "),
        "output must not have 2-space indent (must be minified): {s:?}"
    );

    let obj: serde_json::Value = serde_json::from_str(trimmed).expect("output must be valid JSON");
    assert_eq!(obj["count"], 1, "count must match number of tasks");

    let tasks = obj["tasks"].as_array().expect("tasks must be an array");
    assert_eq!(tasks.len(), 1, "tasks array must have 1 entry");
    assert_eq!(tasks[0]["ref"], "99/55", "ref must be project_id/task_id");
    assert_eq!(tasks[0]["instance"], "jsoninst");
    assert_eq!(tasks[0]["project_id"], 99);
    assert_eq!(tasks[0]["task_number"], 7);
    assert_eq!(tasks[0]["task_id"], 55);
    assert_eq!(tasks[0]["name"], "JSON task");
}

#[tokio::test]
async fn mine_core_json_mode_empty_rows_yields_count_zero_exit_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([]))),
        )
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "emptyinst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, true, false, &mut out, &mut err).await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 0, "json mode must return 0 even with empty rows");
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    let obj: serde_json::Value = serde_json::from_str(trimmed).expect("output must be valid JSON");
    assert_eq!(obj["count"], 0, "empty rows must yield count 0");
    let tasks = obj["tasks"].as_array().expect("tasks must be an array");
    assert!(tasks.is_empty(), "tasks array must be empty");
}

fn comment_inst(base_url: &str) -> Instance {
    Instance {
        name: "testinst".to_owned(),
        base_url: base_url.to_owned(),
        email: "user@example.com".to_owned(),
        token: "tok-comment".to_owned(),
        user_id: Some(1),
    }
}

fn comment_response(comment_id: i64) -> serde_json::Value {
    serde_json::json!({ "id": comment_id, "body": "some body" })
}

#[tokio::test]
async fn comment_core_flag_body_explicit_ref_calls_create_comment_and_returns_0() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .and(body_json(
            serde_json::json!({ "body": "Deploy em homolog." }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment_response(101)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "Deploy em homolog.",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_eq!(code, 0, "err: {}", output_str(&err));
    let s = output_str(&out);
    assert!(
        s.contains("101"),
        "confirmation must contain comment_id: {s}"
    );
    assert!(
        s.contains("75346"),
        "confirmation must contain task_id: {s}"
    );
    assert!(
        s.contains("524"),
        "confirmation must contain project_id: {s}"
    );
}

#[tokio::test]
async fn comment_core_multiline_stdin_body_passed_verbatim() {
    let server = MockServer::start().await;
    let multiline = "Linha 1\nLinha 2\nLinha 3";
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .and(body_json(serde_json::json!({ "body": multiline })))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment_response(202)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        multiline,
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_eq!(code, 0, "err: {}", output_str(&err));
}

#[tokio::test]
async fn comment_core_json_flag_stdout_is_exact_minified_result_line() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment_response(123)))
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "ok",
        &inst,
        &client,
        true,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 0, "err: {}", output_str(&err));
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "json output must be a single line: {s:?}"
    );
    let obj: serde_json::Value = serde_json::from_str(trimmed).expect("stdout must be valid JSON");
    assert_eq!(obj["ok"], true, "ok must be true");
    assert_eq!(obj["comment_id"], 123);
    assert_eq!(obj["task_id"], 75346);
    assert_eq!(obj["project_id"], 524);
    assert!(
        output_str(&err).is_empty(),
        "stderr must be empty on success"
    );
}

#[tokio::test]
async fn comment_core_empty_body_returns_exit2_and_no_create_comment_call() {
    let server = MockServer::start().await;
    // No mock set up — any POST to create_comment would fail the test via unexpected request.

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "empty body must return exit code 2");
    let e = output_str(&err);
    assert!(
        e.contains("no comment body"),
        "error must mention 'no comment body': {e}"
    );
    assert!(
        output_str(&out).is_empty(),
        "stdout must be empty when body is missing"
    );
}

#[tokio::test]
async fn comment_core_whitespace_only_body_returns_exit2() {
    let inst = comment_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "   \n  ",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "whitespace-only body must return exit code 2");
    assert!(output_str(&err).contains("no comment body"));
}

#[tokio::test]
async fn comment_core_branch_resolved_task_posts_to_branch_task() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75159"))
        .and(body_json(serde_json::json!({ "body": "branch comment" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment_response(50)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        None,
        Some("feature/665-75159"),
        "branch comment",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_eq!(code, 0, "branch-resolved task must return 0");
    let s = output_str(&out);
    assert!(
        s.contains("75159"),
        "confirmation must contain task_id: {s}"
    );
}

#[tokio::test]
async fn comment_core_no_ref_and_no_branch_returns_exit2_without_write() {
    let server = MockServer::start().await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        None,
        None,
        "some body",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "no task ref should return exit 2");
    assert!(
        !output_str(&out).contains("posted"),
        "must not print success when task not found"
    );
}

#[tokio::test]
async fn mine_core_401_prints_reauth_message_and_returns_exit1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "revoked".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "bad-token".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, false, false, &mut out, &mut err).await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 1, "401 must yield non-zero exit");
    let e = output_str(&err);
    assert!(
        e.contains("Token invalid or revoked"),
        "re-auth message must appear in stderr: {e}"
    );
    assert!(
        e.contains("ac setup add"),
        "re-auth guidance must name the command: {e}"
    );
    assert!(
        !output_str(&out).contains("INSTANCE"),
        "table header must NOT be printed on 401: {}",
        output_str(&out)
    );
    assert!(
        !output_str(&out).contains("No open tasks"),
        "empty-table message must NOT be printed on 401: {}",
        output_str(&out)
    );
}

#[tokio::test]
async fn mine_core_401_json_mode_prints_reauth_message_and_returns_exit1() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "revoked-json".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "bad-token".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let outcome = mine_core(&repo, &make_http(), None, true, false, &mut out, &mut err).await;
    let code = mine_outcome_code(outcome);

    assert_eq!(code, 1, "401 in json mode must yield non-zero exit");
    let e = output_str(&err);
    assert!(
        e.contains("Token invalid or revoked"),
        "re-auth message must appear on 401 json mode: {e}"
    );
    assert!(
        output_str(&out).is_empty(),
        "stdout must be empty on 401: {}",
        output_str(&out)
    );
}

#[tokio::test]
async fn collect_mine_rows_401_returns_empty_via_unwrap_or_default_seam() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = Instance {
        name: "tui-401".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "bad-token".to_owned(),
        user_id: Some(42),
    };
    let rows = collect_mine_rows(&[inst], &make_http()).await;

    assert!(
        rows.is_empty(),
        "collect_mine_rows on 401 must yield empty rows (TUI non-breaking seam)"
    );
}

#[tokio::test]
async fn comment_core_unresolvable_branch_returns_exit2_without_write() {
    let server = MockServer::start().await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        None,
        Some("main"),
        "some body",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "non-task branch must return exit 2");
    let e = output_str(&err);
    assert!(
        e.contains("main"),
        "error must mention the branch name: {e}"
    );
    assert!(
        output_str(&out).is_empty(),
        "stdout must be empty when branch unresolvable"
    );
}

#[tokio::test]
async fn comment_core_http_4xx_returns_nonzero_without_success_line() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "body text",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 4xx must not return exit 0");
    assert!(
        !output_str(&out).contains("posted"),
        "success line must not appear on HTTP error"
    );
    assert!(
        output_str(&err).contains("403"),
        "stderr must mention the HTTP status: {}",
        output_str(&err)
    );
}

#[tokio::test]
async fn comment_core_http_failure_with_json_flag_emits_error_object() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "body text",
        &inst,
        &client,
        true,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 5xx with --json must not return exit 0");
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    let obj: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout must be valid JSON on --json failure");
    assert_eq!(obj["ok"], false, "ok must be false on failure");
    assert!(
        obj.get("error").is_some(),
        "error field must be present: {obj}"
    );
}

// AC1: get/current 401 → re-auth message, non-zero exit, no "task not found"
#[tokio::test]
async fn do_get_task_401_prints_reauth_message_and_returns_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = do_get_task(
        &inst,
        &cache,
        &client,
        5,
        10,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 401 must return a non-zero exit code");
    let e = output_str(&err);
    assert!(
        e.contains("ac setup add"),
        "re-auth message must mention 'ac setup add': {e}"
    );
    assert!(
        !e.contains("task not found"),
        "must NOT print 'task not found' for 401: {e}"
    );
}

// AC1: load_task 401 writes re-auth message to the err writer
#[tokio::test]
async fn load_task_401_writes_reauth_to_err_and_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(inst, make_http());
    let mut err = Vec::new();

    let result = load_task(&cache, &client, "inst", 10, 99, false, false, &mut err).await;

    server.verify().await;
    assert!(result.is_none(), "401 must return None");
    let e = output_str(&err);
    assert!(
        e.contains("ac setup add"),
        "re-auth message must mention 'ac setup add': {e}"
    );
    assert!(
        !e.contains("task not found"),
        "must NOT print 'task not found' for 401: {e}"
    );
}

// AC2: comment 401 → re-auth message, non-zero exit
#[tokio::test]
async fn comment_core_401_prints_reauth_message_and_returns_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "some body",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 401 must return non-zero exit");
    let e = output_str(&err);
    assert!(
        e.contains("ac setup add"),
        "re-auth message must mention 'ac setup add': {e}"
    );
    assert!(
        !output_str(&out).contains("ok"),
        "success marker must not appear on 401: {}",
        output_str(&out)
    );
}

// AC2: comment 401 with --json → failure shape (no "ok": true)
#[tokio::test]
async fn comment_core_401_json_emits_failure_shape_without_ok_true() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "some body",
        &inst,
        &client,
        true,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 401 with --json must return non-zero");
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    let obj: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout must be valid JSON with --json");
    assert_eq!(obj["ok"], false, "ok must be false on 401");
    assert!(
        obj.get("error").is_some(),
        "error field must be present: {obj}"
    );
    let error_str = obj["error"].as_str().unwrap_or("");
    assert!(
        error_str.contains("ac setup add"),
        "error message must contain re-auth hint: {error_str}"
    );
}

// AC3: non-401 errors (404/500) keep existing output, no re-auth message
#[tokio::test]
async fn do_get_task_404_prints_not_found_without_reauth_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let cache = TaskCache::new(store.conn());
    let inst = make_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = do_get_task(
        &inst,
        &cache,
        &client,
        5,
        10,
        &default_flags(),
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 404 must return non-zero");
    let e = output_str(&err);
    assert!(
        e.contains("404"),
        "existing 404 message must still appear: {e}"
    );
    assert!(
        !e.contains("ac setup add"),
        "re-auth message must NOT appear for 404: {e}"
    );
}

#[tokio::test]
async fn comment_core_404_keeps_existing_output_without_reauth_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = comment_core(
        Some("524/75346"),
        None,
        "some body",
        &inst,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0, "HTTP 404 must return non-zero");
    let e = output_str(&err);
    assert!(
        e.contains("404"),
        "existing HTTP status must still appear: {e}"
    );
    assert!(
        !e.contains("ac setup add"),
        "re-auth message must NOT appear for 404: {e}"
    );
}

// AC4: i18n — message resolves through t(); pt-BR maps to translated string
#[test]
fn reauth_message_resolves_in_pt_br() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let msg = crate::i18n::t("Token invalid or revoked — run `ac setup add` to re-authenticate.");
    set_language("en");
    drop(_guard);

    assert!(
        msg.contains("reautenticar"),
        "pt-BR translation must contain 'reautenticar': {msg}"
    );
    assert!(
        msg.contains("inválido"),
        "pt-BR translation must contain 'inválido': {msg}"
    );
}

#[test]
fn reauth_message_english_key_is_identity() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let msg = crate::i18n::t("Token invalid or revoked — run `ac setup add` to re-authenticate.");
    drop(_guard);

    assert_eq!(
        msg, "Token invalid or revoked — run `ac setup add` to re-authenticate.",
        "English key must be returned as-is (identity)"
    );
}
