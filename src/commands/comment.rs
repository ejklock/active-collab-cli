use super::attach::{upload_files, UploadReporter};
use super::{presenter, resolve};
use crate::client::{ActiveCollabClient, BodyFormat, CommentWriteOutcome};
use crate::i18n::t;
use crate::store::instances::Instance;
use std::io::Write;
use std::path::PathBuf;

/// Non-interactive comment post.
///
/// Resolves the task, guards against an empty body, posts via
/// `client.create_comment` — as HTML-encoded plain text, or unchanged when
/// `html` is set — and writes the result to the injected
/// writers. Returns an exit code: 0 success, 2 usage error, non-zero runtime
/// failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn comment_core(
    task_ref: Option<&str>,
    branch: Option<&str>,
    body: &str,
    html: bool,
    attachments: &[PathBuf],
    reporter: &UploadReporter,
    _instance: &Instance,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if body.trim().is_empty() && attachments.is_empty() {
        writeln!(err, "{}", t("no comment body")).ok();
        return 2;
    }

    let (project_id, task_id) = match resolve::resolve_task_ref_for_comment(task_ref, branch, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };

    let format = if html {
        BodyFormat::Html
    } else {
        BodyFormat::Text
    };
    let uploaded = match upload_files(client, attachments, reporter).await {
        Ok(uploaded) => uploaded,
        Err(failure) => {
            presenter::write_comment_failure(&failure.message(), json, out, err);
            return failure.exit_code();
        }
    };
    let result = client
        .create_comment(task_id, body, format, &uploaded.codes)
        .await;
    write_post_outcome(
        result,
        (project_id, task_id),
        &uploaded.names,
        json,
        out,
        err,
    )
}

/// Write what the comment POST returned and give its exit code.
fn write_post_outcome(
    result: anyhow::Result<CommentWriteOutcome>,
    (project_id, task_id): (i64, i64),
    attachments: &[String],
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
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
            presenter::write_comment_success(
                comment_id,
                task_id,
                project_id,
                attachments,
                json,
                out,
            );
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
