use crate::client::ActiveCollabClient;
use crate::http::Http;
use crate::i18n::{t, SUPPORTED};
use crate::render::{self, MineTableRow};
use crate::store::cache::TaskCache;
use crate::store::instances::{Instance, InstanceRepository};
use crate::store::settings::SettingsRepository;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;

/// Parity: Python _pick_instance — exported for reuse by R4/R5.
///
/// Returns the single matching Instance or an i32 exit code (always 2) on failure.
/// Error messages go to `err`.
#[allow(dead_code)]
pub fn pick_instance(
    instances: &[Instance],
    name: Option<&str>,
    err: &mut dyn Write,
) -> Result<usize, i32> {
    if instances.is_empty() {
        writeln!(
            err,
            "{}",
            t("Error: no instances configured. Run: active_collab.py setup add")
        )
        .ok();
        return Err(2);
    }

    if let Some(n) = name {
        let pos = instances.iter().position(|i| i.name == n);
        match pos {
            Some(idx) => return Ok(idx),
            None => {
                let known: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
                let known_str = known.join(", ");
                writeln!(
                    err,
                    "{}",
                    t(&format!(
                        "Error: instance '{name}' not found. Known: {known}",
                        name = n,
                        known = known_str
                    ))
                )
                .ok();
                return Err(2);
            }
        }
    }

    if instances.len() == 1 {
        return Ok(0);
    }

    let names: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
    let names_str = names.join(", ");
    writeln!(
        err,
        "{}",
        t(&format!(
            "Error: multiple instances configured ({names}). Use --instance NAME.",
            names = names_str
        ))
    )
    .ok();
    Err(2)
}

/// Parity: Python cmd_setup_list.
pub fn setup_list(repo: &InstanceRepository<'_>, out: &mut dyn Write) -> i32 {
    let rows = match repo.list_for_display() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading instances: {e}");
            return 1;
        }
    };

    if rows.is_empty() {
        writeln!(
            out,
            "{}",
            t("No instances configured. Run: active_collab.py setup add")
        )
        .ok();
        return 0;
    }

    writeln!(out, "{:<20} {:<40} {:<30} USER_ID", "NAME", "URL", "EMAIL").ok();
    writeln!(out, "{}", "-".repeat(100)).ok();
    for (name, base_url, email, user_id) in &rows {
        let uid_str = user_id.map(|v| v.to_string()).unwrap_or_default();
        writeln!(
            out,
            "{:<20} {:<40} {:<30} {}",
            name, base_url, email, uid_str
        )
        .ok();
    }
    0
}

/// Parity: Python cmd_setup_remove.
pub fn setup_remove(
    repo: &InstanceRepository<'_>,
    cache: &TaskCache<'_>,
    name: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let deleted = match repo.delete(name) {
        Ok(n) => n,
        Err(e) => {
            writeln!(err, "Error deleting instance: {e}").ok();
            return 1;
        }
    };
    cache.delete_for_instance(name).ok();

    if deleted == 0 {
        writeln!(
            err,
            "{}",
            t(&format!("Error: instance '{name}' not found.", name = name))
        )
        .ok();
        return 2;
    }
    writeln!(
        out,
        "{}",
        t(&format!("Instance '{name}' removed.", name = name))
    )
    .ok();
    0
}

/// Parity: Python cmd_setup_language.
pub fn setup_language(
    settings: &SettingsRepository<'_>,
    code: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match code {
        None => {
            let current = settings
                .get("language", Some("en"))
                .unwrap_or(Some("en".to_owned()))
                .unwrap_or_else(|| "en".to_owned());
            writeln!(
                out,
                "{}",
                t(&format!("Current language: {code}", code = current))
            )
            .ok();
            0
        }
        Some(c) => {
            if !SUPPORTED.contains(&c) {
                let supported = SUPPORTED.join(", ");
                writeln!(
                    err,
                    "{}",
                    t(&format!(
                        "Error: unsupported language '{code}'. Supported: {supported}.",
                        code = c,
                        supported = supported
                    ))
                )
                .ok();
                return 2;
            }
            if let Err(e) = settings.set("language", c) {
                writeln!(err, "Error saving language: {e}").ok();
                return 1;
            }
            writeln!(
                out,
                "{}",
                t(&format!("Language set to '{code}'.", code = c))
            )
            .ok();
            0
        }
    }
}

/// Inner core for connectivity test — takes pre-resolved rows.
///
/// Parity: Python cmd_setup_test inner loop.
pub async fn setup_test_core(
    rows: Vec<(String, String, String)>,
    http: Http,
    out: &mut dyn Write,
) -> i32 {
    let mut exit_code = 0i32;
    for (name, base_url, token) in rows {
        let inst = Instance {
            name: name.clone(),
            base_url: base_url.clone(),
            email: String::new(),
            token: token.clone(),
            user_id: None,
        };
        let client = ActiveCollabClient::new(inst, http.clone());
        match client.test_connectivity().await {
            Ok((200, _)) => {
                writeln!(
                    out,
                    "  {name}: {}",
                    t(&format!("OK ({status})", status = 200))
                )
                .ok();
            }
            Ok((status, _)) => {
                writeln!(
                    out,
                    "  {name}: {}",
                    t(&format!("FAILED (HTTP {status})", status = status))
                )
                .ok();
                exit_code = 1;
            }
            Err(exc) => {
                writeln!(
                    out,
                    "  {name}: {}",
                    t(&format!("FAILED ({exc})", exc = exc))
                )
                .ok();
                exit_code = 1;
            }
        }
    }
    exit_code
}

/// Thin wrapper that resolves rows from repo and delegates to setup_test_core.
///
/// Parity: Python cmd_setup_test (resolution + dispatch).
pub async fn setup_test(
    repo: &InstanceRepository<'_>,
    name: Option<&str>,
    http: Http,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let rows = match name {
        Some(n) => {
            let found = match repo.find_by_name(n) {
                Ok(r) => r,
                Err(e) => {
                    writeln!(err, "Error querying instances: {e}").ok();
                    return 1;
                }
            };
            if found.is_empty() {
                writeln!(
                    err,
                    "{}",
                    t(&format!("Error: instance '{name}' not found.", name = n))
                )
                .ok();
                return 2;
            }
            found
        }
        None => match repo.list_connectivity() {
            Ok(r) => r,
            Err(e) => {
                writeln!(err, "Error querying instances: {e}").ok();
                return 1;
            }
        },
    };

    setup_test_core(rows, http, out).await
}

/// The fields resolved from flags / prompts before calling setup_add.
pub struct SetupAddFields {
    pub name: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

/// Parity: Python cmd_setup_add (the testable core without stdin/rpassword).
///
/// `check_connectivity`: when true (interactive TTY), run connectivity check after save.
#[allow(clippy::too_many_arguments)]
pub async fn setup_add(
    fields: SetupAddFields,
    password: Option<String>,
    repo: &InstanceRepository<'_>,
    http: Http,
    check_connectivity: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (name, url, email) = match (fields.name, fields.url, fields.email) {
        (Some(n), Some(u), Some(e)) if !n.is_empty() && !u.is_empty() && !e.is_empty() => (n, u, e),
        _ => {
            writeln!(
                err,
                "{}",
                t("Error: --name, --url and --email are required.")
            )
            .ok();
            return 2;
        }
    };

    let password = match password {
        Some(p) if !p.is_empty() => p,
        _ => {
            writeln!(err, "{}", t("Error: password is required.")).ok();
            return 2;
        }
    };

    let base_url = url.trim_end_matches('/').to_owned();

    let dummy_inst = Instance {
        name: String::new(),
        base_url: base_url.clone(),
        email: email.clone(),
        token: String::new(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(dummy_inst, http.clone());

    let (token_opt, response) = match client.exchange_token(&base_url, &email, &password).await {
        Ok(pair) => pair,
        Err(exc) => {
            writeln!(err, "{}", t(&format!("Error: {exc}", exc = exc))).ok();
            return 1;
        }
    };

    // Drop the password immediately after token exchange — never retain it.
    let _password = password;

    let token = match token_opt {
        Some(t) => t,
        None => {
            let detail = response
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("token exchange failed")
                .to_owned();
            writeln!(err, "{}", t(&format!("Error: {detail}", detail = detail))).ok();
            return 1;
        }
    };

    let authed_inst = Instance {
        name: name.clone(),
        base_url: base_url.clone(),
        email: email.clone(),
        token: token.clone(),
        user_id: None,
    };
    let authed_client = ActiveCollabClient::new(authed_inst, http.clone());

    let user_id = authed_client
        .resolve_user_id(&base_url, &token, &email)
        .await
        .unwrap_or(None);

    let instance = Instance {
        name: name.clone(),
        base_url,
        email,
        token,
        user_id,
    };

    if let Err(e) = repo.save(&instance) {
        writeln!(err, "Error saving instance: {e}").ok();
        return 1;
    }

    writeln!(
        out,
        "{}",
        t(&format!("Instance '{name}' saved.", name = name))
    )
    .ok();

    if check_connectivity {
        run_connectivity_check(&authed_client, out).await;
    }

    0
}

/// Parity: Python _run_connectivity_check.
pub async fn run_connectivity_check(client: &ActiveCollabClient, out: &mut dyn Write) {
    match client.test_connectivity().await {
        Ok((200, _)) => {
            writeln!(out, "{}", t("Connectivity: OK")).ok();
        }
        Ok((status, _)) => {
            writeln!(
                out,
                "{}",
                t(&format!(
                    "Connectivity: FAILED (HTTP {status})",
                    status = status
                ))
            )
            .ok();
        }
        Err(exc) => {
            writeln!(
                out,
                "{}",
                t(&format!("Connectivity: FAILED ({exc})", exc = exc))
            )
            .ok();
        }
    }
}

fn task_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"/projects/(\d+)/tasks/(\d+)").expect("task_url_re is a valid pattern")
    })
}

fn branch_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(feature|hotfix|fix)/(\d+)-(\d+)$").expect("branch_re is a valid pattern")
    })
}

/// Parity: Python _parse_task_ref.
///
/// Returns (project_id, task_id) from a URL or "P/T" digit-slash-digit form.
/// Writes an error and returns Err(2) on bad input.
pub fn parse_task_ref(ref_: &str, err: &mut dyn Write) -> Result<(i64, i64), i32> {
    if let Some(caps) = task_url_re().captures(ref_) {
        let pid: i64 = caps[1].parse().unwrap();
        let tid: i64 = caps[2].parse().unwrap();
        return Ok((pid, tid));
    }

    let parts: Vec<&str> = ref_.split('/').collect();
    if parts.len() == 2 {
        if let (Ok(pid), Ok(tid)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>()) {
            return Ok((pid, tid));
        }
    }

    writeln!(
        err,
        "{}",
        t(&format!(
            "Error: cannot parse task ref '{ref}'. Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159).",
            ref = ref_
        ))
    )
    .ok();
    Err(2)
}

/// Parity: Python _parse_branch_ref.
///
/// Returns Some((project_id, task_id)) when branch matches
/// `^(feature|hotfix|fix)/<pid>-<tid>$`, else None.
pub fn parse_branch_ref(branch: &str) -> Option<(i64, i64)> {
    let caps = branch_re().captures(branch)?;
    let pid: i64 = caps[2].parse().ok()?;
    let tid: i64 = caps[3].parse().ok()?;
    Some((pid, tid))
}

/// Flags threaded from the CLI into the get/current core.
pub struct DisplayFlags {
    pub json: bool,
    pub short: bool,
    pub refresh: bool,
    pub no_comments: bool,
}

/// Parity: Python _load_task.
///
/// Returns (task, comments) from cache or API, or None on HTTP error.
/// When `refresh` is false and the cache has a hit, no network call is made.
pub async fn load_task(
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    instance_name: &str,
    pid: i64,
    tid: i64,
    refresh: bool,
    no_comments: bool,
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
    if status != 200 {
        render::print_error(&t(&format!(
            "Error: task {p}/{t} not found (HTTP {status}).",
            p = pid,
            t = tid,
            status = status
        )));
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
pub async fn do_get_task(
    inst: &Instance,
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    pid: i64,
    tid: i64,
    flags: &DisplayFlags,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if flags.json {
        let (status, payload_opt) = match client.fetch_task(pid, tid).await {
            Ok(r) => r,
            Err(e) => {
                writeln!(err, "{}", t(&format!("Error: {e}"))).ok();
                return 1;
            }
        };
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
            return 1;
        }
        let json_str =
            serde_json::to_string_pretty(&payload_opt.unwrap_or(Value::Null)).unwrap_or_default();
        writeln!(out, "{json_str}").ok();
        return 0;
    }

    let result = load_task(
        cache,
        client,
        &inst.name,
        pid,
        tid,
        flags.refresh,
        flags.no_comments,
    )
    .await;

    let (task, comments) = match result {
        Some(pair) => pair,
        None => return 1,
    };

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
pub async fn get_core(
    ref_: &str,
    inst: &Instance,
    cache: &TaskCache<'_>,
    client: &ActiveCollabClient,
    flags: &DisplayFlags,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (pid, tid) = match parse_task_ref(ref_, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };
    do_get_task(inst, cache, client, pid, tid, flags, out, err).await
}

/// Parity: Python cmd_current (testable core).
///
/// `branch` is injected from the caller (main supplies `current_git_branch()`).
#[allow(clippy::too_many_arguments)]
pub async fn current_core(
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

    let (pid, tid) = match parse_branch_ref(branch) {
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

/// Parity: Python _render_mine_table (aggregation loop).
///
/// For each target instance, builds a client and fetches open tasks. Maps each
/// MineTask to a MineTableRow using task_number when present (else id as fallback),
/// mirroring Python `t.task_number or t.id`.
pub async fn collect_mine_rows(targets: &[Instance], http: &Http) -> Vec<MineTableRow> {
    let mut rows = Vec::new();
    for inst in targets {
        let client = ActiveCollabClient::new(inst.clone(), http.clone());
        let tasks = client.fetch_open_tasks().await.unwrap_or_default();
        for task in tasks {
            let task_number = match task.task_number {
                Some(n) if n != 0 => n,
                _ => task.id,
            };
            rows.push(MineTableRow {
                instance: task.instance_name.clone(),
                project_id: task.project_id.unwrap_or(0),
                task_number,
                task_id: task.id,
                name: task.name.clone(),
            });
        }
    }
    rows
}

/// Parity: Python cmd_mine (testable core).
///
/// Loads instances, applies optional instance filter, aggregates rows, then either
/// invokes `launch` (TTY path) or writes the table (non-TTY path).
#[allow(clippy::too_many_arguments)]
pub async fn mine_core(
    repo: &InstanceRepository<'_>,
    http: &Http,
    instance_filter: Option<&str>,
    is_tty: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
    launch: impl FnOnce(Vec<MineTableRow>) -> i32,
) -> i32 {
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            writeln!(err, "Error loading instances: {e}").ok();
            return 1;
        }
    };

    if instances.is_empty() {
        writeln!(
            err,
            "{}",
            t("Error: no instances configured. Run: active_collab.py setup add")
        )
        .ok();
        return 2;
    }

    let targets: Vec<Instance> = if let Some(name) = instance_filter {
        let matches: Vec<Instance> = instances
            .iter()
            .filter(|i| i.name == name)
            .cloned()
            .collect();
        if matches.is_empty() {
            let known: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
            let known_str = known.join(", ");
            writeln!(
                err,
                "{}",
                t(&format!(
                    "Error: instance '{name}' not found. Known: {known}",
                    name = name,
                    known = known_str
                ))
            )
            .ok();
            return 2;
        }
        matches
    } else {
        instances
    };

    let rows = collect_mine_rows(&targets, http).await;

    if is_tty {
        return launch(rows);
    }

    if rows.is_empty() {
        writeln!(out, "{}", t("No open tasks assigned to you.")).ok();
        return 0;
    }

    writeln!(out, "{}", render::render_mine_table(&rows)).ok();
    0
}

#[cfg(test)]
#[path = "../tests/unit/commands.rs"]
mod tests;
