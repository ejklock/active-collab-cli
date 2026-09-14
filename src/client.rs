#![allow(dead_code)]

use crate::http::{Http, HTTP_UNAUTHORIZED};
use crate::models::{JobType, MineTask};
use crate::store::instances::Instance;
use anyhow::Result;
use serde_json::Value;

/// Typed error raised by fetch_open_tasks when the server returns HTTP 401.
/// Carried by anyhow::Result so callers can downcast and surface re-auth guidance.
#[derive(Debug)]
pub struct Unauthorized;

impl std::fmt::Display for Unauthorized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP 401 Unauthorized")
    }
}

impl std::error::Error for Unauthorized {}

/// Typed outcome of a comment-write call (create/update/delete), classified
/// once by the client instead of by each caller (ADR 0054).
#[derive(Debug)]
pub enum CommentWriteOutcome {
    /// 2xx — carries the response body when present (`None` for delete).
    Ok(Option<Value>),
    /// HTTP 401.
    Unauthorized,
    /// Any other status.
    Failed(u16),
}

/// Typed outcome of a time-record write call, classified the same way as
/// `CommentWriteOutcome` (ADR 0054).
#[derive(Debug)]
pub enum TimeWriteOutcome {
    /// 2xx — carries the response body when present.
    Ok(Option<Value>),
    /// HTTP 401.
    Unauthorized,
    /// Any other status.
    Failed(u16),
}

/// The three buckets every write response falls into, shared by every
/// per-endpoint `classify_*_write` function so the (200..=299)/401/else
/// logic lives in one place.
enum WriteStatus {
    Ok,
    Unauthorized,
    Failed(u16),
}

fn classify_write_status(status: u16) -> WriteStatus {
    if (200..=299).contains(&status) {
        return WriteStatus::Ok;
    }
    if status == HTTP_UNAUTHORIZED {
        return WriteStatus::Unauthorized;
    }
    WriteStatus::Failed(status)
}

/// Classify a comment-write response status/body into a `CommentWriteOutcome`:
/// (200..=299) -> Ok(body), HTTP_UNAUTHORIZED -> Unauthorized, else -> Failed(status).
fn classify_comment_write(status: u16, body: Option<Value>) -> CommentWriteOutcome {
    match classify_write_status(status) {
        WriteStatus::Ok => CommentWriteOutcome::Ok(body),
        WriteStatus::Unauthorized => CommentWriteOutcome::Unauthorized,
        WriteStatus::Failed(status) => CommentWriteOutcome::Failed(status),
    }
}

/// Classify a time-record-write response status/body into a `TimeWriteOutcome`:
/// (200..=299) -> Ok(body), HTTP_UNAUTHORIZED -> Unauthorized, else -> Failed(status).
fn classify_time_write(status: u16, body: Option<Value>) -> TimeWriteOutcome {
    match classify_write_status(status) {
        WriteStatus::Ok => TimeWriteOutcome::Ok(body),
        WriteStatus::Unauthorized => TimeWriteOutcome::Unauthorized,
        WriteStatus::Failed(status) => TimeWriteOutcome::Failed(status),
    }
}

/// Return the id of the instance's default job type, falling back to the
/// first entry when none is flagged default. ActiveCollab requires a
/// `job_type_id` on every time record, so a caller with no explicit
/// preference needs one resolved from the instance's job-types list.
pub fn pick_default_job_type(job_types: &[JobType]) -> Option<i64> {
    job_types
        .iter()
        .find(|job_type| job_type.is_default)
        .or_else(|| job_types.first())
        .map(|job_type| job_type.id)
}

/// Encodes a plain-text comment body as the HTML ActiveCollab expects: text is
/// escaped, blank-line-separated paragraphs become `<p>...</p>` blocks, and a
/// single in-paragraph newline becomes `<br>` — newlines carry no meaning in
/// the rendered HTML, so without this the server flattens every paragraph
/// break the user typed (issue 0066).
fn encode_comment_body(body: &str) -> String {
    let escaped = html_escape::encode_text(body).into_owned();
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current_lines: Vec<&str> = Vec::new();
    for line in escaped.split('\n') {
        if line.trim().is_empty() {
            if !current_lines.is_empty() {
                paragraphs.push(current_lines.join("<br>"));
                current_lines.clear();
            }
        } else {
            current_lines.push(line);
        }
    }
    if !current_lines.is_empty() {
        paragraphs.push(current_lines.join("<br>"));
    }
    paragraphs
        .into_iter()
        .map(|paragraph| format!("<p>{}</p>", paragraph))
        .collect::<Vec<_>>()
        .join("")
}

pub struct ActiveCollabClient {
    instance: Instance,
    http: Http,
}

impl ActiveCollabClient {
    pub fn new(instance: Instance, http: Http) -> Self {
        ActiveCollabClient { instance, http }
    }

    /// POST to issue-token. Returns (Some(token), data) on 200+is_ok,
    /// (None, data) when 200 but !is_ok, (None, empty) on non-200.
    /// No token header is attached — this is a pre-auth call.
    pub async fn exchange_token(
        &self,
        base_url: &str,
        email: &str,
        password: &str,
    ) -> Result<(Option<String>, Value)> {
        let url = format!("{}/api/v1/issue-token", base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "username": email,
            "password": password,
            "client_name": "active-collab-skill",
            "client_vendor": "klock"
        });
        let (status, raw) = self.http.post_json(&url, &body).await?;
        if status != 200 {
            return Ok((None, serde_json::Value::Object(serde_json::Map::new())));
        }
        let data: Value = serde_json::from_slice(&raw).unwrap_or(Value::Null);
        if !data.get("is_ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Ok((None, data));
        }
        let token = data
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Ok((token, data))
    }

    /// GET /api/v1/users with the provided token and return the id whose email
    /// matches case-insensitively, else None.
    pub async fn resolve_user_id(
        &self,
        base_url: &str,
        token: &str,
        email: &str,
    ) -> Result<Option<i64>> {
        let url = format!("{}/api/v1/users", base_url.trim_end_matches('/'));
        let (status, body) = self.http.authed_get(&url, base_url, token).await?;
        if status != 200 {
            return Ok(None);
        }
        let data: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let users = match data.as_array() {
            Some(arr) => arr,
            None => return Ok(None),
        };
        let email_lower = email.to_lowercase();
        for user in users {
            let user_email = user
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            if user_email == email_lower {
                return Ok(user.get("id").and_then(|v| v.as_i64()));
            }
        }
        Ok(None)
    }

    /// Return {user_id: display_name}. Returns empty map on any failure.
    pub async fn fetch_user_map(&self) -> Result<std::collections::HashMap<i64, String>> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/users", base);
        let (status, body) = self
            .http
            .authed_get(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        if status != 200 {
            return Ok(std::collections::HashMap::new());
        }
        let data: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let users = match data.as_array() {
            Some(arr) => arr,
            None => return Ok(std::collections::HashMap::new()),
        };
        let mut result = std::collections::HashMap::new();
        for user in users {
            let uid = match user.get("id").and_then(|v| v.as_i64()) {
                Some(id) => id,
                None => continue,
            };
            let name = resolve_display_name(user);
            result.insert(uid, name);
        }
        Ok(result)
    }

    /// GET /api/v1/job-types. Returns an empty list on any non-200 response.
    pub async fn fetch_job_types(&self) -> Result<Vec<JobType>> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/job-types", base);
        let (status, body) = self
            .http
            .authed_get(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        if status != 200 {
            return Ok(vec![]);
        }
        Ok(serde_json::from_slice(&body).unwrap_or_default())
    }

    /// GET /api/v1/projects/{pid}/tasks/{tid}.
    /// Returns (200, Some(payload)) on 200, (status, None) otherwise.
    pub async fn fetch_task(&self, project_id: i64, task_id: i64) -> Result<(u16, Option<Value>)> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/projects/{}/tasks/{}", base, project_id, task_id);
        let (status, body) = self
            .http
            .authed_get(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        if status == 200 {
            let payload = serde_json::from_slice(&body).ok();
            return Ok((status, payload));
        }
        Ok((status, None))
    }

    /// Fetch open tasks assigned to this user. Returns empty on missing user_id
    /// or non-200 response. Excludes is_completed and is_trashed tasks.
    pub async fn fetch_open_tasks(&self) -> Result<Vec<MineTask>> {
        let user_id = match self.instance.user_id {
            Some(uid) if uid != 0 => uid,
            _ => return Ok(vec![]),
        };
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/users/{}/tasks", base, user_id);
        let (status, body) = self
            .http
            .authed_get(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        if status == HTTP_UNAUTHORIZED {
            return Err(Unauthorized.into());
        }
        if status != 200 {
            return Ok(vec![]);
        }
        let data: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let raw_tasks = data
            .as_object()
            .and_then(|obj| obj.get("tasks"))
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        let tasks = raw_tasks
            .iter()
            .filter(|t| {
                !t.get("is_completed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                    && !t
                        .get("is_trashed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
            })
            .map(|t| MineTask::from_api(t, &self.instance.name))
            .collect();
        Ok(tasks)
    }

    /// GET /api/v1/projects/{project_id}.
    /// ActiveCollab wraps single-object GET-by-id responses in a `single` object,
    /// so `name` is read from there first, with a flat top-level fallback for
    /// robustness. Returns `Some(name)` on a 200 with a non-empty resolved `name`,
    /// `None` otherwise (non-200, unparseable body, or missing/empty `name`).
    pub async fn fetch_project_name(&self, project_id: i64) -> Result<Option<String>> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/projects/{}", base, project_id);
        let (status, body) = self
            .http
            .authed_get(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        if status != 200 {
            return Ok(None);
        }
        let data: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let name = data
            .get("single")
            .and_then(|s| s.get("name"))
            .or_else(|| data.get("name"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        Ok(name)
    }

    /// GET /api/v1/projects. Used by connectivity checks.
    pub async fn list_projects(&self) -> Result<(u16, bytes::Bytes)> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/projects", base);
        self.http
            .authed_get(&url, &self.instance.base_url, &self.instance.token)
            .await
    }

    /// Alias for list_projects — used by setup test / setup add.
    pub async fn test_connectivity(&self) -> Result<(u16, bytes::Bytes)> {
        self.list_projects().await
    }

    /// POST `payload` as JSON over the authenticated instance seam and parse
    /// the body as JSON when the status is 2xx. Shared tail for every
    /// POST-based write (create_comment, create_time_record) so the
    /// authed_post + status-gated parse logic lives in one place.
    async fn post_json_write(&self, url: &str, payload: &Value) -> Result<(u16, Option<Value>)> {
        let (status, raw) = self
            .http
            .authed_post(url, &self.instance.base_url, &self.instance.token, payload)
            .await?;
        let body = if (200..=299).contains(&status) {
            serde_json::from_slice(&raw).ok()
        } else {
            None
        };
        Ok((status, body))
    }

    /// PUT `payload` as JSON over the authenticated instance seam and parse
    /// the body as JSON when the status is 2xx. Shared tail for every
    /// PUT-based write (update_comment).
    async fn put_json_write(&self, url: &str, payload: &Value) -> Result<(u16, Option<Value>)> {
        let (status, raw) = self
            .http
            .authed_put(url, &self.instance.base_url, &self.instance.token, payload)
            .await?;
        let body = if (200..=299).contains(&status) {
            serde_json::from_slice(&raw).ok()
        } else {
            None
        };
        Ok((status, body))
    }

    /// POST /api/v1/comments/task/{task_id}. Classifies the response into a
    /// `CommentWriteOutcome`: (200..=299) -> Ok(Some(comment)), 401 ->
    /// Unauthorized, else -> Failed(status).
    pub async fn create_comment(&self, task_id: i64, body: &str) -> Result<CommentWriteOutcome> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/comments/task/{}", base, task_id);
        let payload = serde_json::json!({ "body": encode_comment_body(body) });
        let (status, body) = self.post_json_write(&url, &payload).await?;
        Ok(classify_comment_write(status, body))
    }

    /// POST /api/v1/projects/{project_id}/time-records. Classifies the
    /// response into a `TimeWriteOutcome`: (200..=299) -> Ok(Some(record)),
    /// 401 -> Unauthorized, else -> Failed(status). `summary` is included
    /// in the body only when `Some`.
    pub async fn create_time_record(
        &self,
        project_id: i64,
        task_id: i64,
        value_hours: f64,
        record_date: &str,
        job_type_id: i64,
        summary: Option<&str>,
    ) -> Result<TimeWriteOutcome> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/projects/{}/time-records", base, project_id);
        let mut payload = serde_json::json!({
            "task_id": task_id,
            "value": value_hours,
            "record_date": record_date,
            "job_type_id": job_type_id,
        });
        if let Some(summary) = summary {
            payload["summary"] = Value::String(summary.to_string());
        }
        let (status, body) = self.post_json_write(&url, &payload).await?;
        Ok(classify_time_write(status, body))
    }

    /// PUT /api/v1/comments/{comment_id}. Classifies the response into a
    /// `CommentWriteOutcome`: (200..=299) -> Ok(Some(comment)), 401 ->
    /// Unauthorized, else -> Failed(status).
    pub async fn update_comment(&self, comment_id: i64, body: &str) -> Result<CommentWriteOutcome> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/comments/{}", base, comment_id);
        let payload = serde_json::json!({ "body": encode_comment_body(body) });
        let (status, body) = self.put_json_write(&url, &payload).await?;
        Ok(classify_comment_write(status, body))
    }

    /// DELETE /api/v1/comments/{comment_id}. Classifies the response into a
    /// `CommentWriteOutcome`: (200..=299) -> Ok(None) (no response body),
    /// 401 -> Unauthorized, else -> Failed(status).
    pub async fn delete_comment(&self, comment_id: i64) -> Result<CommentWriteOutcome> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/comments/{}", base, comment_id);
        let (status, _) = self
            .http
            .authed_delete(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        Ok(classify_comment_write(status, None))
    }

    /// GET a task asset (attachment or inline image) URL. Thin wrapper over
    /// `Http::authed_get` reusing the existing host-gated token attach/omit
    /// behavior — the token is attached only when `url`'s host matches the
    /// instance host. Returns Ok((status, body)) for any HTTP response; only
    /// transport failures are Err.
    pub async fn fetch_asset_bytes(&self, url: &str) -> Result<(u16, bytes::Bytes)> {
        self.http
            .authed_get(url, &self.instance.base_url, &self.instance.token)
            .await
    }
}

fn resolve_display_name(user: &Value) -> String {
    if let Some(name) = user.get("display_name").and_then(|v| v.as_str()) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    let first = user
        .get("first_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let last = user
        .get("last_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let full: String = [first, last]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !full.is_empty() {
        return full;
    }
    user.get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
#[path = "../tests/unit/client.rs"]
mod tests;
