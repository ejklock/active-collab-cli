use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientRequest, CustomRequest, ErrorCode,
    GetSkillRequestParams, ReadResourceRequestParams, ResourceContents, Tool, SKILLS_EXTENSION_ID,
    TASKS_EXTENSION_ID,
};
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
            set(&[
                "ref",
                "no_comments",
                "refresh",
                "no_dependencies",
                "branch",
                "labels",
                "task_list",
                "instance",
            ]),
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
            let taken_by_every_operation = field == "instance";
            assert_eq!(
                description.ends_with(')'),
                !taken_by_every_operation,
                "{} {field} names the operations that take it exactly when not all do: {schema}",
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
        "Task URL or PROJECT_ID/TASK_ID. (comment, log_time, set_task; required)"
    );
    assert!(write["properties"]["instance"].get("description").is_none());
    assert!(read["properties"]["instance"].get("description").is_none());
    assert_eq!(
        description_of(&read, "ref"),
        "Task URL or PROJECT_ID/TASK_ID. (get; required)"
    );
    assert_eq!(
        description_of(&write, "hours"),
        "Positive hours. (log_time; required)"
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
        RawSession::start_declaring(env, json!({}))
    }

    fn start_declaring_tasks(env: &Env) -> RawSession {
        RawSession::start_declaring(env, json!({ "extensions": { TASKS_EXTENSION_ID: {} } }))
    }

    fn start_declaring(env: &Env, capabilities: Value) -> RawSession {
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
                "capabilities": capabilities,
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

    fn tool_with_progress(&mut self, name: &str, args: Value, token: &str) -> Value {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": args, "_meta": { "progressToken": token } }),
        )
    }

    fn task_state(&mut self, task: &str) -> Value {
        self.request("tasks/get", json!({ "taskId": task }))
    }

    /// Polls `tasks/get` until the task leaves `working`; returns its last state.
    fn task_terminal(&mut self, task: &str) -> Value {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let state = self.task_state(task);
            assert!(state["error"].is_null(), "tasks/get failed: {state}");
            if state["result"]["status"] != "working" {
                return state["result"].clone();
            }
            assert!(std::time::Instant::now() < deadline, "task never ended");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn progress_for(&self, token: &str) -> Vec<Value> {
        self.lines
            .iter()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|frame| frame["method"] == "notifications/progress")
            .map(|frame| frame["params"].clone())
            .filter(|params| params["progressToken"] == json!(token))
            .collect()
    }

    /// Ends the server without waiting for a call that is still running on its thread.
    fn kill(mut self) {
        drop(self.stdin.take());
        self.child.kill().unwrap();
        self.child.wait().unwrap();
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

fn text_in(result: &Value) -> String {
    result["content"]
        .as_array()
        .unwrap_or_else(|| panic!("not a tool result: {result}"))
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect()
}

/// The id of the task a call answered with; the answer must be a task, not a result.
fn task_id_of(response: &Value) -> String {
    let result = &response["result"];
    assert_eq!(result["resultType"], "task", "not a task: {response}");
    result["taskId"].as_str().expect("a task id").to_owned()
}

/// Serves the first attachment of `task` after `delay`, ahead of the ordinary route.
async fn mount_slow_attachment(server: &MockServer, task: &SeedTask, delay: std::time::Duration) {
    let file = &task.attachments[0];
    Mock::given(method("GET"))
        .and(path(format!("/files/{}", file.name)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(file.bytes.unwrap())
                .set_delay(delay),
        )
        .with_priority(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn long_calls_declared_as_tasks_complete_with_the_synchronous_result() {
    let server = MockServer::start().await;
    let task = offsite_task();
    let target = reference(&task);
    mount_seeded(&server, &Fixture::seeded()).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let dest = env.dir.path().join("downloads");
    let download = json!({ "ref": target, "attachments_dir": dest.to_str().unwrap() });
    let report = json!({ "operation": "time_report", "start": "01-11-2026", "end": "02-11-2026", "format": "csv" });

    let mut plain = RawSession::start(&env);
    let sync_download = text_in(&plain.tool("ac_download_attachments", download.clone())["result"]);
    let sync_report = text_in(&plain.tool("ac_write", report.clone())["result"]);
    plain.finish();
    std::fs::remove_dir_all(&dest).unwrap();

    let mut session = RawSession::start_declaring_tasks(&env);
    let response = session.tool("ac_download_attachments", download);
    let id = task_id_of(&response);
    let state = session.task_terminal(&id);
    assert_eq!(state["status"], "completed", "{state}");
    assert_eq!(text_in(&state["result"]), sync_download);
    let good = &task.attachments[0];
    assert_eq!(
        std::fs::read(dest.join(good.name)).unwrap(),
        good.bytes.unwrap()
    );

    let response = session.tool("ac_write", report);
    let id = task_id_of(&response);
    let state = session.task_terminal(&id);
    assert_eq!(state["status"], "completed", "{state}");
    assert_eq!(text_in(&state["result"]), sync_report);
    let written = sync_report
        .strip_prefix("Hours report written to ")
        .and_then(|rest| rest.split(" (").next())
        .unwrap_or_else(|| panic!("unexpected report text: {sync_report}"));
    assert!(Path::new(written).starts_with(env.dir.path()), "{written}");
    assert!(Path::new(written).exists(), "{written}");

    let get = session.tool(
        "ac_read",
        json!({ "operation": "get", "ref": reference(&report_task()) }),
    );
    assert_ne!(get["result"]["resultType"], "task", "{get}");
    assert!(text_in(&get["result"]).contains(COMMENT_TEXT), "{get}");
    let comment = session.tool(
        "ac_write",
        json!({ "operation": "comment", "ref": reference(&report_task()), "body": "hi" }),
    );
    assert_ne!(comment["result"]["resultType"], "task", "{comment}");
    assert_ne!(comment["result"]["isError"], json!(true), "{comment}");
    session.finish();
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_tasks_extension_long_calls_answer_in_the_call_and_a_failing_task_ends_as_the_same_error(
) {
    let server = MockServer::start().await;
    let task = offsite_task();
    let target = reference(&task);
    mount_seeded(&server, &Fixture::seeded()).await;
    mount_status(&server, "/api/v1/projects/10/tasks/500", 500).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);

    let mut plain = RawSession::start(&env);
    let download = plain.tool("ac_download_attachments", json!({ "ref": target }));
    assert_ne!(download["result"]["resultType"], "task", "{download}");
    assert!(
        text_in(&download["result"]).contains(task.name),
        "{download}"
    );
    let report = plain.tool(
        "ac_write",
        json!({ "operation": "time_report", "start": "01-11-2026", "end": "02-11-2026" }),
    );
    assert_ne!(report["result"]["resultType"], "task", "{report}");
    assert!(text_in(&report["result"]).starts_with("Hours report written to "));
    let failing = json!({ "ref": "10/500" });
    let sync_failure = plain.tool("ac_download_attachments", failing.clone());
    assert_eq!(
        sync_failure["result"]["isError"],
        json!(true),
        "{sync_failure}"
    );
    plain.finish();

    let mut session = RawSession::start_declaring_tasks(&env);
    let response = session.tool("ac_download_attachments", failing);
    let id = task_id_of(&response);
    let state = session.task_terminal(&id);
    assert_eq!(state["status"], "completed", "{state}");
    assert_eq!(state["result"]["isError"], json!(true), "{state}");
    assert_eq!(text_in(&state["result"]), text_in(&sync_failure["result"]));
    assert!(text_in(&state["result"]).contains("HTTP 500"), "{state}");
    session.finish();
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_running_task_ends_it_cancelled_and_unknown_task_ids_are_protocol_errors() {
    let server = MockServer::start().await;
    let task = offsite_task();
    mount_seeded(&server, &Fixture::seeded()).await;
    mount_slow_attachment(&server, &task, std::time::Duration::from_secs(4)).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);
    let mut session = RawSession::start_declaring_tasks(&env);

    let response = session.tool(
        "ac_download_attachments",
        json!({ "ref": reference(&task) }),
    );
    let id = task_id_of(&response);
    assert_eq!(session.task_state(&id)["result"]["status"], "working");
    let cancelled_at = std::time::Instant::now();
    let ack = session.request("tasks/cancel", json!({ "taskId": id }));
    assert!(ack["error"].is_null(), "{ack}");
    let state = session.task_terminal(&id);
    assert_eq!(state["status"], "cancelled", "{state}");
    assert!(
        cancelled_at.elapsed() < std::time::Duration::from_secs(2),
        "cancelled after {:?}",
        cancelled_at.elapsed()
    );

    for method in ["tasks/get", "tasks/cancel"] {
        let response = session.request(method, json!({ "taskId": "no-such-task" }));
        assert!(response["result"].is_null(), "{method}: {response}");
        assert!(response["error"]["code"].is_i64(), "{method}: {response}");
    }
    session.kill();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_long_task_call_with_a_progress_token_reports_progress_below_its_total_until_it_ends() {
    let server = MockServer::start().await;
    let task = offsite_task();
    mount_seeded(&server, &Fixture::seeded()).await;
    mount_slow_attachment(&server, &task, std::time::Duration::from_millis(5600)).await;
    let env = Env::new(&[("alpha", &server, Fixture::seeded().user_id)]);

    let mut plain = RawSession::start(&env);
    let answered = plain.tool_with_progress(
        "ac_download_attachments",
        json!({ "ref": reference(&task) }),
        "sync-token",
    );
    assert_ne!(answered["result"]["isError"], json!(true), "{answered}");
    let reported = plain.progress_for("sync-token");
    assert!(reported.len() >= 3, "{reported:?}");
    let progress: Vec<f64> = reported
        .iter()
        .map(|p| p["progress"].as_f64().unwrap())
        .collect();
    let totals: Vec<f64> = reported
        .iter()
        .map(|p| p["total"].as_f64().unwrap())
        .collect();
    assert!(
        progress.windows(2).all(|pair| pair[0] < pair[1]),
        "{progress:?}"
    );
    assert!(
        progress.iter().zip(&totals).all(|(p, t)| p <= t),
        "{reported:?}"
    );
    assert_eq!(progress[1], 50.0, "first heartbeat: {progress:?}");
    let running = progress.len() - 1;
    assert!(
        progress[..running].iter().zip(&totals).all(|(p, t)| p < t),
        "{reported:?}"
    );
    plain.finish();

    let mut session = RawSession::start_declaring_tasks(&env);
    let response = session.tool_with_progress(
        "ac_download_attachments",
        json!({ "ref": reference(&task) }),
        "task-token",
    );
    let id = task_id_of(&response);
    let state = session.task_terminal(&id);
    assert_eq!(state["status"], "completed", "{state}");
    let reported = session.progress_for("task-token");
    assert!(reported.len() >= 2, "{reported:?}");
    session.finish();
}

/// Upper bound on the compact JSON size of the `tools/list` result, which a
/// client pays in context on every session. Set from the measured size of the
/// trimmed list, rounded up by under 1%; it includes the one sentence on each
/// long-call tool that says what cancelling a task does, the two `mine`
/// filter arguments and the `get` argument that skips the dependencies lookup
/// (3925 bytes measured).
const MAX_TOOLS_LIST_BYTES: usize = 3950;

#[tokio::test(flavor = "multi_thread")]
async fn tools_list_stays_within_its_size_bound() {
    let env = Env::new(&[]);
    let client = env.connect().await;
    let tools = client.list_all_tools().await.unwrap();
    let size = serde_json::to_string(&tools).unwrap().len();
    assert!(
        size <= MAX_TOOLS_LIST_BYTES,
        "tools list is {size} bytes, bound is {MAX_TOOLS_LIST_BYTES}"
    );
    client.cancel().await.unwrap();
}

const SKILL_URI: &str = "skill://active-collab/SKILL.md";
const SKILL_TEXT: &str = include_str!("../.claude/skills/active-collab/SKILL.md");

fn skill_description() -> &'static str {
    SKILL_TEXT
        .lines()
        .find_map(|line| line.strip_prefix("description: "))
        .expect("the skill frontmatter has a description")
}

fn protocol_code(error: ServiceError) -> ErrorCode {
    match error {
        ServiceError::McpError(data) => data.code,
        other => panic!("expected a protocol error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_is_declared_listed_read_and_fetched() {
    let env = Env::new(&[]);
    let client = env.connect().await;

    let info = client.peer_info().expect("the server answered initialize");
    assert!(info.capabilities.tools.is_some(), "{:?}", info.capabilities);
    assert!(
        info.capabilities.resources.is_some(),
        "{:?}",
        info.capabilities
    );
    assert!(
        info.capabilities
            .extensions
            .as_ref()
            .is_some_and(|extensions| extensions.contains_key(SKILLS_EXTENSION_ID)),
        "{:?}",
        info.capabilities
    );
    let instructions = info.instructions.as_deref().expect("instructions are set");
    assert!(instructions.contains(SKILL_URI), "{instructions}");

    let resources = client.list_all_resources().await.unwrap();
    assert_eq!(resources.len(), 1, "{resources:?}");
    assert_eq!(resources[0].uri, SKILL_URI);
    assert_eq!(resources[0].mime_type.as_deref(), Some("text/markdown"));
    assert_eq!(resources[0].name, "active-collab");

    let read = client
        .read_resource(ReadResourceRequestParams::new(SKILL_URI))
        .await
        .unwrap();
    let [ResourceContents::TextResourceContents { text, .. }] = read.contents.as_slice() else {
        panic!("expected one text content, got {:?}", read.contents);
    };
    assert_eq!(text, SKILL_TEXT);

    let skills = client.list_all_skills().await.unwrap();
    assert_eq!(skills.len(), 1, "{skills:?}");
    assert_eq!(skills[0].uri, SKILL_URI);
    assert_eq!(skills[0].frontmatter.name, "active-collab");
    assert_eq!(skills[0].frontmatter.description, skill_description());

    let got = client
        .get_skill(GetSkillRequestParams::new(SKILL_URI))
        .await
        .unwrap();
    assert_eq!(got.skill.uri, SKILL_URI);
    assert_eq!(got.skill.frontmatter.name, "active-collab");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn skill_requests_for_anything_else_are_invalid_params_and_tools_keep_working() {
    let env = Env::new(&[]);
    let client = env.connect().await;

    for uri in [
        "skill://active-collab/OTHER.md",
        "skill://other/SKILL.md",
        "file:///etc/passwd",
        "skill://active-collab/../SKILL.md",
    ] {
        let error = client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
            .expect_err(uri);
        assert_eq!(protocol_code(error), ErrorCode::INVALID_PARAMS, "{uri}");
    }
    let error = client
        .get_skill(GetSkillRequestParams::new("skill://other/SKILL.md"))
        .await
        .expect_err("unknown skill");
    assert_eq!(protocol_code(error), ErrorCode::INVALID_PARAMS);

    let missing_uri =
        ClientRequest::CustomRequest(CustomRequest::new("skills/get", Some(json!({}))));
    let error = client
        .send_request(missing_uri)
        .await
        .expect_err("skills/get without a uri");
    assert_eq!(protocol_code(error), ErrorCode::INVALID_PARAMS);

    let result = call(&client, "ac_read", json!({"operation": "mine"}))
        .await
        .unwrap();
    assert!(
        result.content.iter().any(|block| block.as_text().is_some()),
        "{result:?}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn mine_filter_reaches_the_cli_and_the_mcp_read() {
    let server = MockServer::start().await;
    let fixture = Fixture::seeded();
    mount_seeded(&server, &fixture).await;
    let env = Env::new(&[("alpha", &server, fixture.user_id)]);
    let client = env.connect().await;

    let unfiltered = ok_text(&client, "ac_read", json!({ "operation": "mine" })).await;
    assert!(!unfiltered.contains(r#""tasks":[]"#), "{unfiltered}");

    let by_label = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "mine", "labels": ["nope"] }),
    )
    .await;
    assert!(by_label.contains(r#""count":0,"tasks":[]"#), "{by_label}");
    assert_eq!(by_label, env.cli(&["mine", "--label", "nope", "--json"]));

    let by_list = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "mine", "task_list": "99999" }),
    )
    .await;
    assert!(by_list.contains(r#""count":0,"tasks":[]"#), "{by_list}");
    assert_eq!(
        by_list,
        env.cli(&["mine", "--task-list", "99999", "--json"])
    );

    let beyond_i64 = env.cli(&["mine", "--task-list", "99999999999999999999", "--json"]);
    assert!(
        beyond_i64.contains(r#""count":0,"tasks":[]"#),
        "{beyond_i64}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn dependencies_are_printed_the_same_by_the_cli_and_the_mcp_read() {
    let server = MockServer::start().await;
    let fixture = Fixture::seeded();
    mount_seeded(&server, &fixture).await;
    let env = Env::new(&[("alpha", &server, fixture.user_id)]);
    let client = env.connect().await;
    let target = reference(&report_task());
    let target = target.as_str();

    let read = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": target, "refresh": true }),
    )
    .await;
    let cli = env.cli(&["get", target, "--json", "--refresh"]);
    assert_eq!(read, cli);
    let task: Value = serde_json::from_str(read.trim_end()).unwrap();
    assert_eq!(task["parents"][0]["ref"], "10/102");
    assert_eq!(task["parents"][0]["task_list"], TASK_LISTS[0].1);
    assert_eq!(task["parents"][0]["labels"][0]["name"], "STATUS:GO LIVE");
    assert_eq!(task["children"], json!([]));

    let skipped = ok_text(
        &client,
        "ac_read",
        json!({ "operation": "get", "ref": target, "no_dependencies": true }),
    )
    .await;
    assert_eq!(
        skipped,
        env.cli(&["get", target, "--json", "--no-dependencies"])
    );
    let skipped: Value = serde_json::from_str(skipped.trim_end()).unwrap();
    assert!(skipped["parents"].is_null() && skipped["children"].is_null());
    client.cancel().await.unwrap();
}

/// Runs `ac current` as if the working directory were a git checkout on
/// `branch`, by putting a `git` stand-in that reports it first on `PATH`.
fn current_on_branch(env: &Env, branch: &str, args: &[&str]) -> Value {
    use std::os::unix::fs::PermissionsExt;
    let bin = env.dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let git = bin.join("git");
    std::fs::write(&git, format!("#!/bin/sh\necho '{branch}'\n")).unwrap();
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = env
        .command()
        .env("PATH", path)
        .arg("current")
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "ac current failed: {output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

async fn dependency_requests(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.url.path().starts_with("/api/v1/dependencies/"))
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn current_dependencies_are_looked_up_unless_no_dependencies_is_passed() {
    let server = MockServer::start().await;
    let fixture = Fixture::seeded();
    mount_seeded(&server, &fixture).await;
    let env = Env::new(&[("alpha", &server, fixture.user_id)]);
    let task = report_task();
    let branch = format!("feature/{}-{}", task.project_id, task.id);

    let skipped = current_on_branch(&env, &branch, &["--json", "--no-dependencies"]);
    assert!(skipped["parents"].is_null() && skipped["children"].is_null());
    assert_eq!(dependency_requests(&server).await, 0);

    let looked_up = current_on_branch(&env, &branch, &["--json"]);
    assert!(looked_up["parents"].is_array() && looked_up["children"].is_array());
    assert_eq!(dependency_requests(&server).await, 1);
}
