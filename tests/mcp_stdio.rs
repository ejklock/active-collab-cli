use rmcp::model::{CallToolRequestParams, CallToolResult, ErrorCode, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceError};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

type Client = RunningService<RoleClient, ()>;

struct Env {
    dir: TempDir,
    db: PathBuf,
}

impl Env {
    fn new(instances: &[(&str, &MockServer, i64)]) -> Env {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("ac.db");
        std::fs::create_dir(dir.path().join("tmp")).unwrap();
        let env = Env { dir, db };
        let init = env.command().args(["setup", "list"]).output().unwrap();
        assert!(init.status.success(), "store init failed: {init:?}");
        let conn = rusqlite::Connection::open(&env.db).unwrap();
        for (name, server, user_id) in instances {
            conn.execute(
                "INSERT INTO instances (name, base_url, email, token, user_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, '2026-01-01T00:00:00Z')",
                rusqlite::params![
                    name,
                    server.uri(),
                    format!("{name}@example.com"),
                    format!("tok-{name}"),
                    user_id
                ],
            )
            .unwrap();
        }
        env
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ac"));
        cmd.env("ACTIVE_COLLAB_DB", &self.db)
            .env("ACTIVE_COLLAB_LANG", "en")
            .env("HOME", self.dir.path())
            .env("TMPDIR", self.dir.path().join("tmp"))
            .current_dir(self.dir.path());
        cmd
    }

    fn cli(&self, args: &[&str]) -> String {
        let output = self.command().args(args).output().unwrap();
        assert!(output.status.success(), "ac {args:?} failed: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    async fn connect(&self) -> Client {
        let cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_ac"));
        let transport = TokioChildProcess::new(cmd.configure(|c| {
            c.arg("mcp")
                .env("ACTIVE_COLLAB_DB", &self.db)
                .env("ACTIVE_COLLAB_LANG", "en")
                .env("HOME", self.dir.path())
                .env("TMPDIR", self.dir.path().join("tmp"))
                .current_dir(self.dir.path());
        }))
        .unwrap();
        ().serve(transport).await.unwrap()
    }
}

fn object(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("expected a JSON object, got {other}"),
    }
}

async fn call(
    client: &Client,
    tool: &'static str,
    args: Value,
) -> Result<CallToolResult, ServiceError> {
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(object(args)))
        .await
}

fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|t| t.text.clone()))
        .collect()
}

async fn ok_text(client: &Client, tool: &'static str, args: Value) -> String {
    let result = call(client, tool, args.clone()).await.unwrap();
    assert_ne!(
        result.is_error,
        Some(true),
        "{args} failed: {}",
        text_of(&result)
    );
    text_of(&result)
}

async fn mount_json(server: &MockServer, verb: &str, route: &str, status: u16, body: Value) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

async fn mount_status(server: &MockServer, route: &str, status: u16) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mount_api(server: &MockServer, user_id: i64, task_title: &str) {
    mount_json(
        server,
        "GET",
        "/api/v1/projects/10/tasks/99",
        200,
        json!({
            "single": { "id": 99, "name": task_title, "project_id": 10, "task_number": 7 },
            "tracked_time": 3.5,
            "comments": [{ "id": 1, "body": "<p>first comment</p>", "created_by_id": 42 }]
        }),
    )
    .await;
    mount_json(
        server,
        "GET",
        "/api/v1/users",
        200,
        json!([{ "id": 42, "first_name": "Ada", "last_name": "Lovelace", "email": "ada@example.com" }]),
    )
    .await;
    mount_json(
        server,
        "GET",
        &format!("/api/v1/users/{user_id}/tasks"),
        200,
        json!({ "tasks": [
            { "id": 99, "name": task_title, "project_id": 10, "task_number": 7,
              "is_completed": false, "is_trashed": false }
        ] }),
    )
    .await;
    mount_json(
        server,
        "POST",
        "/api/v1/comments/task/99",
        200,
        json!({ "single": { "id": 555 } }),
    )
    .await;
    mount_json(
        server,
        "GET",
        "/api/v1/job-types",
        200,
        json!([{ "id": 3, "name": "Development", "is_default": true }]),
    )
    .await;
    mount_json(
        server,
        "POST",
        "/api/v1/projects/10/time-records",
        200,
        json!({ "single": { "id": 777 } }),
    )
    .await;
    mount_json(
        server,
        "PUT",
        "/api/v1/complete/task/99",
        200,
        json!({ "single": { "id": 99 } }),
    )
    .await;
    mount_json(
        server,
        "PUT",
        "/api/v1/projects/10/tasks/99",
        200,
        json!({ "single": { "id": 99 } }),
    )
    .await;
    mount_json(
        server,
        "GET",
        &format!("/api/v1/users/{user_id}/time-records/filtered-by-date"),
        200,
        json!({ "time_records": [] }),
    )
    .await;
    mount_status(server, "/api/v1/projects/10/tasks/404", 404).await;
    mount_status(server, "/api/v1/projects/10/tasks/401", 401).await;
}

fn operation_schemas(tool: &Tool) -> Vec<Value> {
    let schema = Value::Object((*tool.input_schema).clone());
    schema
        .get("oneOf")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} input schema is not a oneOf union: {schema}", tool.name))
        .clone()
}

fn operation_name(variant: &Value) -> String {
    let tag = &variant["properties"]["operation"];
    tag.get("const")
        .or_else(|| tag.get("enum").and_then(|values| values.get(0)))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("variant has no operation tag: {variant}"))
        .to_owned()
}

fn required_of(variant: &Value) -> BTreeSet<String> {
    variant["required"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn expected_operations(tool: &str) -> Vec<(&'static str, BTreeSet<String>)> {
    match tool {
        "ac_read" => vec![
            ("get", set(&["operation", "ref"])),
            ("current", set(&["operation"])),
            ("mine", set(&["operation"])),
        ],
        "ac_write" => vec![
            ("comment", set(&["operation", "ref", "body"])),
            ("log_time", set(&["operation", "ref", "hours"])),
            ("set_task", set(&["operation", "ref"])),
            ("time_report", set(&["operation"])),
        ],
        other => panic!("unexpected tool {other}"),
    }
}

async fn mount_task_with_attachments(server: &MockServer) {
    let url = |name: &str| format!("{}/files/{name}", server.uri());
    mount_json(
        server,
        "GET",
        "/api/v1/projects/10/tasks/99",
        200,
        json!({
            "single": {
                "id": 99, "name": "Task with files", "project_id": 10, "task_number": 7,
                "attachments": [
                    { "name": "good.txt", "url": url("good.txt") },
                    { "name": "bad.txt", "url": url("bad.txt") }
                ]
            },
            "tracked_time": 0,
            "comments": []
        }),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/files/good.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"mocked bytes".to_vec()))
        .mount(server)
        .await;
    mount_status(server, "/files/bad.txt", 500).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn lists_three_tools_with_annotations_and_operation_unions() {
    let env = Env::new(&[]);
    let client = env.connect().await;
    let tools = client.list_all_tools().await.unwrap();

    let names: BTreeSet<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(
        names,
        set(&["ac_read", "ac_write", "ac_download_attachments"])
    );

    for tool in &tools {
        let annotations = tool.annotations.as_ref().expect("annotations");
        match tool.name.as_ref() {
            "ac_read" => assert_eq!(annotations.read_only_hint, Some(true)),
            "ac_write" => assert_eq!(annotations.destructive_hint, Some(true)),
            _ => {
                assert_eq!(annotations.read_only_hint, Some(false));
                assert_eq!(annotations.destructive_hint, Some(false));
                assert_eq!(annotations.idempotent_hint, Some(true));
                let schema = Value::Object((*tool.input_schema).clone());
                assert_eq!(schema["required"], json!(["ref"]), "{schema}");
                assert!(schema["properties"].get("attachments_dir").is_some());
                assert!(schema["properties"].get("instance").is_some());
                continue;
            }
        }

        let variants = operation_schemas(tool);
        if tool.name == "ac_read" {
            let get = variants
                .iter()
                .find(|variant| operation_name(variant) == "get")
                .expect("get variant");
            for property in ["download_attachments", "attachments_dir"] {
                assert!(
                    get["properties"].get(property).is_none(),
                    "get must not offer {property}: {get}"
                );
            }
        }
        let mut found: Vec<(String, BTreeSet<String>)> = variants
            .iter()
            .map(|variant| (operation_name(variant), required_of(variant)))
            .collect();
        found.sort();
        let mut expected: Vec<(String, BTreeSet<String>)> = expected_operations(&tool.name)
            .into_iter()
            .map(|(name, required)| (name.to_owned(), required))
            .collect();
        expected.sort();
        assert_eq!(found, expected, "{} operations", tool.name);

        for variant in &variants {
            assert!(
                variant["properties"].get("instance").is_some(),
                "{} lacks an optional instance: {variant}",
                operation_name(variant)
            );
            assert!(!required_of(variant).contains("instance"));
        }
    }
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn each_operation_matches_the_json_command() {
    let server = MockServer::start().await;
    mount_api(&server, 42, "Fetched task").await;
    let env = Env::new(&[("alpha", &server, 42)]);
    let client = env.connect().await;

    let get = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": "10/99", "refresh": true }),
    )
    .await;
    assert_eq!(get, env.cli(&["get", "10/99", "--json", "--refresh"]));
    assert!(get.contains("first comment"));

    let no_comments = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": "10/99", "no_comments": true, "refresh": true }),
    )
    .await;
    assert_eq!(
        no_comments,
        env.cli(&["get", "10/99", "--json", "--no-comments", "--refresh"])
    );
    assert!(!no_comments.contains("first comment"));

    let current = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "current", "branch": "feature/10-99" }),
    )
    .await;
    assert_eq!(current, env.cli(&["get", "10/99", "--json"]));

    let comment = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "comment", "ref": "10/99", "body": "hello" }),
    )
    .await;
    assert_eq!(
        comment,
        env.cli(&["comment", "10/99", "-m", "hello", "--json"])
    );

    let log_time = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "log_time", "ref": "10/99", "hours": 1.5, "date": "2026-11-01", "summary": "work" }),
    )
    .await;
    assert_eq!(
        log_time,
        env.cli(&[
            "time",
            "log",
            "10/99",
            "--hours",
            "1.5",
            "--date",
            "2026-11-01",
            "--summary",
            "work",
            "--json"
        ])
    );

    let status_only = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "set_task", "ref": "10/99", "status": "done" }),
    )
    .await;
    assert_eq!(
        status_only,
        env.cli(&["task", "set", "10/99", "--status", "done", "--json"])
    );

    let estimate_only = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "set_task", "ref": "10/99", "estimate": 2.5 }),
    )
    .await;
    assert_eq!(
        estimate_only,
        env.cli(&["task", "set", "10/99", "--estimate", "2.5", "--json"])
    );

    let report = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "time_report", "start": "01-11-2026", "end": "02-11-2026", "format": "csv" }),
    )
    .await;
    let written = report
        .strip_prefix("Hours report written to ")
        .and_then(|rest| rest.split(" (").next())
        .unwrap_or_else(|| panic!("unexpected report text: {report}"));
    assert!(Path::new(written).starts_with(env.dir.path()), "{written}");
    assert!(Path::new(written).exists(), "{written}");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn mine_matches_the_json_command_across_instances_and_with_instance_set() {
    let alpha = MockServer::start().await;
    let beta = MockServer::start().await;
    mount_api(&alpha, 42, "Alpha task").await;
    mount_api(&beta, 7, "Beta task").await;
    let env = Env::new(&[("alpha", &alpha, 42), ("beta", &beta, 7)]);
    let client = env.connect().await;

    let both = ok_text(&client, "ac_read", json!({ "operation": "mine" })).await;
    assert_eq!(both, env.cli(&["mine", "--json"]));
    assert!(both.contains("Alpha task") && both.contains("Beta task"));

    let only_beta = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "mine", "instance": "beta" }),
    )
    .await;
    assert_eq!(
        only_beta,
        env.cli(&["mine", "--instance", "beta", "--json"])
    );
    assert!(only_beta.contains("Beta task") && !only_beta.contains("Alpha task"));
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_core_failures_as_tool_errors_and_keeps_serving() {
    let server = MockServer::start().await;
    mount_api(&server, 42, "Fetched task").await;
    let env = Env::new(&[("alpha", &server, 42)]);
    let client = env.connect().await;

    let failures = [
        (
            "ac_read",
            json!({ "operation": "get", "ref": "not-a-ref" }),
            "cannot parse task ref",
        ),
        (
            "ac_read",
            json!({ "operation": "get", "ref": "10/404" }),
            "not found (HTTP 404)",
        ),
        (
            "ac_read",
            json!({ "operation": "get", "ref": "10/401" }),
            "Token invalid or revoked",
        ),
        (
            "ac_read",
            json!({ "operation": "get", "ref": "10/99", "instance": "ghost" }),
            "instance 'ghost' not found",
        ),
        (
            "ac_write",
            json!({ "operation": "set_task", "ref": "10/99" }),
            "at least one of",
        ),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": "10/99", "hours": 0 }),
            "hours must be a positive number",
        ),
        (
            "ac_write",
            json!({ "operation": "comment", "ref": "10/99", "body": "   " }),
            "no comment body",
        ),
    ];
    for (tool, args, expected) in failures {
        let result = call(&client, tool, args.clone()).await.unwrap();
        assert_eq!(result.is_error, Some(true), "{args} should be a tool error");
        assert!(
            text_of(&result).contains(expected),
            "{args}: expected {expected:?} in {:?}",
            text_of(&result)
        );
        let next = ok_text(
            &client,
            "ac_read",
            json!({ "operation": "get", "ref": "10/99" }),
        )
        .await;
        assert!(next.contains("Fetched task"));
    }

    let protocol_failures = [
        ("ac_read", json!({ "operation": "teleport" })),
        ("ac_read", json!({ "operation": "get" })),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": "10/99" }),
        ),
    ];
    for (tool, args) in protocol_failures {
        match call(&client, tool, args.clone()).await {
            Err(ServiceError::McpError(error)) => {
                assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{args}");
            }
            other => panic!("{args} should be invalid_params, got {other:?}"),
        }
        let next = ok_text(
            &client,
            "ac_read",
            json!({ "operation": "get", "ref": "10/99" }),
        )
        .await;
        assert!(next.contains("Fetched task"));
    }
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn download_tool_matches_the_cli_and_writes_the_files() {
    let server = MockServer::start().await;
    mount_task_with_attachments(&server).await;
    let env = Env::new(&[("alpha", &server, 42)]);
    let client = env.connect().await;
    let dest = env.dir.path().join("downloads");
    let dest_arg = dest.to_str().unwrap();

    let text = ok_text(
        &client,
        "ac_download_attachments",
        json!({ "ref": "10/99", "attachments_dir": dest_arg }),
    )
    .await;
    assert_eq!(
        text,
        env.cli(&[
            "get",
            "10/99",
            "--download-attachments",
            "--attachments-dir",
            dest_arg,
            "--json"
        ])
    );
    assert_eq!(
        std::fs::read(dest.join("good.txt")).unwrap(),
        b"mocked bytes"
    );
    assert!(text.contains("bad.txt"), "{text}");
    assert!(text.contains("HTTP 500"), "{text}");

    let default_dir = env
        .dir
        .path()
        .join("tmp")
        .join("ac-attachments")
        .join("10-99");
    let default_text = ok_text(
        &client,
        "ac_download_attachments",
        json!({ "ref": "10/99" }),
    )
    .await;
    assert_eq!(
        default_text,
        env.cli(&["get", "10/99", "--download-attachments", "--json"])
    );
    assert!(
        default_text.contains(default_dir.join("good.txt").to_str().unwrap()),
        "{default_text}"
    );
    assert!(default_dir.join("good.txt").is_file());

    let ghost = call(
        &client,
        "ac_download_attachments",
        json!({ "ref": "10/99", "instance": "ghost" }),
    )
    .await
    .unwrap();
    assert_eq!(ghost.is_error, Some(true));
    assert!(text_of(&ghost).contains("instance 'ghost' not found"));
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn download_params_are_validated_as_protocol_errors() {
    let server = MockServer::start().await;
    mount_api(&server, 42, "Fetched task").await;
    let env = Env::new(&[("alpha", &server, 42)]);
    let client = env.connect().await;

    let invalid = [
        (
            "ac_read",
            json!({ "operation": "get", "ref": "10/99", "download_attachments": true }),
        ),
        (
            "ac_read",
            json!({ "operation": "get", "ref": "10/99", "attachments_dir": "x" }),
        ),
        ("ac_download_attachments", json!({})),
        ("ac_download_attachments", json!({ "ref": 99 })),
        ("ac_read", json!({ "operation": "mine", "foo": 1 })),
        (
            "ac_read",
            json!({ "operation": "current", "no_comments": true }),
        ),
        (
            "ac_read",
            json!({ "operation": "get", "ref": "10/99", "foo": 1 }),
        ),
        (
            "ac_download_attachments",
            json!({ "ref": "10/99", "download": true }),
        ),
    ];
    for (tool, args) in invalid {
        match call(&client, tool, args.clone()).await {
            Err(ServiceError::McpError(error)) => {
                assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{args}");
            }
            other => panic!("{tool} {args} should be invalid_params, got {other:?}"),
        }
    }
    let plain = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": "10/99", "refresh": true }),
    )
    .await;
    assert_eq!(plain, env.cli(&["get", "10/99", "--json", "--refresh"]));
    client.cancel().await.unwrap();
}

struct RawSession {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    reader: BufReader<std::process::ChildStdout>,
    lines: Vec<String>,
    next_id: u64,
}

impl RawSession {
    fn start(env: &Env) -> RawSession {
        let mut child = env
            .command()
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let reader = BufReader::new(child.stdout.take().unwrap());
        let mut session = RawSession {
            child,
            stdin,
            reader,
            lines: Vec::new(),
            next_id: 1,
        };
        session.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "raw-test", "version": "0" }
            }),
        );
        session.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        session
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            let read = self.reader.read_line(&mut line).unwrap();
            assert!(
                read > 0,
                "server closed stdout before answering request {id}"
            );
            let line = line.trim_end().to_owned();
            self.lines.push(line.clone());
            let parsed: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line:?}"));
            if parsed.get("id") == Some(&json!(id)) && parsed.get("method").is_none() {
                return parsed;
            }
        }
    }

    fn tool(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))
    }

    fn finish(mut self) -> Vec<String> {
        drop(self.stdin.take());
        for line in (&mut self.reader).lines() {
            self.lines.push(line.unwrap());
        }
        let status = self.child.wait().unwrap();
        assert!(status.success(), "ac mcp exited with {status}");
        self.lines
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stdout_carries_only_protocol_frames() {
    let server = MockServer::start().await;
    mount_api(&server, 42, "Fetched task").await;
    let env = Env::new(&[("alpha", &server, 42)]);
    let mut session = RawSession::start(&env);

    let calls = [
        ("ac_read", json!({ "operation": "get", "ref": "10/99" })),
        ("ac_read", json!({ "operation": "get", "ref": "10/404" })),
        (
            "ac_read",
            json!({ "operation": "current", "branch": "feature/10-99" }),
        ),
        ("ac_read", json!({ "operation": "mine" })),
        (
            "ac_write",
            json!({ "operation": "comment", "ref": "10/99", "body": "hi" }),
        ),
        (
            "ac_write",
            json!({ "operation": "comment", "ref": "10/99", "body": "" }),
        ),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": "10/99", "hours": 1 }),
        ),
        (
            "ac_write",
            json!({ "operation": "set_task", "ref": "10/99", "status": "done" }),
        ),
        (
            "ac_write",
            json!({ "operation": "time_report", "period": "week", "format": "csv" }),
        ),
        (
            "ac_write",
            json!({ "operation": "time_report", "start": "01-11-2026", "end": "02-11-2026" }),
        ),
    ];
    for (tool, args) in calls {
        let response = session.tool(tool, args.clone());
        assert!(
            response.get("result").is_some() || response.get("error").is_some(),
            "{args}: {response}"
        );
    }

    let lines = session.finish();
    assert!(lines.len() >= 11);
    for line in lines {
        let frame: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(frame["jsonrpc"], "2.0", "not a JSON-RPC frame: {line}");
    }
}
