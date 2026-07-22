use crate::client::ActiveCollabClient;
use crate::config::Config;
use crate::http::Http;
use crate::models::MineTask;
use crate::render::{is_openable_url, Asset, MineTableRow};
use crate::store::cache::{ProjectNamesCache, TaskCache, UserMapCache};
use crate::store::instances::Instance;
use crate::store::Store;
use crate::tui::model::{ProjectGroup, TaskRow};
use anyhow::anyhow;
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
    ProjectNamesCache::new(store.conn())
        .read_fresh(instance_name, PROJECT_NAMES_TTL_SECS)
        .ok()
        .flatten()
        .map(|cached| cached.names)
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

/// Resolve each row's `project_name` from the per-instance project-names cache
/// (ADR 0014).
///
/// The cache is read at most once per distinct instance (memoised in a local map).
/// When a cache entry is absent (cold cache) or the project id is not present,
/// `project_name` is left as `None` — never wrong, gracefully absent
/// (ADR 0026 amendment).
///
/// Per-instance isolation (ADR 0018 / B1): a project id cached under instance A
/// is never resolved for a row on instance B.
pub fn attach_project_names(db_path: &Path, rows: Vec<MineTableRow>) -> Vec<MineTableRow> {
    let mut instance_cache: HashMap<String, Option<HashMap<i64, String>>> = HashMap::new();

    rows.into_iter()
        .map(|mut row| {
            let names = instance_cache
                .entry(row.instance.clone())
                .or_insert_with(|| fresh_project_names_cache_read(db_path, &row.instance));
            row.project_name = names.as_ref().and_then(|m| m.get(&row.project_id).cloned());
            row
        })
        .collect()
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
            due_on: None,
            project_name: None,
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
///
/// `unauthorized` is true when the fetch returned HTTP 401; other non-200
/// responses leave it false (collapsed to empty content as before).
pub struct TaskCore {
    pub task: Value,
    pub comments: Vec<Value>,
    pub assets: Vec<Asset>,
    pub unauthorized: bool,
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

/// Extract inline `<img src>` assets from HTML — the shared regex walk behind
/// both `assets_from_html` (all assets) and `downloadable_assets` (attachments
/// + images only, never anchor hyperlinks; ADR 0066).
fn image_assets_from_html(html: &str) -> Vec<Asset> {
    img_src_re()
        .captures_iter(html)
        .map(|cap| {
            let url = cap[1].to_string();
            let name = derive_asset_label(&url, None);
            Asset { name, url }
        })
        .collect()
}

fn assets_from_html(html: &str) -> Vec<Asset> {
    let mut assets = image_assets_from_html(html);
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

/// Extract the narrower set of assets worth downloading as files: real
/// `task.attachments` entries plus inline `<img src>` sources from the task
/// body and comments. Never includes anchor-hyperlink assets — a comment
/// author's link is not necessarily a file ActiveCollab is hosting (ADR 0066).
///
/// Deduplicates by URL, preserving first-seen order.
pub fn downloadable_assets(task: &Value, comments: &[Value]) -> Vec<Asset> {
    let mut seen = std::collections::HashSet::new();
    let mut result = vec![];

    let mut add = |asset: Asset| {
        if seen.insert(asset.url.clone()) {
            result.push(asset);
        }
    };

    let body_html = task.get("body").and_then(|v| v.as_str()).unwrap_or("");
    for asset in image_assets_from_html(body_html) {
        add(asset);
    }

    for comment in comments {
        let comment_html = comment.get("body").and_then(|v| v.as_str()).unwrap_or("");
        for asset in image_assets_from_html(comment_html) {
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

const FALLBACK_ASSET_PREFIX: &str = "asset_";

/// Sanitize an untrusted attachment display name into a safe on-disk file
/// name (ADR 0066): keeps only the final path component — splitting on `/`
/// and `\\` and dropping empty segments so a trailing separator collapses
/// like a real path — and falls back to `asset_{fallback_index}` when that
/// component is empty, `.`, `..`, or made up only of dots. Those are the
/// only ways sanitization can invalidate a name, and each is structurally a
/// name with no extension to preserve.
///
/// Pure — performs no filesystem access. Collision suffixing across a batch
/// and the write-containment check happen in `download_task_attachments`.
pub fn sanitize_attachment_filename(name: &str, fallback_index: usize) -> String {
    let candidate = final_path_component(name);
    if is_safe_filename(&candidate) {
        candidate
    } else {
        format!("{FALLBACK_ASSET_PREFIX}{fallback_index}")
    }
}

fn final_path_component(name: &str) -> String {
    name.split(['/', '\\'])
        .rfind(|segment| !segment.is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn is_safe_filename(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.chars().all(|c| c == '.')
}

/// Default destination directory for `download_task_attachments`: a stable,
/// per-task path under the OS temp dir so repeated runs land in the same
/// place (ADR 0066).
pub fn default_attachments_dir(project_id: i64, task_id: i64) -> PathBuf {
    std::env::temp_dir()
        .join("ac-attachments")
        .join(format!("{project_id}-{task_id}"))
}

/// Per-asset outcome of `download_task_attachments`: `path` is set on
/// success, `error` on failure — exactly one of the two is present.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadedAsset {
    pub name: String,
    pub url: String,
    pub path: Option<String>,
    pub error: Option<String>,
}

/// Fixed per-asset body size cap (ADR 0066) — no streaming/Content-Length
/// short-circuit, an oversized body is rejected after the full fetch.
const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;

/// Download each of `assets` over `client`'s host-gated authenticated seam
/// and write it under `dest_dir`, creating the directory once up front.
///
/// Each asset gets an independent outcome, in input order: a transport
/// error, a non-200 response, an oversized body (`MAX_ATTACHMENT_BYTES`), an
/// unsafe write path, or an I/O error produces an `error` entry for that
/// asset only — no other asset is skipped or aborted (ADR 0066).
pub async fn download_task_attachments(
    client: &ActiveCollabClient,
    assets: &[Asset],
    dest_dir: &Path,
) -> Vec<DownloadedAsset> {
    if let Err(err) = std::fs::create_dir_all(dest_dir) {
        return assets
            .iter()
            .enumerate()
            .map(|(index, asset)| DownloadedAsset {
                name: sanitize_attachment_filename(&asset.name, index + 1),
                url: asset.url.clone(),
                path: None,
                error: Some(format!("failed to create destination directory: {err}")),
            })
            .collect();
    }

    let mut used_names = std::collections::HashSet::new();
    let mut results = Vec::with_capacity(assets.len());
    for (index, asset) in assets.iter().enumerate() {
        let outcome = download_one_asset(client, asset, dest_dir, index + 1, &mut used_names).await;
        results.push(outcome);
    }
    results
}

async fn download_one_asset(
    client: &ActiveCollabClient,
    asset: &Asset,
    dest_dir: &Path,
    fallback_index: usize,
    used_names: &mut std::collections::HashSet<String>,
) -> DownloadedAsset {
    let base_name = sanitize_attachment_filename(&asset.name, fallback_index);
    let unique_name = unique_filename(base_name, used_names);

    match fetch_and_write_asset(client, asset, dest_dir, &unique_name).await {
        Ok(written_path) => DownloadedAsset {
            name: unique_name,
            url: asset.url.clone(),
            path: Some(written_path),
            error: None,
        },
        Err(err) => DownloadedAsset {
            name: unique_name,
            url: asset.url.clone(),
            path: None,
            error: Some(err),
        },
    }
}

/// Resolve `base_name` to a name not yet used within this batch, suffixing
/// `_2`, `_3`, … (preserving the extension) on collision.
fn unique_filename(
    base_name: String,
    used_names: &mut std::collections::HashSet<String>,
) -> String {
    if used_names.insert(base_name.clone()) {
        return base_name;
    }
    let (stem, ext) = split_stem_ext(&base_name);
    let mut suffix = 2;
    loop {
        let candidate = match &ext {
            Some(ext) => format!("{stem}_{suffix}.{ext}"),
            None => format!("{stem}_{suffix}"),
        };
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn split_stem_ext(name: &str) -> (String, Option<String>) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => {
            (stem.to_string(), Some(ext.to_string()))
        }
        _ => (name.to_string(), None),
    }
}

async fn fetch_and_write_asset(
    client: &ActiveCollabClient,
    asset: &Asset,
    dest_dir: &Path,
    file_name: &str,
) -> Result<String, String> {
    let (status, body) = client
        .fetch_asset_bytes(&asset.url)
        .await
        .map_err(|err| err.to_string())?;
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }
    if body.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "attachment exceeds max size of {MAX_ATTACHMENT_BYTES} bytes"
        ));
    }
    let write_path = contained_write_path(dest_dir, file_name)?;
    std::fs::write(&write_path, &body).map_err(|err| err.to_string())?;
    Ok(write_path.to_string_lossy().to_string())
}

/// Join `dest_dir`/`file_name` and verify the result stays inside `dest_dir`
/// once resolved — defense in depth against a sanitizer gap or a pre-existing
/// symlink escape (ADR 0066). Never writes; only computes the target path.
fn contained_write_path(dest_dir: &Path, file_name: &str) -> Result<PathBuf, String> {
    if file_name.is_empty() || file_name.contains(['/', '\\']) {
        return Err(format!("unsafe file name: {file_name}"));
    }
    let canonical_dest = dest_dir
        .canonicalize()
        .map_err(|err| format!("failed to resolve destination directory: {err}"))?;
    let candidate = canonical_dest.join(file_name);
    let resolved = resolve_existing_ancestor(&candidate);
    if !resolved.starts_with(&canonical_dest) {
        return Err(format!(
            "write path escapes destination directory: {file_name}"
        ));
    }
    Ok(candidate)
}

/// Canonicalize the nearest existing ancestor of `path` and re-append the
/// non-existent tail, so a target that does not exist yet (the common case
/// for a fresh download) can still be checked for symlink-based escape.
fn resolve_existing_ancestor(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if parent != path => {
            resolve_existing_ancestor(parent).join(name)
        }
        _ => path.to_path_buf(),
    }
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
    let (mut task, comments, unauthorized) =
        load_task_data_from_path(&db_path, &inst, &client, project_id, task_id, refresh).await;
    enrich_task_with_project_name(&mut task, &db_path, &inst.name, project_id, &client).await;
    let assets = extract_assets(&task, &comments);
    TaskCore {
        task,
        comments,
        assets,
        unauthorized,
    }
}

/// Injects `project_name` into `task`, resolving it on a cache miss (ADR 0056).
///
/// A fresh cache hit for `project_id` is used with no network call. On a miss the
/// name is fetched via one `GET /api/v1/projects/{id}`, written back to
/// `ProjectNamesCache` merged into the instance's existing map, and used; a fetch
/// failure or nameless response falls back to `t("(unknown)")` so the Projeto row
/// in the Details panel is never blank.
async fn enrich_task_with_project_name(
    task: &mut Value,
    db_path: &Path,
    instance_name: &str,
    project_id: i64,
    client: &ActiveCollabClient,
) {
    let name = resolve_project_name(db_path, instance_name, project_id, client).await;
    if let Some(obj) = task.as_object_mut() {
        obj.insert("project_name".to_string(), Value::String(name));
    }
}

/// Resolve the display name for `project_id`: fresh cache hit first, otherwise a
/// single `fetch_project_name` call with write-back on success and the
/// `t("(unknown)")` fallback otherwise.
async fn resolve_project_name(
    db_path: &Path,
    instance_name: &str,
    project_id: i64,
    client: &ActiveCollabClient,
) -> String {
    if let Some(name) = fresh_project_names_cache_read(db_path, instance_name)
        .and_then(|names| names.get(&project_id).cloned())
    {
        return name;
    }
    match client.fetch_project_name(project_id).await {
        Ok(Some(name)) => {
            try_project_name_cache_merge_write(db_path, instance_name, project_id, &name);
            name
        }
        _ => project_name_from_cache(db_path, instance_name, project_id),
    }
}

/// Merge `{project_id: name}` into the instance's existing project-names map and
/// write the merged map back, preserving sibling entries.
fn try_project_name_cache_merge_write(
    db_path: &Path,
    instance_name: &str,
    project_id: i64,
    name: &str,
) {
    let config = Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };
    let store = match Store::open(&config) {
        Ok(store) => store,
        Err(_) => return,
    };
    let cache = ProjectNamesCache::new(store.conn());
    let mut names = cache
        .read(instance_name)
        .ok()
        .flatten()
        .map(|cached| cached.names)
        .unwrap_or_default();
    names.insert(project_id, name.to_string());
    cache.write(instance_name, &names).ok();
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
/// Returns `(task, comments, unauthorized)`. `unauthorized` is true only when
/// the network returned HTTP 401; cache hits and other non-200 responses leave
/// it false (non-401 errors still collapse to Value::Null / empty as before).
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
) -> (Value, Vec<Value>, bool) {
    let config = Config {
        db_path: db_path.to_path_buf(),
        task_cache_ttl_hours: 24,
    };

    if !refresh {
        let t = std::time::Instant::now();
        let cache_hit = try_cache_read(&config, &inst.name, project_id, task_id);
        crate::timing::record("task_cache_read", t.elapsed());
        if let Some(hit) = cache_hit {
            return (hit.0, hit.1, false);
        }
    }

    let t = std::time::Instant::now();
    let result = fetch_from_network(client, project_id, task_id).await;
    crate::timing::record("fetch_task", t.elapsed());
    match result {
        FetchResult::Unauthorized => (Value::Null, vec![], true),
        FetchResult::Err => (Value::Null, vec![], false),
        FetchResult::Ok(task, comments) => {
            try_cache_write(&config, &inst.name, project_id, task_id, &task, &comments);
            (task, comments, false)
        }
    }
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

enum FetchResult {
    Ok(Value, Vec<Value>),
    Unauthorized,
    Err,
}

async fn fetch_from_network(
    client: &ActiveCollabClient,
    project_id: i64,
    task_id: i64,
) -> FetchResult {
    let Ok((status, payload_opt)) = client.fetch_task(project_id, task_id).await else {
        return FetchResult::Err;
    };
    if status == crate::http::HTTP_UNAUTHORIZED {
        return FetchResult::Unauthorized;
    }
    if status != 200 {
        return FetchResult::Err;
    }
    let (task, comments) = parse_task_payload(payload_opt.unwrap_or(Value::Null));
    FetchResult::Ok(task, comments)
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
                FetchResult::Unauthorized | FetchResult::Err => (Value::Null, vec![]),
                FetchResult::Ok(task, comments) => {
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
