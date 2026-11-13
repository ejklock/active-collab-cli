use serde_json::Value;
use std::process::Output;
use wiremock::MockServer;

#[path = "support/ac_env.rs"]
mod ac_env;
#[path = "support/mock_collab.rs"]
mod mock_collab;

use ac_env::Env;
use mock_collab::{mount_routes, Fixture, COMMENT_TEXT};

const TASK_ROUTE: &str = "/api/v1/projects/10/tasks/101";

async fn seeded_env() -> (MockServer, Env) {
    let fixture = Fixture::seeded();
    let server = MockServer::start().await;
    mount_routes(&server, &fixture, None).await;
    let env = Env::new(&[("alpha", &server, fixture.user_id)]);
    (server, env)
}

fn run(env: &Env, args: &[&str]) -> Output {
    env.command().args(args).output().unwrap()
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn json_of(env: &Env, args: &[&str]) -> Value {
    serde_json::from_str(&env.cli(args)).unwrap()
}

async fn task_puts(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.method.as_str() == "PUT" && request.url.path() == TASK_ROUTE)
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .collect()
}

#[tokio::test]
async fn get_prints_the_task_with_assignee_and_comment() {
    let (_server, env) = seeded_env().await;
    let text = env.cli(&["get", "10/101"]);
    let assignee = text
        .lines()
        .find(|line| line.starts_with("Assignee:"))
        .unwrap_or_else(|| panic!("no assignee line in {text}"));
    assert!(assignee.contains("Ada Lovelace"), "{assignee:?} in {text}");
    for expected in ["Draft the quarterly report", COMMENT_TEXT] {
        assert!(text.contains(expected), "missing {expected:?} in {text}");
    }
}

#[tokio::test]
async fn get_json_carries_the_task_with_assignee_comment_and_attachment() {
    let (_server, env) = seeded_env().await;
    let json = json_of(&env, &["get", "10/101", "--json"]);
    assert_eq!(json["name"], "Draft the quarterly report", "{json}");
    assert_eq!(json["assignee"], "Ada Lovelace", "{json}");
    assert_eq!(json["comments"][0]["body"], COMMENT_TEXT, "{json}");
    let assets = json["assets"]
        .as_array()
        .unwrap_or_else(|| panic!("{json}"));
    assert!(
        assets.iter().any(|asset| asset["name"] == "report.txt"),
        "{json}"
    );
}

#[tokio::test]
async fn get_of_an_unseeded_task_fails_with_not_found() {
    let (_server, env) = seeded_env().await;
    let output = run(&env, &["get", "10/999"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        stderr_of(&output).to_lowercase().contains("not found"),
        "{output:?}"
    );
}

#[tokio::test]
async fn mine_json_lists_the_seeded_tasks_with_their_hours() {
    let (_server, env) = seeded_env().await;
    let json = json_of(&env, &["mine", "--json"]);
    let tasks = json["tasks"].as_array().unwrap_or_else(|| panic!("{json}"));
    let mut rows: Vec<(i64, Value, Value, Value)> = tasks
        .iter()
        .map(|task| {
            (
                task["task_id"].as_i64().unwrap(),
                task["estimate_hours"].clone(),
                task["logged_week_hours"].clone(),
                task["logged_total_hours"].clone(),
            )
        })
        .collect();
    rows.sort_by_key(|row| row.0);
    let hours = |id, estimate: Option<&str>, week, total| {
        (
            id,
            estimate.map_or(Value::Null, Value::from),
            Value::from(week),
            Value::from(total),
        )
    };
    let expected = [
        hours(101, Some("8"), "2.5", "6"),
        hours(102, Some("4"), "0", "3"),
        hours(103, None, "0", "0"),
    ];
    assert_eq!(rows, expected, "{json}");
}

#[tokio::test]
async fn mine_prints_the_titles_and_the_totals_line() {
    let (_server, env) = seeded_env().await;
    let text = env.cli(&["mine"]);
    for task in Fixture::seeded().tasks {
        assert!(
            text.contains(task.name),
            "missing {:?} in {text}",
            task.name
        );
    }
    assert!(text.contains("EST 12 · WEEK 2.5 · TOTAL 9"), "{text}");
}

#[tokio::test]
async fn task_set_list_by_lower_case_name_moves_the_task() {
    let (server, env) = seeded_env().await;
    let output = run(&env, &["task", "set", "10/101", "--list", "review"]);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout_of(&output).contains("list=Review"), "{output:?}");
    let puts = task_puts(&server).await;
    assert_eq!(puts.len(), 1, "{puts:?}");
    assert_eq!(puts[0]["task_list_id"], 6, "{puts:?}");
}

#[tokio::test]
async fn task_set_list_by_id_moves_the_task() {
    let (server, env) = seeded_env().await;
    let output = run(&env, &["task", "set", "10/101", "--list", "5", "--json"]);
    assert!(output.status.success(), "{output:?}");
    let json: Value = serde_json::from_str(&stdout_of(&output)).unwrap();
    assert_eq!(json["task_list_id"], 5, "{json}");
    assert_eq!(json["task_list"], "In progress", "{json}");
    let puts = task_puts(&server).await;
    assert_eq!(puts.len(), 1, "{puts:?}");
    assert_eq!(puts[0]["task_list_id"], 5, "{puts:?}");
}

#[tokio::test]
async fn task_set_list_with_an_unknown_name_fails_and_sends_no_update() {
    let (server, env) = seeded_env().await;
    let output = run(&env, &["task", "set", "10/101", "--list", "nowhere"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(stderr_of(&output).contains("nowhere"), "{output:?}");
    assert_eq!(task_puts(&server).await.len(), 0);
}
