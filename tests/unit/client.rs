use super::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_instance(base_url: &str) -> Instance {
    Instance {
        name: "test-inst".to_string(),
        base_url: base_url.to_string(),
        email: "user@example.com".to_string(),
        token: "test-token".to_string(),
        user_id: Some(7),
    }
}

fn make_client(base_url: &str) -> ActiveCollabClient {
    let instance = make_instance(base_url);
    let http = Http::new().unwrap();
    ActiveCollabClient::new(instance, http)
}

#[tokio::test]
async fn exchange_token_success_returns_token_and_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .and(body_json(serde_json::json!({
            "username": "u@x.com",
            "password": "pass",
            "client_name": "active-collab-skill",
            "client_vendor": "klock"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": true,
            "token": "tok-abc"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (tok, data) = client
        .exchange_token(&server.uri(), "u@x.com", "pass")
        .await
        .unwrap();
    assert_eq!(tok, Some("tok-abc".to_string()));
    assert_eq!(data["is_ok"], true);
    server.verify().await;
}

#[tokio::test]
async fn exchange_token_sends_no_token_header() {
    let server = MockServer::start().await;
    // Mount a catch-all that rejects requests containing X-Angie-AuthApiToken
    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": true,
            "token": "tok-xyz"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    client
        .exchange_token(&server.uri(), "u@x.com", "pass")
        .await
        .unwrap();

    // Verify no token was attached by inspecting received requests
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    let has_token = reqs[0].headers.get("x-angie-authapitoken").is_some();
    assert!(
        !has_token,
        "issue-token POST must not carry auth token header"
    );
}

#[tokio::test]
async fn exchange_token_non_200_returns_none_and_empty_object() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (tok, data) = client
        .exchange_token(&server.uri(), "u@x.com", "wrong")
        .await
        .unwrap();
    assert!(tok.is_none());
    assert_eq!(data, serde_json::json!({}));
}

#[tokio::test]
async fn exchange_token_is_ok_false_returns_none_with_data() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": false,
            "message": "bad credentials"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (tok, data) = client
        .exchange_token(&server.uri(), "u@x.com", "wrong")
        .await
        .unwrap();
    assert!(tok.is_none());
    assert_eq!(data["message"], "bad credentials");
}

#[tokio::test]
async fn resolve_user_id_case_insensitive_match() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "email": "Admin@Example.COM" },
            { "id": 2, "email": "other@example.com" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let uid = client
        .resolve_user_id(&server.uri(), "tok", "admin@example.com")
        .await
        .unwrap();
    assert_eq!(uid, Some(1));
}

#[tokio::test]
async fn resolve_user_id_no_match_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "email": "other@example.com" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let uid = client
        .resolve_user_id(&server.uri(), "tok", "notfound@example.com")
        .await
        .unwrap();
    assert!(uid.is_none());
}

#[tokio::test]
async fn resolve_user_id_non_200_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let uid = client
        .resolve_user_id(&server.uri(), "tok", "x@y.com")
        .await
        .unwrap();
    assert!(uid.is_none());
}

#[tokio::test]
async fn fetch_user_map_display_name_takes_priority() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 1, "display_name": "Alice Smith", "first_name": "Alice", "last_name": "Smith", "email": "a@a.com" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let map = client.fetch_user_map().await.unwrap();
    assert_eq!(map.get(&1).map(|s| s.as_str()), Some("Alice Smith"));
}

#[tokio::test]
async fn fetch_user_map_falls_back_to_first_last_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 2, "display_name": null, "first_name": "Bob", "last_name": "Jones", "email": "b@b.com" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let map = client.fetch_user_map().await.unwrap();
    assert_eq!(map.get(&2).map(|s| s.as_str()), Some("Bob Jones"));
}

#[tokio::test]
async fn fetch_user_map_falls_back_to_email() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "id": 3, "display_name": null, "first_name": null, "last_name": null, "email": "c@c.com" }
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let map = client.fetch_user_map().await.unwrap();
    assert_eq!(map.get(&3).map(|s| s.as_str()), Some("c@c.com"));
}

#[tokio::test]
async fn fetch_user_map_non_200_returns_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(500).set_body_string("error"))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let map = client.fetch_user_map().await.unwrap();
    assert!(map.is_empty());
}

#[tokio::test]
async fn fetch_user_map_non_list_response_returns_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": "val"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let map = client.fetch_user_map().await.unwrap();
    assert!(map.is_empty());
}

#[tokio::test]
async fn fetch_task_200_returns_some_payload() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10,
            "name": "Do work"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (status, payload) = client.fetch_task(5, 10).await.unwrap();
    assert_eq!(status, 200);
    assert!(payload.is_some());
    assert_eq!(payload.unwrap()["name"], "Do work");
}

#[tokio::test]
async fn fetch_task_404_returns_none() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects/5/tasks/99"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (status, payload) = client.fetch_task(5, 99).await.unwrap();
    assert_eq!(status, 404);
    assert!(payload.is_none());
}

#[tokio::test]
async fn fetch_open_tasks_no_user_id_returns_empty() {
    let http = Http::new().unwrap();
    let instance = Instance {
        name: "inst".to_string(),
        base_url: "https://example.com".to_string(),
        email: "x@x.com".to_string(),
        token: "tok".to_string(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(instance, http);
    let tasks = client.fetch_open_tasks().await.unwrap();
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn fetch_open_tasks_filters_completed_and_trashed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/7/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 1, "name": "Open task",      "is_completed": false, "is_trashed": false, "project_id": 1 },
                { "id": 2, "name": "Completed task",  "is_completed": true,  "is_trashed": false, "project_id": 1 },
                { "id": 3, "name": "Trashed task",    "is_completed": false, "is_trashed": true,  "project_id": 1 }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let tasks = client.fetch_open_tasks().await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, 1);
    assert_eq!(tasks[0].name, "Open task");
}

#[tokio::test]
async fn fetch_open_tasks_sets_instance_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/7/tasks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "tasks": [
                { "id": 1, "name": "Task A", "is_completed": false, "is_trashed": false }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let tasks = client.fetch_open_tasks().await.unwrap();
    assert_eq!(tasks[0].instance_name, "test-inst");
}

#[tokio::test]
async fn fetch_open_tasks_non_200_returns_empty() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/users/7/tasks"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let tasks = client.fetch_open_tasks().await.unwrap();
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn list_projects_returns_status_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (status, body) = client.list_projects().await.unwrap();
    assert_eq!(status, 200);
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_connectivity_aliases_list_projects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (status, _) = client.test_connectivity().await.unwrap();
    assert_eq!(status, 200);
}

#[tokio::test]
async fn same_host_authed_request_carries_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .and(header("x-angie-authapitoken", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let (status, _) = client.list_projects().await.unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}

#[tokio::test]
async fn http_error_status_is_data_not_transport_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let client = make_client(&server.uri());
    let result = client.list_projects().await;
    assert!(
        result.is_ok(),
        "HTTP 403 must be Ok((status, body)), not Err"
    );
    let (status, _) = result.unwrap();
    assert_eq!(status, 403);
}

#[tokio::test]
async fn redirect_to_second_host_is_not_followed() {
    let redirect_server = MockServer::start().await;
    let second_server = MockServer::start().await;
    let second_uri = second_server.uri();

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/api/v1/projects", second_uri)),
        )
        .expect(1)
        .mount(&redirect_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&second_server)
        .await;

    let http = Http::new().unwrap();
    let instance = Instance {
        name: "inst".to_string(),
        base_url: redirect_server.uri(),
        email: "x@x.com".to_string(),
        token: "tok".to_string(),
        user_id: Some(1),
    };
    let client = ActiveCollabClient::new(instance, http);
    let (status, _) = client.list_projects().await.unwrap();
    assert_eq!(status, 302, "302 must be returned, not followed");
    second_server.verify().await;
}

#[tokio::test]
async fn base_url_trailing_slash_is_trimmed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let base_with_slash = format!("{}/", server.uri());
    let http = Http::new().unwrap();
    let instance = Instance {
        name: "inst".to_string(),
        base_url: base_with_slash,
        email: "x@x.com".to_string(),
        token: "tok".to_string(),
        user_id: Some(1),
    };
    let client = ActiveCollabClient::new(instance, http);
    let (status, _) = client.list_projects().await.unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}
