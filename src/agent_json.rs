use crate::board::Phase;
use crate::client::TaskList;
use crate::controller::extract_assets;
use crate::models::{parse_labels, MineTask};
use crate::render::buckets::{due_bucket, group_by_bucket};
use crate::render::{fmt_date, fmt_hours, fmt_ts, html_to_text};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Build the get/current task object for agent/LLM consumption.
///
/// Pure: no network, no I/O.  All fields are derived from the same helpers
/// used by the human renderer so JSON and text stay in sync.
pub fn task_object(
    task: &Value,
    comments: &[Value],
    user_map: &HashMap<i64, String>,
    instance: &str,
    no_comments: bool,
) -> Value {
    let project_id = extract_i64(task, "project_id");
    let task_id = extract_i64(task, "id");
    let ref_ = format!("{}/{}", project_id, task_id);

    let assignee_id = task.get("assignee_id").and_then(|v| v.as_i64());
    let assignee = resolve_assignee(assignee_id, user_map);

    json!({
        "ref": ref_,
        "instance": instance,
        "project_id": project_id,
        "task_id": task_id,
        "name": task.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "status": status_literal(task),
        "assignee": assignee,
        "assignee_id": assignee_id,
        "project_name": task.get("project_name").and_then(|v| v.as_str()).unwrap_or(""),
        "start_on": date_or_null(task, "start_on"),
        "due_on": date_or_null(task, "due_on"),
        "estimate_hours": fmt_hours(task.get("estimate").unwrap_or(&Value::Null)),
        "logged_hours": fmt_hours(task.get("tracked_time").unwrap_or(&Value::Null)),
        "url": build_task_url(instance, project_id, task_id),
        "description": html_to_text(task.get("body").and_then(|v| v.as_str()).unwrap_or("")),
        "assets": shape_assets(task, comments),
        "comments": shape_comments(comments, no_comments),
        "task_list_id": task.get("task_list_id").and_then(|v| v.as_i64()),
        "task_list": task
            .get("task_list")
            .and_then(|list| list.get("name"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty()),
        "labels": parse_labels(task),
    })
}

/// One parent or child task of `get --json`; `task_list` is the resolved list
/// name, `None` when it could not be resolved.
pub fn dependency_object(dep: &MineTask, task_list: Option<&str>) -> Value {
    json!({
        "ref": format!("{}/{}", dep.project_id, dep.id),
        "project_id": dep.project_id,
        "task_id": dep.id,
        "task_number": dep.task_number,
        "name": dep.name,
        "status": if dep.is_completed { "completed" } else { "open" },
        "task_list_id": dep.task_list_id,
        "task_list": task_list,
        "labels": dep.labels,
    })
}

fn extract_i64(obj: &Value, key: &str) -> i64 {
    obj.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
}

fn status_literal(task: &Value) -> &'static str {
    if task
        .get("is_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "completed"
    } else {
        "open"
    }
}

fn resolve_assignee(id: Option<i64>, user_map: &HashMap<i64, String>) -> Value {
    match id {
        None => Value::Null,
        Some(i) => match user_map.get(&i) {
            Some(name) => Value::String(name.clone()),
            None => Value::Null,
        },
    }
}

fn date_or_null(task: &Value, key: &str) -> Value {
    let raw = task.get(key).unwrap_or(&Value::Null);
    let s = fmt_date(raw);
    if s.is_empty() {
        Value::Null
    } else {
        Value::String(s)
    }
}

fn build_task_url(instance_base: &str, project_id: i64, task_id: i64) -> String {
    let base = instance_base.trim_end_matches('/');
    format!("{base}/projects/{project_id}/tasks/{task_id}")
}

fn shape_assets(task: &Value, comments: &[Value]) -> Value {
    let assets = extract_assets(task, comments);
    Value::Array(
        assets
            .into_iter()
            .map(|a| json!({ "name": a.name, "url": a.url }))
            .collect(),
    )
}

fn shape_comments(comments: &[Value], no_comments: bool) -> Value {
    if no_comments {
        return Value::Array(vec![]);
    }
    Value::Array(comments.iter().map(shape_comment).collect())
}

fn shape_comment(c: &Value) -> Value {
    let author_id = c.get("created_by_id").and_then(|v| v.as_i64());
    let author = c
        .get("created_by_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| Value::String(s.to_owned()))
        .unwrap_or_else(|| match author_id {
            Some(id) => Value::String(id.to_string()),
            None => Value::Null,
        });
    let created_on = fmt_ts(c.get("created_on").unwrap_or(&Value::Null));
    let body = html_to_text(c.get("body").and_then(|v| v.as_str()).unwrap_or(""));
    json!({
        "author": author,
        "author_id": author_id,
        "created_on": created_on,
        "body": body,
    })
}

/// Build the write-result for a newly posted comment.
///
/// Returns a minified single-line JSON string on success:
/// `{"ok":true,"comment_id":N,"task_id":N,"project_id":N}`
pub fn comment_result(comment_id: i64, task_id: i64, project_id: i64) -> String {
    format!(
        r#"{{"ok":true,"comment_id":{comment_id},"task_id":{task_id},"project_id":{project_id}}}"#,
        comment_id = comment_id,
        task_id = task_id,
        project_id = project_id,
    )
}

/// Build the write-result for a comment posted with attachments: the
/// `comment_result` fields plus the attached file names in upload order.
pub fn attached_comment_result(
    comment_id: i64,
    task_id: i64,
    project_id: i64,
    attachments: &[String],
) -> String {
    serde_json::json!({
        "ok": true,
        "comment_id": comment_id,
        "task_id": task_id,
        "project_id": project_id,
        "attachments": attachments,
    })
    .to_string()
}

/// Build the write-result for files attached directly to a task.
pub fn task_attach_result(task_id: i64, project_id: i64, attachments: &[String]) -> String {
    serde_json::json!({
        "ok": true,
        "task_id": task_id,
        "project_id": project_id,
        "attachments": attachments,
    })
    .to_string()
}

/// Build the error object for a failed comment write.
///
/// Returns a minified single-line JSON string:
/// `{"ok":false,"error":"<reason>"}`
pub fn comment_error(reason: &str) -> String {
    let escaped = serde_json::to_string(reason).unwrap_or_else(|_| r#""error""#.to_owned());
    format!(r#"{{"ok":false,"error":{escaped}}}"#, escaped = escaped)
}

/// Build the write-result for a newly posted time record.
///
/// Returns a minified single-line JSON string on success:
/// `{"ok":true,"time_record_id":N,"task_id":N,"project_id":N,"hours":H}`
pub fn time_result(time_record_id: i64, task_id: i64, project_id: i64, hours: f64) -> String {
    format!(
        r#"{{"ok":true,"time_record_id":{time_record_id},"task_id":{task_id},"project_id":{project_id},"hours":{hours}}}"#,
        time_record_id = time_record_id,
        task_id = task_id,
        project_id = project_id,
        hours = hours,
    )
}

/// Build the write-result for an applied task field edit.
///
/// Returns a minified single-line JSON string on success, carrying only the
/// fields the caller actually applied:
/// `{"ok":true,"task_id":N,"project_id":N[,"status":"completed"|"open"][,"assignee_id":N][,"estimate":H][,"task_list_id":N,"task_list":"name"]}`
pub fn task_result(
    task_id: i64,
    project_id: i64,
    completed: Option<bool>,
    assignee_id: Option<i64>,
    estimate: Option<f64>,
    task_list: Option<&TaskList>,
) -> String {
    let mut line = format!(
        r#"{{"ok":true,"task_id":{task_id},"project_id":{project_id}"#,
        task_id = task_id,
        project_id = project_id,
    );
    if let Some(completed) = completed {
        let status = if completed { "completed" } else { "open" };
        line.push_str(&format!(r#","status":"{status}""#, status = status));
    }
    if let Some(assignee_id) = assignee_id {
        line.push_str(&format!(
            r#","assignee_id":{assignee_id}"#,
            assignee_id = assignee_id
        ));
    }
    if let Some(estimate) = estimate {
        line.push_str(&format!(r#","estimate":{estimate}"#, estimate = estimate));
    }
    if let Some(list) = task_list {
        line.push_str(&format!(
            r#","task_list_id":{id},"task_list":{name}"#,
            id = list.id,
            name = Value::from(list.name.as_str()),
        ));
    }
    line.push('}');
    line
}

/// Build the browse schema for agent/LLM consumption.
///
/// Pure: no network, no I/O. Accepts the same `ProjectGroup` slice the TUI
/// browser receives so JSON and TUI stay in sync.
pub fn browse_object(groups: &[crate::tui::model::ProjectGroup]) -> Value {
    json!({
        "projects": groups.iter().map(browse_project_object).collect::<Vec<_>>(),
    })
}

fn browse_project_object(group: &crate::tui::model::ProjectGroup) -> Value {
    json!({
        "project_id": group.project_id,
        "project_name": group.project_name,
        "instance": group.instance,
        "task_count": group.tasks.len(),
        "tasks": group.tasks.iter().map(|row| browse_task_object(group.project_id, row)).collect::<Vec<_>>(),
    })
}

fn browse_task_object(group_project_id: i64, row: &crate::tui::model::TaskRow) -> Value {
    let ref_ = format!("{}/{}", group_project_id, row.task_id);
    json!({
        "ref": ref_,
        "task_number": row.task_number,
        "task_id": row.task_id,
        "name": row.name,
    })
}

/// Build the mine schema for agent/LLM consumption.
///
/// Pure: no network, no I/O. Accepts the same `MineTableRow` slice the human
/// renderer receives so JSON and table stay in sync.
pub fn mine_object(rows: &[crate::render::MineTableRow]) -> Value {
    mine_object_on(rows, chrono::Local::now().date_naive())
}

/// `mine_object` with an explicit "today" for the due-date buckets.
pub fn mine_object_on(rows: &[crate::render::MineTableRow], today: chrono::NaiveDate) -> Value {
    let buckets: Vec<Value> = group_by_bucket(rows, today)
        .iter()
        .map(|group| {
            json!({
                "key": group.bucket.key(),
                "count": group.items.len(),
                "estimate_hours": hours_or_null(group.sums.estimate),
                "logged_week_hours": hours_or_null(group.sums.logged_week),
                "logged_total_hours": hours_or_null(group.sums.logged_total),
            })
        })
        .collect();
    let phases: Vec<Value> = Phase::ALL
        .iter()
        .map(|phase| {
            json!({
                "key": phase.key(),
                "count": rows.iter().filter(|row| row.phase == *phase).count(),
            })
        })
        .collect();
    json!({
        "count": rows.len(),
        "tasks": rows.iter().map(|row| mine_task_object(row, today)).collect::<Vec<_>>(),
        "buckets": buckets,
        "phases": phases,
    })
}

fn mine_task_object(row: &crate::render::MineTableRow, today: chrono::NaiveDate) -> Value {
    let ref_ = format!("{}/{}", row.project_id, row.task_id);
    json!({
        "due_bucket": due_bucket(row.due_on.as_deref(), today).key(),
        "ref": ref_,
        "instance": row.instance,
        "project_id": row.project_id,
        "task_number": row.task_number,
        "task_id": row.task_id,
        "name": row.name,
        "due_on": row.due_on,
        "estimate_hours": hours_or_null(row.estimate),
        "logged_week_hours": hours_or_null(row.logged_week),
        "logged_total_hours": hours_or_null(row.logged_total),
        "task_list_id": row.task_list_id,
        "task_list": row.task_list,
        "labels": row.labels,
        "phase": row.phase.key(),
        "flagged": row.flagged,
    })
}

/// Hours formatted as in the task JSON (`fmt_hours`), or null when unknown.
fn hours_or_null(hours: Option<f64>) -> Value {
    match hours {
        Some(h) => Value::String(fmt_hours(&json!(h))),
        None => Value::Null,
    }
}

#[cfg(test)]
#[path = "../tests/unit/agent_json.rs"]
mod tests;
