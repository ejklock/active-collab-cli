use super::{presenter, resolve};
use crate::client::ActiveCollabClient;
use crate::http::HTTP_UNAUTHORIZED;
use crate::i18n::t;
use crate::render;
use crate::store::cache::TaskCache;
use crate::store::instances::Instance;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;

/// Flags threaded from the CLI into the get/current core.
pub(crate) struct DisplayFlags {
    pub json: bool,
    pub short: bool,
    pub refresh: bool,
    pub no_comments: bool,
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

    if flags.json {
        let user_map: HashMap<i64, String> = client.fetch_user_map().await.unwrap_or_default();
        let obj = crate::agent_json::task_object(
            &task,
            &comments,
            &user_map,
            &inst.base_url,
            flags.no_comments,
        );
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
    0
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
