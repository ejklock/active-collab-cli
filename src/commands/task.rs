use super::{presenter, resolve};
use crate::client::ActiveCollabClient;
use crate::controller;
use crate::http::HTTP_UNAUTHORIZED;
use crate::i18n::t;
use crate::render;
use crate::store::cache::TaskCache;
use crate::store::instances::Instance;
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
