use super::*;
use serde_json::json;

#[test]
fn task_deserializes_with_all_fields_present() {
    let raw = json!({
        "id": 42,
        "name": "Fix the thing",
        "task_number": 7,
        "is_completed": false,
        "is_trashed": false,
        "assignee_id": 99,
        "project_id": 5,
        "body": "Details here",
        "start_on": null,
        "due_on": 1700000000,
        "estimate": 2.5
    });
    let task: Task = serde_json::from_value(raw).unwrap();
    assert_eq!(task.id, 42);
    assert_eq!(task.name, "Fix the thing");
    assert_eq!(task.task_number, Some(7));
    assert!(!task.is_completed);
    assert_eq!(task.assignee_id, Some(99));
    assert_eq!(task.body, "Details here");
    assert_eq!(task.estimate, Some(2.5));
    assert_eq!(task.tracked_time, None);
}

#[test]
fn task_missing_optional_fields_defaults_safely() {
    let raw = json!({ "id": 1 });
    let task: Task = serde_json::from_value(raw).unwrap();
    assert_eq!(task.id, 1);
    assert_eq!(task.name, "");
    assert_eq!(task.task_number, None);
    assert!(!task.is_completed);
    assert!(!task.is_trashed);
    assert_eq!(task.assignee_id, None);
    assert_eq!(task.project_id, None);
    assert_eq!(task.body, "");
    assert_eq!(task.estimate, None);
    assert_eq!(task.tracked_time, None);
}

#[test]
fn task_explicit_null_fields_default_to_safe_values() {
    let raw = json!({
        "id": 2,
        "name": null,
        "body": null,
        "assignee_id": null,
        "project_id": null
    });
    let task: Task = serde_json::from_value(raw).unwrap();
    assert_eq!(task.name, "");
    assert_eq!(task.body, "");
    assert_eq!(task.assignee_id, None);
    assert_eq!(task.project_id, None);
}

#[test]
fn comment_deserializes_with_defaults_on_missing_fields() {
    let raw = json!({ "id": 10 });
    let comment: Comment = serde_json::from_value(raw).unwrap();
    assert_eq!(comment.id, 10);
    assert_eq!(comment.body, "");
    assert_eq!(comment.body_plain_text, "");
    assert_eq!(comment.created_by_name, "");
    assert_eq!(comment.created_by_id, None);
}

#[test]
fn comment_null_text_fields_yield_empty_strings() {
    let raw = json!({
        "id": 11,
        "body": null,
        "body_plain_text": null,
        "created_by_name": null
    });
    let comment: Comment = serde_json::from_value(raw).unwrap();
    assert_eq!(comment.body, "");
    assert_eq!(comment.body_plain_text, "");
    assert_eq!(comment.created_by_name, "");
}

#[test]
fn project_defaults_safely_on_minimal_payload() {
    let raw = json!({ "id": 3 });
    let project: Project = serde_json::from_value(raw).unwrap();
    assert_eq!(project.id, 3);
    assert_eq!(project.name, "");
    assert!(!project.is_trashed);
}

#[test]
fn project_null_name_yields_empty_string() {
    let raw = json!({ "id": 4, "name": null });
    let project: Project = serde_json::from_value(raw).unwrap();
    assert_eq!(project.name, "");
}

#[test]
fn mine_task_from_api_sets_instance_name() {
    let data = json!({
        "id": 55,
        "task_number": 3,
        "name": "A task",
        "is_completed": false,
        "is_trashed": false,
        "project_id": 10
    });
    let mine = MineTask::from_api(&data, "my-instance");
    assert_eq!(mine.id, 55);
    assert_eq!(mine.task_number, Some(3));
    assert_eq!(mine.name, "A task");
    assert_eq!(mine.project_id, Some(10));
    assert_eq!(mine.instance_name, "my-instance");
    assert!(!mine.is_completed);
    assert!(!mine.is_trashed);
}

#[test]
fn mine_task_from_api_defaults_on_missing_fields() {
    let data = json!({ "id": 1 });
    let mine = MineTask::from_api(&data, "inst");
    assert_eq!(mine.id, 1);
    assert_eq!(mine.name, "");
    assert_eq!(mine.task_number, None);
    assert_eq!(mine.project_id, None);
    assert!(!mine.is_completed);
    assert!(!mine.is_trashed);
    assert_eq!(mine.instance_name, "inst");
}

#[test]
fn mine_task_from_api_null_name_yields_empty_string() {
    let data = json!({ "id": 2, "name": null });
    let mine = MineTask::from_api(&data, "x");
    assert_eq!(mine.name, "");
}
