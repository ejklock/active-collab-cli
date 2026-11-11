use super::*;
use crate::tui::model::{init_mine, Header};
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
