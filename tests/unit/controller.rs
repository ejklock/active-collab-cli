use super::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN_HEADER: &str = "x-angie-authapitoken";

fn make_instance(name: &str, base_url: &str, user_id: Option<i64>) -> Instance {
    Instance {
        name: name.to_string(),
        base_url: base_url.to_string(),
        email: format!("{name}@example.com"),
        token: format!("tok-{name}"),
        user_id,
    }
}

fn make_http() -> Http {
    Http::new().unwrap()
}

#[tokio::test]
async fn tasks_by_project_aggregates_across_two_instances() {
    let server1 = MockServer::start().await;
    let server2 = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 10, "task_number": 1, "name": "Alpha", "project_id": 100,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 100, "name": "Acme Project" }
        ])))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 20, "task_number": 2, "name": "Beta", "project_id": 200,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server2)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 200, "name": "Zeta Project" }
        ])))
        .mount(&server2)
        .await;

    let inst1 = make_instance("inst1", &server1.uri(), Some(1));
    let inst2 = make_instance("inst2", &server2.uri(), Some(2));
    let http = make_http();

    let groups = tasks_by_project(&[inst1, inst2], &http).await;

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].project_name, "Acme Project");
    assert_eq!(groups[1].project_name, "Zeta Project");
    assert_eq!(groups[0].tasks[0].name, "Alpha");
    assert_eq!(groups[1].tasks[0].name, "Beta");
    assert_eq!(groups[0].tasks[0].instance, "inst1");
    assert_eq!(groups[1].tasks[0].instance, "inst2");
}

#[tokio::test]
async fn tasks_by_project_groups_sorted_alphabetically() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/7/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 1, "task_number": 1, "name": "Task Z", "project_id": 2,
                  "is_completed": false, "is_trashed": false },
                { "id": 2, "task_number": 2, "name": "Task A", "project_id": 1,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "name": "Beta" },
            { "id": 2, "name": "Alpha" }
        ])))
        .mount(&server)
        .await;

    let inst = make_instance("inst", &server.uri(), Some(7));
    let http = make_http();
    let groups = tasks_by_project(&[inst], &http).await;

    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].project_name, "Alpha");
    assert_eq!(groups[1].project_name, "Beta");
}

#[tokio::test]
async fn tasks_by_project_falls_back_to_numeric_id_when_list_projects_fails() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/5/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 1, "task_number": 1, "name": "My Task", "project_id": 42,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server)
        .await;

    let inst = make_instance("inst", &server.uri(), Some(5));
    let http = make_http();
    let groups = tasks_by_project(&[inst], &http).await;

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project_name, "42");
    assert_eq!(groups[0].tasks[0].name, "My Task");
}

#[tokio::test]
async fn tasks_by_project_empty_when_no_tasks() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/3/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "tasks": [] })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let inst = make_instance("inst", &server.uri(), Some(3));
    let http = make_http();
    let groups = tasks_by_project(&[inst], &http).await;

    assert!(groups.is_empty());
}

#[tokio::test]
async fn tasks_within_a_group_sorted_by_task_number() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/9/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 3, "task_number": 3, "name": "C", "project_id": 1,
                  "is_completed": false, "is_trashed": false },
                { "id": 1, "task_number": 1, "name": "A", "project_id": 1,
                  "is_completed": false, "is_trashed": false },
                { "id": 2, "task_number": 2, "name": "B", "project_id": 1,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "name": "Proj" }
        ])))
        .mount(&server)
        .await;

    let inst = make_instance("inst", &server.uri(), Some(9));
    let http = make_http();
    let groups = tasks_by_project(&[inst], &http).await;

    assert_eq!(groups.len(), 1);
    let task_names: Vec<&str> = groups[0].tasks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(task_names, vec!["A", "B", "C"]);
}

#[test]
fn build_groups_uses_numeric_fallback_for_unknown_project() {
    let task = MineTask {
        id: 1,
        task_number: Some(1),
        name: "X".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(999),
        instance_name: "inst".into(),
    };
    let groups = build_groups(vec![(task, "inst".into())], &HashMap::new());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project_name, "999");
}

fn make_store() -> (tempfile::TempDir, crate::store::Store) {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let config = crate::config::Config {
        db_path,
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    (dir, store)
}

fn make_task_payload() -> serde_json::Value {
    serde_json::json!({
        "single": {
            "id": 99,
            "task_number": 7,
            "name": "Do work",
            "is_completed": false,
            "project_id": 10,
            "assignee_id": null
        },
        "tracked_time": 1.5,
        "comments": [
            { "id": 1, "created_by_name": "Alice", "body_plain_text": "LGTM" }
        ]
    })
}

#[tokio::test]
async fn task_detail_refresh_false_serves_cache_hit_without_task_fetch() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let cache = crate::store::cache::TaskCache::new(store.conn());
    let task_fields = serde_json::json!({
        "id": 99, "name": "Cached task", "project_id": 10
    });
    let comments_val = serde_json::json!([
        { "created_by_name": "Bob", "body_plain_text": "cached comment" }
    ]);
    cache
        .write("inst", 10, 99, &task_fields, &comments_val)
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;

    assert_eq!(detail.task["name"], "Cached task");
    assert_eq!(detail.comments.len(), 1);
    assert_eq!(detail.comments[0]["created_by_name"], "Bob");

    let reqs = server.received_requests().await.unwrap();
    let task_fetches: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path().contains("/tasks/"))
        .collect();
    assert!(
        task_fetches.is_empty(),
        "cache hit must not fetch the task from the API: got {task_fetches:?}"
    );
}

#[tokio::test]
async fn task_detail_cache_hit_resolves_assignee_name() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let cache = crate::store::cache::TaskCache::new(store.conn());
    let task_fields = serde_json::json!({
        "id": 99, "name": "Cached task", "project_id": 10, "assignee_id": 7
    });
    cache
        .write("inst", 10, 99, &task_fields, &serde_json::json!([]))
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 7, "display_name": "Carol Jones" }
        ])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;

    assert_eq!(detail.task["name"], "Cached task");
    assert_eq!(
        detail.user_map.get(&7).map(|s| s.as_str()),
        Some("Carol Jones"),
        "cache-hit path must enrich assignee_name from user map"
    );

    let reqs = server.received_requests().await.unwrap();
    let task_fetches: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path().contains("/tasks/"))
        .collect();
    assert!(
        task_fetches.is_empty(),
        "cache hit must NOT fetch the task from the API"
    );
}

#[tokio::test]
async fn task_detail_refresh_true_always_fetches_network() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let cache = crate::store::cache::TaskCache::new(store.conn());
    let stale = serde_json::json!({ "id": 99, "name": "Stale task" });
    cache
        .write("inst", 10, 99, &stale, &serde_json::json!([]))
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_task_payload()))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, true).await;
    assert_eq!(detail.task["name"], "Do work");
    server.verify().await;
}

#[tokio::test]
async fn task_detail_cache_miss_fetches_and_writes_cache() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_task_payload()))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;
    assert_eq!(detail.task["name"], "Do work");
    assert_eq!(detail.comments.len(), 1);

    let cache = crate::store::cache::TaskCache::new(store.conn());
    let cached = cache.read("inst", 10, 99).unwrap();
    assert!(
        cached.is_some(),
        "task must be written to cache after fetch"
    );
    server.verify().await;
}

#[tokio::test]
async fn task_detail_enriches_assignee_name_from_user_map() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": { "id": 99, "name": "Task", "assignee_id": 5 },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 5, "display_name": "Alice Smith" }
        ])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;
    assert_eq!(
        detail.user_map.get(&5).map(|s| s.as_str()),
        Some("Alice Smith")
    );
}

#[tokio::test]
async fn task_detail_user_map_failure_is_graceful() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_task_payload()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;
    assert!(
        detail.user_map.is_empty(),
        "user map failure must yield empty map"
    );
}

#[tokio::test]
async fn task_detail_extracts_assets_from_task_body() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Task",
                "body": r#"<a href="https://example.com/file.pdf">link</a>"#
            },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;
    assert_eq!(detail.assets.len(), 1);
    assert_eq!(detail.assets[0].url, "https://example.com/file.pdf");
}

#[tokio::test]
async fn download_asset_attaches_token_when_same_host() {
    let server = MockServer::start().await;
    let inst = make_instance("inst", &server.uri(), None);
    let http = make_http();

    let asset_path = "/files/doc.pdf";
    Mock::given(method("GET"))
        .and(path(asset_path))
        .and(wiremock::matchers::header(TOKEN_HEADER, "tok-inst"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"pdf-bytes".to_vec()))
        .expect(1)
        .mount(&server)
        .await;

    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("doc.pdf");
    let url = format!("{}{}", server.uri(), asset_path);
    download_asset(&http, &inst, &url, &dest).await.unwrap();

    assert_eq!(std::fs::read(&dest).unwrap(), b"pdf-bytes");
    server.verify().await;
}

#[tokio::test]
async fn download_asset_no_token_when_different_host() {
    let asset_server = MockServer::start().await;
    let inst = make_instance("inst", "https://my-instance.example.com", None);
    let http = make_http();

    let asset_path = "/cdn/file.pdf";
    Mock::given(method("GET"))
        .and(path(asset_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file".to_vec()))
        .expect(1)
        .mount(&asset_server)
        .await;

    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("file.pdf");
    let url = format!("{}{}", asset_server.uri(), asset_path);
    download_asset(&http, &inst, &url, &dest).await.unwrap();

    let reqs = asset_server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(
        reqs[0].headers.get(TOKEN_HEADER).is_none(),
        "token must NOT be attached to a foreign-host download"
    );
    asset_server.verify().await;
}

#[tokio::test]
async fn download_asset_non_200_returns_err() {
    let server = MockServer::start().await;
    let inst = make_instance("inst", &server.uri(), None);
    let http = make_http();

    Mock::given(method("GET"))
        .and(path("/files/missing.pdf"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let tmp = tempfile::TempDir::new().unwrap();
    let dest = tmp.path().join("missing.pdf");
    let url = format!("{}/files/missing.pdf", server.uri());
    let result = download_asset(&http, &inst, &url, &dest).await;
    assert!(result.is_err(), "non-200 must return Err");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("404"), "error must mention the status: {err}");
}

#[test]
fn open_asset_http_returns_ok() {
    assert!(open_asset("http://example.com/file.pdf").is_ok());
}

#[test]
fn open_asset_https_returns_ok() {
    assert!(open_asset("https://example.com/doc").is_ok());
}

#[test]
fn open_asset_file_scheme_returns_err() {
    let result = open_asset("file:///etc/passwd");
    assert!(result.is_err(), "file:// must be rejected");
}

#[test]
fn open_asset_javascript_scheme_returns_err() {
    let result = open_asset("javascript:alert(1)");
    assert!(result.is_err(), "javascript: must be rejected");
}

#[test]
fn open_asset_empty_url_returns_err() {
    let result = open_asset("");
    assert!(result.is_err(), "empty url must be rejected");
}
