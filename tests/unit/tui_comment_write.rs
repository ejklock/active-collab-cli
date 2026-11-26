use super::*;
use crate::client::ActiveCollabClient;
use crate::http::Http;
use crate::store::instances::Instance;
use std::path::PathBuf;
use tempfile::TempDir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const UPLOAD_PATH: &str = "/api/v1/upload-files";
const COMMENT_PATH: &str = "/api/v1/comments/task/7";

fn client(server: &MockServer) -> ActiveCollabClient {
    let inst = Instance {
        name: "inst".to_owned(),
        base_url: server.uri(),
        email: "user@example.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(1),
    };
    ActiveCollabClient::new(inst, Http::new().expect("http client builds"))
}

fn file(dir: &TempDir, name: &str) -> PathBuf {
    let p = dir.path().join(name);
    std::fs::write(&p, name).expect("temp file is writable");
    p
}

#[tokio::test]
async fn picked_files_are_uploaded_and_their_codes_ride_on_the_comment() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(UPLOAD_PATH))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{ "code": "code-a" }])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(COMMENT_PATH))
        .and(body_json(serde_json::json!({
            "body": "<p>QA</p>",
            "attach_uploaded_files": ["code-a"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 1 })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");

    let msg = post_comment_with_files(&client(&server), 7, "QA", &[file(&dir, "a.txt")]).await;

    server.verify().await;
    assert!(matches!(msg, Msg::CommentMutationOk));
}

#[tokio::test]
async fn a_missing_file_reports_why_and_posts_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("nope.png");

    let msg =
        post_comment_with_files(&client(&server), 7, "QA", std::slice::from_ref(&missing)).await;

    server.verify().await;
    match msg {
        Msg::CommentMutationErr(reason) => {
            assert!(reason.contains(&missing.display().to_string()), "{reason}")
        }
        _ => panic!("expected CommentMutationErr"),
    }
}

#[tokio::test]
async fn an_unauthorized_upload_expires_the_session_and_posts_nothing() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(UPLOAD_PATH))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(COMMENT_PATH))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");

    let msg = post_comment_with_files(&client(&server), 7, "QA", &[file(&dir, "a.txt")]).await;

    server.verify().await;
    assert!(matches!(msg, Msg::AuthExpired));
}

#[tokio::test]
async fn without_files_the_comment_posts_as_before() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(COMMENT_PATH))
        .and(body_json(serde_json::json!({ "body": "<p>QA</p>" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 1 })))
        .expect(1)
        .mount(&server)
        .await;

    let msg = post_comment_with_files(&client(&server), 7, "QA", &[]).await;

    server.verify().await;
    assert!(matches!(msg, Msg::CommentMutationOk));
}
