use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;

fn open_task() -> Value {
    json!({
        "id": 75159,
        "project_id": 665,
        "project_name": "Acme Project",
        "name": "Fix the widget",
        "is_completed": false,
        "assignee_id": 12,
        "start_on": 1735776000_i64,
        "due_on": 1736380800_i64,
        "estimate": 8,
        "tracked_time": 3,
        "body": "<p>Do the <b>thing</b></p>",
    })
}

fn completed_task() -> Value {
    json!({
        "id": 100,
        "project_id": 1,
        "project_name": "Beta",
        "name": "Old task",
        "is_completed": true,
        "assignee_id": Value::Null,
        "estimate": 0,
        "tracked_time": 0,
        "body": "",
    })
}

fn user_map_with_12() -> HashMap<i64, String> {
    let mut m = HashMap::new();
    m.insert(12, "Jane Doe".to_string());
    m
}

fn sample_comments() -> Vec<Value> {
    vec![json!({
        "created_by_name": "John",
        "created_by_id": 7,
        "created_on": 1736000000_i64,
        "body": "<em>Nice</em>",
    })]
}

// --- J1-A1: ref form ---

#[test]
fn task_object_ref_equals_project_id_slash_task_id() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "work", false);
    assert_eq!(obj["ref"], "665/75159", "ref must be project_id/task_id");
}

// --- J1-A1: status literal ---

#[test]
fn task_object_open_status_is_literal_open() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "work", false);
    assert_eq!(obj["status"], "open", "open task must have status 'open'");
}

#[test]
fn task_object_completed_status_is_literal_completed() {
    let obj = task_object(&completed_task(), &[], &HashMap::new(), "work", false);
    assert_eq!(
        obj["status"], "completed",
        "completed task must have status 'completed'"
    );
}

// --- J1-A1: assignee resolved name vs null ---

#[test]
fn task_object_assignee_resolved_from_user_map() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "work", false);
    assert_eq!(
        obj["assignee"], "Jane Doe",
        "assignee must be resolved name when id is in user_map"
    );
    assert_eq!(obj["assignee_id"], 12);
}

#[test]
fn task_object_assignee_null_when_id_not_in_user_map() {
    let obj = task_object(&open_task(), &[], &HashMap::new(), "work", false);
    assert_eq!(
        obj["assignee"],
        Value::Null,
        "assignee must be null when assignee_id not in user_map"
    );
    assert_eq!(
        obj["assignee_id"], 12,
        "assignee_id must still be set even when name not found"
    );
}

#[test]
fn task_object_assignee_and_id_null_when_no_assignee_id() {
    let obj = task_object(&completed_task(), &[], &HashMap::new(), "work", false);
    assert_eq!(
        obj["assignee"],
        Value::Null,
        "assignee must be null when no assignee_id"
    );
    assert_eq!(
        obj["assignee_id"],
        Value::Null,
        "assignee_id must be null when absent from task"
    );
}

// --- J1-A1 / J1-A2: no_comments => comments [] ---

#[test]
fn task_object_no_comments_flag_yields_empty_comments_array() {
    let obj = task_object(
        &open_task(),
        &sample_comments(),
        &user_map_with_12(),
        "work",
        true,
    );
    assert_eq!(
        obj["comments"],
        Value::Array(vec![]),
        "no_comments=true must produce empty comments array"
    );
}

#[test]
fn task_object_with_comments_includes_them_when_flag_false() {
    let obj = task_object(
        &open_task(),
        &sample_comments(),
        &user_map_with_12(),
        "work",
        false,
    );
    let arr = obj["comments"].as_array().unwrap();
    assert_eq!(
        arr.len(),
        1,
        "one comment must appear when no_comments=false"
    );
    assert_eq!(arr[0]["author"], "John");
    assert_eq!(arr[0]["author_id"], 7);
    assert_eq!(arr[0]["body"], "Nice");
}

// --- J1-A1: absent start_on/due_on => JSON null ---

#[test]
fn task_object_absent_dates_are_json_null() {
    let task = json!({
        "id": 1,
        "project_id": 2,
        "project_name": "",
        "name": "no dates",
        "is_completed": false,
        "estimate": 0,
        "tracked_time": 0,
        "body": "",
    });
    let obj = task_object(&task, &[], &HashMap::new(), "inst", false);
    assert_eq!(
        obj["start_on"],
        Value::Null,
        "absent start_on must be JSON null"
    );
    assert_eq!(
        obj["due_on"],
        Value::Null,
        "absent due_on must be JSON null"
    );
}

#[test]
fn task_object_present_dates_are_strings() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "work", false);
    assert!(
        obj["start_on"].as_str().is_some(),
        "present start_on must be a string"
    );
    assert!(
        obj["due_on"].as_str().is_some(),
        "present due_on must be a string"
    );
}

// --- J1-A1: assets shape ---

#[test]
fn task_object_assets_each_have_name_and_url() {
    let task = json!({
        "id": 1,
        "project_id": 2,
        "project_name": "",
        "name": "asset task",
        "is_completed": false,
        "estimate": 0,
        "tracked_time": 0,
        "body": "",
        "attachments": [{"name": "spec.pdf", "url": "https://cdn.example.com/spec.pdf"}],
    });
    let obj = task_object(&task, &[], &HashMap::new(), "inst", false);
    let assets = obj["assets"].as_array().unwrap();
    assert_eq!(assets.len(), 1, "one attachment must appear as an asset");
    assert_eq!(assets[0]["name"], "spec.pdf");
    assert_eq!(assets[0]["url"], "https://cdn.example.com/spec.pdf");
}

// --- J1-A1: description is HTML-stripped ---

#[test]
fn task_object_description_strips_html() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "work", false);
    let desc = obj["description"].as_str().unwrap();
    assert!(
        !desc.contains('<'),
        "description must not contain HTML tags"
    );
    assert!(
        desc.contains("thing"),
        "description must contain text content"
    );
}

// --- J1-A2: output is single line (no embedded newline) ---

#[test]
fn task_object_serializes_to_single_minified_line() {
    let obj = task_object(
        &open_task(),
        &sample_comments(),
        &user_map_with_12(),
        "work",
        false,
    );
    let line = serde_json::to_string(&obj).unwrap();
    assert!(
        !line.contains('\n'),
        "minified JSON must not contain newlines: {line}"
    );
    assert!(
        !line.contains("  "),
        "minified JSON must not have 2-space indent: {line}"
    );
}

// --- url field ---

#[test]
fn task_object_url_contains_project_and_task_ids() {
    let obj = task_object(
        &open_task(),
        &[],
        &user_map_with_12(),
        "https://collab.example.com",
        false,
    );
    let url = obj["url"].as_str().unwrap();
    assert!(
        url.contains("/projects/665/tasks/75159"),
        "url must contain the project and task path: {url}"
    );
    assert!(
        url.starts_with("https://collab.example.com"),
        "url must start with instance base: {url}"
    );
}

// --- instance + field presence ---

#[test]
fn task_object_carries_instance_name() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "myinst", false);
    assert_eq!(obj["instance"], "myinst");
}

#[test]
fn task_object_has_all_required_top_level_keys() {
    let obj = task_object(&open_task(), &[], &user_map_with_12(), "work", false);
    for key in &[
        "ref",
        "instance",
        "project_id",
        "task_id",
        "name",
        "status",
        "assignee",
        "assignee_id",
        "project_name",
        "start_on",
        "due_on",
        "estimate_hours",
        "logged_hours",
        "url",
        "description",
        "assets",
        "comments",
    ] {
        assert!(
            obj.get(key).is_some(),
            "task_object must contain key '{key}'"
        );
    }
}

// --- J2-A1: mine_object shape ---

fn sample_mine_rows() -> Vec<crate::render::MineTableRow> {
    vec![
        crate::render::MineTableRow {
            instance: "work".to_owned(),
            project_id: 665,
            task_number: 100,
            task_id: 75159,
            name: "Fix the widget".to_owned(),
        },
        crate::render::MineTableRow {
            instance: "personal".to_owned(),
            project_id: 10,
            task_number: 5,
            task_id: 500,
            name: "Another task".to_owned(),
        },
    ]
}

#[test]
fn mine_object_count_matches_row_count() {
    let rows = sample_mine_rows();
    let obj = mine_object(&rows);
    assert_eq!(obj["count"], 2, "count must equal number of rows");
}

#[test]
fn mine_object_tasks_array_length_matches_count() {
    let rows = sample_mine_rows();
    let obj = mine_object(&rows);
    let tasks = obj["tasks"].as_array().expect("tasks must be an array");
    assert_eq!(tasks.len(), 2, "tasks array length must equal count");
}

#[test]
fn mine_object_task_ref_is_project_id_slash_task_id() {
    let rows = sample_mine_rows();
    let obj = mine_object(&rows);
    let tasks = obj["tasks"].as_array().unwrap();
    assert_eq!(
        tasks[0]["ref"], "665/75159",
        "ref must be project_id/task_id"
    );
    assert_eq!(tasks[1]["ref"], "10/500", "ref must be project_id/task_id");
}

#[test]
fn mine_object_task_has_all_required_fields() {
    let rows = sample_mine_rows();
    let obj = mine_object(&rows);
    let task = &obj["tasks"].as_array().unwrap()[0];
    for key in &[
        "ref",
        "instance",
        "project_id",
        "task_number",
        "task_id",
        "name",
    ] {
        assert!(
            task.get(key).is_some(),
            "mine task must contain key '{key}'"
        );
    }
}

#[test]
fn mine_object_task_field_values_match_row() {
    let rows = sample_mine_rows();
    let obj = mine_object(&rows);
    let task = &obj["tasks"].as_array().unwrap()[0];
    assert_eq!(task["instance"], "work");
    assert_eq!(task["project_id"], 665);
    assert_eq!(task["task_number"], 100);
    assert_eq!(task["task_id"], 75159);
    assert_eq!(task["name"], "Fix the widget");
}

#[test]
fn mine_object_serializes_to_single_minified_line() {
    let rows = sample_mine_rows();
    let obj = mine_object(&rows);
    let line = serde_json::to_string(&obj).unwrap();
    assert!(
        !line.contains('\n'),
        "minified JSON must not contain newlines: {line}"
    );
    assert!(
        !line.contains("  "),
        "minified JSON must not have 2-space indent: {line}"
    );
}

#[test]
fn mine_object_empty_rows_yields_count_zero_and_empty_tasks() {
    let obj = mine_object(&[]);
    assert_eq!(obj["count"], 0, "empty rows must yield count 0");
    let tasks = obj["tasks"].as_array().expect("tasks must be present");
    assert!(tasks.is_empty(), "empty rows must yield empty tasks array");
}

// --- J3-A1: browse_object shape and fields ---

fn sample_task_row(
    task_id: i64,
    task_number: i64,
    name: &str,
    project_id: i64,
) -> crate::tui::model::TaskRow {
    crate::tui::model::TaskRow {
        task_id,
        task_number,
        name: name.to_owned(),
        instance: "work".to_owned(),
        project_id,
    }
}

fn sample_project_groups() -> Vec<crate::tui::model::ProjectGroup> {
    vec![
        crate::tui::model::ProjectGroup {
            project_id: 665,
            project_name: "Acme Project".to_owned(),
            instance: "work".to_owned(),
            tasks: vec![
                sample_task_row(75159, 100, "Fix the widget", 665),
                sample_task_row(75200, 101, "Add the feature", 665),
            ],
        },
        crate::tui::model::ProjectGroup {
            project_id: 10,
            project_name: "Beta Project".to_owned(),
            instance: "personal".to_owned(),
            tasks: vec![sample_task_row(500, 5, "Another task", 10)],
        },
    ]
}

#[test]
fn browse_object_has_projects_array_at_top_level() {
    let obj = browse_object(&sample_project_groups());
    assert!(
        obj.get("projects").is_some(),
        "browse_object must have a 'projects' key"
    );
    let projects = obj["projects"]
        .as_array()
        .expect("projects must be an array");
    assert_eq!(projects.len(), 2, "projects length must match group count");
}

#[test]
fn browse_object_project_has_all_required_fields() {
    let obj = browse_object(&sample_project_groups());
    let project = &obj["projects"].as_array().unwrap()[0];
    for key in &[
        "project_id",
        "project_name",
        "instance",
        "task_count",
        "tasks",
    ] {
        assert!(
            project.get(key).is_some(),
            "project must contain key '{key}'"
        );
    }
}

#[test]
fn browse_object_project_field_values_match_group() {
    let obj = browse_object(&sample_project_groups());
    let project = &obj["projects"].as_array().unwrap()[0];
    assert_eq!(project["project_id"], 665);
    assert_eq!(project["project_name"], "Acme Project");
    assert_eq!(project["instance"], "work");
}

#[test]
fn browse_object_task_count_equals_group_tasks_len() {
    let obj = browse_object(&sample_project_groups());
    let projects = obj["projects"].as_array().unwrap();
    assert_eq!(
        projects[0]["task_count"], 2,
        "task_count must equal group.tasks.len()"
    );
    assert_eq!(
        projects[1]["task_count"], 1,
        "task_count must equal group.tasks.len()"
    );
}

#[test]
fn browse_object_task_ref_uses_group_project_id_and_row_task_id() {
    let obj = browse_object(&sample_project_groups());
    let tasks = obj["projects"].as_array().unwrap()[0]["tasks"]
        .as_array()
        .unwrap();
    assert_eq!(
        tasks[0]["ref"], "665/75159",
        "ref must be group.project_id/task_id"
    );
    assert_eq!(
        tasks[1]["ref"], "665/75200",
        "ref must be group.project_id/task_id"
    );
}

#[test]
fn browse_object_task_has_all_required_fields() {
    let obj = browse_object(&sample_project_groups());
    let task = &obj["projects"].as_array().unwrap()[0]["tasks"]
        .as_array()
        .unwrap()[0];
    for key in &["ref", "task_number", "task_id", "name"] {
        assert!(task.get(key).is_some(), "task must contain key '{key}'");
    }
}

#[test]
fn browse_object_task_field_values_match_row() {
    let obj = browse_object(&sample_project_groups());
    let task = &obj["projects"].as_array().unwrap()[0]["tasks"]
        .as_array()
        .unwrap()[0];
    assert_eq!(task["task_number"], 100);
    assert_eq!(task["task_id"], 75159);
    assert_eq!(task["name"], "Fix the widget");
}

#[test]
fn browse_object_serializes_to_single_minified_line() {
    let obj = browse_object(&sample_project_groups());
    let line = serde_json::to_string(&obj).unwrap();
    assert!(
        !line.contains('\n'),
        "minified JSON must not contain newlines: {line}"
    );
    assert!(
        !line.contains("  "),
        "minified JSON must not have 2-space indent: {line}"
    );
}

#[test]
fn browse_object_empty_groups_yields_empty_projects_array() {
    let obj = browse_object(&[]);
    let projects = obj["projects"]
        .as_array()
        .expect("projects must be present");
    assert!(
        projects.is_empty(),
        "empty groups must yield empty projects array"
    );
}
