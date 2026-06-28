use super::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[test]
fn host_gated_token_same_host_returns_header() {
    let result = Http::host_gated_token_header(
        "https://acme.example.com/api/v1/tasks",
        "https://acme.example.com",
        "tok-123",
    );
    assert!(
        result.is_some(),
        "same-host request should carry token header"
    );
    let (name, value) = result.unwrap();
    assert_eq!(name.as_str(), TOKEN_HEADER);
    assert_eq!(value.to_str().unwrap(), "tok-123");
}

#[test]
fn host_gated_token_different_host_returns_none() {
    let result = Http::host_gated_token_header(
        "https://evil.other.com/steal",
        "https://acme.example.com",
        "tok-123",
    );
    assert!(
        result.is_none(),
        "foreign-host request must not carry token"
    );
}

#[test]
fn host_gated_token_case_insensitive_host_match() {
    let result = Http::host_gated_token_header(
        "https://ACME.EXAMPLE.COM/api/v1/tasks",
        "https://acme.example.com",
        "tok",
    );
    assert!(result.is_some(), "host matching should be case-insensitive");
}

#[tokio::test]
async fn authed_get_attaches_token_to_same_host() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/projects"))
        .and(wiremock::matchers::header(TOKEN_HEADER, "my-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/projects", server.uri());
    let (status, _body) = http
        .authed_get(&url, &server.uri(), "my-token")
        .await
        .unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}

#[tokio::test]
async fn authed_get_no_token_for_foreign_host() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/foreign"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/foreign", server.uri());
    http.authed_get(&url, "https://different-instance.example.com", "secret-tok")
        .await
        .unwrap();

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(
        reqs[0].headers.get("x-angie-authapitoken").is_none(),
        "token must not be attached to a foreign-host request"
    );
}

#[tokio::test]
async fn http_error_status_returned_as_ok_not_err() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tasks"))
        .respond_with(ResponseTemplate::new(403).set_body_string("forbidden"))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/tasks", server.uri());
    let result = http.authed_get(&url, &server.uri(), "tok").await;
    assert!(result.is_ok(), "HTTP 403 must be Ok, not transport error");
    let (status, body) = result.unwrap();
    assert_eq!(status, 403);
    assert_eq!(body.as_ref(), b"forbidden");
}

#[tokio::test]
async fn redirect_is_not_followed_and_status_returned() {
    let redirect_server = MockServer::start().await;
    let second_server = MockServer::start().await;

    let second_uri = second_server.uri();

    Mock::given(method("GET"))
        .and(path("/redirect-me"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", format!("{}/landed", second_uri)),
        )
        .expect(1)
        .mount(&redirect_server)
        .await;

    // The second server should receive ZERO requests.
    Mock::given(method("GET"))
        .and(path("/landed"))
        .respond_with(ResponseTemplate::new(200).set_body_string("you followed"))
        .expect(0)
        .mount(&second_server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/redirect-me", redirect_server.uri());
    let (status, _) = http
        .authed_get(&url, &redirect_server.uri(), "tok")
        .await
        .unwrap();
    assert_eq!(status, 302, "302 must be returned to caller, not followed");
    second_server.verify().await;
}

#[tokio::test]
async fn authed_post_attaches_token_to_same_host() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/42"))
        .and(wiremock::matchers::header(TOKEN_HEADER, "my-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 1 })))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/task/42", server.uri());
    let body = serde_json::json!({ "body": "hello" });
    let (status, _) = http
        .authed_post(&url, &server.uri(), "my-token", &body)
        .await
        .unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}

#[tokio::test]
async fn authed_post_no_token_for_foreign_host() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 1 })))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/task/42", server.uri());
    let body = serde_json::json!({ "body": "hello" });
    http.authed_post(
        &url,
        "https://different-instance.example.com",
        "secret",
        &body,
    )
    .await
    .unwrap();

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(
        reqs[0].headers.get("x-angie-authapitoken").is_none(),
        "token must not be attached to a foreign-host POST"
    );
}

#[tokio::test]
async fn authed_post_returns_status_and_body_on_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/comments/task/1"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "id": 5 })))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/task/1", server.uri());
    let body = serde_json::json!({ "body": "text" });
    let (status, bytes) = http
        .authed_post(&url, &server.uri(), "tok", &body)
        .await
        .unwrap();
    assert_eq!(status, 201);
    assert!(!bytes.is_empty());
}

#[tokio::test]
async fn authed_put_attaches_token_to_same_host() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/comments/99"))
        .and(wiremock::matchers::header(TOKEN_HEADER, "my-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 99 })))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/99", server.uri());
    let body = serde_json::json!({ "body": "updated" });
    let (status, _) = http
        .authed_put(&url, &server.uri(), "my-token", &body)
        .await
        .unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}

#[tokio::test]
async fn authed_put_no_token_for_foreign_host() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/comments/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 99 })))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/99", server.uri());
    let body = serde_json::json!({ "body": "updated" });
    http.authed_put(
        &url,
        "https://different-instance.example.com",
        "secret",
        &body,
    )
    .await
    .unwrap();

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(
        reqs[0].headers.get("x-angie-authapitoken").is_none(),
        "token must not be attached to a foreign-host PUT"
    );
}

#[tokio::test]
async fn authed_put_returns_status_and_body_on_2xx() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/api/v1/comments/7"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 7, "body": "new" })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/7", server.uri());
    let body = serde_json::json!({ "body": "new" });
    let (status, bytes) = http
        .authed_put(&url, &server.uri(), "tok", &body)
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert!(!bytes.is_empty());
}

#[tokio::test]
async fn authed_delete_attaches_token_to_same_host() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/comments/55"))
        .and(wiremock::matchers::header(TOKEN_HEADER, "my-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/55", server.uri());
    let (status, _) = http
        .authed_delete(&url, &server.uri(), "my-token")
        .await
        .unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}

#[tokio::test]
async fn authed_delete_no_token_for_foreign_host() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/comments/55"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/55", server.uri());
    http.authed_delete(&url, "https://different-instance.example.com", "secret")
        .await
        .unwrap();

    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert!(
        reqs[0].headers.get("x-angie-authapitoken").is_none(),
        "token must not be attached to a foreign-host DELETE"
    );
}

#[tokio::test]
async fn authed_delete_returns_status() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/v1/comments/3"))
        .respond_with(ResponseTemplate::new(204).set_body_string(""))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/comments/3", server.uri());
    let (status, _) = http
        .authed_delete(&url, &server.uri(), "tok")
        .await
        .unwrap();
    assert_eq!(status, 204);
}

#[tokio::test]
async fn post_json_sends_correct_content_type_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/issue-token"))
        .and(wiremock::matchers::body_json(serde_json::json!({
            "username": "user@example.com",
            "password": "s3cr3t"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "is_ok": true,
            "token": "tok-abc"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let http = Http::new().unwrap();
    let url = format!("{}/api/v1/issue-token", server.uri());
    let body = serde_json::json!({
        "username": "user@example.com",
        "password": "s3cr3t"
    });
    let (status, _) = http.post_json(&url, &body).await.unwrap();
    assert_eq!(status, 200);
    server.verify().await;
}
