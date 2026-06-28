use crate::controller::extract_assets;
use crate::render::{fmt_date, fmt_hours, fmt_ts, html_to_text};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Build the ADR 0011 get/current task object for agent/LLM consumption.
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

/// Build the ADR 0011 browse schema for agent/LLM consumption.
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

/// Build the ADR 0011 mine schema for agent/LLM consumption.
///
/// Pure: no network, no I/O. Accepts the same `MineTableRow` slice the human
/// renderer receives so JSON and table stay in sync.
pub fn mine_object(rows: &[crate::render::MineTableRow]) -> Value {
    json!({
        "count": rows.len(),
        "tasks": rows.iter().map(mine_task_object).collect::<Vec<_>>(),
    })
}

fn mine_task_object(row: &crate::render::MineTableRow) -> Value {
    let ref_ = format!("{}/{}", row.project_id, row.task_id);
    json!({
        "ref": ref_,
        "instance": row.instance,
        "project_id": row.project_id,
        "task_number": row.task_number,
        "task_id": row.task_id,
        "name": row.name,
    })
}

#[cfg(test)]
#[path = "../tests/unit/agent_json.rs"]
mod tests;
