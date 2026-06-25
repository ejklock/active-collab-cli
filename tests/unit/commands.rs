use super::*;
use crate::config::Config;
use crate::i18n::set_language;
use crate::store::Store;
use std::sync::Mutex;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

    let result = load_task(&cache, &client, "inst", 10, 99, flags_refresh, false).await;
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

    let result = load_task(&cache, &client, "inst", 10, 99, false, false).await;
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

    let result = load_task(&cache, &client, "inst", 10, 99, false, false).await;
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

    let result = load_task(&cache, &client, "inst", 10, 99, true, false).await;
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

    let result = load_task(&cache, &client, "inst", 10, 99, false, true).await;
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
    }
}

#[tokio::test]
async fn do_get_task_json_mode_prints_pretty_json_and_returns_0() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 10, "name": "Test task" },
            "tracked_time": 0
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
        json: true,
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 0);
    let s = output_str(&out);
    // Pretty JSON has 2-space indent
    assert!(s.contains("\"single\""), "got: {s}");
    assert!(s.contains("  "), "should be pretty-printed: {s}");
}

#[tokio::test]
async fn do_get_task_json_mode_http_error_returns_1_with_message() {
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
    let flags = DisplayFlags {
        json: true,
        ..default_flags()
    };

    let code = do_get_task(&inst, &cache, &client, 5, 10, &flags, &mut out, &mut err).await;
    assert_eq!(code, 1);
    let e = output_str(&err);
    assert!(e.contains("not found"), "err: {e}");
    assert!(e.contains("5/10"), "err: {e}");
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

    let code = mine_core(&repo, &make_http(), None, false, &mut out, &mut err, |_| 0).await;

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

    let code = mine_core(
        &repo,
        &make_http(),
        Some("nosuch"),
        false,
        &mut out,
        &mut err,
        |_| 0,
    )
    .await;

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

    let code = mine_core(
        &repo,
        &make_http(),
        Some("work"),
        false,
        &mut out,
        &mut err,
        |_| 0,
    )
    .await;

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

    let code = mine_core(&repo, &make_http(), None, false, &mut out, &mut err, |_| 99).await;

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

    let code = mine_core(&repo, &make_http(), None, false, &mut out, &mut err, |_| 99).await;

    assert_eq!(code, 0);
    let s = output_str(&out);
    assert!(
        s.contains("No open tasks assigned to you"),
        "missing message: {s}"
    );
}

#[tokio::test]
async fn mine_core_tty_invokes_launch_closure_with_rows_not_table() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tasks_response(serde_json::json!([
            { "id": 5, "task_number": 2, "name": "TUI task", "is_completed": false, "is_trashed": false, "project_id": 3 }
        ]))))
        .mount(&server)
        .await;

    let (_dir, store) = make_store();
    let inst = Instance {
        name: "tui-inst".to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    };
    InstanceRepository::new(store.conn()).save(&inst).unwrap();

    let repo = InstanceRepository::new(store.conn());
    let mut out = Vec::new();
    let mut err = Vec::new();
    let received_rows = std::cell::Cell::new(0usize);

    let code = mine_core(
        &repo,
        &make_http(),
        None,
        true,
        &mut out,
        &mut err,
        |rows| {
            received_rows.set(rows.len());
            42
        },
    )
    .await;

    assert_eq!(code, 42, "should return launch closure exit code");
    assert_eq!(received_rows.get(), 1, "launch should receive 1 row");
    let s = output_str(&out);
    assert!(
        !s.contains("INSTANCE"),
        "table must NOT be printed on TTY path: {s}"
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
