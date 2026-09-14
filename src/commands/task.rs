use super::{presenter, resolve};
use crate::client::{ActiveCollabClient, TaskWriteOutcome};
use crate::controller;
use crate::http::HTTP_UNAUTHORIZED;
use crate::i18n::t;
use crate::render;
use crate::store::cache::TaskCache;
use crate::store::instances::Instance;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Flags threaded from the CLI into the get/current core.
pub(crate) struct DisplayFlags {
    pub json: bool,
    pub short: bool,
    pub refresh: bool,
    pub no_comments: bool,
    pub download_attachments: bool,
    pub attachments_dir: Option<String>,
}

/// Parity: Python _load_task.
///
/// Returns (task, comments) from cache or API, or None on HTTP error.
/// When `refresh` is false and the cache has a hit, no network call is made.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn load_task(
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    instance_name: &str,
    pid: i64,
    tid: i64,
    refresh: bool,
    no_comments: bool,
    err: &mut dyn Write,
) -> Option<(Value, Vec<Value>)> {
    if !refresh {
        if let Ok(Some(cached)) = cache.read(instance_name, pid, tid) {
            let mut task = cached.fields;
            let comments = task
                .as_object_mut()
                .and_then(|obj| obj.remove("comments"))
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            return Some((task, comments));
        }
    }

    let (status, payload_opt) = client.fetch_task(pid, tid).await.ok()?;
    if status == HTTP_UNAUTHORIZED {
        writeln!(err, "{}", presenter::reauth_message()).ok();
        return None;
    }
    if status != 200 {
        writeln!(
            err,
            "{}",
            t(&format!(
                "Error: task {p}/{t} not found (HTTP {status}).",
                p = pid,
                t = tid,
                status = status
            ))
        )
        .ok();
        return None;
    }

    let payload = payload_opt.unwrap_or(Value::Null);
    let mut task = payload
        .get("single")
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));
    task["tracked_time"] = payload.get("tracked_time").cloned().unwrap_or(Value::Null);
    let comments: Vec<Value> = if no_comments {
        vec![]
    } else {
        payload
            .get("comments")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };

    let comments_value = Value::Array(comments.clone());
    cache
        .write(instance_name, pid, tid, &task, &comments_value)
        .ok();

    Some((task, comments))
}

/// Parity: Python _do_get_task.
///
/// Shared fetch-and-render logic for both `get` and `current`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn do_get_task(
    inst: &Instance,
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    pid: i64,
    tid: i64,
    flags: &DisplayFlags,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let result = load_task(
        cache,
        client,
        &inst.name,
        pid,
        tid,
        flags.refresh,
        flags.no_comments,
        err,
    )
    .await;

    let (task, comments) = match result {
        Some(pair) => pair,
        None => return 1,
    };

    let downloaded = maybe_download_attachments(client, &task, &comments, pid, tid, flags).await;

    if flags.json {
        let user_map: HashMap<i64, String> = client.fetch_user_map().await.unwrap_or_default();
        let mut obj = crate::agent_json::task_object(
            &task,
            &comments,
            &user_map,
            &inst.base_url,
            flags.no_comments,
        );
        if let Some((_, downloaded)) = &downloaded {
            splice_downloaded_attachments(&mut obj, downloaded);
        }
        writeln!(out, "{}", serde_json::to_string(&obj).unwrap_or_default()).ok();
        return 0;
    }

    if flags.short {
        let name = task.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let name = crate::sanitize::strip_control_chars(name);
        writeln!(out, "{pid}/{tid}\t{name}").ok();
        return 0;
    }

    let user_map: HashMap<i64, String> = client.fetch_user_map().await.unwrap_or_default();
    render::render_task(&task, &comments, flags.no_comments, &user_map, out);
    if let Some((dest_dir, downloaded)) = &downloaded {
        writeln!(out, "{}", download_summary_line(dest_dir, downloaded)).ok();
    }
    0
}

/// Run the ADR 0066 attachment download when `flags.download_attachments` is
/// set: resolve the destination directory (`flags.attachments_dir` override,
/// else `controller::default_attachments_dir`), then extract and fetch the
/// downloadable asset set. Returns `None` (no-op, no side effect) otherwise.
async fn maybe_download_attachments(
    client: &ActiveCollabClient,
    task: &Value,
    comments: &[Value],
    pid: i64,
    tid: i64,
    flags: &DisplayFlags,
) -> Option<(PathBuf, Vec<controller::DownloadedAsset>)> {
    if !flags.download_attachments {
        return None;
    }
    let dest_dir = flags
        .attachments_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| controller::default_attachments_dir(pid, tid));
    let assets = controller::downloadable_assets(task, comments);
    let downloaded = controller::download_task_attachments(client, &assets, &dest_dir).await;
    Some((dest_dir, downloaded))
}

/// Splice the ADR 0066 `downloaded_attachments` array into an already-built
/// `task_object` `Value`. `task_object` itself stays pure and untouched.
fn splice_downloaded_attachments(obj: &mut Value, downloaded: &[controller::DownloadedAsset]) {
    let Value::Object(map) = obj else {
        return;
    };
    map.insert(
        "downloaded_attachments".to_owned(),
        Value::Array(
            downloaded
                .iter()
                .map(|d| {
                    json!({
                        "name": d.name,
                        "url": d.url,
                        "path": d.path,
                        "error": d.error,
                    })
                })
                .collect(),
        ),
    );
}

/// One human-readable summary line for a completed attachment download:
/// success/total counts, the destination directory, and any per-asset
/// failure reasons (ADR 0066).
fn download_summary_line(dest_dir: &Path, downloaded: &[controller::DownloadedAsset]) -> String {
    let total = downloaded.len();
    let ok = downloaded.iter().filter(|d| d.error.is_none()).count();
    t(&format!(
        "Downloaded {ok} of {total} attachment(s) to {dir}{failures}",
        ok = ok,
        total = total,
        dir = dest_dir.display(),
        failures = download_failures_suffix(downloaded),
    ))
}

/// Render `" (failed: name: reason, ...)"` for every asset with an `error`,
/// or an empty string when every asset succeeded.
fn download_failures_suffix(downloaded: &[controller::DownloadedAsset]) -> String {
    let failed: Vec<String> = downloaded
        .iter()
        .filter_map(|d| {
            d.error
                .as_ref()
                .map(|reason| format!("{name}: {reason}", name = d.name, reason = reason))
        })
        .collect();
    if failed.is_empty() {
        String::new()
    } else {
        format!(" (failed: {})", failed.join(", "))
    }
}

/// Parity: Python cmd_get (testable core).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn get_core(
    ref_: &str,
    inst: &Instance,
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    flags: &DisplayFlags,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (pid, tid) = match resolve::parse_task_ref(ref_, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };
    do_get_task(inst, cache, client, pid, tid, flags, out, err).await
}

/// Parity: Python cmd_current (testable core).
///
/// `branch` is injected from the caller (main supplies `current_git_branch()`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn current_core(
    branch: Option<&str>,
    inst: &Instance,
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    flags: &DisplayFlags,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let branch = match branch {
        Some(b) => b,
        None => {
            writeln!(
                err,
                "{}",
                t("Error: not in a git repository or HEAD is detached.")
            )
            .ok();
            return 2;
        }
    };

    let (pid, tid) = match resolve::parse_branch_ref(branch) {
        Some(ids) => ids,
        None => {
            writeln!(
                err,
                "{}",
                t(&format!(
                    "Error: branch '{branch}' does not match expected pattern \
                     (feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159).",
                    branch = branch
                ))
            )
            .ok();
            return 2;
        }
    };

    do_get_task(inst, cache, client, pid, tid, flags, out, err).await
}

/// Outcome of matching an `--assignee` argument against a user directory.
enum AssigneeResolution {
    /// A single user id, either passed directly or matched by name.
    Id(i64),
    /// No user in the directory matched the given name.
    NotFound,
    /// More than one user in the directory matched the given name.
    Ambiguous,
}

/// Parse a `--status` value into the `set_task_completion` boolean.
///
/// Accepts `complete`/`completed`/`done`/`closed` for `true` and
/// `open`/`reopen`/`todo` for `false`, case-insensitively. Returns `None`
/// for any other value.
fn parse_completion(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "complete" | "completed" | "done" | "closed" => Some(true),
        "open" | "reopen" | "todo" => Some(false),
        _ => None,
    }
}

/// Match `arg` against a `{user_id: display_name}` directory: exact
/// case-insensitive name match. Pure so the id/not-found/ambiguous branches
/// are unit-testable without a directory fetch.
fn match_assignee_name(arg: &str, directory: &HashMap<i64, String>) -> AssigneeResolution {
    let matches: Vec<i64> = directory
        .iter()
        .filter(|(_, name)| name.eq_ignore_ascii_case(arg))
        .map(|(id, _)| *id)
        .collect();
    match matches.as_slice() {
        [single] => AssigneeResolution::Id(*single),
        [] => AssigneeResolution::NotFound,
        _ => AssigneeResolution::Ambiguous,
    }
}

/// Resolve `--assignee` to a user id: an all-digits argument parses
/// directly, with no directory fetch. Otherwise fetch the instance's user
/// directory and match the argument as a display name.
async fn resolve_assignee_id(arg: &str, client: &ActiveCollabClient) -> AssigneeResolution {
    if !arg.is_empty() && arg.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(id) = arg.parse::<i64>() {
            return AssigneeResolution::Id(id);
        }
    }
    let directory = client.fetch_user_map().await.unwrap_or_default();
    match_assignee_name(arg, &directory)
}

/// Route a `TaskWriteOutcome` (or transport error) to the shared failure
/// presenter, returning `Err(exit_code)` on anything but success. Shared by
/// every write `task_set_core` issues so the outcome-mapping logic lives in
/// one place.
fn handle_task_outcome(
    outcome: Result<TaskWriteOutcome>,
    project_id: i64,
    task_id: i64,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> std::result::Result<(), i32> {
    match outcome {
        Err(e) => {
            presenter::write_comment_failure(&e.to_string(), json, out, err);
            Err(1)
        }
        Ok(TaskWriteOutcome::Ok(_)) => Ok(()),
        Ok(TaskWriteOutcome::Unauthorized) => {
            presenter::write_comment_failure(&presenter::reauth_message(), json, out, err);
            Err(1)
        }
        Ok(TaskWriteOutcome::Failed(status)) => {
            let reason = format!(
                "HTTP {status} updating task {project_id}/{task_id}",
                status = status,
                project_id = project_id,
                task_id = task_id
            );
            presenter::write_comment_failure(&reason, json, out, err);
            Err(1)
        }
    }
}

/// Non-interactive task field edit (issue 0068 slice 2).
///
/// Requires at least one of `status`/`assignee`/`estimate`; resolves the
/// task ref, validates and resolves each given field, applies the writes
/// (`set_task_completion` then `update_task`, stopping on the first
/// failure), and writes the result to the injected writers. Returns an exit
/// code: 0 success, 2 usage error, non-zero runtime failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn task_set_core(
    task_ref: Option<&str>,
    branch: Option<&str>,
    status: Option<&str>,
    assignee: Option<&str>,
    estimate: Option<f64>,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if status.is_none() && assignee.is_none() && estimate.is_none() {
        writeln!(
            err,
            "{}",
            t("Error: at least one of --status, --assignee, or --estimate is required.")
        )
        .ok();
        return 2;
    }

    if estimate.is_some_and(|value| value < 0.0) {
        writeln!(
            err,
            "{}",
            t("Error: estimate must be zero or a positive number.")
        )
        .ok();
        return 2;
    }

    let (project_id, task_id) = match resolve::resolve_task_ref_for_comment(task_ref, branch, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };

    let completed = match status {
        Some(value) => match parse_completion(value) {
            Some(completed) => Some(completed),
            None => {
                writeln!(
                    err,
                    "{}",
                    t(&format!(
                        "Error: unrecognized --status value '{status}'. Use complete, \
                         completed, done, closed, open, reopen, or todo.",
                        status = value
                    ))
                )
                .ok();
                return 2;
            }
        },
        None => None,
    };

    let assignee_id = match assignee {
        Some(arg) => match resolve_assignee_id(arg, client).await {
            AssigneeResolution::Id(id) => Some(id),
            AssigneeResolution::NotFound => {
                writeln!(
                    err,
                    "{}",
                    t(&format!(
                        "Error: no user matches assignee '{assignee}'.",
                        assignee = arg
                    ))
                )
                .ok();
                return 2;
            }
            AssigneeResolution::Ambiguous => {
                writeln!(
                    err,
                    "{}",
                    t(&format!(
                        "Error: assignee '{assignee}' matches more than one user; \
                         use the numeric id instead.",
                        assignee = arg
                    ))
                )
                .ok();
                return 2;
            }
        },
        None => None,
    };

    if let Some(completed) = completed {
        let outcome = client.set_task_completion(task_id, completed).await;
        if let Err(code) = handle_task_outcome(outcome, project_id, task_id, json, out, err) {
            return code;
        }
    }

    if assignee_id.is_some() || estimate.is_some() {
        let outcome = client
            .update_task(project_id, task_id, assignee_id, estimate)
            .await;
        if let Err(code) = handle_task_outcome(outcome, project_id, task_id, json, out, err) {
            return code;
        }
    }

    presenter::write_task_success(
        task_id,
        project_id,
        completed,
        assignee_id,
        estimate,
        json,
        out,
    );
    0
}
