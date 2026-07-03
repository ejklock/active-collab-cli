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
