use rmcp::model::{CallToolRequestParams, CallToolResult, ErrorCode, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceError};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Stdio;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/ac_env.rs"]
mod ac_env;
#[path = "support/mock_collab.rs"]
mod mock_collab;

use ac_env::Env;
use mock_collab::{mount_routes, Fixture, SeedTask, COMMENT_TEXT, TASK_LISTS};

type Client = RunningService<RoleClient, ()>;

trait Connect {
    async fn connect(&self) -> Client;
}

impl Connect for Env {
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

async fn mount_status(server: &MockServer, route: &str, status: u16) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mount_seeded(server: &MockServer, fixture: &Fixture) {
    mount_routes(server, fixture, None).await;
    mount_status(server, "/api/v1/projects/10/tasks/404", 404).await;
    mount_status(server, "/api/v1/projects/10/tasks/401", 401).await;
}

fn report_task() -> SeedTask {
    Fixture::seeded().tasks.remove(0)
}

fn offsite_task() -> SeedTask {
    Fixture::seeded().tasks.remove(2)
}

fn reference(task: &SeedTask) -> String {
    format!("{}/{}", task.project_id, task.id)
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

fn input_schema(tool: &Tool) -> Value {
    Value::Object(tool.input_schema.as_ref().clone())
}

fn property_names(schema: &Value) -> BTreeSet<String> {
    schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{schema} has no properties"))
        .keys()
        .cloned()
        .collect()
}

fn expected_flat_schema(tool: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    match tool {
        "ac_read" => (
            set(&["get", "current", "mine"]),
            set(&["ref", "no_comments", "refresh", "branch", "instance"]),
        ),
        "ac_write" => (
            set(&["comment", "log_time", "set_task", "time_report"]),
            set(&[
                "ref", "body", "html", "hours", "date", "summary", "job_type", "status",
                "assignee", "estimate", "list", "period", "start", "end", "format", "instance",
            ]),
        ),
        other => panic!("unexpected tool {other}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn lists_three_tools_with_annotations_and_flat_object_schemas() {
    let env = Env::new(&[]);
    let client = env.connect().await;
    let tools = client.list_all_tools().await.unwrap();

    let names: BTreeSet<String> = tools.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(
        names,
        set(&["ac_read", "ac_write", "ac_download_attachments"])
    );

    for tool in &tools {
        let schema = input_schema(tool);
        assert_eq!(schema["type"], "object", "{} root: {schema}", tool.name);
        for keyword in ["oneOf", "anyOf", "allOf"] {
            assert!(
                schema.get(keyword).is_none(),
                "{} has a top-level {keyword}: {schema}",
                tool.name
            );
        }
        let annotations = tool.annotations.as_ref().expect("annotations");
        match tool.name.as_ref() {
            "ac_read" => assert_eq!(annotations.read_only_hint, Some(true)),
            "ac_write" => assert_eq!(annotations.destructive_hint, Some(true)),
            _ => {
                assert_eq!(annotations.read_only_hint, Some(false));
                assert_eq!(annotations.destructive_hint, Some(false));
                assert_eq!(annotations.idempotent_hint, Some(true));
                assert_eq!(schema["required"], json!(["ref"]), "{schema}");
                assert!(schema["properties"].get("attachments_dir").is_some());
                assert!(schema["properties"].get("instance").is_some());
            }
        }
    }
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn operation_tools_publish_exactly_the_fields_their_operations_take() {
    let env = Env::new(&[]);
    let client = env.connect().await;
    let tools = client.list_all_tools().await.unwrap();

    for tool in tools.iter().filter(|t| t.name != "ac_download_attachments") {
        let schema = input_schema(tool);
        let (operations, fields) = expected_flat_schema(&tool.name);
        let operation = &schema["properties"]["operation"];
        assert_eq!(operation["type"], "string", "{} {schema}", tool.name);
        let listed: BTreeSet<String> = operation["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{} operation has no enum: {schema}", tool.name))
            .iter()
            .map(|item| item.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(listed, operations, "{} operations", tool.name);
        assert_eq!(schema["required"], json!(["operation"]), "{schema}");

        let mut published = property_names(&schema);
        assert!(published.remove("operation"));
        assert_eq!(published, fields, "{} fields", tool.name);
        for field in &fields {
            let description = schema["properties"][field]["description"]
                .as_str()
                .unwrap_or_default();
            assert!(
                description.contains("Taken by:"),
                "{} {field} does not say which operations take it: {schema}",
                tool.name
            );
        }
    }
    client.cancel().await.unwrap();
}

fn description_of(schema: &Value, field: &str) -> String {
    schema["properties"][field]["description"]
        .as_str()
        .unwrap_or_else(|| panic!("{field} has no description: {schema}"))
        .to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn operation_description_names_every_operation_with_its_own_text() {
    let env = Env::new(&[]);
    let client = env.connect().await;
    let tools = client.list_all_tools().await.unwrap();
    let schema_of = |name: &str| input_schema(tools.iter().find(|t| t.name == name).unwrap());

    let read = description_of(&schema_of("ac_read"), "operation");
    let write = description_of(&schema_of("ac_write"), "operation");
    for (description, operation, text) in [
        (&read, "get", "Fetch one task with its comments"),
        (&read, "current", "Fetch the task of a git branch"),
        (&read, "mine", "List open tasks assigned to you"),
        (&write, "comment", "Post a comment as the logged-in user"),
        (&write, "log_time", "Log time against a task"),
        (
            &write,
            "set_task",
            "Edit task fields; give at least one of status, assignee, estimate, list",
        ),
        (&write, "time_report", "Write your hours report to a file"),
    ] {
        let paired = format!("{operation}: {text}");
        assert!(description.contains(&paired), "{paired}: {description}");
    }
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn field_descriptions_name_exactly_the_operations_that_take_each_field() {
    let env = Env::new(&[]);
    let client = env.connect().await;
    let tools = client.list_all_tools().await.unwrap();
    let schema_of = |name: &str| input_schema(tools.iter().find(|t| t.name == name).unwrap());
    let read = schema_of("ac_read");
    let write = schema_of("ac_write");

    assert_eq!(
        description_of(&write, "ref"),
        "Task URL or PROJECT_ID/TASK_ID. Taken by: comment (required), log_time (required), set_task (required)."
    );
    assert_eq!(
        description_of(&write, "instance"),
        "Taken by: comment, log_time, set_task, time_report."
    );
    assert_eq!(
        description_of(&read, "ref"),
        "Task URL or PROJECT_ID/TASK_ID. Taken by: get (required)."
    );
    assert_eq!(
        description_of(&write, "hours"),
        "Positive number of hours. Taken by: log_time (required)."
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn write_operations_reject_a_field_they_do_not_take_before_any_request() {
    let server = MockServer::start().await;
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let client = env.connect().await;

    for (args, unknown) in [
        (
            json!({ "operation": "comment", "ref": target, "body": "x", "hours": 3 }),
            "hours",
        ),
        (
            json!({ "operation": "set_task", "ref": target, "status": "open", "body": "x" }),
            "body",
        ),
    ] {
        match call(&client, "ac_write", args.clone()).await {
            Err(ServiceError::McpError(error)) => {
                assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{args}");
                assert!(error.message.contains(unknown), "{}", error.message);
            }
            other => panic!("{args} should be invalid_params, got {other:?}"),
        }
    }
    assert!(server.received_requests().await.unwrap().is_empty());

    ok_text(
        &client,
        "ac_write",
        json!({ "operation": "comment", "ref": target, "body": "posted once" }),
    )
    .await;
    let posts = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method.as_str() == "POST")
        .count();
    assert_eq!(posts, 1);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn each_operation_matches_the_json_command() {
    let server = MockServer::start().await;
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let client = env.connect().await;

    let get = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": target, "refresh": true }),
    )
    .await;
    assert_eq!(get, env.cli(&["get", target, "--json", "--refresh"]));
    assert!(get.contains(COMMENT_TEXT));

    let no_comments = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": target, "no_comments": true, "refresh": true }),
    )
    .await;
    assert_eq!(
        no_comments,
        env.cli(&["get", target, "--json", "--no-comments", "--refresh"])
    );
    assert!(!no_comments.contains(COMMENT_TEXT));

    let current = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "current", "branch": format!("feature/{}-{}", task.project_id, task.id) }),
    )
    .await;
    assert_eq!(current, env.cli(&["get", target, "--json"]));

    let comment = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "comment", "ref": target, "body": "hello" }),
    )
    .await;
    assert_eq!(
        comment,
        env.cli(&["comment", target, "-m", "hello", "--json"])
    );

    let log_time = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "log_time", "ref": target, "hours": 1.5, "date": "2026-11-01", "summary": "work" }),
    )
    .await;
    assert_eq!(
        log_time,
        env.cli(&[
            "time",
            "log",
            target,
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
        json!({ "operation": "set_task", "ref": target, "status": "done" }),
    )
    .await;
    assert_eq!(
        status_only,
        env.cli(&["task", "set", target, "--status", "done", "--json"])
    );

    let estimate_only = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "set_task", "ref": target, "estimate": 2.5 }),
    )
    .await;
    assert_eq!(
        estimate_only,
        env.cli(&["task", "set", target, "--estimate", "2.5", "--json"])
    );

    let (review_id, review) = TASK_LISTS[2];
    let list_move = ok_text(
        &client,
        "ac_write",
        json!({ "operation": "set_task", "ref": target, "list": review.to_lowercase() }),
    )
    .await;
    assert_eq!(
        list_move,
        env.cli(&[
            "task",
            "set",
            target,
            "--list",
            &review.to_lowercase(),
            "--json"
        ])
    );
    assert!(list_move.contains(&format!(
        r#""task_list_id":{review_id},"task_list":"{review}""#
    )));

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
async fn set_task_with_an_unknown_list_sends_no_write() {
    let server = MockServer::start().await;
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let client = env.connect().await;

    let result = call(
        &client,
        "ac_write",
        json!({ "operation": "set_task", "ref": target, "list": "Backlog" }),
    )
    .await
    .unwrap();

    assert_eq!(result.is_error, Some(true));
    assert!(
        text_of(&result).contains(&format!("{} {}", TASK_LISTS[1].0, TASK_LISTS[1].1)),
        "{}",
        text_of(&result)
    );
    let writes = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method.as_str() == "PUT")
        .count();
    assert_eq!(writes, 0);
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn mine_matches_the_json_command_across_instances_and_with_instance_set() {
    let alpha = MockServer::start().await;
    let beta = MockServer::start().await;
    let alpha_fixture = Fixture::seeded();
    let beta_fixture = Fixture {
        user_id: alpha_fixture.user_id + 1,
        ..Fixture::seeded()
    };
    mount_seeded(&alpha, &alpha_fixture).await;
    mount_seeded(&beta, &beta_fixture).await;
    let env = Env::new(&[
        ("alpha", &alpha, alpha_fixture.user_id),
        ("beta", &beta, beta_fixture.user_id),
    ]);
    let client = env.connect().await;

    let both = ok_text(&client, "ac_read", json!({ "operation": "mine" })).await;
    assert_eq!(both, env.cli(&["mine", "--json"]));
    assert!(both.contains(r#""instance":"alpha""#) && both.contains(r#""instance":"beta""#));

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
    assert!(
        only_beta.contains(r#""instance":"beta""#) && !only_beta.contains(r#""instance":"alpha""#)
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_core_failures_as_tool_errors_and_keeps_serving() {
    let server = MockServer::start().await;
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
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
            json!({ "operation": "get", "ref": target, "instance": "ghost" }),
            "instance 'ghost' not found",
        ),
        (
            "ac_write",
            json!({ "operation": "set_task", "ref": target }),
            "at least one of",
        ),
        (
            "ac_write",
            json!({ "operation": "set_task", "ref": target, "list": "Backlog" }),
            "no task list matches",
        ),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": target, "hours": 0 }),
            "hours must be a positive number",
        ),
        (
            "ac_write",
            json!({ "operation": "comment", "ref": target, "body": "   " }),
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
            json!({ "operation": "get", "ref": target }),
        )
        .await;
        assert!(next.contains(task.name));
    }

    let protocol_failures = [
        ("ac_read", json!({ "operation": "teleport" })),
        ("ac_read", json!({ "operation": "get" })),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": target }),
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
            json!({ "operation": "get", "ref": target }),
        )
        .await;
        assert!(next.contains(task.name));
    }
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn download_tool_matches_the_cli_and_writes_the_files() {
    let server = MockServer::start().await;
    let task = offsite_task();
    let target = reference(&task);
    let target = target.as_str();
    let (good, bad) = (&task.attachments[0], &task.attachments[1]);
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let client = env.connect().await;
    let dest = env.dir.path().join("downloads");
    let dest_arg = dest.to_str().unwrap();

    let text = ok_text(
        &client,
        "ac_download_attachments",
        json!({ "ref": target, "attachments_dir": dest_arg }),
    )
    .await;
    assert_eq!(
        text,
        env.cli(&[
            "get",
            target,
            "--download-attachments",
            "--attachments-dir",
            dest_arg,
            "--json"
        ])
    );
    assert_eq!(
        std::fs::read(dest.join(good.name)).unwrap(),
        good.bytes.unwrap()
    );
    assert!(text.contains(bad.name), "{text}");
    assert!(text.contains("HTTP 500"), "{text}");

    let default_dir = env
        .dir
        .path()
        .join("tmp")
        .join("ac-attachments")
        .join(format!("{}-{}", task.project_id, task.id));
    let default_text = ok_text(&client, "ac_download_attachments", json!({ "ref": target })).await;
    assert_eq!(
        default_text,
        env.cli(&["get", target, "--download-attachments", "--json"])
    );
    assert!(
        default_text.contains(default_dir.join(good.name).to_str().unwrap()),
        "{default_text}"
    );
    assert!(default_dir.join(good.name).is_file());

    let ghost = call(
        &client,
        "ac_download_attachments",
        json!({ "ref": target, "instance": "ghost" }),
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
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let client = env.connect().await;

    let invalid = [
        (
            "ac_read",
            json!({ "operation": "get", "ref": target, "download_attachments": true }),
        ),
        (
            "ac_read",
            json!({ "operation": "get", "ref": target, "attachments_dir": "x" }),
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
            json!({ "operation": "get", "ref": target, "foo": 1 }),
        ),
        (
            "ac_download_attachments",
            json!({ "ref": target, "download": true }),
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
        json!({ "operation": "get", "ref": target, "refresh": true }),
    )
    .await;
    assert_eq!(plain, env.cli(&["get", target, "--json", "--refresh"]));
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn malformed_operation_arguments_are_protocol_errors() {
    let server = MockServer::start().await;
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let client = env.connect().await;

    let invalid = [
        ("ac_read", json!({})),
        ("ac_read", json!({ "operation": "nope" })),
        ("ac_read", json!({ "operation": "get" })),
        ("ac_read", json!({ "operation": "mine", "ref": target })),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": target, "hours": "2" }),
        ),
    ];
    for (tool, args) in invalid {
        match call(&client, tool, args.clone()).await {
            Err(ServiceError::McpError(error)) => {
                assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{args}");
                assert!(error.message.starts_with("invalid arguments:"), "{args}");
            }
            other => panic!("{tool} {args} should be invalid_params, got {other:?}"),
        }
    }
    let task_json = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": target }),
    )
    .await;
    assert!(task_json.contains(&task.id.to_string()), "{task_json}");
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
    let task = report_task();
    let target = reference(&task);
    let target = target.as_str();
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let mut session = RawSession::start(&env);

    let calls = [
        ("ac_read", json!({ "operation": "get", "ref": target })),
        ("ac_read", json!({ "operation": "get", "ref": "10/404" })),
        (
            "ac_read",
            json!({ "operation": "current", "branch": format!("feature/{}-{}", task.project_id, task.id) }),
        ),
        ("ac_read", json!({ "operation": "mine" })),
        (
            "ac_write",
            json!({ "operation": "comment", "ref": target, "body": "hi" }),
        ),
        (
            "ac_write",
            json!({ "operation": "comment", "ref": target, "body": "" }),
        ),
        (
            "ac_write",
            json!({ "operation": "log_time", "ref": target, "hours": 1 }),
        ),
        (
            "ac_write",
            json!({ "operation": "set_task", "ref": target, "status": "done" }),
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
