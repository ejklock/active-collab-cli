use super::*;
use crate::tui::model::{init_mine, Header, Screen};
use tempfile::TempDir;

fn target() -> Instance {
    Instance {
        name: "inst".to_owned(),
        base_url: "https://inst.example.com".to_owned(),
        email: "x@x.com".to_owned(),
        token: "tok".to_owned(),
        user_id: Some(42),
    }
}

fn row(
    task_id: i64,
    estimate: Option<f64>,
    logged_week: Option<f64>,
    logged_total: Option<f64>,
) -> MineTableRow {
    MineTableRow {
        instance: "inst".to_owned(),
        project_id: 1,
        task_number: task_id,
        task_list_id: None,
        task_list: None,
        labels: Vec::new(),
        task_id,
        name: format!("Task {task_id}"),
        due_on: None,
        project_name: None,
        estimate,
        logged_week,
        logged_total,
    }
}

fn round_trip(rows: Vec<MineTableRow>) -> Vec<MineTableRow> {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("snapshot.db");
    let targets = [target()];
    let (model, _) = init_mine(Header::from_instances(&[], None), Some(rows));
    write_mine_snapshot(&model, &targets, &db_path);
    read_mine_snapshot(&targets, &db_path).expect("snapshot must load")
}

#[test]
fn mine_snapshot_write_then_read_keeps_each_rows_hours() {
    let read = round_trip(vec![
        row(1, Some(4.0), Some(1.5), Some(7.25)),
        row(2, Some(2.0), Some(0.0), Some(0.0)),
    ]);
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].estimate, Some(4.0));
    assert_eq!(read[0].logged_week, Some(1.5));
    assert_eq!(read[0].logged_total, Some(7.25));
    assert_eq!(read[1].estimate, Some(2.0));
    assert_eq!(read[1].logged_week, Some(0.0));
    assert_eq!(read[1].logged_total, Some(0.0));
}

#[test]
fn mine_snapshot_write_then_read_keeps_unknown_hours_unknown() {
    let read = round_trip(vec![
        row(3, None, None, None),
        row(4, Some(1.0), None, Some(2.0)),
    ]);
    assert_eq!(read[0].estimate, None);
    assert_eq!(read[0].logged_week, None);
    assert_eq!(read[0].logged_total, None);
    assert_eq!(read[1].estimate, Some(1.0));
    assert_eq!(read[1].logged_week, None);
    assert_eq!(read[1].logged_total, Some(2.0));
}

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const HOURS_DELAY: Duration = Duration::from_millis(400);

fn named_instance(server: &MockServer, name: &str) -> Instance {
    Instance {
        name: name.to_owned(),
        base_url: server.uri(),
        email: "x@x.com".to_owned(),
        token: "fixture-token".to_owned(),
        user_id: Some(42),
    }
}

async fn mount_open_tasks(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/tasks"))
        .respond_with(response)
        .mount(server)
        .await;
}

async fn mount_delayed_records(server: &MockServer, status: u16, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/v1/users/42/time-records/filtered-by-date"))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_json(body)
                .set_delay(HOURS_DELAY),
        )
        .mount(server)
        .await;
}

async fn mount_delayed_total(server: &MockServer, task: i64, status: u16, tracked: f64) {
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/projects/5/tasks/{task}")))
        .respond_with(
            ResponseTemplate::new(status)
                .set_body_json(serde_json::json!({ "tracked_time": tracked }))
                .set_delay(HOURS_DELAY),
        )
        .mount(server)
        .await;
}

fn open_tasks_body() -> serde_json::Value {
    serde_json::json!({ "tasks": [
        { "id": 10, "task_number": 1, "name": "A", "project_id": 5, "estimate": 4.0 },
        { "id": 11, "task_number": 2, "name": "B", "project_id": 5 }
    ] })
}

/// Runs the load and returns the rows message and the hours message, failing
/// when the hours are already queued behind the rows.
async fn run_mine_load(
    targets: Vec<Instance>,
) -> (Vec<MineTableRow>, Vec<crate::commands::MineHours>) {
    let dir = TempDir::new().unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::spawn(load_mine_tasks(
        targets,
        Http::new().unwrap(),
        dir.path().join("load.db"),
        tx,
    ));
    let Some(Msg::LoadedMineTasks { rows, .. }) = rx.recv().await else {
        panic!("the rows message must come first");
    };
    assert!(
        rx.try_recv().is_err(),
        "hours arrived together with the rows"
    );
    let hours = match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
        Ok(Some(Msg::LoadedMineHours { hours })) => hours,
        _ => panic!("the hours message must follow the rows message"),
    };
    (rows, hours)
}

#[tokio::test]
async fn mine_rows_before_hours_paints_rows_without_hours_then_fills_them() {
    let server = MockServer::start().await;
    mount_open_tasks(
        &server,
        ResponseTemplate::new(200).set_body_json(open_tasks_body()),
    )
    .await;
    mount_delayed_records(
        &server,
        200,
        serde_json::json!({ "time_records": [
            { "id": 1, "value": 1.5, "record_date": 1_762_819_200, "summary": "",
              "parent_type": "Task", "parent_id": 10 }
        ], "related": [] }),
    )
    .await;
    mount_delayed_total(&server, 10, 200, 7.25).await;
    mount_delayed_total(&server, 11, 200, 2.0).await;

    let (rows, hours) = run_mine_load(vec![named_instance(&server, "a")]).await;

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].estimate, Some(4.0));
    assert!(rows
        .iter()
        .all(|r| r.logged_week.is_none() && r.logged_total.is_none()));
    assert_eq!(hours.len(), 2);
    assert_eq!(
        (
            hours[0].task_id,
            hours[0].logged_week,
            hours[0].logged_total
        ),
        (10, Some(1.5), Some(7.25))
    );
    assert_eq!(
        (
            hours[1].task_id,
            hours[1].logged_week,
            hours[1].logged_total
        ),
        (11, Some(0.0), Some(2.0))
    );
}

#[tokio::test]
async fn mine_rows_before_hours_marks_failed_requests_unknown() {
    let server = MockServer::start().await;
    mount_open_tasks(
        &server,
        ResponseTemplate::new(200).set_body_json(open_tasks_body()),
    )
    .await;
    mount_delayed_records(&server, 500, serde_json::json!({})).await;
    mount_delayed_total(&server, 10, 200, 3.0).await;
    mount_delayed_total(&server, 11, 500, 0.0).await;

    let (rows, hours) = run_mine_load(vec![named_instance(&server, "a")]).await;

    assert_eq!(rows.len(), 2);
    assert_eq!(hours.len(), 2);
    assert_eq!(
        (hours[0].logged_week, hours[0].logged_total),
        (None, Some(3.0))
    );
    assert_eq!((hours[1].logged_week, hours[1].logged_total), (None, None));
}

#[tokio::test]
async fn mine_rows_before_hours_keeps_each_instances_failures_to_itself() {
    let first = MockServer::start().await;
    let second = MockServer::start().await;
    for server in [&first, &second] {
        mount_open_tasks(
            server,
            ResponseTemplate::new(200).set_body_json(open_tasks_body()),
        )
        .await;
        mount_delayed_total(server, 10, 200, 3.0).await;
        mount_delayed_total(server, 11, 200, 4.0).await;
    }
    mount_delayed_records(&first, 500, serde_json::json!({})).await;
    mount_delayed_records(
        &second,
        200,
        serde_json::json!({ "time_records": [
            { "id": 1, "value": 2.5, "record_date": 1_762_819_200, "summary": "",
              "parent_type": "Task", "parent_id": 10 }
        ], "related": [] }),
    )
    .await;

    let (rows, hours) = run_mine_load(vec![
        named_instance(&first, "first"),
        named_instance(&second, "second"),
    ])
    .await;

    assert_eq!(rows.len(), 4);
    assert_eq!(hours.len(), 4);
    for entry in &hours[..2] {
        assert_eq!(entry.instance, "first");
        assert_eq!(entry.logged_week, None);
    }
    assert_eq!(hours[0].logged_total, Some(3.0));
    assert_eq!(hours[1].logged_total, Some(4.0));
    assert_eq!(
        (hours[2].instance.as_str(), hours[2].logged_week),
        ("second", Some(2.5))
    );
    assert_eq!(
        (hours[3].logged_week, hours[3].logged_total),
        (Some(0.0), Some(4.0))
    );
}

#[tokio::test]
async fn mine_rows_before_hours_skips_an_instance_whose_list_fails() {
    let good = MockServer::start().await;
    mount_open_tasks(
        &good,
        ResponseTemplate::new(200).set_body_json(open_tasks_body()),
    )
    .await;
    mount_delayed_records(
        &good,
        200,
        serde_json::json!({ "time_records": [], "related": [] }),
    )
    .await;
    mount_delayed_total(&good, 10, 200, 1.0).await;
    mount_delayed_total(&good, 11, 200, 2.0).await;
    let broken = MockServer::start().await;
    mount_open_tasks(&broken, ResponseTemplate::new(500)).await;

    let (rows, hours) = run_mine_load(vec![
        named_instance(&broken, "broken"),
        named_instance(&good, "good"),
    ])
    .await;

    assert!(rows.iter().all(|r| r.instance == "good"));
    assert_eq!(rows.len(), 2);
    assert!(hours.iter().all(|h| h.instance == "good"));
    assert_eq!(hours.len(), 2);
    assert_eq!(hours[0].logged_total, Some(1.0));
}

fn snapshot_model(rows: Vec<MineTableRow>) -> Model {
    init_mine(Header::from_instances(&[], None), Some(rows)).0
}

fn apply_to_snapshot(msg: Msg, model: Model, db_path: &Path) -> Model {
    let (tx, _rx) = mpsc::unbounded_channel();
    handle_channel_msg_mine(msg, model, &[target()], &Http::new().unwrap(), db_path, &tx)
}

#[test]
fn mine_snapshot_carries_the_hours_after_the_hours_message() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("snapshot.db");
    let model = snapshot_model(vec![row(1, Some(4.0), None, None)]);
    let hours = vec![crate::commands::MineHours {
        instance: "inst".to_owned(),
        task_id: 1,
        logged_week: Some(1.5),
        logged_total: Some(7.25),
    }];
    apply_to_snapshot(Msg::LoadedMineHours { hours }, model, &db_path);
    let read = read_mine_snapshot(&[target()], &db_path).expect("snapshot must load");
    assert_eq!(read[0].logged_week, Some(1.5));
    assert_eq!(read[0].logged_total, Some(7.25));
}

#[test]
fn mine_snapshot_keeps_the_earlier_hours_after_only_the_rows_message() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("snapshot.db");
    let model = snapshot_model(vec![row(1, Some(4.0), Some(2.0), Some(9.0))]);
    let msg = Msg::LoadedMineTasks {
        rows: vec![row(1, Some(5.0), None, None)],
        loaded_at: "2026-06-26T12:00:00Z".to_owned(),
    };
    apply_to_snapshot(msg, model, &db_path);
    let read = read_mine_snapshot(&[target()], &db_path).expect("snapshot must load");
    assert_eq!(read[0].estimate, Some(5.0));
    assert_eq!(read[0].logged_week, Some(2.0));
    assert_eq!(read[0].logged_total, Some(9.0));
}

#[test]
fn mine_list_under_detail_is_written_to_the_snapshot_with_the_hours() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("snapshot.db");
    let (model, _) = update(
        snapshot_model(vec![row(1, Some(4.0), None, None)]),
        Msg::Select,
    );
    assert!(matches!(model.top(), Some(Screen::Detail { .. })));
    let hours = vec![crate::commands::MineHours {
        instance: "inst".to_owned(),
        task_id: 1,
        logged_week: Some(1.5),
        logged_total: Some(7.0),
    }];
    apply_to_snapshot(Msg::LoadedMineHours { hours }, model, &db_path);
    let read = read_mine_snapshot(&[target()], &db_path).expect("snapshot must load");
    assert_eq!(read[0].logged_week, Some(1.5));
    assert_eq!(read[0].logged_total, Some(7.0));
}
