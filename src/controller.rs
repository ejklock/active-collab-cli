use crate::client::ActiveCollabClient;
use crate::config::Config;
use crate::http::Http;
use crate::models::MineTask;
use crate::render::{is_openable_url, Asset};
use crate::store::cache::{ProjectNamesCache, TaskCache, UserMapCache};
use crate::store::instances::Instance;
use crate::store::Store;
use crate::tui::model::{ProjectGroup, TaskRow};
use anyhow::{anyhow, Context};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PROJECT_NAMES_TTL_SECS: i64 = 24 * 3600;

/// Aggregates open tasks across all target instances, maps project ids to
/// names, groups tasks by project, and sorts alphabetically.
///
/// Open tasks are always fetched from the network. Project names are served
/// from `ProjectNamesCache` when fresh; a miss or stale entry triggers one
/// `list_projects` call and writes the result back to cache.
///
/// When `list_projects` fails for an instance, task groups fall back to
/// using the numeric project id as the name — no crash.
pub async fn tasks_by_project(
    db_path: PathBuf,
    targets: &[Instance],
    http: &Http,
) -> Vec<ProjectGroup> {
    let t = std::time::Instant::now();

    let mut set: tokio::task::JoinSet<(String, Vec<MineTask>, HashMap<i64, String>)> =
        tokio::task::JoinSet::new();

    for inst in targets {
        let client = ActiveCollabClient::new(inst.clone(), http.clone());
        let inst_name = inst.name.clone();
        let db = db_path.clone();
        set.spawn(async move {
            let (tasks, names) = tokio::join!(
                client.fetch_open_tasks(),
                resolve_project_names(&db, &inst_name, &client)
            );
            (inst_name, tasks.unwrap_or_default(), names)
        });
    }

    let mut all_tasks: Vec<(MineTask, String)> = vec![];
    let mut project_names: HashMap<(String, i64), String> = HashMap::new();

    while let Some(joined) = set.join_next().await {
        if let Ok((inst_name, tasks, names)) = joined {
            for (pid, name) in names {
                project_names.insert((inst_name.clone(), pid), name);
            }
            for task in tasks {
                all_tasks.push((task, inst_name.clone()));
            }
        }
    }

    let result = build_groups(all_tasks, &project_names);
    crate::timing::record("browse_list_load", t.elapsed());
    result
}

async fn resolve_project_names(
    db_path: &Path,
    instance_name: &str,
    client: &ActiveCollabClient,
) -> HashMap<i64, String> {
    if let Some(names) = fresh_project_names_cache_read(db_path, instance_name) {
        return names;
    }
    let names = fetch_project_names(client).await;
    if !names.is_empty() {
        try_project_names_cache_write(db_path, instance_name, &names);
    }
    names
}

fn fresh_project_names_cache_read(
    db_path: &Path,
    instance_name: &str,
) -> Option<HashMap<i64, String>> {
    let config = Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = Store::open(&config).ok()?;
    let cached = ProjectNamesCache::new(store.conn())
        .read(instance_name)
        .ok()??;
    let age = crate::store::now_epoch_secs() - cached.fetched_at;
    if age <= PROJECT_NAMES_TTL_SECS {
        Some(cached.names)
    } else {
        None
    }
}

fn try_project_names_cache_write(
    db_path: &Path,
    instance_name: &str,
    names: &HashMap<i64, String>,
) {
    let config = Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    if let Ok(store) = Store::open(&config) {
        ProjectNamesCache::new(store.conn())
            .write(instance_name, names)
            .ok();
    }
}

async fn fetch_project_names(client: &ActiveCollabClient) -> HashMap<i64, String> {
    let (status, body) = match client.list_projects().await {
        Ok(pair) => pair,
        Err(_) => return HashMap::new(),
    };
    if status != 200 {
        return HashMap::new();
    }
    let data: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let projects = match data.as_array() {
        Some(arr) => arr,
        None => return HashMap::new(),
    };
    projects
        .iter()
        .filter_map(|p| {
            let id = p.get("id")?.as_i64()?;
            let name = p
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some((id, name))
        })
        .collect()
}

fn build_groups(
    tasks: Vec<(MineTask, String)>,
    project_names: &HashMap<(String, i64), String>,
) -> Vec<ProjectGroup> {
    let mut groups: HashMap<(String, i64), ProjectGroup> = HashMap::new();

    for (task, instance_name) in tasks {
        let pid = task.project_id.unwrap_or(0);
        let project_name = project_names
            .get(&(instance_name.clone(), pid))
            .cloned()
            .unwrap_or_else(|| pid.to_string());

        let key = (instance_name.clone(), pid);
        let group = groups.entry(key).or_insert_with(|| ProjectGroup {
            project_id: pid,
            project_name,
            instance: instance_name.clone(),
            tasks: vec![],
        });

        group.tasks.push(TaskRow {
            task_id: task.id,
            task_number: task.task_number.unwrap_or(task.id),
            name: task.name,
            instance: instance_name,
            project_id: pid,
        });
    }

    let mut sorted: Vec<ProjectGroup> = groups.into_values().collect();
    sorted.sort_by(|a, b| {
        a.project_name
            .cmp(&b.project_name)
            .then_with(|| a.instance.cmp(&b.instance))
    });
    for group in &mut sorted {
        group.tasks.sort_by_key(|t| t.task_number);
    }
    sorted
}

/// All data needed to render a task detail screen (used by the test seam).
#[cfg(test)]
pub struct DetailData {
    pub task: Value,
    pub comments: Vec<Value>,
    pub assets: Vec<Asset>,
    pub user_map: HashMap<i64, String>,
}

/// Task content (meta + comments + assets) fetched without the user directory.
///
/// Caller renders this immediately, then resolves user_map in the background
/// for a second paint that updates the Assignee line.
pub struct TaskCore {
    pub task: Value,
    pub comments: Vec<Value>,
    pub assets: Vec<Asset>,
}

static IMG_SRC_RE: OnceLock<Regex> = OnceLock::new();
static HREF_WITH_TEXT_RE: OnceLock<Regex> = OnceLock::new();

fn img_src_re() -> &'static Regex {
    IMG_SRC_RE.get_or_init(|| {
        Regex::new(r#"(?i)<img\b[^>]*\bsrc=["']([^"']+)["']"#)
            .expect("img_src_re is a valid pattern")
    })
}

fn href_with_text_re() -> &'static Regex {
    HREF_WITH_TEXT_RE.get_or_init(|| {
        Regex::new(r#"(?i)<a\b[^>]*\bhref=["']([^"']+)["'][^>]*>(.*?)</a>"#)
            .expect("href_with_text_re is a valid pattern")
    })
}

fn url_basename(url: &str) -> String {
    url.split('/')
        .next_back()
        .filter(|s| !s.is_empty())
        .unwrap_or(url)
        .to_string()
}

fn url_host(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .filter(|h| !h.is_empty())
}

/// Derive the display label for an asset using anchor text → real filename → host precedence.
///
/// Returns an empty string when none of the three fallbacks yield a meaningful label
/// (signals the renderer to use its own final fallback).
pub(crate) fn derive_asset_label(url: &str, anchor_text: Option<&str>) -> String {
    if let Some(text) = anchor_text {
        let text = text.trim();
        if !text.is_empty() && text != url {
            return text.to_string();
        }
    }
    let basename = url_basename(url);
    if crate::render::looks_like_filename(&basename) {
        return basename;
    }
    url_host(url).unwrap_or_default()
}

fn assets_from_html(html: &str) -> Vec<Asset> {
    let mut assets = vec![];
    for cap in img_src_re().captures_iter(html) {
        let url = cap[1].to_string();
        let name = derive_asset_label(&url, None);
        assets.push(Asset { name, url });
    }
    for cap in href_with_text_re().captures_iter(html) {
        let url = cap[1].to_string();
        let raw_text = cap[2].to_string();
        let anchor_text = crate::render::html_to_text(&raw_text);
        let name = derive_asset_label(&url, Some(&anchor_text));
        assets.push(Asset { name, url });
    }
    assets
}

fn assets_from_attachments(attachments: &Value) -> Vec<Asset> {
    let arr = match attachments.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    arr.iter()
        .filter_map(|att| {
            let url = att
                .get("url")
                .or_else(|| att.get("download_url"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())?
                .to_string();
            let name = att
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| url_basename(&url));
            Some(Asset { name, url })
        })
        .collect()
}

/// Extract all assets (images, links, attachments) from a task JSON.
///
/// Deduplicates by URL, preserving first-seen order.
pub fn extract_assets(task: &Value, comments: &[Value]) -> Vec<Asset> {
    let mut seen = std::collections::HashSet::new();
    let mut result = vec![];

    let mut add = |asset: Asset| {
        if seen.insert(asset.url.clone()) {
            result.push(asset);
        }
    };

    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    for asset in assets_from_html(body_html) {
        add(asset);
    }

    for comment in comments {
        let comment_html = comment.get("body").and_then(|v| v.as_str()).unwrap_or("");
        for asset in assets_from_html(comment_html) {
            add(asset);
        }
    }

    if let Some(attachments) = task.get("attachments") {
        for asset in assets_from_attachments(attachments) {
            add(asset);
        }
    }

    result
}

/// Fetch or serve from cache task content — no user-directory request.
///
/// Opens its own DB connection so the caller can pass it to tokio::spawn.
/// Never holds a Connection across an await boundary.
pub async fn load_task_core(
    db_path: PathBuf,
    inst: Instance,
    http: Http,
    project_id: i64,
    task_id: i64,
    refresh: bool,
) -> TaskCore {
    let client = ActiveCollabClient::new(inst.clone(), http);
    let (mut task, comments) =
        load_task_data_from_path(&db_path, &inst, &client, project_id, task_id, refresh).await;
    enrich_task_with_project_name(&mut task, &db_path, &inst.name, project_id);
    let assets = extract_assets(&task, &comments);
    TaskCore {
        task,
        comments,
        assets,
    }
}

/// Injects `project_name` into `task` by reading the per-instance project-name cache.
///
/// No network call is made. On cache miss the key is set to `t("(unknown)")` so the
/// Projeto row in the Details panel is never blank.
fn enrich_task_with_project_name(
    task: &mut Value,
    db_path: &Path,
    instance_name: &str,
    project_id: i64,
) {
    let name = project_name_from_cache(db_path, instance_name, project_id);
    if let Some(obj) = task.as_object_mut() {
        obj.insert("project_name".to_string(), Value::String(name));
    }
}

/// Read the project name for `project_id` from the per-instance cache.
///
/// Returns the cached name when fresh, otherwise the `t("(unknown)")` fallback.
/// Never issues a network request.
pub fn project_name_from_cache(db_path: &Path, instance_name: &str, project_id: i64) -> String {
    fresh_project_names_cache_read(db_path, instance_name)
        .and_then(|names| names.get(&project_id).cloned())
        .unwrap_or_else(|| crate::i18n::t("(unknown)"))
}

/// Read the user map from the per-instance cache without any network call.
pub fn cached_user_map(db_path: &Path, inst: &Instance) -> Option<HashMap<i64, String>> {
    let config = Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let t = std::time::Instant::now();
    let result = try_user_map_cache_read(&config, &inst.name);
    crate::timing::record("user_map_cache_read", t.elapsed());
    result
}

/// Force-fetch the user directory, write to cache, and return the result.
///
/// Never holds a Connection across an await — cache write happens synchronously
/// before this function returns, satisfying tokio::spawn's Send bound.
pub async fn refresh_user_map(
    db_path: PathBuf,
    inst: Instance,
    http: Http,
) -> HashMap<i64, String> {
    let config = Config {
        db_path,
        task_cache_ttl_hours: 24,
    };
    let client = ActiveCollabClient::new(inst.clone(), http);
    let t = std::time::Instant::now();
    let map = client.fetch_user_map().await.unwrap_or_default();
    crate::timing::record("fetch_user_map", t.elapsed());
    if !map.is_empty() {
        try_user_map_cache_write(&config, &inst.name, &map);
    }
    map
}

/// Open a DB connection and serve cache or network for the task.
///
/// Structured to avoid holding any non-Send reference (TaskCache/Connection)
/// across an await point, satisfying tokio::spawn's Send bound.
async fn load_task_data_from_path(
    db_path: &Path,
    inst: &Instance,
    client: &ActiveCollabClient,
    project_id: i64,
    task_id: i64,
    refresh: bool,
) -> (Value, Vec<Value>) {
    let config = Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };

    if !refresh {
        let t = std::time::Instant::now();
        let cache_hit = try_cache_read(&config, &inst.name, project_id, task_id);
        crate::timing::record("task_cache_read", t.elapsed());
        if let Some(hit) = cache_hit {
            return hit;
        }
    }

    let t = std::time::Instant::now();
    let result = fetch_from_network(client, project_id, task_id).await;
    crate::timing::record("fetch_task", t.elapsed());
    let (task, comments) = match result {
        None => return (Value::Null, vec![]),
        Some(pair) => pair,
    };

    try_cache_write(&config, &inst.name, project_id, task_id, &task, &comments);
    (task, comments)
}

/// Remove the embedded `comments` array from cached task fields, returning both parts.
///
/// Cache entries store comments inside the task JSON to avoid a second row; this
/// helper splits them so callers can treat task metadata and comments separately.
fn split_comments_from_fields(mut fields: Value) -> (Value, Vec<Value>) {
    let comments = fields
        .as_object_mut()
        .and_then(|obj| obj.remove("comments"))
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (fields, comments)
}

fn try_cache_read(
    config: &Config,
    instance_name: &str,
    project_id: i64,
    task_id: i64,
) -> Option<(Value, Vec<Value>)> {
    let store = Store::open(config).ok()?;
    let cache = TaskCache::new(store.conn());
    let cached = cache.read(instance_name, project_id, task_id).ok()??;
    Some(split_comments_from_fields(cached.fields))
}

fn try_cache_write(
    config: &Config,
    instance_name: &str,
    project_id: i64,
    task_id: i64,
    task: &Value,
    comments: &[Value],
) {
    if let Ok(store) = Store::open(config) {
        let cache = TaskCache::new(store.conn());
        let comments_value = Value::Array(comments.to_vec());
        cache
            .write(instance_name, project_id, task_id, task, &comments_value)
            .ok();
    }
}

fn parse_task_payload(payload: Value) -> (Value, Vec<Value>) {
    let mut task = payload
        .get("single")
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));
    task["tracked_time"] = payload.get("tracked_time").cloned().unwrap_or(Value::Null);
    let comments: Vec<Value> = payload
        .get("comments")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    (task, comments)
}

async fn fetch_from_network(
    client: &ActiveCollabClient,
    project_id: i64,
    task_id: i64,
) -> Option<(Value, Vec<Value>)> {
    let (status, payload_opt) = client.fetch_task(project_id, task_id).await.ok()?;
    if status != 200 {
        return None;
    }
    Some(parse_task_payload(payload_opt.unwrap_or(Value::Null)))
}

fn try_user_map_cache_read(config: &Config, instance_name: &str) -> Option<HashMap<i64, String>> {
    let store = Store::open(config).ok()?;
    let cache = UserMapCache::new(store.conn());
    cache.read(instance_name).ok()?.map(|hit| hit.users)
}

fn try_user_map_cache_write(config: &Config, instance_name: &str, users: &HashMap<i64, String>) {
    if let Ok(store) = Store::open(config) {
        let cache = UserMapCache::new(store.conn());
        cache.write(instance_name, users).ok();
    }
}

/// Parity: Python assets.py open_asset / tui/controller.py open_asset.
///
/// Returns Err when the url scheme is not http or https — the caller must never
/// spawn an OS opener for non-http schemes (security: no file://, javascript:, etc.).
/// The actual process spawn lives in the shell layer (tui.rs), not here.
pub fn open_asset(url: &str) -> anyhow::Result<()> {
    if !is_openable_url(url) {
        return Err(anyhow!("non-http/https URL rejected: {url}"));
    }
    Ok(())
}

/// Parity: Python assets.py download_asset.
///
/// Performs a GET for `url`, attaching the auth token only when the asset
/// host matches the instance host (via http::host_gated_token_header).
/// Writes the response body bytes to `dest_path`. Returns Err on non-2xx.
pub async fn download_asset(
    http: &Http,
    inst: &Instance,
    url: &str,
    dest_path: &Path,
) -> anyhow::Result<()> {
    let (status, body) = http
        .authed_get(url, &inst.base_url, &inst.token)
        .await
        .with_context(|| format!("GET {url}"))?;
    if !(200..300).contains(&status) {
        return Err(anyhow!("download failed for {url}: HTTP {status}"));
    }
    std::fs::write(dest_path, &body)
        .with_context(|| format!("writing download to {}", dest_path.display()))?;
    Ok(())
}

/// Testable variant of task_detail that takes a direct DB connection reference.
///
/// Used in integration tests where the caller already holds a connection.
/// The cache read/write is done synchronously (no await) so the cache borrow
/// does not cross an await boundary.
///
/// On a cache hit (refresh=false and entry present) NO network call is made.
/// On a cache miss or refresh=true, fetches from the API and writes back.
#[cfg(test)]
pub async fn task_detail_with_conn(
    conn: &rusqlite::Connection,
    inst: &Instance,
    http: &Http,
    project_id: i64,
    task_id: i64,
    refresh: bool,
) -> DetailData {
    let client = ActiveCollabClient::new(inst.clone(), http.clone());

    let cache_hit: Option<(Value, Vec<Value>)> = if !refresh {
        let cache = TaskCache::new(conn);
        cache
            .read(&inst.name, project_id, task_id)
            .ok()
            .flatten()
            .map(|cached| split_comments_from_fields(cached.fields))
    } else {
        None
    };

    let (task, comments) = match cache_hit {
        Some(pair) => pair,
        None => {
            let result = fetch_from_network(&client, project_id, task_id).await;
            match result {
                None => (Value::Null, vec![]),
                Some((task, comments)) => {
                    let cache = TaskCache::new(conn);
                    let comments_value = Value::Array(comments.clone());
                    cache
                        .write(&inst.name, project_id, task_id, &task, &comments_value)
                        .ok();
                    (task, comments)
                }
            }
        }
    };

    let user_map = resolve_user_map_with_conn(conn, &inst.name, &client, refresh).await;
    let assets = extract_assets(&task, &comments);

    DetailData {
        task,
        comments,
        assets,
        user_map,
    }
}

/// Resolve the user map using the provided connection (test seam).
///
/// Reads from UserMapCache synchronously (no Connection held across await),
/// then fetches the network only on miss or refresh, and writes the result back.
#[cfg(test)]
async fn resolve_user_map_with_conn(
    conn: &rusqlite::Connection,
    instance_name: &str,
    client: &ActiveCollabClient,
    refresh: bool,
) -> HashMap<i64, String> {
    let cached: Option<HashMap<i64, String>> = if !refresh {
        UserMapCache::new(conn)
            .read(instance_name)
            .ok()
            .flatten()
            .map(|hit| hit.users)
    } else {
        None
    };

    if let Some(map) = cached {
        return map;
    }

    let map = client.fetch_user_map().await.unwrap_or_default();
    if !map.is_empty() {
        UserMapCache::new(conn).write(instance_name, &map).ok();
    }
    map
}

#[cfg(test)]
#[path = "../tests/unit/controller.rs"]
mod tests;
