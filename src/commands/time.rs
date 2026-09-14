use super::{presenter, resolve};
use crate::client::{ActiveCollabClient, TimeWriteOutcome};
use crate::i18n::t;
use crate::models::JobType;
use std::io::Write;

/// Non-interactive time log (issue 0067).
///
/// Resolves the task, guards against non-positive hours, resolves the job
/// type (instance default or `--job-type` override), defaults the record
/// date to today, posts via `client.create_time_record`, and writes the
/// result to the injected writers. Returns an exit code: 0 success, 2 usage
/// error, non-zero runtime failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn time_log_core(
    task_ref: Option<&str>,
    branch: Option<&str>,
    hours: f64,
    date: Option<&str>,
    summary: Option<&str>,
    job_type_arg: Option<&str>,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if hours <= 0.0 {
        writeln!(err, "{}", t("hours must be a positive number")).ok();
        return 2;
    }

    let (project_id, task_id) = match resolve::resolve_task_ref_for_comment(task_ref, branch, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };

    let job_types = match client.fetch_job_types().await {
        Ok(v) => v,
        Err(e) => {
            presenter::write_comment_failure(&e.to_string(), json, out, err);
            return 1;
        }
    };

    let job_type_id = match resolve_job_type_id(&job_types, job_type_arg) {
        Some(id) => id,
        None => {
            presenter::write_comment_failure(
                &t("no job type available; configure job types on this instance or pass --job-type"),
                json,
                out,
                err,
            );
            return 1;
        }
    };

    let record_date = resolve_record_date(date);

    let result = client
        .create_time_record(
            project_id,
            task_id,
            hours,
            &record_date,
            job_type_id,
            summary,
        )
        .await;

    match result {
        Err(e) => {
            presenter::write_comment_failure(&e.to_string(), json, out, err);
            1
        }
        Ok(TimeWriteOutcome::Ok(record_opt)) => {
            let record_id = record_opt
                .as_ref()
                .and_then(|r| r.get("id").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            presenter::write_time_success(record_id, task_id, project_id, hours, json, out);
            0
        }
        Ok(TimeWriteOutcome::Unauthorized) => {
            presenter::write_comment_failure(&presenter::reauth_message(), json, out, err);
            1
        }
        Ok(TimeWriteOutcome::Failed(status)) => {
            let reason = format!(
                "HTTP {status} posting time record to task {project_id}/{task_id}",
                status = status,
                project_id = project_id,
                task_id = task_id
            );
            presenter::write_comment_failure(&reason, json, out, err);
            1
        }
    }
}

/// Resolve `--job-type` (matched by id or case-insensitive name) against the
/// instance's job types, falling back to the instance default when absent.
/// `None` means no job type could be resolved from either source.
fn resolve_job_type_id(job_types: &[JobType], job_type_arg: Option<&str>) -> Option<i64> {
    match job_type_arg {
        Some(arg) => job_types
            .iter()
            .find(|jt| jt.id.to_string() == arg || jt.name.eq_ignore_ascii_case(arg))
            .map(|jt| jt.id),
        None => crate::client::pick_default_job_type(job_types),
    }
}

/// Resolve the record date: the given `--date` value verbatim, or today
/// (local time, `YYYY-MM-DD`) when omitted. Kept as a small pure function so
/// tests can pass an explicit date and assert the posted body deterministically.
fn resolve_record_date(date: Option<&str>) -> String {
    date.map(str::to_owned)
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string())
}
