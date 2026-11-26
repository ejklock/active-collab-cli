//! The board phase of a task, read from its `STATUS:` labels and its task
//! list name through editable keyword tables.

use crate::models::Label;
use crate::store::settings::SettingsRepository;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const STATUS_PREFIX: &str = "status:";

/// A column of the board, in display order (`Ord` follows it).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    ToDo,
    InProgress,
    InReview,
    ReadyForRelease,
}

impl Phase {
    pub const ALL: [Phase; 4] = [
        Phase::ToDo,
        Phase::InProgress,
        Phase::InReview,
        Phase::ReadyForRelease,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Phase::ToDo => "to_do",
            Phase::InProgress => "in_progress",
            Phase::InReview => "in_review",
            Phase::ReadyForRelease => "ready_for_release",
        }
    }

    pub fn from_key(key: &str) -> Option<Phase> {
        Phase::ALL.into_iter().find(|phase| phase.key() == key)
    }
}

/// The words that put a task in one column.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ColumnKeywords {
    /// Matched against the task list name.
    pub lists: Vec<String>,
    /// Matched against the value of the `STATUS:` labels.
    pub labels: Vec<String>,
}

/// Every column's keywords plus the words that flag a task as waiting. The
/// stored value holds only the fields the user changed; the others take the
/// built-in keywords of the running version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardMapping {
    pub to_do: ColumnKeywords,
    pub in_progress: ColumnKeywords,
    pub in_review: ColumnKeywords,
    pub ready_for_release: ColumnKeywords,
    pub flag_labels: Vec<String>,
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

impl Default for BoardMapping {
    fn default() -> Self {
        Self {
            to_do: ColumnKeywords {
                lists: words(&["backlog", "to do"]),
                labels: words(&["estimativa", "nova demanda", "análise", "analise"]),
            },
            in_progress: ColumnKeywords {
                lists: words(&["doing", "desenvolvimento"]),
                labels: words(&["desenvolvimento"]),
            },
            in_review: ColumnKeywords {
                lists: words(&["to test", "review", "com o cliente", "homolog"]),
                labels: words(&["validação", "validacao"]),
            },
            ready_for_release: ColumnKeywords {
                lists: words(&["go live", "ready to go", "done"]),
                labels: words(&[
                    "go live",
                    "goli",
                    "deploy",
                    "publicação",
                    "publicacao",
                    "done",
                ]),
            },
            flag_labels: words(&["aguardando", "aguarndo", "bloqueado", "stand-by"]),
        }
    }
}

impl BoardMapping {
    pub fn column(&self, phase: Phase) -> &ColumnKeywords {
        match phase {
            Phase::ToDo => &self.to_do,
            Phase::InProgress => &self.in_progress,
            Phase::InReview => &self.in_review,
            Phase::ReadyForRelease => &self.ready_for_release,
        }
    }

    pub fn column_mut(&mut self, phase: Phase) -> &mut ColumnKeywords {
        match phase {
            Phase::ToDo => &mut self.to_do,
            Phase::InProgress => &mut self.in_progress,
            Phase::InReview => &mut self.in_review,
            Phase::ReadyForRelease => &mut self.ready_for_release,
        }
    }
}

/// The settings key the overrides are stored under, as one JSON object.
pub const SETTINGS_KEY: &str = "board_mapping";

/// The mapping in effect, and whether a stored value had to be discarded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMapping {
    pub mapping: BoardMapping,
    /// A stored value, or a field of it, is malformed and was ignored.
    pub invalid: bool,
}

/// The two keyword lists of a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnField {
    Lists,
    Labels,
}

impl ColumnField {
    fn key(self) -> &'static str {
        match self {
            ColumnField::Lists => "lists",
            ColumnField::Labels => "labels",
        }
    }
}

const FLAG_LABELS_KEY: &str = "flag_labels";

fn string_array(value: &serde_json::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// Replaces `target` with the array at `key` of `object` when present; a
/// present value that is not an array of strings leaves `target` and returns
/// `false`.
fn overlay_words(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    target: &mut Vec<String>,
) -> bool {
    match object.get(key) {
        None => true,
        Some(value) => match string_array(value) {
            Some(words) => {
                *target = words;
                true
            }
            None => false,
        },
    }
}

/// Reads the stored overrides over the built-in keywords of this version; an
/// absent, unreadable or malformed value, or field, keeps the built-ins, so a
/// bad value never stops a caller.
pub fn load_checked(settings: &SettingsRepository<'_>) -> StoredMapping {
    let mut mapping = BoardMapping::default();
    let Some(text) = settings.get(SETTINGS_KEY, None).ok().flatten() else {
        return StoredMapping {
            mapping,
            invalid: false,
        };
    };
    let Ok(serde_json::Value::Object(stored)) = serde_json::from_str(&text) else {
        return StoredMapping {
            mapping,
            invalid: true,
        };
    };
    let mut valid = overlay_words(&stored, FLAG_LABELS_KEY, &mut mapping.flag_labels);
    for phase in Phase::ALL {
        let Some(column) = stored.get(phase.key()) else {
            continue;
        };
        let Some(column) = column.as_object() else {
            valid = false;
            continue;
        };
        let keywords = mapping.column_mut(phase);
        valid &= overlay_words(column, ColumnField::Lists.key(), &mut keywords.lists);
        valid &= overlay_words(column, ColumnField::Labels.key(), &mut keywords.labels);
    }
    StoredMapping {
        mapping,
        invalid: !valid,
    }
}

pub fn load(settings: &SettingsRepository<'_>) -> BoardMapping {
    load_checked(settings).mapping
}

/// Edits the stored override object (an empty one when nothing usable is
/// stored) and writes it back.
fn update_overrides(
    settings: &SettingsRepository<'_>,
    edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) -> anyhow::Result<()> {
    let stored = settings.get(SETTINGS_KEY, None).ok().flatten();
    let mut overrides = match stored.and_then(|text| serde_json::from_str(&text).ok()) {
        Some(serde_json::Value::Object(object)) => object,
        _ => serde_json::Map::new(),
    };
    edit(&mut overrides);
    settings.set(
        SETTINGS_KEY,
        &serde_json::Value::Object(overrides).to_string(),
    )
}

/// Stores one keyword list of a column, keeping the other stored overrides.
pub fn save_column_override(
    settings: &SettingsRepository<'_>,
    phase: Phase,
    field: ColumnField,
    words: Vec<String>,
) -> anyhow::Result<()> {
    update_overrides(settings, |overrides| {
        let slot = overrides
            .entry(phase.key())
            .or_insert_with(|| serde_json::json!({}));
        if !slot.is_object() {
            *slot = serde_json::json!({});
        }
        slot[field.key()] = serde_json::json!(words);
    })
}

/// Stores the flag keywords, keeping the other stored overrides.
pub fn save_flag_override(
    settings: &SettingsRepository<'_>,
    words: Vec<String>,
) -> anyhow::Result<()> {
    update_overrides(settings, |overrides| {
        overrides.insert(FLAG_LABELS_KEY.to_owned(), serde_json::json!(words));
    })
}

/// Drops every override, so the built-in keywords apply again.
pub fn reset(settings: &SettingsRepository<'_>) -> anyhow::Result<()> {
    update_overrides(settings, serde_json::Map::clear)
}

/// Splits comma-separated keywords, trimming each and dropping empty ones.
pub fn parse_keywords(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Where a task sits on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placement {
    pub phase: Phase,
    /// A status label says the task is waiting or blocked.
    pub flagged: bool,
}

/// Places a task: the furthest phase a status label names, else the furthest
/// phase its list name names, else `ToDo`.
pub fn place(labels: &[Label], task_list: Option<&str>, mapping: &BoardMapping) -> Placement {
    let values: Vec<String> = labels.iter().filter_map(status_value).collect();
    let by_label = furthest(|phase| {
        let keywords = &mapping.column(phase).labels;
        values.iter().any(|value| matches_any(value, keywords))
    });
    let phase = by_label
        .or_else(|| {
            let list = task_list.map(normalize)?;
            furthest(|phase| matches_any(&list, &mapping.column(phase).lists))
        })
        .unwrap_or_default();
    let flagged = values
        .iter()
        .any(|value| matches_any(value, &mapping.flag_labels));
    Placement { phase, flagged }
}

fn furthest(matches: impl Fn(Phase) -> bool) -> Option<Phase> {
    Phase::ALL.into_iter().rev().find(|phase| matches(*phase))
}

/// The normalized text after the `STATUS:` prefix, for a status label only.
fn status_value(label: &Label) -> Option<String> {
    let lowered = normalize(&label.name);
    let value = lowered.strip_prefix(STATUS_PREFIX)?;
    Some(value.trim().to_owned())
}

fn normalize(text: &str) -> String {
    text.trim().to_lowercase()
}

fn matches_any(haystack: &str, keywords: &[String]) -> bool {
    keywords.iter().any(|keyword| {
        let keyword = normalize(keyword);
        !keyword.is_empty() && haystack.contains(&keyword)
    })
}

#[cfg(test)]
#[path = "../tests/unit/board.rs"]
mod tests;
