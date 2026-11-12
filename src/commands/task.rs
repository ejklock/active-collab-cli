use super::{presenter, resolve};
use crate::client::{ActiveCollabClient, TaskList, TaskListsOutcome, TaskWriteOutcome};
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

/// The fields a `ac task set` call asked for, as given on the command line.
struct TaskEditRequest<'a> {
    status: Option<&'a str>,
    assignee: Option<&'a str>,
    estimate: Option<f64>,
    list: Option<&'a str>,
}

/// A validated task edit with every field resolved to what the API takes.
struct ResolvedTaskEdit {
    project_id: i64,
    task_id: i64,
    completed: Option<bool>,
    assignee_id: Option<i64>,
    estimate: Option<f64>,
    task_list: Option<TaskList>,
}

/// Outcome of matching a `--list` argument against a project's task lists.
enum TaskListResolution {
    /// The list the argument names.
    Found(TaskList),
    /// No list has that id or name.
    NotFound,
    /// More than one list has that name; carries the matching lists.
    Ambiguous(Vec<TaskList>),
}

/// Match `arg` against a project's task lists. An all-digits argument is an
/// id that must be one of the lists; any other argument is a name matched
/// exactly, ignoring case (Unicode-aware) and surrounding whitespace. Pure so
/// every outcome is unit-testable without a read.
fn match_task_list(arg: &str, lists: &[TaskList]) -> TaskListResolution {
    let arg = arg.trim();
    if !arg.is_empty() && arg.chars().all(|c| c.is_ascii_digit()) {
        let found = arg
            .parse::<i64>()
            .ok()
            .and_then(|id| lists.iter().find(|list| list.id == id));
        return found.map_or(TaskListResolution::NotFound, |list| {
            TaskListResolution::Found(list.clone())
        });
    }
    let wanted = arg.to_lowercase();
    let mut matches: Vec<TaskList> = lists
        .iter()
        .filter(|list| list.name.trim().to_lowercase() == wanted)
        .cloned()
        .collect();
    match matches.len() {
        0 => TaskListResolution::NotFound,
        1 => TaskListResolution::Found(matches.remove(0)),
        _ => TaskListResolution::Ambiguous(matches),
    }
}

fn write_list_candidates(lists: &[TaskList], err: &mut dyn Write) {
    for list in lists {
        writeln!(err, "  {} {}", list.id, list.name).ok();
    }
}

/// Read the project's task lists, mapping a failed read to its exit code: a
/// read that fails is never "no list matches". Failures go through the
/// command's single failure printer, so `--json` gets the error object.
async fn read_task_lists(
    client: &ActiveCollabClient,
    project_id: i64,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> std::result::Result<Vec<TaskList>, i32> {
    let reason = match client.fetch_task_lists(project_id).await {
        Ok(TaskListsOutcome::Ok(lists)) => return Ok(lists),
        Ok(TaskListsOutcome::Unauthorized) => presenter::reauth_message(),
        Ok(TaskListsOutcome::Failed(status)) => {
            t("HTTP {status} reading the task lists of project {project_id}")
                .replace("{status}", &status.to_string())
                .replace("{project_id}", &project_id.to_string())
        }
        Err(e) => e.to_string(),
    };
    presenter::write_comment_failure(&reason, json, out, err);
    Err(1)
}

/// Resolve `--list` against the project's task lists, read once. Exit code 2
/// for an unknown or ambiguous list (the candidates are printed), 1 for a
/// failed read.
async fn resolve_task_list(
    arg: &str,
    project_id: i64,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> std::result::Result<TaskList, i32> {
    let lists = read_task_lists(client, project_id, json, out, err).await?;
    match match_task_list(arg, &lists) {
        TaskListResolution::Found(list) => Ok(list),
        TaskListResolution::NotFound => {
            writeln!(
                err,
                "{}",
                t(&format!(
                    "Error: no task list matches '{list}'. Task lists of this project:",
                    list = arg
                ))
            )
            .ok();
            write_list_candidates(&lists, err);
            Err(2)
        }
        TaskListResolution::Ambiguous(matching) => {
            writeln!(
                err,
                "{}",
                t(&format!(
                    "Error: task list '{list}' matches more than one list; \
                     use the numeric id instead:",
                    list = arg
                ))
            )
            .ok();
            write_list_candidates(&matching, err);
            Err(2)
        }
    }
}

/// Resolve `--assignee` to a user id, printing the usage error (exit 2) when
/// it names no user or several.
async fn resolve_assignee_arg(
    arg: &str,
    client: &ActiveCollabClient,
    err: &mut dyn Write,
) -> std::result::Result<i64, i32> {
    match resolve_assignee_id(arg, client).await {
        AssigneeResolution::Id(id) => Ok(id),
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
            Err(2)
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
            Err(2)
        }
    }
}

/// Check the request's shape before any lookup: at least one field, a
/// non-negative estimate and a recognized status. Returns the parsed status.
fn check_task_edit_request(
    request: &TaskEditRequest<'_>,
    err: &mut dyn Write,
) -> std::result::Result<Option<bool>, i32> {
    if request.status.is_none()
        && request.assignee.is_none()
        && request.estimate.is_none()
        && request.list.is_none()
    {
        writeln!(
            err,
            "{}",
            t("Error: at least one of --status, --assignee, --estimate, or --list is required.")
        )
        .ok();
        return Err(2);
    }
    if request.estimate.is_some_and(|value| value < 0.0) {
        writeln!(
            err,
            "{}",
            t("Error: estimate must be zero or a positive number.")
        )
        .ok();
        return Err(2);
    }
    let Some(value) = request.status else {
        return Ok(None);
    };
    match parse_completion(value) {
        Some(completed) => Ok(Some(completed)),
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
            Err(2)
        }
    }
}

/// Validate the request and resolve every field (task ref, status, assignee,
/// task list) before any write. Owns every usage error of `ac task set`;
/// returns the resolved edit or the exit code to stop with.
async fn resolve_task_edit(
    task_ref: Option<&str>,
    branch: Option<&str>,
    request: &TaskEditRequest<'_>,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> std::result::Result<ResolvedTaskEdit, i32> {
    let completed = check_task_edit_request(request, err)?;
    let (project_id, task_id) = resolve::resolve_task_ref_for_comment(task_ref, branch, err)?;
    let assignee_id = match request.assignee {
        Some(arg) => Some(resolve_assignee_arg(arg, client, err).await?),
        None => None,
    };
    let task_list = match request.list {
        Some(arg) => Some(resolve_task_list(arg, project_id, client, json, out, err).await?),
        None => None,
    };
    Ok(ResolvedTaskEdit {
        project_id,
        task_id,
        completed,
        assignee_id,
        estimate: request.estimate,
        task_list,
    })
}

/// Non-interactive task field edit.
///
/// Requires at least one of `status`/`assignee`/`estimate`/`list`; resolves
/// the task ref and every given field before any write, applies the writes
/// (`set_task_completion` then one `update_task`, stopping on the first
/// failure), and writes the result to the injected writers. Returns an exit
/// code: 0 success, 2 usage error, non-zero runtime failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn task_set_core(
    task_ref: Option<&str>,
    branch: Option<&str>,
    status: Option<&str>,
    assignee: Option<&str>,
    estimate: Option<f64>,
    list: Option<&str>,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let request = TaskEditRequest {
        status,
        assignee,
        estimate,
        list,
    };
    let edit = match resolve_task_edit(task_ref, branch, &request, client, json, out, err).await {
        Ok(edit) => edit,
        Err(code) => return code,
    };
    let (project_id, task_id) = (edit.project_id, edit.task_id);

    if let Some(completed) = edit.completed {
        let outcome = client.set_task_completion(task_id, completed).await;
        if let Err(code) = handle_task_outcome(outcome, project_id, task_id, json, out, err) {
            return code;
        }
    }

    if edit.assignee_id.is_some() || edit.estimate.is_some() || edit.task_list.is_some() {
        let outcome = client
            .update_task(
                project_id,
                task_id,
                edit.assignee_id,
                edit.estimate,
                edit.task_list.as_ref().map(|list| list.id),
            )
            .await;
        if let Err(code) = handle_task_outcome(outcome, project_id, task_id, json, out, err) {
            return code;
        }
    }

    presenter::write_task_success(
        task_id,
        project_id,
        edit.completed,
        edit.assignee_id,
        edit.estimate,
        edit.task_list.as_ref(),
        json,
        out,
    );
    0
}
