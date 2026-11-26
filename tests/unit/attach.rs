use super::*;
use crate::client::ActiveCollabClient;
use crate::commands::comment_core;
use crate::http::Http;
use crate::store::instances::Instance;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use wiremock::matchers::{body_json, body_string_contains, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const UPLOAD_PATH: &str = "/api/v1/upload-files";
const COMMENT_PATH: &str = "/api/v1/comments/task/75346";
const TASK_PATH: &str = "/api/v1/projects/524/tasks/75346";

fn instance(base_url: &str) -> Instance {
    Instance {
        name: "testinst".to_owned(),
        base_url: base_url.to_owned(),
        email: "user@example.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(1),
    }
}

fn client(server: &MockServer) -> (Instance, ActiveCollabClient) {
    let inst = instance(&server.uri());
    let client = ActiveCollabClient::new(inst.clone(), Http::new().expect("http client builds"));
    (inst, client)
}

fn files(dir: &TempDir, names: &[(&str, &str)]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|(name, content)| {
            let p = dir.path().join(name);
            std::fs::write(&p, content).expect("temp file is writable");
            p
        })
        .collect()
}

fn uploaded(code: &str, name: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!([{ "code": code, "name": name }]))
}

async fn mount_upload(
    server: &MockServer,
    file_name: &str,
    response: ResponseTemplate,
    times: u64,
) {
    Mock::given(method("POST"))
        .and(path(UPLOAD_PATH))
        .and(body_string_contains(file_name))
        .respond_with(response)
        .expect(times)
        .mount(server)
        .await;
}

async fn mount_comment(server: &MockServer, times: u64) {
    Mock::given(method("POST"))
        .and(path(COMMENT_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 9 })))
        .expect(times)
        .mount(server)
        .await;
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

async fn comment(
    client: &ActiveCollabClient,
    inst: &Instance,
    body: &str,
    paths: &[PathBuf],
    reporter: &UploadReporter,
) -> Run {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = comment_core(
        Some("524/75346"),
        None,
        body,
        false,
        paths,
        reporter,
        inst,
        client,
        true,
        &mut out,
        &mut err,
    )
    .await;
    Run {
        code,
        out: String::from_utf8_lossy(&out).into_owned(),
        err: String::from_utf8_lossy(&err).into_owned(),
    }
}

async fn attach(client: &ActiveCollabClient, paths: &[PathBuf], reporter: &UploadReporter) -> Run {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = task_attach_core(
        Some("524/75346"),
        None,
        paths,
        reporter,
        client,
        true,
        &mut out,
        &mut err,
    )
    .await;
    Run {
        code,
        out: String::from_utf8_lossy(&out).into_owned(),
        err: String::from_utf8_lossy(&err).into_owned(),
    }
}

fn json_of(run: &Run) -> serde_json::Value {
    let line = if run.out.trim().is_empty() {
        run.err.trim()
    } else {
        run.out.trim()
    };
    serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON ({e}): {line:?}"))
}

#[tokio::test]
async fn comment_with_two_files_uploads_each_then_posts_their_codes_in_order() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    mount_upload(&server, "b.txt", uploaded("code-b", "b.txt"), 1).await;
    Mock::given(method("POST"))
        .and(path(COMMENT_PATH))
        .and(body_json(serde_json::json!({
            "body": "<p>QA</p>",
            "attach_uploaded_files": ["code-a", "code-b"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 9 })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha"), ("b.txt", "beta")]);
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "QA", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 0, "err: {}", run.err);
    assert_eq!(
        json_of(&run)["attachments"],
        serde_json::json!(["a.txt", "b.txt"])
    );
}

#[tokio::test]
async fn upload_request_carries_the_file_content_under_its_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(UPLOAD_PATH))
        .and(body_string_contains("filename=\"report.html\""))
        .and(body_string_contains("<h1>QA report</h1>"))
        .respond_with(uploaded("code-r", "report.html"))
        .expect(1)
        .mount(&server)
        .await;
    mount_comment(&server, 1).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("report.html", "<h1>QA report</h1>")]);
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "QA", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 0, "err: {}", run.err);
}

#[tokio::test]
async fn empty_body_with_a_file_posts_an_empty_body_and_the_code() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    Mock::given(method("POST"))
        .and(path(COMMENT_PATH))
        .and(body_json(serde_json::json!({
            "body": "",
            "attach_uploaded_files": ["code-a"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": 9 })))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "  \n", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 0, "err: {}", run.err);
}

#[tokio::test]
async fn empty_body_without_files_is_a_usage_error_and_sends_nothing() {
    let server = MockServer::start().await;
    mount_comment(&server, 0).await;
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "   ", &[], &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 2);
}

#[tokio::test]
async fn a_missing_file_stops_before_any_upload_and_names_the_path() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 0).await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let mut paths = files(&dir, &[("a.txt", "alpha")]);
    let missing = dir.path().join("missing.png");
    paths.push(missing.clone());
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "QA", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 2);
    let json = json_of(&run);
    assert_eq!(json["ok"], false);
    let error = json["error"].as_str().expect("error is text");
    assert!(error.contains(&missing.display().to_string()), "{error}");
}

#[tokio::test]
async fn a_directory_is_refused_as_an_attachment() {
    let server = MockServer::start().await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let (inst, client) = client(&server);

    let run = comment(
        &client,
        &inst,
        "QA",
        &[dir.path().to_path_buf()],
        &UploadReporter::silent(),
    )
    .await;

    server.verify().await;
    assert_eq!(run.code, 2);
    assert!(json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .contains(&dir.path().display().to_string()));
}

#[tokio::test]
async fn a_rejected_second_upload_posts_no_comment_and_names_the_file() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    mount_upload(&server, "b.txt", ResponseTemplate::new(500), 1).await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha"), ("b.txt", "beta")]);
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "QA", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 1);
    let error = json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .to_owned();
    assert!(error.contains("b.txt") && error.contains("500"), "{error}");
}

#[tokio::test]
async fn an_unauthorized_upload_asks_to_reauthenticate_and_posts_no_comment() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", ResponseTemplate::new(401), 1).await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "QA", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 1);
    assert!(json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .contains("ac setup add"));
}

#[tokio::test]
async fn an_upload_reply_without_a_code_posts_no_comment() {
    let server = MockServer::start().await;
    mount_upload(
        &server,
        "a.txt",
        ResponseTemplate::new(200).set_body_json(serde_json::json!([])),
        1,
    )
    .await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (inst, client) = client(&server);

    let run = comment(&client, &inst, "QA", &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 1);
    assert!(json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .contains("a.txt"));
}

#[tokio::test]
async fn a_reporter_cancelled_before_start_uploads_and_posts_nothing() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 0).await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (inst, client) = client(&server);
    let cancelled = Arc::new(AtomicBool::new(true));
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

    let run = comment(
        &client,
        &inst,
        "QA",
        &paths,
        &UploadReporter::new(tx, cancelled),
    )
    .await;

    server.verify().await;
    assert_eq!(run.code, 1);
    assert!(json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .contains("cancelled"));
}

struct CancelWhileUploading {
    cancelled: Arc<AtomicBool>,
}

impl Respond for CancelWhileUploading {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        self.cancelled.store(true, Ordering::SeqCst);
        uploaded("code-a", "a.txt")
    }
}

#[tokio::test]
async fn a_cancel_during_an_upload_stops_before_the_next_file() {
    let server = MockServer::start().await;
    let cancelled = Arc::new(AtomicBool::new(false));
    Mock::given(method("POST"))
        .and(path(UPLOAD_PATH))
        .respond_with(CancelWhileUploading {
            cancelled: Arc::clone(&cancelled),
        })
        .expect(1)
        .mount(&server)
        .await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha"), ("b.txt", "beta")]);
    let (inst, client) = client(&server);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

    let run = comment(
        &client,
        &inst,
        "QA",
        &paths,
        &UploadReporter::new(tx, cancelled),
    )
    .await;

    server.verify().await;
    assert_eq!(run.code, 1);
}

#[tokio::test]
async fn the_reporter_hears_each_file_as_it_starts() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    mount_upload(&server, "b.txt", uploaded("code-b", "b.txt"), 1).await;
    mount_comment(&server, 1).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha"), ("b.txt", "beta")]);
    let (inst, client) = client(&server);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let reporter = UploadReporter::new(tx, Arc::new(AtomicBool::new(false)));

    let run = comment(&client, &inst, "QA", &paths, &reporter).await;
    drop(reporter);

    assert_eq!(run.code, 0, "err: {}", run.err);
    let mut heard = Vec::new();
    while let Some(message) = rx.recv().await {
        heard.push(message);
    }
    assert_eq!(heard, vec!["uploading 1/2 a.txt", "uploading 2/2 b.txt"]);
}

#[tokio::test]
async fn task_attach_uploads_then_puts_the_codes_on_the_task() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    Mock::given(method("PUT"))
        .and(path(TASK_PATH))
        .and(body_json(
            serde_json::json!({ "attach_uploaded_files": ["code-a"] }),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "single": { "id": 75346 } })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (_inst, client) = client(&server);

    let run = attach(&client, &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 0, "err: {}", run.err);
    assert_eq!(
        json_of(&run),
        serde_json::json!({
            "ok": true,
            "task_id": 75346,
            "project_id": 524,
            "attachments": ["a.txt"]
        })
    );
}

#[tokio::test]
async fn task_attach_without_files_is_a_usage_error() {
    let server = MockServer::start().await;
    let (_inst, client) = client(&server);

    let run = attach(&client, &[], &UploadReporter::silent()).await;

    assert_eq!(run.code, 2);
    assert!(server
        .received_requests()
        .await
        .unwrap_or_default()
        .is_empty());
}

#[tokio::test]
async fn task_attach_with_a_failed_upload_leaves_the_task_alone() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", ResponseTemplate::new(500), 1).await;
    Mock::given(method("PUT"))
        .and(path(TASK_PATH))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (_inst, client) = client(&server);

    let run = attach(&client, &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 1);
}

#[tokio::test]
async fn task_attach_with_an_unauthorized_put_asks_to_reauthenticate() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    Mock::given(method("PUT"))
        .and(path(TASK_PATH))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (_inst, client) = client(&server);

    let run = attach(&client, &paths, &UploadReporter::silent()).await;

    server.verify().await;
    assert_eq!(run.code, 1);
    assert!(json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .contains("ac setup add"));
}

#[tokio::test]
async fn a_cancel_during_the_only_upload_is_caught_before_the_post() {
    let server = MockServer::start().await;
    let cancelled = Arc::new(AtomicBool::new(false));
    Mock::given(method("POST"))
        .and(path(UPLOAD_PATH))
        .respond_with(CancelWhileUploading {
            cancelled: Arc::clone(&cancelled),
        })
        .expect(1)
        .mount(&server)
        .await;
    mount_comment(&server, 0).await;
    let dir = TempDir::new().expect("temp dir");
    let paths = files(&dir, &[("a.txt", "alpha")]);
    let (inst, client) = client(&server);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

    let run = comment(
        &client,
        &inst,
        "QA",
        &paths,
        &UploadReporter::new(tx, cancelled),
    )
    .await;

    server.verify().await;
    assert_eq!(run.code, 1);
    assert!(json_of(&run)["error"]
        .as_str()
        .expect("error is text")
        .contains("cancelled"));
}

#[test]
fn a_leading_tilde_slash_expands_to_the_home_directory() {
    let home = std::path::Path::new("/home/someone");
    assert_eq!(
        expand_home(std::path::Path::new("~/qa/shot.png"), Some(home)),
        PathBuf::from("/home/someone/qa/shot.png")
    );
    assert_eq!(
        expand_home(std::path::Path::new("~"), Some(home)),
        PathBuf::from("/home/someone")
    );
}

#[test]
fn other_paths_and_a_missing_home_are_left_as_typed() {
    let home = std::path::Path::new("/home/someone");
    for typed in [
        "qa/~/shot.png",
        "~other/shot.png",
        "/abs/shot.png",
        "shot~.png",
    ] {
        assert_eq!(
            expand_home(std::path::Path::new(typed), Some(home)),
            PathBuf::from(typed)
        );
    }
    assert_eq!(
        expand_home(std::path::Path::new("~/shot.png"), None),
        PathBuf::from("~/shot.png")
    );
}

#[tokio::test]
async fn a_tilde_path_in_a_batch_uploads_the_file_under_home() {
    let server = MockServer::start().await;
    mount_upload(&server, "a.txt", uploaded("code-a", "a.txt"), 1).await;
    let home = TempDir::new().expect("temp home");
    files(&home, &[("a.txt", "alpha")]);
    let (_inst, client) = client(&server);

    let uploaded = upload_files_from_home(
        &client,
        &[PathBuf::from("~/a.txt")],
        &UploadReporter::silent(),
        Some(home.path()),
    )
    .await
    .expect("the expanded path uploads");

    server.verify().await;
    assert_eq!(uploaded.codes, vec!["code-a"]);
    assert_eq!(uploaded.names, vec!["a.txt"]);
}
