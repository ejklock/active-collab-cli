use crate::app::{ProjectGroup, TaskRow};
use crate::client::ActiveCollabClient;
use crate::config::Config;
use crate::http::Http;
use crate::models::MineTask;
use crate::render::{extract_assets, is_openable_url, Asset};
use crate::store::cache::TaskCache;
use crate::store::instances::Instance;
use crate::store::Store;
use anyhow::{anyhow, Context};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Aggregates open tasks across all target instances, maps project ids to
/// names, groups tasks by project, and sorts alphabetically.
///
/// When `list_projects` fails for an instance, task groups fall back to
/// using the numeric project id as the name — no crash.
pub async fn tasks_by_project(targets: &[Instance], http: &Http) -> Vec<ProjectGroup> {
    let mut all_tasks: Vec<(MineTask, String)> = vec![];
    let mut project_names: HashMap<i64, String> = HashMap::new();

    for inst in targets {
        let client = ActiveCollabClient::new(inst.clone(), http.clone());

        let tasks = client.fetch_open_tasks().await.unwrap_or_default();

        let names = fetch_project_names(&client).await;
        project_names.extend(names);

        for task in tasks {
            all_tasks.push((task, inst.name.clone()));
        }
    }

    build_groups(all_tasks, &project_names)
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
    project_names: &HashMap<i64, String>,
) -> Vec<ProjectGroup> {
    let mut groups: HashMap<i64, ProjectGroup> = HashMap::new();

    for (task, instance_name) in tasks {
        let pid = task.project_id.unwrap_or(0);
        let project_name = project_names
            .get(&pid)
            .cloned()
            .unwrap_or_else(|| pid.to_string());

        let group = groups.entry(pid).or_insert_with(|| ProjectGroup {
            project_id: pid,
            project_name,
            tasks: vec![],
        });

        group.tasks.push(TaskRow {
            task_id: task.id,
            task_number: task.task_number.unwrap_or(task.id),
            name: task.name,
            instance: instance_name,
        });
    }

    let mut sorted: Vec<ProjectGroup> = groups.into_values().collect();
    sorted.sort_by(|a, b| a.project_name.cmp(&b.project_name));
    for group in &mut sorted {
        group.tasks.sort_by_key(|t| t.task_number);
    }
    sorted
}

/// All data needed to render a task detail screen.
pub struct DetailData {
    pub task: Value,
    pub comments: Vec<Value>,
    pub assets: Vec<Asset>,
    pub user_map: HashMap<i64, String>,
}

/// Fetch or serve from cache the full detail for a single task.
///
/// When `refresh` is false and a cache entry exists, no network call is made.
/// When `refresh` is true or the cache misses, the API is called and the result
/// is written back to the cache. The user map is fetched to resolve assignee
/// names; failures yield an empty map. Opens its own DB connection so the
/// caller can pass it to tokio::spawn without lifetime issues.
pub async fn task_detail(
    db_path: PathBuf,
    inst: Instance,
    http: Http,
    project_id: i64,
    task_id: i64,
    refresh: bool,
) -> DetailData {
    let client = ActiveCollabClient::new(inst.clone(), http);

    let (task, comments) =
        load_task_data_from_path(&db_path, &inst, &client, project_id, task_id, refresh).await;
    let user_map = fetch_user_map_graceful(&client).await;
    let assets = extract_assets(&task, &comments);

    DetailData {
        task,
        comments,
        assets,
        user_map,
    }
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
        if let Some(hit) = try_cache_read(&config, &inst.name, project_id, task_id) {
            return hit;
        }
    }

    let result = fetch_from_network(client, project_id, task_id).await;
    let (task, comments) = match result {
        None => return (Value::Null, vec![]),
        Some(pair) => pair,
    };

    try_cache_write(&config, &inst.name, project_id, task_id, &task, &comments);
    (task, comments)
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
    let mut task = cached.fields;
    let comments = task
        .as_object_mut()
        .and_then(|obj| obj.remove("comments"))
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    Some((task, comments))
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

async fn fetch_user_map_graceful(client: &ActiveCollabClient) -> HashMap<i64, String> {
    client.fetch_user_map().await.unwrap_or_default()
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
            .map(|cached| {
                let mut task = cached.fields;
                let comments = task
                    .as_object_mut()
                    .and_then(|obj| obj.remove("comments"))
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default();
                (task, comments)
            })
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

    let user_map = fetch_user_map_graceful(&client).await;
    let assets = extract_assets(&task, &comments);

    DetailData {
        task,
        comments,
        assets,
        user_map,
    }
}

#[cfg(test)]
#[path = "../tests/unit/controller.rs"]
mod tests;
