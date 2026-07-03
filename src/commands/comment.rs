use super::{presenter, resolve};
use crate::client::{ActiveCollabClient, CommentWriteOutcome};
use crate::i18n::t;
use crate::store::instances::Instance;
use std::io::Write;

/// Non-interactive comment post (ADR 0040, BDR 0027).
///
/// Resolves the task, guards against an empty body, posts via
/// `client.create_comment`, and writes the result to the injected writers.
/// Returns an exit code: 0 success, 2 usage error, non-zero runtime failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn comment_core(
    task_ref: Option<&str>,
    branch: Option<&str>,
    body: &str,
    _instance: &Instance,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if body.trim().is_empty() {
        writeln!(err, "{}", t("no comment body")).ok();
        return 2;
    }

    let (project_id, task_id) = match resolve::resolve_task_ref_for_comment(task_ref, branch, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };

    let result = client.create_comment(task_id, body).await;

    match result {
        Err(e) => {
            presenter::write_comment_failure(&e.to_string(), json, out, err);
            1
        }
        Ok(CommentWriteOutcome::Ok(comment_opt)) => {
            let comment_id = comment_opt
                .as_ref()
                .and_then(|c| c.get("id").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            presenter::write_comment_success(comment_id, task_id, project_id, json, out);
            0
        }
        Ok(CommentWriteOutcome::Unauthorized) => {
            presenter::write_comment_failure(&presenter::reauth_message(), json, out, err);
            1
        }
        Ok(CommentWriteOutcome::Failed(status)) => {
            let reason = format!(
                "HTTP {status} posting comment to task {project_id}/{task_id}",
                status = status,
                project_id = project_id,
                task_id = task_id
            );
            presenter::write_comment_failure(&reason, json, out, err);
            1
        }
    }
}
