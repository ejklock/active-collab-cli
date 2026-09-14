use crate::agent_json;
use crate::i18n::t;
use std::io::Write;

pub(crate) fn write_comment_success(
    comment_id: i64,
    task_id: i64,
    project_id: i64,
    json: bool,
    out: &mut dyn Write,
) {
    if json {
        writeln!(
            out,
            "{}",
            agent_json::comment_result(comment_id, task_id, project_id)
        )
        .ok();
    } else {
        writeln!(
            out,
            "{}",
            t(&format!(
                "Comment posted (comment_id={comment_id}, task {project_id}/{task_id}).",
                comment_id = comment_id,
                project_id = project_id,
                task_id = task_id
            ))
        )
        .ok();
    }
}

pub(crate) fn write_time_success(
    time_record_id: i64,
    task_id: i64,
    project_id: i64,
    hours: f64,
    json: bool,
    out: &mut dyn Write,
) {
    if json {
        writeln!(
            out,
            "{}",
            agent_json::time_result(time_record_id, task_id, project_id, hours)
        )
        .ok();
    } else {
        writeln!(
            out,
            "{}",
            t(&format!(
                "Time logged (time_record_id={time_record_id}, {hours} hours, task {project_id}/{task_id}).",
                time_record_id = time_record_id,
                hours = hours,
                project_id = project_id,
                task_id = task_id
            ))
        )
        .ok();
    }
}

/// Human-readable summary of which fields `ac task set` applied, e.g.
/// `": status=completed, estimate=8"`, or an empty string when none did.
fn task_changes_suffix(
    completed: Option<bool>,
    assignee_id: Option<i64>,
    estimate: Option<f64>,
) -> String {
    let mut parts = Vec::new();
    if let Some(completed) = completed {
        let status = if completed { "completed" } else { "open" };
        parts.push(format!("status={status}"));
    }
    if let Some(assignee_id) = assignee_id {
        parts.push(format!("assignee_id={assignee_id}"));
    }
    if let Some(estimate) = estimate {
        parts.push(format!("estimate={estimate}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(": {}", parts.join(", "))
    }
}

pub(crate) fn write_task_success(
    task_id: i64,
    project_id: i64,
    completed: Option<bool>,
    assignee_id: Option<i64>,
    estimate: Option<f64>,
    json: bool,
    out: &mut dyn Write,
) {
    if json {
        writeln!(
            out,
            "{}",
            agent_json::task_result(task_id, project_id, completed, assignee_id, estimate)
        )
        .ok();
    } else {
        writeln!(
            out,
            "{}",
            t(&format!(
                "Task updated (task {project_id}/{task_id}){changes}.",
                project_id = project_id,
                task_id = task_id,
                changes = task_changes_suffix(completed, assignee_id, estimate),
            ))
        )
        .ok();
    }
}

pub(crate) fn write_comment_failure(
    reason: &str,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) {
    if json {
        writeln!(out, "{}", agent_json::comment_error(reason)).ok();
    } else {
        writeln!(err, "Error: {reason}").ok();
    }
}

/// The re-authentication message shown when the API reports the stored token
/// is invalid or revoked. Single-homed here so the literal appears exactly
/// once in the crate.
pub(crate) fn reauth_message() -> String {
    t("Token invalid or revoked — run `ac setup add` to re-authenticate.")
}
