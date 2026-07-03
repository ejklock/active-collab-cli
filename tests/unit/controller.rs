use super::*;
use crate::store::cache::{ProjectNamesCache, UserMapCache};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    let (_dir, db_path) = make_db_path();

    let groups = tasks_by_project(db_path, &[inst1, inst2], &http).await;

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
    let (_dir, db_path) = make_db_path();
    let groups = tasks_by_project(db_path, &[inst], &http).await;

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
    let (_dir, db_path) = make_db_path();
    let groups = tasks_by_project(db_path, &[inst], &http).await;

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
    let (_dir, db_path) = make_db_path();
    let groups = tasks_by_project(db_path, &[inst], &http).await;

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
    let (_dir, db_path) = make_db_path();
    let groups = tasks_by_project(db_path, &[inst], &http).await;

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
        due_on: None,
        instance_name: "inst".into(),
    };
    let groups = build_groups(vec![(task, "inst".into())], &HashMap::new());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project_name, "999");
    assert_eq!(groups[0].instance, "inst");
}

#[test]
fn build_groups_same_project_id_on_two_instances_produces_two_groups() {
    let task_a = MineTask {
        id: 10,
        task_number: Some(1),
        name: "Task A".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(100),
        due_on: None,
        instance_name: "alpha".into(),
    };
    let task_b = MineTask {
        id: 20,
        task_number: Some(2),
        name: "Task B".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(100),
        due_on: None,
        instance_name: "beta".into(),
    };
    let mut names: HashMap<(String, i64), String> = HashMap::new();
    names.insert(("alpha".to_string(), 100i64), "Shared Project".to_string());
    names.insert(("beta".to_string(), 100i64), "Shared Project".to_string());

    let groups = build_groups(
        vec![(task_a, "alpha".into()), (task_b, "beta".into())],
        &names,
    );

    assert_eq!(
        groups.len(),
        2,
        "same project_id on two instances must produce two separate groups"
    );

    let alpha = groups
        .iter()
        .find(|g| g.instance == "alpha")
        .expect("alpha group must exist");
    let beta = groups
        .iter()
        .find(|g| g.instance == "beta")
        .expect("beta group must exist");

    assert_eq!(alpha.project_name, "Shared Project");
    assert_eq!(alpha.tasks.len(), 1);
    assert_eq!(alpha.tasks[0].name, "Task A");

    assert_eq!(beta.project_name, "Shared Project");
    assert_eq!(beta.tasks.len(), 1);
    assert_eq!(beta.tasks[0].name, "Task B");
}

#[test]
fn build_groups_instance_field_matches_task_instance() {
    let task = MineTask {
        id: 5,
        task_number: Some(5),
        name: "My Task".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(42),
        due_on: None,
        instance_name: "prod".into(),
    };
    let mut names: HashMap<(String, i64), String> = HashMap::new();
    names.insert(
        ("prod".to_string(), 42i64),
        "Production Project".to_string(),
    );

    let groups = build_groups(vec![(task, "prod".into())], &names);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].instance, "prod");
    assert_eq!(groups[0].project_name, "Production Project");
}

#[test]
fn build_groups_sorted_by_project_name_then_instance() {
    let task_b_z = MineTask {
        id: 1,
        task_number: Some(1),
        name: "T1".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(1),
        due_on: None,
        instance_name: "z-inst".into(),
    };
    let task_b_a = MineTask {
        id: 2,
        task_number: Some(2),
        name: "T2".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(1),
        due_on: None,
        instance_name: "a-inst".into(),
    };
    let task_a = MineTask {
        id: 3,
        task_number: Some(3),
        name: "T3".into(),
        is_completed: false,
        is_trashed: false,
        project_id: Some(2),
        due_on: None,
        instance_name: "m-inst".into(),
    };
    let mut names: HashMap<(String, i64), String> = HashMap::new();
    names.insert(("z-inst".to_string(), 1i64), "Beta Project".to_string());
    names.insert(("a-inst".to_string(), 1i64), "Beta Project".to_string());
    names.insert(("m-inst".to_string(), 2i64), "Alpha Project".to_string());

    let groups = build_groups(
        vec![
            (task_b_z, "z-inst".into()),
            (task_b_a, "a-inst".into()),
            (task_a, "m-inst".into()),
        ],
        &names,
    );

    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0].project_name, "Alpha Project");
    assert_eq!(groups[1].project_name, "Beta Project");
    assert_eq!(
        groups[1].instance, "a-inst",
        "same project_name sorted by instance asc"
    );
    assert_eq!(groups[2].project_name, "Beta Project");
    assert_eq!(groups[2].instance, "z-inst");
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
    // anchor text "link" is non-empty and different from the URL → anchor text wins
    assert_eq!(detail.assets[0].name, "link");
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

// S5.1 tests: load_task_core, cached_user_map, refresh_user_map

fn make_db_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    (dir, db_path)
}

fn seed_task_cache(db_path: &std::path::Path, inst_name: &str, task: &serde_json::Value) {
    let config = crate::config::Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    crate::store::cache::TaskCache::new(store.conn())
        .write(inst_name, 10, 99, task, &serde_json::json!([]))
        .unwrap();
}

fn seed_user_map_cache(
    db_path: &std::path::Path,
    inst_name: &str,
    users: &std::collections::HashMap<i64, String>,
) {
    let config = crate::config::Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    crate::store::cache::UserMapCache::new(store.conn())
        .write(inst_name, users)
        .unwrap();
}

struct CoreTestFixture {
    _dir: tempfile::TempDir,
    db_path: std::path::PathBuf,
    inst: Instance,
    http: Http,
}

impl CoreTestFixture {
    async fn with_server(server: &wiremock::MockServer) -> Self {
        let (_dir, db_path) = make_db_path();
        let inst = make_instance("inst", &server.uri(), Some(1));
        let http = make_http();
        CoreTestFixture {
            _dir,
            db_path,
            inst,
            http,
        }
    }

    fn without_server() -> Self {
        let (_dir, db_path) = make_db_path();
        let inst = make_instance("inst", "http://localhost", Some(1));
        let http = make_http();
        CoreTestFixture {
            _dir,
            db_path,
            inst,
            http,
        }
    }
}

async fn assert_no_user_fetches(server: &wiremock::MockServer, context: &str) {
    let reqs = server.received_requests().await.unwrap();
    let user_fetches: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path() == "/api/v1/users")
        .collect();
    assert!(user_fetches.is_empty(), "{context}: got {user_fetches:?}");
}

#[tokio::test]
async fn load_task_core_returns_task_and_comments_without_user_fetch() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Core Task",
                "assignee_id": 42,
                "project_id": 10,
                "body": "Task body text"
            },
            "tracked_time": null,
            "comments": [
                { "id": 1, "created_by_name": "Alice", "body_plain_text": "Great work" }
            ]
        })))
        .mount(&server)
        .await;

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;

    assert_eq!(core.task["name"], "Core Task");
    assert_eq!(core.comments.len(), 1);
    assert_eq!(core.comments[0]["created_by_name"], "Alice");
    assert_no_user_fetches(&server, "load_task_core must not fetch users").await;
}

#[tokio::test]
async fn load_task_core_assignee_line_shows_id_when_user_map_empty() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Assigned Task",
                "assignee_id": 7,
                "project_id": 10
            },
            "tracked_time": null,
            "comments": [
                { "id": 1, "created_by_name": "Bob", "body_plain_text": "comment text" }
            ]
        })))
        .mount(&server)
        .await;

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;

    let empty_map = std::collections::HashMap::new();
    let lines =
        crate::render::build_detail_content(&core.task, &core.comments, &empty_map, 80, None).lines;

    let assignee_line = lines
        .iter()
        .find(|l| l.contains("Assignee"))
        .cloned()
        .unwrap_or_default();
    assert!(
        assignee_line.contains("(7)"),
        "assignee line must show '(id)' when user_map is empty, got: {assignee_line:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("comment text")),
        "comments must render fully even without user_map"
    );
    // The task name appears in the Título meta row inside the Details panel.
    assert!(
        lines.iter().any(|l| l.contains("Assigned Task")),
        "task name must appear in the Title meta row of the Details panel: {lines:?}"
    );
}

#[tokio::test]
async fn load_task_core_extracts_assets() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Task with asset",
                "body": r#"<a href="https://files.example.com/doc.pdf">doc</a>"#
            },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;

    assert_eq!(core.assets.len(), 1);
    assert_eq!(core.assets[0].url, "https://files.example.com/doc.pdf");
    // anchor text "doc" is non-empty and different from the URL → anchor text wins
    assert_eq!(core.assets[0].name, "doc");
}

#[tokio::test]
async fn cached_user_map_returns_none_when_cache_empty() {
    let fix = CoreTestFixture::without_server();

    let result = cached_user_map(&fix.db_path, &fix.inst);
    assert!(
        result.is_none(),
        "cold cache must return None, got: {result:?}"
    );
}

#[tokio::test]
async fn cached_user_map_returns_some_when_cache_populated() {
    let fix = CoreTestFixture::without_server();

    let users: std::collections::HashMap<i64, String> =
        [(5i64, "Eve".to_string())].into_iter().collect();
    seed_user_map_cache(&fix.db_path, "inst", &users);

    let result = cached_user_map(&fix.db_path, &fix.inst);
    assert!(result.is_some(), "populated cache must return Some");
    assert_eq!(result.unwrap().get(&5).map(|s| s.as_str()), Some("Eve"));
}

#[tokio::test]
async fn refresh_user_map_fetches_from_network_and_writes_cache() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 9, "display_name": "Frank" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let map = refresh_user_map(fix.db_path.clone(), fix.inst.clone(), fix.http).await;
    assert_eq!(map.get(&9).map(|s| s.as_str()), Some("Frank"));

    let cached = cached_user_map(&fix.db_path, &fix.inst);
    assert!(cached.is_some(), "refresh_user_map must write to cache");
    assert_eq!(
        cached.unwrap().get(&9).map(|s| s.as_str()),
        Some("Frank"),
        "cached value must match fetched value"
    );

    server.verify().await;
}

#[tokio::test]
async fn cached_user_map_present_means_no_refresh_needed() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    let users: std::collections::HashMap<i64, String> =
        [(3i64, "Grace".to_string())].into_iter().collect();
    seed_user_map_cache(&fix.db_path, "inst", &users);

    assert!(
        cached_user_map(&fix.db_path, &fix.inst).is_some(),
        "cached map present means no user-directory network call is needed"
    );
    assert_no_user_fetches(&server, "cached_user_map must not make network calls").await;
}

#[tokio::test]
async fn load_task_core_served_from_cache_makes_no_task_fetch() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    let task_fields = serde_json::json!({
        "id": 99,
        "name": "Cached Core Task",
        "project_id": 10
    });
    seed_task_cache(&fix.db_path, "inst", &task_fields);

    let names: std::collections::HashMap<i64, String> = [(10i64, "Cached Project".to_string())]
        .into_iter()
        .collect();
    seed_project_names_cache(&fix.db_path, "inst", &names);

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;
    assert_eq!(core.task["name"], "Cached Core Task");

    let reqs = server.received_requests().await.unwrap();
    assert!(
        reqs.is_empty(),
        "cache hit must make zero network calls: got {reqs:?}"
    );
}

// D1a-A3: load_task_core injects project_name from cache → Projeto row shows resolved name.
#[tokio::test]
async fn load_task_core_enriches_task_with_project_name_from_cache() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Work item",
                "project_id": 10
            },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    let names: std::collections::HashMap<i64, String> =
        [(10i64, "Acme Corp".to_string())].into_iter().collect();
    seed_project_names_cache(&fix.db_path, "inst", &names);

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;

    assert_eq!(
        core.task["project_name"].as_str(),
        Some("Acme Corp"),
        "project_name must be injected from cache: {:?}",
        core.task
    );

    let empty_map = std::collections::HashMap::new();
    let lines = crate::render::build_detail_content(&core.task, &[], &empty_map, 80, None).lines;
    let project_line = lines
        .iter()
        .find(|l| l.contains("Project"))
        .cloned()
        .unwrap_or_default();
    assert!(
        project_line.contains("Acme Corp"),
        "Projeto row must show resolved project name: {project_line:?}"
    );

    assert_no_project_fetches(
        &server,
        10,
        "warm cache hit must issue zero projects/{id} requests (ADR 0014 guarantee)",
    )
    .await;
}

async fn assert_no_project_fetches(server: &wiremock::MockServer, project_id: i64, context: &str) {
    let reqs = server.received_requests().await.unwrap();
    let expected_path = format!("/api/v1/projects/{project_id}");
    let hits: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path() == expected_path)
        .collect();
    assert!(hits.is_empty(), "{context}: got {hits:?}");
}

// D1a-A5 / AC2: on a cache miss, when the server names the project, load_task_core
// resolves the name over the network and issues exactly one request.
#[tokio::test]
async fn load_task_core_resolves_project_name_on_cache_miss() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Work item",
                "project_id": 10
            },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10,
            "name": "Base · Sustentação"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;

    assert_eq!(
        core.task["project_name"].as_str(),
        Some("Base · Sustentação"),
        "project_name must be resolved over the network on a cache miss: {:?}",
        core.task
    );
    server.verify().await;
}

// D1a-A6 / AC3: after a miss resolves and writes back, a second load_task_core for
// the same project id serves the name from cache with zero further requests.
#[tokio::test]
async fn load_task_core_caches_resolved_project_name() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Work item",
                "project_id": 10
            },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10,
            "name": "Base · Sustentação"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let first = load_task_core(
        fix.db_path.clone(),
        fix.inst.clone(),
        fix.http.clone(),
        10,
        99,
        false,
    )
    .await;
    assert_eq!(
        first.task["project_name"].as_str(),
        Some("Base · Sustentação")
    );

    let second = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;
    assert_eq!(
        second.task["project_name"].as_str(),
        Some("Base · Sustentação"),
        "second load must still show the resolved name from cache"
    );

    server.verify().await;
}

// D1a-A4 / AC5: on a cache miss where the server yields no usable name (non-200),
// load_task_core injects the fallback → Projeto row never blank.
#[tokio::test]
async fn load_task_core_project_name_fallback_when_cache_miss() {
    let server = MockServer::start().await;
    let fix = CoreTestFixture::with_server(&server).await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "single": {
                "id": 99,
                "name": "Work item",
                "project_id": 10
            },
            "tracked_time": null,
            "comments": []
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let core = load_task_core(fix.db_path, fix.inst, fix.http, 10, 99, false).await;

    let project_name = core.task["project_name"].as_str().unwrap_or("");
    assert!(
        !project_name.is_empty(),
        "project_name must never be blank on cache miss: got {:?}",
        core.task
    );

    let empty_map = std::collections::HashMap::new();
    let lines = crate::render::build_detail_content(&core.task, &[], &empty_map, 80, None).lines;
    let project_line = lines
        .iter()
        .find(|l| l.contains("Project"))
        .cloned()
        .unwrap_or_default();
    assert!(
        !project_line.trim_end().ends_with("Project"),
        "Project row value must not be blank: {project_line:?}"
    );
    server.verify().await;
}

// D1a-A3: project_name_from_cache returns the cached name when fresh.
#[test]
fn project_name_from_cache_returns_cached_name() {
    let fix = CoreTestFixture::without_server();

    let names: std::collections::HashMap<i64, String> =
        [(42i64, "Test Project".to_string())].into_iter().collect();
    seed_project_names_cache(&fix.db_path, "inst", &names);

    let result = project_name_from_cache(&fix.db_path, "inst", 42);
    assert_eq!(result, "Test Project");
}

// D1a-A4: project_name_from_cache returns fallback on miss.
#[test]
fn project_name_from_cache_returns_fallback_on_miss() {
    let fix = CoreTestFixture::without_server();

    let result = project_name_from_cache(&fix.db_path, "inst", 999);
    assert!(
        !result.is_empty(),
        "fallback must not be empty on cache miss: {result:?}"
    );
    assert_ne!(result, "", "fallback must not be an empty string");
}

// S5-A1: pre-populated user_map_cache → zero GET /api/v1/users calls on detail open
#[tokio::test]
async fn task_detail_with_cached_user_map_makes_no_users_request() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let task_fields = serde_json::json!({ "id": 99, "name": "Task", "project_id": 10 });
    crate::store::cache::TaskCache::new(store.conn())
        .write("inst", 10, 99, &task_fields, &serde_json::json!([]))
        .unwrap();

    let user_map: std::collections::HashMap<i64, String> = [(7i64, "Pre-Cached User".to_string())]
        .into_iter()
        .collect();
    UserMapCache::new(store.conn())
        .write("inst", &user_map)
        .unwrap();

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;

    assert_eq!(
        detail.user_map.get(&7).map(|s| s.as_str()),
        Some("Pre-Cached User")
    );

    let reqs = server.received_requests().await.unwrap();
    let user_fetches: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path() == "/api/v1/users")
        .collect();
    assert!(
        user_fetches.is_empty(),
        "pre-populated user_map_cache must not trigger GET /api/v1/users: got {user_fetches:?}"
    );
}

// S5-A2: empty user_map_cache → first open fetches once and writes; second open hits cache
#[tokio::test]
async fn task_detail_empty_user_cache_fetches_once_then_serves_from_cache() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let task_fields = serde_json::json!({ "id": 99, "name": "Task", "project_id": 10 });
    crate::store::cache::TaskCache::new(store.conn())
        .write("inst", 10, 99, &task_fields, &serde_json::json!([]))
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 42, "display_name": "Network User" }
        ])))
        .mount(&server)
        .await;

    // First open: cache miss → network fetch
    let detail1 = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;
    assert_eq!(
        detail1.user_map.get(&42).map(|s| s.as_str()),
        Some("Network User")
    );

    let reqs_after_first = server.received_requests().await.unwrap();
    let user_hits_first: Vec<_> = reqs_after_first
        .iter()
        .filter(|r| r.url.path() == "/api/v1/users")
        .collect();
    assert_eq!(
        user_hits_first.len(),
        1,
        "first open must fetch users exactly once"
    );

    // Second open: user_map_cache populated → no further network calls
    let detail2 = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, false).await;
    assert_eq!(
        detail2.user_map.get(&42).map(|s| s.as_str()),
        Some("Network User")
    );

    let reqs_after_second = server.received_requests().await.unwrap();
    let user_hits_second: Vec<_> = reqs_after_second
        .iter()
        .filter(|r| r.url.path() == "/api/v1/users")
        .collect();
    assert_eq!(
        user_hits_second.len(),
        1,
        "second open must serve from cache — still only one users request total"
    );
}

// BDR-0008 Scenario 6: two instances both expose project_id=N with DIFFERENT names —
// each group must show ITS OWN name; the last-joined instance must not clobber the first.
#[tokio::test]
async fn tasks_by_project_colliding_project_ids_show_per_instance_names() {
    let server1 = MockServer::start().await;
    let server2 = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 10, "task_number": 1, "name": "Task from Inst1", "project_id": 42,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 42, "name": "Inst1 Project Name" }
        ])))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 20, "task_number": 2, "name": "Task from Inst2", "project_id": 42,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server2)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 42, "name": "Inst2 Project Name" }
        ])))
        .mount(&server2)
        .await;

    let inst1 = make_instance("inst1", &server1.uri(), Some(1));
    let inst2 = make_instance("inst2", &server2.uri(), Some(2));
    let http = make_http();
    let (_dir, db_path) = make_db_path();

    let groups = tasks_by_project(db_path, &[inst1, inst2], &http).await;

    assert_eq!(
        groups.len(),
        2,
        "colliding project_id must produce two groups"
    );

    let g1 = groups
        .iter()
        .find(|g| g.instance == "inst1")
        .expect("group for inst1 must exist");
    let g2 = groups
        .iter()
        .find(|g| g.instance == "inst2")
        .expect("group for inst2 must exist");

    assert_eq!(
        g1.project_name, "Inst1 Project Name",
        "inst1 group must show inst1's project name, not inst2's"
    );
    assert_eq!(
        g2.project_name, "Inst2 Project Name",
        "inst2 group must show inst2's project name, not inst1's"
    );

    assert_eq!(g1.tasks[0].name, "Task from Inst1");
    assert_eq!(g2.tasks[0].name, "Task from Inst2");
}

// S5.3 tests: concurrent tasks_by_project

#[tokio::test]
async fn tasks_by_project_concurrent_aggregates_across_two_instances() {
    let server1 = MockServer::start().await;
    let server2 = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 10, "task_number": 1, "name": "Concurrent Alpha", "project_id": 100,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 100, "name": "Project Gamma" }
        ])))
        .mount(&server1)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 20, "task_number": 2, "name": "Concurrent Beta", "project_id": 200,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server2)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 200, "name": "Project Delta" }
        ])))
        .mount(&server2)
        .await;

    let inst1 = make_instance("c-inst1", &server1.uri(), Some(1));
    let inst2 = make_instance("c-inst2", &server2.uri(), Some(2));
    let http = make_http();
    let (_dir, db_path) = make_db_path();

    let groups = tasks_by_project(db_path, &[inst1, inst2], &http).await;

    assert_eq!(groups.len(), 2, "must aggregate tasks from both instances");

    let project_names: Vec<&str> = groups.iter().map(|g| g.project_name.as_str()).collect();
    assert!(
        project_names.contains(&"Project Gamma"),
        "Project Gamma missing from groups: {project_names:?}"
    );
    assert!(
        project_names.contains(&"Project Delta"),
        "Project Delta missing from groups: {project_names:?}"
    );

    let alpha_group = groups
        .iter()
        .find(|g| g.project_name == "Project Gamma")
        .unwrap();
    assert_eq!(alpha_group.tasks[0].name, "Concurrent Alpha");
    assert_eq!(alpha_group.tasks[0].instance, "c-inst1");

    let beta_group = groups
        .iter()
        .find(|g| g.project_name == "Project Delta")
        .unwrap();
    assert_eq!(beta_group.tasks[0].name, "Concurrent Beta");
    assert_eq!(beta_group.tasks[0].instance, "c-inst2");
}

#[tokio::test]
async fn tasks_by_project_failing_instance_excluded_other_still_present() {
    let server_ok = MockServer::start().await;
    let server_err = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 5, "task_number": 5, "name": "Survivor Task", "project_id": 10,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server_ok)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 10, "name": "Survivor Project" }
        ])))
        .mount(&server_ok)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users/2/tasks"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server_err)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .mount(&server_err)
        .await;

    let inst_ok = make_instance("ok-inst", &server_ok.uri(), Some(1));
    let inst_err = make_instance("err-inst", &server_err.uri(), Some(2));
    let http = make_http();
    let (_dir, db_path) = make_db_path();

    let groups = tasks_by_project(db_path, &[inst_ok, inst_err], &http).await;

    assert_eq!(
        groups.len(),
        1,
        "failing instance must contribute no groups"
    );
    assert_eq!(groups[0].project_name, "Survivor Project");
    assert_eq!(groups[0].tasks[0].name, "Survivor Task");
    assert_eq!(groups[0].tasks[0].instance, "ok-inst");
}

// S5-A3: refresh=true bypasses user_map_cache and writes fresh result back
#[tokio::test]
async fn task_detail_refresh_true_bypasses_user_map_cache() {
    let server = MockServer::start().await;
    let (_dir, store) = make_store();
    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let task_fields = serde_json::json!({ "id": 99, "name": "Task", "project_id": 10 });
    crate::store::cache::TaskCache::new(store.conn())
        .write("inst", 10, 99, &task_fields, &serde_json::json!([]))
        .unwrap();

    let stale_map: std::collections::HashMap<i64, String> =
        [(7i64, "Stale User".to_string())].into_iter().collect();
    UserMapCache::new(store.conn())
        .write("inst", &stale_map)
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects/10/tasks/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(make_task_payload()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 7, "display_name": "Fresh User" }
        ])))
        .mount(&server)
        .await;

    let detail = task_detail_with_conn(store.conn(), &inst, &http, 10, 99, true).await;

    assert_eq!(
        detail.user_map.get(&7).map(|s| s.as_str()),
        Some("Fresh User"),
        "refresh=true must return the freshly fetched user name"
    );

    let reqs = server.received_requests().await.unwrap();
    let user_hits: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path() == "/api/v1/users")
        .collect();
    assert_eq!(user_hits.len(), 1, "refresh=true must fetch users once");

    let cached = UserMapCache::new(store.conn())
        .read("inst")
        .unwrap()
        .unwrap();
    assert_eq!(
        cached.users.get(&7).map(|s| s.as_str()),
        Some("Fresh User"),
        "fresh result must be written back to user_map_cache"
    );
}

fn seed_project_names_cache(
    db_path: &std::path::Path,
    inst_name: &str,
    names: &std::collections::HashMap<i64, String>,
) {
    let config = crate::config::Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    ProjectNamesCache::new(store.conn())
        .write(inst_name, names)
        .unwrap();
}

// --- D1b: derive_asset_label ---

// D1b-A1: URL with query tail → host label (BDR 0017 Sc.1)
#[test]
fn derive_asset_label_query_tail_url_yields_host() {
    let url = "https://docs.google.com/document/d/ABC/edit?tab=t.0";
    let label = derive_asset_label(url, None);
    assert_eq!(
        label, "docs.google.com",
        "query-tail URL must label as host, not the query tail: {label:?}"
    );
}

// D1b-A2: URL ending in a real file → filename label (BDR 0017 Sc.2)
#[test]
fn derive_asset_label_real_filename_yields_filename() {
    let url = "https://x.example.com/uploads/relatorio.pdf";
    let label = derive_asset_label(url, None);
    assert_eq!(
        label, "relatorio.pdf",
        "URL with real filename must label as filename: {label:?}"
    );
}

// D1b-A3: anchor text present and different from URL → anchor text wins (BDR 0017 Sc.3)
#[test]
fn derive_asset_label_anchor_text_preferred_over_filename() {
    let url = "https://x.example.com/uploads/relatorio.pdf";
    let label = derive_asset_label(url, Some("Especificação V1"));
    assert_eq!(
        label, "Especificação V1",
        "non-empty anchor text must be preferred over filename: {label:?}"
    );
}

// D1b-A4: query tail segment like `edit?tab=t.0` is never a filename (BDR 0017 Sc.4)
#[test]
fn derive_asset_label_query_tail_segment_is_never_a_filename() {
    let url = "https://docs.google.com/document/d/ABC/edit?tab=t.0";
    let label = derive_asset_label(url, None);
    assert!(
        !label.contains("edit?tab=t.0"),
        "query tail must never become a label: {label:?}"
    );
    assert!(
        !label.contains(".0"),
        "numeric extension must never become a label: {label:?}"
    );
}

// D1b-A4: purely-numeric extension like `.0` is not a valid filename
#[test]
fn derive_asset_label_numeric_extension_falls_through_to_host() {
    let url = "https://example.com/path/page.0";
    let label = derive_asset_label(url, None);
    assert_eq!(
        label, "example.com",
        "purely-numeric extension must fall through to host: {label:?}"
    );
}

// Anchor text that equals the URL is treated as absent
#[test]
fn derive_asset_label_anchor_text_equal_to_url_is_ignored() {
    let url = "https://example.com/doc.pdf";
    let label = derive_asset_label(url, Some(url));
    assert_eq!(
        label, "doc.pdf",
        "anchor text equal to URL must be treated as absent → filename used: {label:?}"
    );
}

// Empty anchor text is treated as absent
#[test]
fn derive_asset_label_empty_anchor_text_falls_through() {
    let url = "https://example.com/doc.pdf";
    let label = derive_asset_label(url, Some(""));
    assert_eq!(
        label, "doc.pdf",
        "empty anchor text must fall through to filename: {label:?}"
    );
}

// extract_assets for an <a> with query-tail URL and no anchor text → host stored in name
#[test]
fn extract_assets_href_with_query_tail_stores_host_in_name() {
    let task = serde_json::json!({
        "id": 1,
        "body": r#"<a href="https://docs.google.com/document/d/ABC/edit?tab=t.0">https://docs.google.com/document/d/ABC/edit?tab=t.0</a>"#
    });
    let assets = extract_assets(&task, &[]);
    assert_eq!(assets.len(), 1);
    assert_eq!(
        assets[0].name, "docs.google.com",
        "query-tail href with URL-as-anchor-text must store host in name: {:?}",
        assets[0].name
    );
}

// extract_assets for an <a> with distinct anchor text → anchor text wins
#[test]
fn extract_assets_href_with_anchor_text_stores_anchor_in_name() {
    let task = serde_json::json!({
        "id": 1,
        "body": r#"<a href="https://x.example.com/y">Especificação V1</a>"#
    });
    let assets = extract_assets(&task, &[]);
    assert_eq!(assets.len(), 1);
    assert_eq!(
        assets[0].name, "Especificação V1",
        "anchor text must be stored as label: {:?}",
        assets[0].name
    );
}

fn seed_project_names_cache_stale(
    db_path: &std::path::Path,
    inst_name: &str,
    names: &std::collections::HashMap<i64, String>,
) {
    let config = crate::config::Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).unwrap();
    let stale_ts = crate::store::now_epoch_secs() - (25 * 3600);
    ProjectNamesCache::new(store.conn())
        .write_with_fetched_at(inst_name, names, stale_ts)
        .unwrap();
}

fn read_project_names_cache(
    db_path: &std::path::Path,
    inst_name: &str,
) -> Option<std::collections::HashMap<i64, String>> {
    let config = crate::config::Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = crate::store::Store::open(&config).ok()?;
    ProjectNamesCache::new(store.conn())
        .read(inst_name)
        .ok()?
        .map(|c| c.names)
}

// R2-A1: warm cache — list_projects NOT called, open-tasks IS called, names from cache
#[tokio::test]
async fn tasks_by_project_warm_cache_skips_list_projects() {
    let server = MockServer::start().await;
    let (_dir, db_path) = make_db_path();

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 10, "task_number": 1, "name": "Warm Task", "project_id": 100,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server)
        .await;

    let names: std::collections::HashMap<i64, String> = [(100i64, "Cached Project".to_string())]
        .into_iter()
        .collect();
    seed_project_names_cache(&db_path, "inst", &names);

    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let groups = tasks_by_project(db_path, &[inst], &http).await;

    let reqs = server.received_requests().await.unwrap();
    let projects_calls: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path() == "/api/v1/projects")
        .collect();
    assert!(
        projects_calls.is_empty(),
        "warm cache must not call list_projects: got {projects_calls:?}"
    );

    let tasks_calls: Vec<_> = reqs
        .iter()
        .filter(|r| r.url.path().contains("/tasks"))
        .collect();
    assert!(!tasks_calls.is_empty(), "open-tasks must always be fetched");

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project_name, "Cached Project");
    assert_eq!(groups[0].tasks[0].name, "Warm Task");
}

// R2-A2: cold cache — list_projects called once, written to cache
#[tokio::test]
async fn tasks_by_project_cold_cache_fetches_and_writes_project_names() {
    let server = MockServer::start().await;
    let (_dir, db_path) = make_db_path();

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 10, "task_number": 1, "name": "Cold Task", "project_id": 200,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 200, "name": "Fetched Project" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let groups = tasks_by_project(db_path.clone(), &[inst], &http).await;

    server.verify().await;

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project_name, "Fetched Project");

    let cached = read_project_names_cache(&db_path, "inst");
    assert!(
        cached.is_some(),
        "project names must be written to cache after fetch"
    );
    assert_eq!(
        cached.unwrap().get(&200).map(|s| s.as_str()),
        Some("Fetched Project"),
        "cached names must match fetched names"
    );
}

// R2-A2 (stale branch): cache entry older than PROJECT_NAMES_TTL_SECS triggers one list_projects
// re-fetch and writes refreshed names back to the cache.
#[tokio::test]
async fn tasks_by_project_stale_cache_refetches_list_projects() {
    let server = MockServer::start().await;
    let (_dir, db_path) = make_db_path();

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 10, "task_number": 1, "name": "Stale Task", "project_id": 300,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 300, "name": "Refreshed Project" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let stale_names: std::collections::HashMap<i64, String> =
        [(300i64, "Stale Project".to_string())]
            .into_iter()
            .collect();
    seed_project_names_cache_stale(&db_path, "inst", &stale_names);

    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    let groups = tasks_by_project(db_path.clone(), &[inst], &http).await;

    server.verify().await;

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].project_name, "Refreshed Project");

    let cached = read_project_names_cache(&db_path, "inst");
    assert!(
        cached.is_some(),
        "refreshed project names must be written back to cache after stale re-fetch"
    );
    assert_eq!(
        cached.unwrap().get(&300).map(|s| s.as_str()),
        Some("Refreshed Project"),
        "cache must contain the freshly fetched name, not the stale one"
    );
}

// R2-A3: open tasks always fetched regardless of cache state
#[tokio::test]
async fn tasks_by_project_always_fetches_open_tasks() {
    let server = MockServer::start().await;
    let (_dir, db_path) = make_db_path();

    Mock::given(method("GET"))
        .and(path("/api/v1/users/1/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 1, "task_number": 1, "name": "Always Fresh", "project_id": 300,
                  "is_completed": false, "is_trashed": false }
            ]
        })))
        .expect(2)
        .mount(&server)
        .await;

    let names: std::collections::HashMap<i64, String> = [(300i64, "Stable Project".to_string())]
        .into_iter()
        .collect();
    seed_project_names_cache(&db_path, "inst", &names);

    let inst = make_instance("inst", &server.uri(), Some(1));
    let http = make_http();

    tasks_by_project(db_path.clone(), std::slice::from_ref(&inst), &http).await;
    tasks_by_project(db_path.clone(), &[inst], &http).await;

    server.verify().await;
}

// --- extract_assets (moved from render domain to controller domain) ---

#[test]
fn extract_assets_from_body_html() {
    let task = serde_json::json!({
        "id": 1,
        "body": r#"<img src="https://example.com/img.png"><a href="https://example.com/file.pdf">link</a>"#
    });
    let assets = extract_assets(&task, &[]);
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0].name, "img.png");
    assert_eq!(assets[0].url, "https://example.com/img.png");
    // anchor text "link" is non-empty and different from the URL → anchor text wins
    assert_eq!(assets[1].name, "link");
}

#[test]
fn extract_assets_deduplicates_by_url() {
    let task = serde_json::json!({
        "id": 1,
        "body": r#"<img src="https://example.com/img.png">"#
    });
    let comments = vec![serde_json::json!({
        "body": r#"<img src="https://example.com/img.png">"#
    })];
    let assets = extract_assets(&task, &comments);
    assert_eq!(assets.len(), 1, "duplicate URLs must be deduplicated");
}

#[test]
fn extract_assets_from_attachments() {
    let task = serde_json::json!({
        "id": 1,
        "attachments": [
            { "name": "report.pdf", "url": "https://example.com/report.pdf" }
        ]
    });
    let assets = extract_assets(&task, &[]);
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].name, "report.pdf");
}

#[test]
fn extract_assets_empty_when_no_body_or_attachments() {
    let task = serde_json::json!({ "id": 1 });
    let assets = extract_assets(&task, &[]);
    assert!(assets.is_empty());
}

// --- D2d-i: attach_project_names ---

fn make_mine_row(instance: &str, project_id: i64, task_id: i64) -> crate::render::MineTableRow {
    crate::render::MineTableRow {
        instance: instance.to_owned(),
        project_id,
        task_number: task_id,
        task_id,
        name: format!("Task {task_id}"),
        due_on: None,
        project_name: None,
    }
}

// D2d-i AC1: warm cache → Some(name) for a known project_id.
#[test]
fn attach_project_names_warm_cache_resolves_name() {
    let (_dir, db_path) = make_db_path();
    let names: std::collections::HashMap<i64, String> =
        [(10i64, "Alpha Project".to_string())].into_iter().collect();
    seed_project_names_cache(&db_path, "inst-a", &names);

    let rows = vec![make_mine_row("inst-a", 10, 1)];
    let result = attach_project_names(&db_path, rows);

    assert_eq!(
        result[0].project_name.as_deref(),
        Some("Alpha Project"),
        "warm cache must resolve the project name for a known project_id"
    );
}

// D2d-i AC1 (cold path): cold cache → project_name is None.
#[test]
fn attach_project_names_cold_cache_yields_none() {
    let (_dir, db_path) = make_db_path();

    let rows = vec![make_mine_row("inst-cold", 99, 2)];
    let result = attach_project_names(&db_path, rows);

    assert_eq!(
        result[0].project_name, None,
        "cold cache must leave project_name as None"
    );
}

// D2d-i AC1 (absent id): project_id present in warm cache but this id is not in the map → None.
#[test]
fn attach_project_names_absent_id_in_warm_cache_yields_none() {
    let (_dir, db_path) = make_db_path();
    let names: std::collections::HashMap<i64, String> =
        [(10i64, "Known Project".to_string())].into_iter().collect();
    seed_project_names_cache(&db_path, "inst-b", &names);

    let rows = vec![make_mine_row("inst-b", 999, 3)];
    let result = attach_project_names(&db_path, rows);

    assert_eq!(
        result[0].project_name, None,
        "project_id absent from warm cache must yield None"
    );
}

// D2d-i AC2: per-instance isolation — project_id cached under instance A must NOT
// resolve for a row on instance B.
#[test]
fn attach_project_names_cross_instance_isolation() {
    let (_dir, db_path) = make_db_path();
    let names: std::collections::HashMap<i64, String> =
        [(100i64, "Instance A Project".to_string())]
            .into_iter()
            .collect();
    seed_project_names_cache(&db_path, "inst-a", &names);

    let rows = vec![make_mine_row("inst-b", 100, 4)];
    let result = attach_project_names(&db_path, rows);

    assert_eq!(
        result[0].project_name, None,
        "project_id cached under inst-a must NOT resolve for a row on inst-b"
    );
}

// D2d-i: cache is read at most once per instance (memoisation correctness).
// Two rows on the same instance must both resolve, even though the reader
// is called only once.
#[test]
fn attach_project_names_memoises_per_instance_read() {
    let (_dir, db_path) = make_db_path();
    let names: std::collections::HashMap<i64, String> = [
        (1i64, "Project One".to_string()),
        (2i64, "Project Two".to_string()),
    ]
    .into_iter()
    .collect();
    seed_project_names_cache(&db_path, "inst-m", &names);

    let rows = vec![
        make_mine_row("inst-m", 1, 10),
        make_mine_row("inst-m", 2, 11),
    ];
    let result = attach_project_names(&db_path, rows);

    assert_eq!(result[0].project_name.as_deref(), Some("Project One"));
    assert_eq!(result[1].project_name.as_deref(), Some("Project Two"));
}
