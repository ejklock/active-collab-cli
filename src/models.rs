#![allow(dead_code)]

use serde::Deserialize;

fn default_string() -> String {
    String::new()
}

fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// A single ActiveCollab task, unwrapped from the API `single` key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Task {
    #[serde(default)]
    pub id: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
    #[serde(default)]
    pub task_number: Option<i64>,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub is_trashed: bool,
    #[serde(default)]
    pub assignee_id: Option<i64>,
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub body: String,
    #[serde(default)]
    pub start_on: Option<serde_json::Value>,
    #[serde(default)]
    pub due_on: Option<serde_json::Value>,
    #[serde(default)]
    pub estimate: Option<f64>,
    /// Supplied by the caller, never from the API `single` dict.
    #[serde(skip)]
    pub tracked_time: Option<f64>,
}

/// A comment on an ActiveCollab task.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Comment {
    #[serde(default)]
    pub id: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub body: String,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub body_plain_text: String,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub created_by_name: String,
    #[serde(default)]
    pub created_by_id: Option<i64>,
    #[serde(default)]
    pub created_on: Option<serde_json::Value>,
}

/// An ActiveCollab project from the projects list endpoint.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub id: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
    #[serde(default)]
    pub is_trashed: bool,
}

/// A lightweight task entry from the /users/{id}/tasks endpoint.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MineTask {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub task_number: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub is_trashed: bool,
    #[serde(default)]
    pub project_id: Option<i64>,
    /// Set by the client from the instance name, not from the API payload.
    #[serde(skip)]
    pub instance_name: String,
}

impl MineTask {
    pub fn from_api(data: &serde_json::Value, instance_name: &str) -> Self {
        let obj = data.as_object();
        MineTask {
            id: obj
                .and_then(|o| o.get("id"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            task_number: obj
                .and_then(|o| o.get("task_number"))
                .and_then(|v| v.as_i64()),
            name: obj
                .and_then(|o| o.get("name"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("")
                .to_string(),
            is_completed: obj
                .and_then(|o| o.get("is_completed"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            is_trashed: obj
                .and_then(|o| o.get("is_trashed"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            project_id: obj
                .and_then(|o| o.get("project_id"))
                .and_then(|v| v.as_i64()),
            instance_name: instance_name.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/models.rs"]
mod tests;
