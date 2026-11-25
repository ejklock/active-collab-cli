#![allow(dead_code)]

use crate::render::normalize_due_to_ymd;
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
    pub id: i64,
    #[serde(default)]
    pub task_number: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub is_trashed: bool,
    pub project_id: i64,
    /// Normalized to `YYYY-MM-DD`; `None` when absent, null, or unparseable.
    #[serde(default)]
    pub due_on: Option<String>,
    /// Estimated hours; `None` when the task has no estimate (absent, null or 0).
    #[serde(default)]
    pub estimate: Option<f64>,
    /// The task list the task sits in; `None` when the payload carries no integer id.
    #[serde(default)]
    pub task_list_id: Option<i64>,
    /// Name of that list; `None` until resolved from the response's `related.TaskList`.
    #[serde(skip)]
    pub task_list: Option<String>,
    /// Labels in payload order; entries without an integer id or a non-empty name are dropped.
    #[serde(skip)]
    pub labels: Vec<Label>,
    /// Set by the client from the instance name, not from the API payload.
    #[serde(skip)]
    pub instance_name: String,
}

/// A task label reduced to what an agent needs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
}

/// The labels of a task payload; absent, null or non-array yields none, and an
/// entry without an integer `id` or a non-empty string `name` is skipped.
pub fn parse_labels(task: &serde_json::Value) -> Vec<Label> {
    task.get("labels")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.get("id").and_then(|v| v.as_i64())?;
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())?;
            Some(Label {
                id,
                name: name.to_string(),
            })
        })
        .collect()
}

/// The name of task list `id` in a response's `related` side-load. The side-load
/// keys lists by id string; PHP serializes an empty map as `[]`, so an array or
/// a missing level simply has no name.
pub fn related_task_list_name(related: &serde_json::Value, id: i64) -> Option<String> {
    related
        .get("TaskList")
        .and_then(|lists| lists.get(id.to_string()))
        .and_then(|list| list.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl MineTask {
    /// Parses an API task entry; `None` when `id` or `project_id` is not an
    /// integer, since such an entry has no fetchable task reference.
    pub fn from_api(data: &serde_json::Value, instance_name: &str) -> Option<Self> {
        let obj = data.as_object();
        let id = obj.and_then(|o| o.get("id")).and_then(|v| v.as_i64())?;
        let project_id = obj
            .and_then(|o| o.get("project_id"))
            .and_then(|v| v.as_i64())?;
        Some(MineTask {
            id,
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
            project_id,
            due_on: obj
                .and_then(|o| o.get("due_on"))
                .and_then(normalize_due_to_ymd),
            estimate: obj
                .and_then(|o| o.get("estimate"))
                .and_then(|v| v.as_f64())
                .filter(|hours| *hours > 0.0),
            task_list_id: obj
                .and_then(|o| o.get("task_list_id"))
                .and_then(|v| v.as_i64()),
            task_list: None,
            labels: parse_labels(data),
            instance_name: instance_name.to_string(),
        })
    }
}

/// A time-tracking job type, used to classify a time record on creation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct JobType {
    #[serde(default)]
    pub id: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
}

fn deserialize_related<'de, D>(deserializer: D) -> Result<TimeRecordsRelated, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_array() || value.is_null() {
        return Ok(TimeRecordsRelated::default());
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

/// One time record of the filtered-by-date listing. `parent_type` is `Task`
/// or `Project`; `record_date` is a UTC-midnight unix timestamp.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TimeRecord {
    pub id: i64,
    pub value: f64,
    pub record_date: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub summary: String,
    pub parent_type: String,
    pub parent_id: i64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RelatedProject {
    pub id: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RelatedTask {
    pub id: i64,
    pub project_id: i64,
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub name: String,
}

/// The API keys each related entity by its id; the key repeats the entity's
/// own `id`, so only the values are kept.
fn deserialize_keyed_values<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Keyed<T> {
        Map(std::collections::HashMap<String, T>),
        Empty(Vec<serde::de::IgnoredAny>),
        Null(()),
    }
    match Keyed::<T>::deserialize(deserializer).map_err(serde::de::Error::custom)? {
        Keyed::Map(map) => Ok(map.into_values().collect()),
        Keyed::Empty(items) if items.is_empty() => Ok(Vec::new()),
        Keyed::Empty(_) => Err(serde::de::Error::custom(
            "expected an object keyed by id, got a non-empty array",
        )),
        Keyed::Null(()) => Ok(Vec::new()),
    }
}

/// The `related` side-load of a time-records response; either key may be
/// absent when empty.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TimeRecordsRelated {
    #[serde(
        default,
        rename = "Project",
        deserialize_with = "deserialize_keyed_values"
    )]
    pub projects: Vec<RelatedProject>,
    #[serde(
        default,
        rename = "Task",
        deserialize_with = "deserialize_keyed_values"
    )]
    pub tasks: Vec<RelatedTask>,
}

/// Response of `GET /users/{id}/time-records/filtered-by-date`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TimeRecordsReport {
    pub time_records: Vec<TimeRecord>,
    #[serde(default, deserialize_with = "deserialize_related")]
    pub related: TimeRecordsRelated,
}

#[cfg(test)]
#[path = "../tests/unit/models.rs"]
mod tests;
