#![allow(dead_code)]

use crate::http::{Http, HTTP_UNAUTHORIZED};
use crate::models::MineTask;
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

    /// POST /api/v1/comments/task/{task_id}. Returns (status, Some(comment))
    /// on 2xx, (status, None) otherwise.
    pub async fn create_comment(&self, task_id: i64, body: &str) -> Result<(u16, Option<Value>)> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/comments/task/{}", base, task_id);
        let payload = serde_json::json!({ "body": body });
        let (status, raw) = self
            .http
            .authed_post(
                &url,
                &self.instance.base_url,
                &self.instance.token,
                &payload,
            )
            .await?;
        if (200..=299).contains(&status) {
            return Ok((status, serde_json::from_slice(&raw).ok()));
        }
        Ok((status, None))
    }

    /// PUT /api/v1/comments/{comment_id}. Returns (status, Some(comment))
    /// on 2xx, (status, None) otherwise.
    pub async fn update_comment(
        &self,
        comment_id: i64,
        body: &str,
    ) -> Result<(u16, Option<Value>)> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/comments/{}", base, comment_id);
        let payload = serde_json::json!({ "body": body });
        let (status, raw) = self
            .http
            .authed_put(
                &url,
                &self.instance.base_url,
                &self.instance.token,
                &payload,
            )
            .await?;
        if (200..=299).contains(&status) {
            return Ok((status, serde_json::from_slice(&raw).ok()));
        }
        Ok((status, None))
    }

    /// DELETE /api/v1/comments/{comment_id}. Returns the response status.
    pub async fn delete_comment(&self, comment_id: i64) -> Result<u16> {
        let base = self.instance.base_url.trim_end_matches('/');
        let url = format!("{}/api/v1/comments/{}", base, comment_id);
        let (status, _) = self
            .http
            .authed_delete(&url, &self.instance.base_url, &self.instance.token)
            .await?;
        Ok(status)
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
