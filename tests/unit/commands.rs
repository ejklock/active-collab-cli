use super::*;
use crate::config::Config;
use crate::i18n::set_language;
use crate::render;
use crate::store::Store;
use std::sync::Mutex;
use tempfile::TempDir;
use wiremock::matchers::{body_json, header, method, path};
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
fn parse_task_ref_overflowing_digit_run_returns_exit2_without_panic() {
    let mut err = Vec::new();
    let overflowing_id = "9".repeat(30);
    let ref_ = format!("https://example.com/projects/{overflowing_id}/tasks/{overflowing_id}");
    let result = parse_task_ref(&ref_, &mut err);
    assert_eq!(result, Err(2));
    let e = output_str(&err);
    assert!(e.contains("cannot parse task ref"), "err: {e}");
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
            serde_json::json!({ "body": "<p>Deploy em homolog.</p>" }),
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
        false,
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
async fn comment_core_multiline_stdin_body_encoded_as_br() {
    let server = MockServer::start().await;
    let multiline = "Linha 1\nLinha 2\nLinha 3";
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .and(body_json(
            serde_json::json!({ "body": "<p>Linha 1<br>Linha 2<br>Linha 3</p>" }),
        ))
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
        false,
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
        false,
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
        false,
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
        false,
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
        .and(body_json(
            serde_json::json!({ "body": "<p>branch comment</p>" }),
        ))
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
        false,
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
        false,
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
        false,
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
        false,
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
        false,
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
        false,
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
        false,
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
        false,
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

// Issue 0071 AC2: with --html, the posted body equals the caller's input
// byte for byte — no escaping, no paragraph wrapping — reaching the server
// through the same client.create_comment seam the TUI uses.
#[tokio::test]
async fn comment_core_html_flag_posts_body_verbatim() {
    let cases = [
        "<p><strong>H</strong></p><p>&nbsp;</p>",
        "line one\nline two",
        "<p>unclosed",
        "a < b & c",
        "<hr>",
        "<script>x</script>",
    ];

    for (i, body) in cases.iter().enumerate() {
        let server = MockServer::start().await;
        let task_id = 75346 + i as i64;
        Mock::given(method("POST"))
            .and(path(format!("/api/v1/comments/task/{task_id}")))
            .and(body_json(serde_json::json!({ "body": body })))
            .and(header("x-angie-authapitoken", "tok-comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(comment_response(1)))
            .expect(1)
            .mount(&server)
            .await;

        let inst = comment_inst(&server.uri());
        let client = ActiveCollabClient::new(inst.clone(), make_http());
        let mut out = Vec::new();
        let mut err = Vec::new();

        let code = comment_core(
            Some(&format!("524/{task_id}")),
            None,
            body,
            true,
            &inst,
            &client,
            false,
            &mut out,
            &mut err,
        )
        .await;

        server.verify().await;
        assert_eq!(code, 0, "case {i} ({body:?}) err: {}", output_str(&err));
    }
}

// Issue 0071 AC3: an empty or whitespace-only body under --html still exits
// 2 with the existing message and sends no request — the html flag changes
// encoding, never the empty-body guard.
#[tokio::test]
async fn comment_core_html_flag_whitespace_only_body_returns_exit2() {
    let bodies = ["", "   ", "\n\n", "\u{00A0}"];

    for body in bodies {
        let server = MockServer::start().await;
        // No mock mounted — any POST would fail the test via unexpected request.

        let inst = comment_inst(&server.uri());
        let client = ActiveCollabClient::new(inst.clone(), make_http());
        let mut out = Vec::new();
        let mut err = Vec::new();

        let code = comment_core(
            Some("524/75346"),
            None,
            body,
            true,
            &inst,
            &client,
            false,
            &mut out,
            &mut err,
        )
        .await;

        assert_eq!(code, 2, "body {body:?} must return exit code 2");
        assert!(output_str(&err).contains("no comment body"));
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "body {body:?} must send no request"
        );
    }
}

// Issue 0071: a markup-only body such as `<p></p>` is accepted under --html
// (owner decision) — the empty-body guard stays `str::trim`, which does not
// treat markup as blank.
#[tokio::test]
async fn comment_core_html_flag_markup_only_body_is_posted() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/75346"))
        .and(body_json(serde_json::json!({ "body": "<p></p>" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(comment_response(1)))
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
        "<p></p>",
        true,
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

// Issue 0071 AC5: --html changes only the posted body, never the --json
// output shape, the HTTP-4xx failure path or the 401 re-auth message.
#[tokio::test]
async fn comment_core_html_flag_json_output_is_unchanged() {
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
        "<p>ok</p>",
        true,
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
}

#[tokio::test]
async fn comment_core_html_flag_http_4xx_returns_nonzero() {
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
        "<p>body</p>",
        true,
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
}

#[tokio::test]
async fn comment_core_html_flag_401_prints_reauth_message() {
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
        "<p>body</p>",
        true,
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

fn job_type_response(id: i64, name: &str, is_default: bool) -> serde_json::Value {
    serde_json::json!({ "id": id, "name": name, "is_default": is_default })
}

fn time_record_response(id: i64) -> serde_json::Value {
    serde_json::json!({ "id": id })
}

async fn mount_job_types(server: &MockServer, types: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/v1/job-types"))
        .respond_with(ResponseTemplate::new(200).set_body_json(types))
        .mount(server)
        .await;
}

#[tokio::test]
async fn time_log_core_explicit_date_and_default_job_type_posts_and_returns_0() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([job_type_response(7, "Development", true)]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .and(body_json(serde_json::json!({
            "task_id": 75346,
            "value": 1.5,
            "record_date": "2026-01-15",
            "job_type_id": 7,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(time_record_response(901)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.5,
        Some("2026-01-15"),
        None,
        None,
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
        s.contains("901"),
        "confirmation must contain record id: {s}"
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
async fn time_log_core_summary_is_included_in_posted_body() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([job_type_response(7, "Development", true)]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .and(body_json(serde_json::json!({
            "task_id": 75346,
            "value": 2.0,
            "record_date": "2026-01-15",
            "job_type_id": 7,
            "summary": "Deploy em homolog.",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(time_record_response(902)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        2.0,
        Some("2026-01-15"),
        Some("Deploy em homolog."),
        None,
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
async fn time_log_core_job_type_override_matched_by_name_case_insensitive() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([
            job_type_response(7, "Development", true),
            job_type_response(9, "Design", false),
        ]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .and(body_json(serde_json::json!({
            "task_id": 75346,
            "value": 1.0,
            "record_date": "2026-01-15",
            "job_type_id": 9,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(time_record_response(903)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.0,
        Some("2026-01-15"),
        None,
        Some("design"),
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
async fn time_log_core_job_type_override_matched_by_id() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([
            job_type_response(7, "Development", true),
            job_type_response(9, "Design", false),
        ]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .and(body_json(serde_json::json!({
            "task_id": 75346,
            "value": 1.0,
            "record_date": "2026-01-15",
            "job_type_id": 9,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(time_record_response(904)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.0,
        Some("2026-01-15"),
        None,
        Some("9"),
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
async fn time_log_core_no_job_types_available_errors_without_posting_time_record() {
    let server = MockServer::start().await;
    mount_job_types(&server, serde_json::json!([])).await;
    // No POST mock — a create_time_record call would fail the test via an unexpected request.

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.0,
        Some("2026-01-15"),
        None,
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_ne!(code, 0, "no resolvable job type must not return exit 0");
    assert!(
        output_str(&err).contains("job type"),
        "error must mention job type: {}",
        output_str(&err)
    );
}

#[tokio::test]
async fn time_log_core_nonpositive_hours_returns_exit2_without_network_call() {
    // No mocks configured at all — any HTTP call fails the test via connection refused
    // to an unreachable address, but the point is time_log_core must never attempt one.
    let inst = comment_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        0.0,
        None,
        None,
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "non-positive hours must return exit code 2");
    assert!(output_str(&err).contains("positive"));
    assert!(output_str(&out).is_empty());
}

#[tokio::test]
async fn time_log_core_json_flag_stdout_is_exact_minified_result_line() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([job_type_response(7, "Development", true)]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .respond_with(ResponseTemplate::new(200).set_body_json(time_record_response(905)))
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.5,
        Some("2026-01-15"),
        None,
        None,
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
    assert_eq!(obj["ok"], true);
    assert_eq!(obj["time_record_id"], 905);
    assert_eq!(obj["task_id"], 75346);
    assert_eq!(obj["project_id"], 524);
    assert_eq!(obj["hours"], 1.5);
}

#[tokio::test]
async fn time_log_core_401_prints_reauth_message_and_returns_nonzero() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([job_type_response(7, "Development", true)]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.0,
        Some("2026-01-15"),
        None,
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0);
    assert!(output_str(&err).contains("ac setup add"));
}

#[tokio::test]
async fn time_log_core_http_failure_with_json_flag_emits_error_object() {
    let server = MockServer::start().await;
    mount_job_types(
        &server,
        serde_json::json!([job_type_response(7, "Development", true)]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/api/v1/projects/524/time-records"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = time_log_core(
        Some("524/75346"),
        None,
        1.0,
        Some("2026-01-15"),
        None,
        None,
        &client,
        true,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0);
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    let obj: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout must be valid JSON on --json failure");
    assert_eq!(obj["ok"], false);
    assert!(obj.get("error").is_some());
}

// --- task_set_core (issue 0068 slice 2) ---

fn task_write_response(id: i64) -> serde_json::Value {
    serde_json::json!({ "id": id })
}

fn user_directory_response(id: i64, display_name: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "display_name": display_name,
        "first_name": null,
        "last_name": null,
        "email": format!("user{id}@example.com"),
    })
}

#[tokio::test]
async fn task_set_core_status_done_puts_to_complete_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/complete/task/75346"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("done"),
        None,
        None,
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
async fn task_set_core_status_open_puts_to_open_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/open/task/75346"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("open"),
        None,
        None,
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
async fn task_set_core_numeric_assignee_updates_task_without_directory_fetch() {
    let server = MockServer::start().await;
    // No /api/v1/users mock — an all-digits assignee must never fetch the directory.
    Mock::given(method("PUT"))
        .and(path("/api/v1/projects/524/tasks/75346"))
        .and(body_json(serde_json::json!({ "assignee_id": 9 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        None,
        Some("9"),
        None,
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
async fn task_set_core_named_assignee_resolves_via_user_directory() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_directory_response(9, "Alice Smith"),
            user_directory_response(10, "Bob Jones"),
        ])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/projects/524/tasks/75346"))
        .and(body_json(serde_json::json!({ "assignee_id": 9 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        None,
        Some("alice smith"),
        None,
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
async fn task_set_core_estimate_only_updates_task_with_estimate_body() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/projects/524/tasks/75346"))
        .and(body_json(serde_json::json!({ "estimate": 8.0 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        None,
        None,
        Some(8.0),
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
async fn task_set_core_combined_fields_issues_both_writes() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/complete/task/75346"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/projects/524/tasks/75346"))
        .and(body_json(
            serde_json::json!({ "assignee_id": 9, "estimate": 8.0 }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("completed"),
        Some("9"),
        Some(8.0),
        &client,
        true,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_eq!(code, 0, "err: {}", output_str(&err));
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    let obj: serde_json::Value = serde_json::from_str(trimmed).expect("stdout must be valid JSON");
    assert_eq!(obj["ok"], true);
    assert_eq!(obj["status"], "completed");
    assert_eq!(obj["assignee_id"], 9);
    assert_eq!(obj["estimate"], 8.0);
}

#[tokio::test]
async fn task_set_core_no_field_given_returns_exit2_without_network_call() {
    let inst = comment_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        None,
        None,
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "no field given must return exit code 2");
    assert!(output_str(&err).contains("--status"));
    assert!(output_str(&out).is_empty());
}

#[tokio::test]
async fn task_set_core_invalid_status_returns_exit2_without_network_call() {
    let inst = comment_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("frozen"),
        None,
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "invalid --status must return exit code 2");
    assert!(output_str(&err).contains("--status"));
}

#[tokio::test]
async fn task_set_core_unknown_assignee_name_returns_exit2_without_update() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            user_directory_response(9, "Alice Smith"),
        ])))
        .mount(&server)
        .await;
    // No PUT mock — an unresolved assignee must never reach update_task.

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        None,
        Some("nobody"),
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "unresolved assignee name must return exit code 2");
    assert!(output_str(&err).contains("nobody"));
}

#[tokio::test]
async fn task_set_core_negative_estimate_returns_exit2_without_network_call() {
    let inst = comment_inst("http://127.0.0.1:1");
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        None,
        None,
        Some(-1.0),
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    assert_eq!(code, 2, "negative estimate must return exit code 2");
}

#[tokio::test]
async fn task_set_core_401_prints_reauth_message_and_returns_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/complete/task/75346"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("completed"),
        None,
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0);
    assert!(output_str(&err).contains("ac setup add"));
}

#[tokio::test]
async fn task_set_core_http_failure_with_json_flag_emits_error_object() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/complete/task/75346"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("completed"),
        None,
        None,
        &client,
        true,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(code, 0);
    let s = output_str(&out);
    let trimmed = s.trim_end_matches('\n');
    let obj: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout must be valid JSON on --json failure");
    assert_eq!(obj["ok"], false);
    assert!(obj.get("error").is_some());
}

#[tokio::test]
async fn task_set_core_second_write_failure_stops_after_first_write() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/complete/task/75346"))
        .respond_with(ResponseTemplate::new(200).set_body_json(task_write_response(75346)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/projects/524/tasks/75346"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .expect(1)
        .mount(&server)
        .await;

    let inst = comment_inst(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = task_set_core(
        Some("524/75346"),
        None,
        Some("completed"),
        Some("9"),
        None,
        &client,
        false,
        &mut out,
        &mut err,
    )
    .await;

    server.verify().await;
    assert_ne!(
        code, 0,
        "the second write's failure must surface as a non-zero exit"
    );
}

const REPORT_BOM: &str = "\u{FEFF}";
const DAY_ONE: i64 = 1785715200;
const DAY_TWO: i64 = 1785801600;

fn hours_report_fixture(value: serde_json::Value) -> crate::models::TimeRecordsReport {
    serde_json::from_value(value).unwrap()
}

fn hours_report_lines(csv: &str) -> Vec<String> {
    csv.trim_start_matches('\u{FEFF}')
        .split('\n')
        .map(|l| l.to_string())
        .collect()
}

fn single_task_record(value: f64) -> serde_json::Value {
    serde_json::json!({
        "time_records": [
            {"id": 1, "value": value, "record_date": DAY_ONE, "summary": "", "parent_type": "Task", "parent_id": 10}
        ],
        "related": {
            "Project": {"5": {"id": 5, "name": "Projeto Ação"}},
            "Task": {"10": {"id": 10, "project_id": 5, "name": "Tarefa única"}}
        }
    })
}

#[test]
fn hours_report_csv_total_sums_all_values() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5},
            {"id": 2, "value": 2.0, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5},
            {"id": 3, "value": 1.5, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5},
            {"id": 4, "value": 5.42, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5},
            {"id": 5, "value": 1.58, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5}
        ],
        "related": {"Project": {"5": {"id": 5, "name": "P"}}}
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[0], "Total hours for the period;11,5");
}

#[test]
fn hours_report_csv_integral_total_has_no_decimal_comma() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 39.5, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5},
            {"id": 2, "value": 39.5, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5}
        ],
        "related": {"Project": {"5": {"id": 5, "name": "P"}}}
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[0], "Total hours for the period;79");
}

#[test]
fn hours_report_csv_formats_hours_with_decimal_comma_and_trimmed_zeros() {
    for (value, expected) in [
        (1.0, "1"),
        (1.5, "1,5"),
        (5.42, "5,42"),
        (0.25, "0,25"),
        (1.50, "1,5"),
    ] {
        let csv =
            hours_report::render_hours_report_csv(&hours_report_fixture(single_task_record(value)));
        let lines = hours_report_lines(&csv);
        assert_eq!(
            lines[3],
            format!("Projeto Ação;Tarefa única;{expected};03/08/26"),
            "value {value}"
        );
    }
}

#[test]
fn hours_report_csv_sorts_by_date_then_project_then_id() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 3, "value": 1.0, "record_date": DAY_TWO, "parent_type": "Task", "parent_id": 20},
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 21},
            {"id": 9, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 22},
            {"id": 4, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 22}
        ],
        "related": {
            "Project": {
                "1": {"id": 1, "name": "[Manutenção] Zeta"},
                "2": {"id": 2, "name": "[X][BASE] Alfa"},
                "3": {"id": 3, "name": "[X] Demandas"}
            },
            "Task": {
                "20": {"id": 20, "project_id": 1, "name": "t20"},
                "21": {"id": 21, "project_id": 2, "name": "t21"},
                "22": {"id": 22, "project_id": 3, "name": "t22"}
            }
        }
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(
        &lines[3..],
        [
            "[X] Demandas;t22;1;03/08/26",
            "[X] Demandas;t22;1;03/08/26",
            "[X][BASE] Alfa;t21;1;03/08/26",
            "[Manutenção] Zeta;t20;1;04/08/26",
        ]
    );
}

#[test]
fn hours_report_csv_breaks_ties_by_record_id() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 9, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 22},
            {"id": 4, "value": 2.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 22}
        ],
        "related": {
            "Project": {"3": {"id": 3, "name": "P"}},
            "Task": {"22": {"id": 22, "project_id": 3, "name": "t"}}
        }
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], "P;t;2;03/08/26");
    assert_eq!(lines[4], "P;t;1;03/08/26");
}

#[test]
fn hours_report_csv_project_record_uses_summary_as_activity() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "summary": "Reunião", "parent_type": "Project", "parent_id": 5},
            {"id": 2, "value": 1.0, "record_date": DAY_ONE, "summary": "", "parent_type": "Project", "parent_id": 5},
            {"id": 3, "value": 1.0, "record_date": DAY_ONE, "summary": null, "parent_type": "Project", "parent_id": 5}
        ],
        "related": {"Project": {"5": {"id": 5, "name": "Projeto"}}}
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], "Projeto;Reunião;1;03/08/26");
    assert_eq!(lines[4], "Projeto;;1;03/08/26");
    assert_eq!(lines[5], "Projeto;;1;03/08/26");
}

#[test]
fn hours_report_csv_missing_related_renders_empty_cells() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 99},
            {"id": 2, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 98}
        ],
        "related": {}
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], ";;1;03/08/26");
    assert_eq!(lines[4], ";;1;03/08/26");
}

#[test]
fn hours_report_csv_task_with_unknown_project_keeps_task_name() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 10}
        ],
        "related": {"Task": {"10": {"id": 10, "project_id": 77, "name": "Só a tarefa"}}}
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], ";Só a tarefa;1;03/08/26");
}

#[test]
fn hours_report_csv_reads_the_record_date_in_utc() {
    let csv = hours_report::render_hours_report_csv(&hours_report_fixture(single_task_record(1.0)));
    assert!(
        hours_report_lines(&csv)[3].ends_with(";03/08/26"),
        "got: {csv}"
    );
}

#[test]
fn hours_report_csv_quotes_fields_holding_separator_or_quote() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 10},
            {"id": 2, "value": 1.0, "record_date": DAY_ONE, "parent_type": "Task", "parent_id": 11}
        ],
        "related": {
            "Project": {"5": {"id": 5, "name": "a;b"}},
            "Task": {
                "10": {"id": 10, "project_id": 5, "name": "say \"hi\""},
                "11": {"id": 11, "project_id": 5, "name": "linha\ndupla"}
            }
        }
    }));
    let csv = hours_report::render_hours_report_csv(&report);
    assert!(
        csv.contains("\"a;b\";\"say \"\"hi\"\"\";1;03/08/26"),
        "got: {csv}"
    );
    assert!(
        csv.contains("\"a;b\";\"linha\ndupla\";1;03/08/26"),
        "got: {csv}"
    );
}

#[test]
fn hours_report_csv_starts_with_bom_and_exact_header() {
    let csv = hours_report::render_hours_report_csv(&hours_report_fixture(single_task_record(1.0)));
    assert!(csv.starts_with(REPORT_BOM));
    let lines = hours_report_lines(&csv);
    assert_eq!(lines[1], "");
    assert_eq!(lines[2], "Project;Activity;Hours;Date");
}

#[test]
fn hours_report_csv_empty_response_has_zero_total_and_header_only() {
    let csv = hours_report::render_hours_report_csv(&hours_report_fixture(serde_json::json!({
        "time_records": [],
        "related": []
    })));
    assert_eq!(
        csv,
        format!("{REPORT_BOM}Total hours for the period;0\n\nProject;Activity;Hours;Date")
    );
}

fn report_response() -> serde_json::Value {
    serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "summary": "", "parent_type": "Task", "parent_id": 10},
            {"id": 2, "value": 0.5, "record_date": DAY_TWO, "summary": "", "parent_type": "Task", "parent_id": 10}
        ],
        "related": {
            "Project": {"5": {"id": 5, "name": "Projeto Ação"}},
            "Task": {"10": {"id": 10, "project_id": 5, "name": "Tarefa"}}
        }
    })
}

async fn mount_report(server: &MockServer, response: ResponseTemplate, expected_calls: u64) {
    mount_report_range(server, "2026-08-01", "2026-08-30", response, expected_calls).await;
}

async fn mount_report_range(
    server: &MockServer,
    from: &str,
    to: &str,
    response: ResponseTemplate,
    expected_calls: u64,
) {
    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/time-records/filtered-by-date"))
        .and(wiremock::matchers::query_param("from", from))
        .and(wiremock::matchers::query_param("to", to))
        .respond_with(response)
        .expect(expected_calls)
        .mount(server)
        .await;
}

struct ReportRun {
    code: i32,
    out: String,
    err: String,
}

fn report_today() -> chrono::NaiveDate {
    chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
}

async fn run_time_report(
    server: &MockServer,
    dir: &std::path::Path,
    start: &str,
    end: &str,
    user_id: Option<i64>,
) -> ReportRun {
    run_time_report_as(
        server,
        dir,
        hours_report::ReportPeriod::Range { start, end },
        hours_report::ReportFormat::Csv,
        user_id,
    )
    .await
}

async fn run_time_report_as(
    server: &MockServer,
    dir: &std::path::Path,
    period: hours_report::ReportPeriod<'_>,
    format: hours_report::ReportFormat,
    user_id: Option<i64>,
) -> ReportRun {
    let client = ActiveCollabClient::new(comment_inst(&server.uri()), make_http());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let request = hours_report::ReportRequest {
        period,
        format,
        today: report_today(),
        user_id,
        out_dir: dir,
    };
    let code = time_report_core(&request, &client, &mut out, &mut err).await;
    ReportRun {
        code,
        out: output_str(&out).to_string(),
        err: output_str(&err).to_string(),
    }
}

fn dir_is_empty(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).unwrap().next().is_none()
}

#[tokio::test]
async fn time_report_writes_the_named_file_and_prints_path_and_total() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    server.verify().await;
    assert_eq!(run.code, 0, "err: {}", run.err);
    let file = dir.path().join("hours-report-01-08-2026-30-08-2026.csv");
    let content = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        content,
        format!(
            "{REPORT_BOM}Total hours for the period;1,5\n\nProject;Activity;Hours;Date\nProjeto Ação;Tarefa;1;03/08/26\nProjeto Ação;Tarefa;0,5;04/08/26"
        )
    );
    assert_eq!(
        run.out,
        format!(
            "Hours report written to {} (total hours: 1,5)\n",
            file.display()
        )
    );
}

#[tokio::test]
async fn time_report_accepts_start_equal_to_end() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/time-records/filtered-by-date"))
        .and(wiremock::matchers::query_param("from", "2026-08-03"))
        .and(wiremock::matchers::query_param("to", "2026-08-03"))
        .respond_with(ResponseTemplate::new(200).set_body_json(report_response()))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report(&server, dir.path(), "03-08-2026", "03-08-2026", Some(1)).await;

    server.verify().await;
    assert_eq!(run.code, 0, "err: {}", run.err);
    assert!(dir
        .path()
        .join("hours-report-03-08-2026-03-08-2026.csv")
        .exists());
}

#[tokio::test]
async fn time_report_start_after_end_is_usage_error_without_request_or_file() {
    let server = MockServer::start().await;
    mount_report(&server, ResponseTemplate::new(200), 0).await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report(&server, dir.path(), "30-08-2026", "01-08-2026", Some(1)).await;

    server.verify().await;
    assert_eq!(run.code, 2);
    assert!(run.err.contains("after"), "{}", run.err);
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn time_report_malformed_or_impossible_dates_are_usage_errors_without_request() {
    for (start, end) in [
        ("2026-08-01", "30-08-2026"),
        ("01-08-2026", "2026-08-30"),
        ("1-8-2026", "30-08-2026"),
        ("aa-bb-cccc", "30-08-2026"),
        ("01-08-2026", "31-02-2026"),
    ] {
        let server = MockServer::start().await;
        mount_report(&server, ResponseTemplate::new(200), 0).await;
        let dir = TempDir::new().unwrap();

        let run = run_time_report(&server, dir.path(), start, end, Some(1)).await;

        server.verify().await;
        assert_eq!(run.code, 2, "{start} {end}");
        assert!(run.err.contains("DD-MM-YYYY"), "{start} {end}: {}", run.err);
        assert!(dir_is_empty(dir.path()), "{start} {end}");
    }
}

#[tokio::test]
async fn time_report_without_stored_user_id_fails_with_setup_hint_and_no_request() {
    let server = MockServer::start().await;
    mount_report(&server, ResponseTemplate::new(200), 0).await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", None).await;

    server.verify().await;
    assert_ne!(run.code, 0);
    assert!(run.err.contains("ac setup add"), "{}", run.err);
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn time_report_unauthorized_shows_reauth_message_and_writes_no_file() {
    let server = MockServer::start().await;
    mount_report(&server, ResponseTemplate::new(401), 1).await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    server.verify().await;
    assert_ne!(run.code, 0);
    assert!(
        run.err.contains(&presenter::reauth_message()),
        "{}",
        run.err
    );
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn time_report_server_error_names_the_status_and_writes_no_file() {
    let server = MockServer::start().await;
    mount_report(&server, ResponseTemplate::new(500), 1).await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    server.verify().await;
    assert_ne!(run.code, 0);
    assert!(
        run.err
            .contains("hours report request failed with HTTP 500"),
        "{}",
        run.err
    );
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn time_report_overwrites_an_existing_file() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("hours-report-01-08-2026-30-08-2026.csv");
    std::fs::write(&file, "stale content that is much longer than the new report ........................................................................................................................").unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    assert_eq!(run.code, 0, "err: {}", run.err);
    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.starts_with(REPORT_BOM), "{content}");
    assert!(!content.contains("stale"));
}

#[test]
fn hours_report_csv_parses_related_keyed_by_id_with_only_projects() {
    let report: crate::models::TimeRecordsReport = serde_json::from_str(
        r#"{"time_records":[{"id":1,"value":2.0,"record_date":1785715200,"summary":null,"parent_type":"Project","parent_id":694}],
            "related":{"Project":{"694":{"id":694,"name":"Keyed"}}}}"#,
    )
    .unwrap();
    assert_eq!(report.related.projects.len(), 1);
    assert!(report.related.tasks.is_empty());
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], "Keyed;;2;03/08/26");
}

#[test]
fn hours_report_csv_parses_task_keyed_by_its_id() {
    let report: crate::models::TimeRecordsReport = serde_json::from_str(
        r#"{"time_records":[{"id":1,"value":1.0,"record_date":1785715200,"summary":"","parent_type":"Task","parent_id":31525}],
            "related":{"Project":{"459":{"id":459,"name":"Proj"}},"Task":{"31525":{"id":31525,"project_id":459,"name":"Keyed task"}}}}"#,
    )
    .unwrap();
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], "Proj;Keyed task;1;03/08/26");
}

#[test]
fn hours_report_csv_record_missing_a_required_field_is_a_parse_error() {
    for body in [
        r#"{"time_records":[{"id":1,"record_date":1785715200,"parent_type":"Project","parent_id":5}],"related":{}}"#,
        r#"{"time_records":[{"id":1,"value":1.0,"parent_type":"Project","parent_id":5}],"related":{}}"#,
        r#"{"time_records":[{"id":1,"value":1.0,"record_date":1785715200,"parent_id":5}],"related":{}}"#,
        r#"{"time_records":[{"id":1,"value":1.0,"record_date":1785715200,"parent_type":"Project"}],"related":{}}"#,
        r#"{"time_records":[{"value":1.0,"record_date":1785715200,"parent_type":"Project","parent_id":5}],"related":{}}"#,
        r#"{"time_records":[],"related":{"Task":{"1":{"id":1,"name":"x"}}}}"#,
    ] {
        assert!(
            serde_json::from_str::<crate::models::TimeRecordsReport>(body).is_err(),
            "{body}"
        );
    }
}

#[test]
fn hours_report_csv_related_as_empty_array_parses_as_empty() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 2.0, "record_date": DAY_ONE, "parent_type": "Project", "parent_id": 5}
        ],
        "related": []
    }));
    assert!(report.related.projects.is_empty());
    assert!(report.related.tasks.is_empty());
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], ";;2;03/08/26");
}

fn hours_report_xlsx_range(
    report: &crate::models::TimeRecordsReport,
) -> calamine::Range<calamine::Data> {
    use calamine::Reader;
    let bytes = hours_report::render_hours_report_xlsx(report).unwrap();
    let mut workbook: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(workbook.sheet_names(), vec!["Hours".to_string()]);
    workbook.worksheet_range("Hours").unwrap()
}

fn xlsx_text(range: &calamine::Range<calamine::Data>, row: u32, col: u32) -> String {
    match range.get_value((row, col)) {
        Some(calamine::Data::String(s)) => s.clone(),
        Some(calamine::Data::Empty) | None => String::new(),
        other => panic!("cell ({row},{col}) is not text: {other:?}"),
    }
}

fn xlsx_number(range: &calamine::Range<calamine::Data>, row: u32, col: u32) -> f64 {
    match range.get_value((row, col)) {
        Some(calamine::Data::Float(f)) => *f,
        Some(calamine::Data::Int(i)) => *i as f64,
        other => panic!("cell ({row},{col}) is not a number: {other:?}"),
    }
}

fn xlsx_date(range: &calamine::Range<calamine::Data>, row: u32, col: u32) -> chrono::NaiveDateTime {
    use calamine::DataType;
    match range.get_value((row, col)) {
        Some(cell @ calamine::Data::DateTime(_)) => cell.as_datetime().unwrap(),
        other => panic!("cell ({row},{col}) is not a date: {other:?}"),
    }
}

fn xlsx_fixture() -> crate::models::TimeRecordsReport {
    hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "summary": "", "parent_type": "Task", "parent_id": 10},
            {"id": 2, "value": 2.0, "record_date": DAY_ONE, "summary": "Plan; \"phase\" 1", "parent_type": "Project", "parent_id": 6},
            {"id": 3, "value": 1.5, "record_date": DAY_TWO, "summary": "", "parent_type": "Task", "parent_id": 10},
            {"id": 4, "value": 5.42, "record_date": DAY_TWO, "summary": "Revisão", "parent_type": "Project", "parent_id": 5},
            {"id": 5, "value": 1.58, "record_date": DAY_TWO, "summary": "", "parent_type": "Project", "parent_id": 5}
        ],
        "related": {
            "Project": {
                "5": {"id": 5, "name": "Projeto Ação"},
                "6": {"id": 6, "name": "Semi; \"Quote\""}
            },
            "Task": {"10": {"id": 10, "project_id": 5, "name": "Tarefa única"}}
        }
    }))
}

#[test]
fn hours_report_xlsx_has_total_header_and_typed_rows_in_csv_order() {
    let range = hours_report_xlsx_range(&xlsx_fixture());

    assert_eq!(xlsx_text(&range, 0, 0), "Total hours for the period");
    assert!((xlsx_number(&range, 0, 1) - 11.5).abs() < 1e-9);
    assert_eq!(xlsx_text(&range, 1, 0), "");
    assert_eq!(xlsx_text(&range, 1, 1), "");
    let header: Vec<String> = (0..4).map(|c| xlsx_text(&range, 2, c)).collect();
    assert_eq!(header, ["Project", "Activity", "Hours", "Date"]);

    let expected = [
        ("Projeto Ação", "Tarefa única", 1.0, (2026, 8, 3)),
        ("Semi; \"Quote\"", "Plan; \"phase\" 1", 2.0, (2026, 8, 3)),
        ("Projeto Ação", "Tarefa única", 1.5, (2026, 8, 4)),
        ("Projeto Ação", "Revisão", 5.42, (2026, 8, 4)),
        ("Projeto Ação", "", 1.58, (2026, 8, 4)),
    ];
    assert_eq!(range.height(), 3 + expected.len());
    for (i, (project, activity, hours, (y, m, d))) in expected.iter().enumerate() {
        let row = 3 + i as u32;
        assert_eq!(xlsx_text(&range, row, 0), *project, "row {row}");
        assert_eq!(xlsx_text(&range, row, 1), *activity, "row {row}");
        assert!(
            (xlsx_number(&range, row, 2) - hours).abs() < 1e-9,
            "row {row}"
        );
        assert_eq!(
            xlsx_date(&range, row, 3),
            chrono::NaiveDate::from_ymd_opt(*y, *m, *d)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            "row {row}"
        );
    }
}

#[test]
fn hours_report_xlsx_rows_follow_the_csv_row_order() {
    let report = xlsx_fixture();
    let range = hours_report_xlsx_range(&report);
    let csv = hours_report::render_hours_report_csv(&report);
    let csv_hours: Vec<String> = hours_report_lines(&csv)[3..]
        .iter()
        .map(|line| line.rsplit(';').nth(1).unwrap().replace(',', "."))
        .collect();
    let xlsx_hours: Vec<String> = (3..range.height() as u32)
        .map(|row| xlsx_number(&range, row, 2).to_string())
        .collect();
    assert_eq!(xlsx_hours, csv_hours);
}

#[test]
fn hours_report_xlsx_empty_report_has_zero_total_and_header_only() {
    let range = hours_report_xlsx_range(&hours_report_fixture(serde_json::json!({
        "time_records": [],
        "related": {}
    })));
    assert_eq!(xlsx_text(&range, 0, 0), "Total hours for the period");
    assert_eq!(xlsx_number(&range, 0, 1), 0.0);
    assert_eq!(xlsx_text(&range, 2, 0), "Project");
    assert_eq!(xlsx_text(&range, 2, 3), "Date");
    assert_eq!(range.height(), 3);
}

const REPORT_FORMATS: [hours_report::ReportFormat; 2] = [
    hours_report::ReportFormat::Xlsx,
    hours_report::ReportFormat::Csv,
];

fn report_file_names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn time_report_xlsx_writes_only_the_xlsx_file_with_typed_cells() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report_as(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        },
        hours_report::ReportFormat::Xlsx,
        Some(1),
    )
    .await;

    assert_eq!(run.code, 0, "err: {}", run.err);
    assert_eq!(
        report_file_names(dir.path()),
        ["hours-report-01-08-2026-30-08-2026.xlsx"]
    );
    assert!(
        run.out.contains("hours-report-01-08-2026-30-08-2026.xlsx"),
        "{}",
        run.out
    );
    let bytes = std::fs::read(dir.path().join("hours-report-01-08-2026-30-08-2026.xlsx")).unwrap();
    let mut workbook: calamine::Xlsx<_> =
        calamine::open_workbook_from_rs(std::io::Cursor::new(bytes)).unwrap();
    let range = calamine::Reader::worksheet_range(&mut workbook, "Hours").unwrap();
    assert!((xlsx_number(&range, 0, 1) - 1.5).abs() < 1e-9);
}

#[tokio::test]
async fn time_report_presets_request_the_resolved_dates_and_name_the_file() {
    for (preset, from, to, file) in [
        (
            hours_report::PeriodPreset::Month,
            "2026-10-01",
            "2026-10-31",
            "hours-report-01-10-2026-31-10-2026.xlsx",
        ),
        (
            hours_report::PeriodPreset::Fortnight,
            "2026-10-16",
            "2026-10-31",
            "hours-report-16-10-2026-31-10-2026.xlsx",
        ),
        (
            hours_report::PeriodPreset::Week,
            "2026-10-26",
            "2026-11-01",
            "hours-report-26-10-2026-01-11-2026.xlsx",
        ),
    ] {
        let server = MockServer::start().await;
        mount_report_range(
            &server,
            from,
            to,
            ResponseTemplate::new(200).set_body_json(report_response()),
            1,
        )
        .await;
        let dir = TempDir::new().unwrap();

        let run = run_time_report_as(
            &server,
            dir.path(),
            hours_report::ReportPeriod::Preset(preset),
            hours_report::ReportFormat::Xlsx,
            Some(1),
        )
        .await;

        server.verify().await;
        assert_eq!(run.code, 0, "{preset:?} err: {}", run.err);
        assert_eq!(report_file_names(dir.path()), [file], "{preset:?}");
    }
}

#[tokio::test]
async fn time_report_failures_write_no_file_in_either_format() {
    for format in REPORT_FORMATS {
        let dir = TempDir::new().unwrap();
        let server = MockServer::start().await;
        let range = hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        };

        let bad_dates = run_time_report_as(
            &server,
            dir.path(),
            hours_report::ReportPeriod::Range {
                start: "30-08-2026",
                end: "01-08-2026",
            },
            format,
            Some(1),
        )
        .await;
        assert_eq!(bad_dates.code, 2, "{format:?}");

        let no_user = run_time_report_as(&server, dir.path(), range, format, None).await;
        assert_eq!(no_user.code, 1, "{format:?}");

        for (status, needle) in [(401, ""), (500, "500")] {
            let server = MockServer::start().await;
            mount_report(&server, ResponseTemplate::new(status), 1).await;
            let run = run_time_report_as(&server, dir.path(), range, format, Some(1)).await;
            assert_eq!(run.code, 1, "{format:?} {status}");
            assert!(run.err.contains(needle), "{format:?} {status}: {}", run.err);
        }
        assert!(dir_is_empty(dir.path()), "{format:?}");
    }
}

#[tokio::test]
async fn time_report_overwrites_an_existing_xlsx_file() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("hours-report-01-08-2026-30-08-2026.xlsx");
    std::fs::write(&file, "stale").unwrap();

    let run = run_time_report_as(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        },
        hours_report::ReportFormat::Xlsx,
        Some(1),
    )
    .await;

    assert_eq!(run.code, 0, "err: {}", run.err);
    assert!(std::fs::read(&file).unwrap().starts_with(b"PK"));
}

#[tokio::test]
async fn time_report_xlsx_render_failure_creates_no_file() {
    let server = MockServer::start().await;
    let mut response = report_response();
    response["time_records"][0]["parent_type"] = "Project".into();
    response["time_records"][0]["parent_id"] = 5.into();
    response["time_records"][0]["summary"] = "x".repeat(40_000).into();
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(response),
        1,
    )
    .await;
    let dir = TempDir::new().unwrap();

    let run = run_time_report_as(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        },
        hours_report::ReportFormat::Xlsx,
        Some(1),
    )
    .await;

    assert_eq!(run.code, 1);
    assert!(run.err.contains("Error:"), "{}", run.err);
    assert!(dir_is_empty(dir.path()));
}

async fn run_export(
    server: &MockServer,
    dir: &std::path::Path,
    period: hours_report::ReportPeriod<'_>,
    format: hours_report::ReportFormat,
    user_id: Option<i64>,
) -> Result<hours_report::ExportedReport, hours_report::ExportError> {
    let client = ActiveCollabClient::new(comment_inst(&server.uri()), make_http());
    let request = hours_report::ReportRequest {
        period,
        format,
        today: report_today(),
        user_id,
        out_dir: dir,
    };
    hours_report::export_hours_report(&request, &client).await
}

#[tokio::test]
async fn hours_report_export_preset_requests_the_resolved_dates_and_returns_path_and_total() {
    let server = MockServer::start().await;
    mount_report_range(
        &server,
        "2026-10-01",
        "2026-10-31",
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let dir = TempDir::new().unwrap();

    let exported = run_export(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Preset(hours_report::PeriodPreset::Month),
        hours_report::ReportFormat::Xlsx,
        Some(1),
    )
    .await
    .expect("export must succeed");

    server.verify().await;
    assert_eq!(
        exported.path,
        dir.path().join("hours-report-01-10-2026-31-10-2026.xlsx")
    );
    assert_eq!(exported.total, "1,5");
    assert!(exported.path.exists());
}

#[tokio::test]
async fn hours_report_export_csv_is_byte_identical_to_the_cli_file() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        2,
    )
    .await;
    let cli_dir = TempDir::new().unwrap();
    let export_dir = TempDir::new().unwrap();
    let range = hours_report::ReportPeriod::Range {
        start: "01-08-2026",
        end: "30-08-2026",
    };

    let run = run_time_report(&server, cli_dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;
    let exported = run_export(
        &server,
        export_dir.path(),
        range,
        hours_report::ReportFormat::Csv,
        Some(1),
    )
    .await
    .expect("export must succeed");

    assert_eq!(run.code, 0, "err: {}", run.err);
    let cli_bytes = std::fs::read(
        cli_dir
            .path()
            .join("hours-report-01-08-2026-30-08-2026.csv"),
    )
    .unwrap();
    assert_eq!(std::fs::read(&exported.path).unwrap(), cli_bytes);
}

#[tokio::test]
async fn hours_report_export_unauthorized_fails_without_a_file() {
    let server = MockServer::start().await;
    mount_report(&server, ResponseTemplate::new(401), 1).await;
    let dir = TempDir::new().unwrap();

    let result = run_export(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        },
        hours_report::ReportFormat::Xlsx,
        Some(1),
    )
    .await;

    server.verify().await;
    assert_eq!(result, Err(hours_report::ExportError::Unauthorized));
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn hours_report_export_server_error_names_the_status_without_a_file() {
    let server = MockServer::start().await;
    mount_report(&server, ResponseTemplate::new(500), 1).await;
    let dir = TempDir::new().unwrap();

    let result = run_export(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        },
        hours_report::ReportFormat::Xlsx,
        Some(1),
    )
    .await;

    server.verify().await;
    assert_eq!(result, Err(hours_report::ExportError::Http(500)));
    assert!(result.unwrap_err().to_string().contains("500"));
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn hours_report_export_without_a_user_id_fails_before_any_request() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        0,
    )
    .await;
    let dir = TempDir::new().unwrap();

    let result = run_export(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "01-08-2026",
            end: "30-08-2026",
        },
        hours_report::ReportFormat::Csv,
        None,
    )
    .await;

    server.verify().await;
    assert_eq!(result, Err(hours_report::ExportError::MissingUserId));
    assert!(dir_is_empty(dir.path()));
}

#[tokio::test]
async fn hours_report_export_bad_range_is_a_usage_failure_before_any_request() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        0,
    )
    .await;
    let dir = TempDir::new().unwrap();

    let result = run_export(
        &server,
        dir.path(),
        hours_report::ReportPeriod::Range {
            start: "30-08-2026",
            end: "01-08-2026",
        },
        hours_report::ReportFormat::Csv,
        Some(1),
    )
    .await;

    server.verify().await;
    assert!(
        matches!(result, Err(hours_report::ExportError::Usage(_))),
        "{result:?}"
    );
    assert!(dir_is_empty(dir.path()));
}

const ZONE_CHILD_MARKER: &str = "AC_HOURS_REPORT_ZONE_CHILD";

/// Re-runs itself in a child process whose zone is UTC+5 so the test process's
/// own environment is never mutated; the child performs the assertions.
#[test]
fn hours_report_hardening_reads_the_record_date_in_utc_whatever_the_zone() {
    if std::env::var_os(ZONE_CHILD_MARKER).is_some() {
        record_date_is_read_in_utc();
        return;
    }
    let test_name = format!(
        "{}::hours_report_hardening_reads_the_record_date_in_utc_whatever_the_zone",
        module_path!().split_once("::").map_or("", |(_, rest)| rest)
    );
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test_name, "--test-threads=1"])
        .env("TZ", "UTC-5")
        .env(ZONE_CHILD_MARKER, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child run failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child must actually run the test"
    );
}

fn record_date_is_read_in_utc() {
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE + 86_399, "summary": "", "parent_type": "Project", "parent_id": 5}
        ],
        "related": {}
    }));
    let csv = hours_report::render_hours_report_csv(&report);
    let xlsx_day = xlsx_date(&hours_report_xlsx_range(&report), 3, 3).date();

    assert!(hours_report_lines(&csv)[3].ends_with(";03/08/26"), "{csv}");
    assert_eq!(
        xlsx_day,
        chrono::NaiveDate::from_ymd_opt(2026, 8, 3).unwrap()
    );
}

fn xlsx_part(bytes: &[u8], name: &str) -> String {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let pattern = regex::Regex::new(&format!(r#"\b{name}="([^"]*)""#)).unwrap();
    pattern.captures(tag).map(|c| c[1].to_string())
}

/// Style index (`s`) of every cell in the sheet, keyed by its reference.
fn xlsx_cell_styles(sheet: &str) -> Vec<(String, usize)> {
    let cell = regex::Regex::new(r"<c [^>]*>").unwrap();
    cell.find_iter(sheet)
        .map(|m| {
            let tag = m.as_str();
            (
                xml_attr(tag, "r").unwrap(),
                xml_attr(tag, "s").map_or(0, |s| s.parse().unwrap()),
            )
        })
        .collect()
}

/// `(numFmtId, fontId)` of every cell format, in style-index order.
fn xlsx_cell_formats(styles: &str) -> Vec<(u32, usize)> {
    let block = regex::Regex::new(r"(?s)<cellXfs[^>]*>(.*?)</cellXfs>").unwrap();
    let xf = regex::Regex::new(r"<xf [^>]*>").unwrap();
    let inner = &block.captures(styles).unwrap()[1];
    xf.find_iter(inner)
        .map(|m| {
            (
                xml_attr(m.as_str(), "numFmtId").unwrap().parse().unwrap(),
                xml_attr(m.as_str(), "fontId").unwrap().parse().unwrap(),
            )
        })
        .collect()
}

fn xlsx_font_is_bold(styles: &str, font_id: usize) -> bool {
    let block = regex::Regex::new(r"(?s)<fonts[^>]*>(.*?)</fonts>").unwrap();
    let font = regex::Regex::new(r"(?s)<font>(.*?)</font>|<font/>").unwrap();
    let inner = &block.captures(styles).unwrap()[1];
    let bold = font
        .find_iter(inner)
        .nth(font_id)
        .is_some_and(|m| m.as_str().contains("<b/>"));
    bold
}

fn xlsx_format_code(styles: &str, num_fmt_id: u32) -> Option<String> {
    let fmt = regex::Regex::new(r"<numFmt [^>]*>").unwrap();
    let code = fmt
        .find_iter(styles)
        .find(|m| xml_attr(m.as_str(), "numFmtId") == Some(num_fmt_id.to_string()))
        .and_then(|m| xml_attr(m.as_str(), "formatCode"));
    code
}

#[test]
fn hours_report_xlsx_style_dates_use_the_short_format_and_the_header_is_bold() {
    let bytes = hours_report::render_hours_report_xlsx(&xlsx_fixture()).unwrap();
    let styles = xlsx_part(&bytes, "xl/styles.xml");
    let sheet = xlsx_part(&bytes, "xl/worksheets/sheet1.xml");
    let formats = xlsx_cell_formats(&styles);
    let cells = xlsx_cell_styles(&sheet);

    let date_cells: Vec<_> = cells
        .iter()
        .filter(|(r, _)| r.starts_with('D') && r != "D3")
        .collect();
    assert!(!date_cells.is_empty(), "the fixture must have date cells");
    for (reference, style) in date_cells {
        let (num_fmt, _) = formats[*style];
        assert_eq!(
            xlsx_format_code(&styles, num_fmt).as_deref(),
            Some("dd/mm/yy"),
            "{reference} must use the short date format"
        );
    }
    for (reference, style) in cells
        .iter()
        .filter(|(r, _)| r.trim_start_matches(|c: char| c.is_ascii_alphabetic()) == "3")
    {
        let (_, font) = formats[*style];
        assert!(
            xlsx_font_is_bold(&styles, font),
            "header cell {reference} must be bold"
        );
    }
}

fn atomic_report_run_dir() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("hours-report-01-08-2026-30-08-2026.csv");
    (dir, file)
}

#[tokio::test]
async fn hours_report_hardening_failed_write_keeps_the_existing_report_and_leaves_no_temp_file() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let (dir, file) = atomic_report_run_dir();
    std::fs::write(&file, "previous report").unwrap();
    std::fs::create_dir(
        dir.path()
            .join("hours-report-01-08-2026-30-08-2026.csv.tmp"),
    )
    .unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    assert_eq!(run.code, 1);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "previous report");
}

#[tokio::test]
async fn hours_report_hardening_failed_rename_cleans_the_temp_file() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let (dir, file) = atomic_report_run_dir();
    std::fs::create_dir(&file).unwrap();
    std::fs::write(file.join("keep"), "x").unwrap();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    assert_eq!(run.code, 1);
    assert_eq!(
        report_file_names(dir.path()),
        ["hours-report-01-08-2026-30-08-2026.csv"]
    );
}

#[tokio::test]
async fn hours_report_hardening_success_leaves_only_the_report() {
    let server = MockServer::start().await;
    mount_report(
        &server,
        ResponseTemplate::new(200).set_body_json(report_response()),
        1,
    )
    .await;
    let (dir, _file) = atomic_report_run_dir();

    let run = run_time_report(&server, dir.path(), "01-08-2026", "30-08-2026", Some(1)).await;

    assert_eq!(run.code, 0, "err: {}", run.err);
    assert_eq!(
        report_file_names(dir.path()),
        ["hours-report-01-08-2026-30-08-2026.csv"]
    );
}

#[test]
fn hours_report_hardening_related_key_as_array_or_null_parses_as_empty() {
    for empty in ["[]", "null"] {
        for key in ["Project", "Task"] {
            let other = if key == "Project" { "Task" } else { "Project" };
            let body = format!(
                r#"{{"time_records":[{{"id":1,"value":1.0,"record_date":1785715200,"summary":"","parent_type":"Project","parent_id":5}}],
                    "related":{{"{other}":{{}},"{key}":{empty}}}}}"#
            );
            let report: crate::models::TimeRecordsReport = serde_json::from_str(&body).unwrap();
            assert!(report.related.projects.is_empty() && report.related.tasks.is_empty());
        }
    }
    let report = hours_report_fixture(serde_json::json!({
        "time_records": [
            {"id": 1, "value": 1.0, "record_date": DAY_ONE, "summary": "", "parent_type": "Project", "parent_id": 5}
        ],
        "related": {"Project": {"5": {"id": 5, "name": "P"}}, "Task": []}
    }));
    let lines = hours_report_lines(&hours_report::render_hours_report_csv(&report));
    assert_eq!(lines[3], "P;;1;03/08/26");
}

#[test]
fn hours_report_hardening_non_empty_array_or_entity_without_id_is_a_parse_error() {
    for related in [
        r#"{"Project":[{"id":5,"name":"P"}]}"#,
        r#"{"Task":[{"id":1,"project_id":5,"name":"T"}]}"#,
        r#"{"Project":{"5":{"name":"P"}}}"#,
        r#"{"Task":{"1":{"project_id":5,"name":"T"}}}"#,
    ] {
        let body = format!(r#"{{"time_records":[],"related":{related}}}"#);
        assert!(
            serde_json::from_str::<crate::models::TimeRecordsReport>(&body).is_err(),
            "{body}"
        );
    }
}

fn resolved_period(preset: hours_report::PeriodPreset, today: (i32, u32, u32)) -> (String, String) {
    let today = chrono::NaiveDate::from_ymd_opt(today.0, today.1, today.2).unwrap();
    let (from, to) = hours_report::resolve_period(preset, today);
    (
        from.format("%d-%m-%Y").to_string(),
        to.format("%d-%m-%Y").to_string(),
    )
}

#[test]
fn hours_report_period_month_spans_the_whole_month() {
    use hours_report::PeriodPreset::Month;
    for (today, from, to) in [
        ((2026, 10, 30), "01-10-2026", "31-10-2026"),
        ((2026, 2, 10), "01-02-2026", "28-02-2026"),
        ((2028, 2, 10), "01-02-2028", "29-02-2028"),
        ((2026, 12, 31), "01-12-2026", "31-12-2026"),
        ((2026, 9, 1), "01-09-2026", "30-09-2026"),
    ] {
        assert_eq!(
            resolved_period(Month, today),
            (from.to_string(), to.to_string()),
            "today {today:?}"
        );
    }
}

#[test]
fn hours_report_period_fortnight_splits_at_the_fifteenth() {
    use hours_report::PeriodPreset::Fortnight;
    for (today, from, to) in [
        ((2026, 10, 1), "01-10-2026", "15-10-2026"),
        ((2026, 10, 15), "01-10-2026", "15-10-2026"),
        ((2026, 10, 16), "16-10-2026", "31-10-2026"),
        ((2026, 9, 20), "16-09-2026", "30-09-2026"),
        ((2026, 2, 28), "16-02-2026", "28-02-2026"),
        ((2026, 12, 31), "16-12-2026", "31-12-2026"),
    ] {
        assert_eq!(
            resolved_period(Fortnight, today),
            (from.to_string(), to.to_string()),
            "today {today:?}"
        );
    }
}

#[test]
fn hours_report_period_week_runs_monday_to_sunday() {
    use hours_report::PeriodPreset::Week;
    for (today, from, to) in [
        ((2026, 10, 26), "26-10-2026", "01-11-2026"),
        ((2026, 11, 1), "26-10-2026", "01-11-2026"),
        ((2026, 10, 30), "26-10-2026", "01-11-2026"),
        ((2026, 12, 31), "28-12-2026", "03-01-2027"),
    ] {
        assert_eq!(
            resolved_period(Week, today),
            (from.to_string(), to.to_string()),
            "today {today:?}"
        );
    }
}
