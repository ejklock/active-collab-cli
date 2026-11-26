use super::hours_report::{resolve_period, PeriodPreset};
use super::presenter;
use super::resolve::load_targets;
use crate::agent_json;
use crate::board::{place, BoardMapping, Phase, Placement};
use crate::client::{ActiveCollabClient, TimeRecordsOutcome, Unauthorized};
use crate::http::Http;
use crate::i18n::t;
use crate::render::{self, MineTableRow};
use crate::store::instances::{Instance, InstanceRepository};
use std::io::Write;

/// One task's hours as fetched after its row was listed; `None` marks a value
/// the API did not give.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MineHours {
    pub instance: String,
    pub task_id: i64,
    pub logged_week: Option<f64>,
    pub logged_total: Option<f64>,
}

/// Which open tasks `ac mine` keeps. The default keeps every task.
#[derive(Debug, Clone, Default)]
pub(crate) struct MineFilter {
    /// Lowercased label names a task must all carry.
    labels: Vec<String>,
    task_list: Option<TaskListMatch>,
    /// The one board phase a task must sit in.
    phase: Option<Phase>,
    /// Places each task in a phase, for the phase requirement and the rows.
    mapping: BoardMapping,
}

/// A task list named by id (all ASCII digits) or by name, lowercased.
#[derive(Debug, Clone)]
enum TaskListMatch {
    Id(i64),
    Name(String),
    /// All digits but beyond `i64`: no task list has such an id.
    NoSuchId,
}

impl MineFilter {
    pub(crate) fn new(labels: &[String], task_list: Option<&str>) -> Self {
        Self {
            labels: labels.iter().map(|l| l.to_lowercase()).collect(),
            task_list: task_list.map(|value| {
                if !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()) {
                    value
                        .parse()
                        .map_or(TaskListMatch::NoSuchId, TaskListMatch::Id)
                } else {
                    TaskListMatch::Name(value.to_lowercase())
                }
            }),
            phase: None,
            mapping: Default::default(),
        }
    }

    /// Also requires the task to sit in `phase`.
    pub(crate) fn with_phase(self, phase: Option<Phase>) -> Self {
        Self { phase, ..self }
    }

    /// Places tasks with `mapping` instead of the built-in keywords.
    pub(crate) fn with_mapping(self, mapping: BoardMapping) -> Self {
        Self { mapping, ..self }
    }

    pub(crate) fn is_active(&self) -> bool {
        !self.labels.is_empty() || self.task_list.is_some() || self.phase.is_some()
    }

    fn keeps(&self, task: &crate::models::MineTask) -> bool {
        let phase_ok = self
            .phase
            .is_none_or(|wanted| placement_of(task, &self.mapping).phase == wanted);
        let list_ok = match &self.task_list {
            None => true,
            Some(TaskListMatch::Id(id)) => task.task_list_id == Some(*id),
            Some(TaskListMatch::Name(name)) => task
                .task_list
                .as_ref()
                .is_some_and(|list| list.to_lowercase() == *name),
            Some(TaskListMatch::NoSuchId) => false,
        };
        phase_ok
            && list_ok
            && self.labels.iter().all(|wanted| {
                task.labels
                    .iter()
                    .any(|label| label.name.to_lowercase() == *wanted)
            })
    }
}

/// Loads every target instance's rows concurrently, one result per target in
/// target order; an instance whose task did not finish yields no rows.
async fn fan_out_instance_rows(
    targets: &[Instance],
    http: &Http,
    today: chrono::NaiveDate,
    filter: &MineFilter,
) -> Vec<anyhow::Result<Vec<MineTableRow>>> {
    let mut set: tokio::task::JoinSet<(usize, anyhow::Result<Vec<MineTableRow>>)> =
        tokio::task::JoinSet::new();

    for (idx, inst) in targets.iter().enumerate() {
        let inst = inst.clone();
        let http = http.clone();
        let filter = filter.clone();
        set.spawn(async move {
            let client = ActiveCollabClient::new(inst.clone(), http.clone());
            let rows = match client.fetch_open_tasks().await {
                Ok(mut tasks) => {
                    tasks.retain(|task| filter.keeps(task));
                    Ok(fill_task_hours(&inst, &http, tasks, today, &filter.mapping).await)
                }
                Err(e) => Err(e),
            };
            (idx, rows)
        });
    }

    let mut per_index: Vec<anyhow::Result<Vec<MineTableRow>>> =
        (0..targets.len()).map(|_| Ok(Vec::new())).collect();

    while let Some(joined) = set.join_next().await {
        if let Ok((idx, result)) = joined {
            per_index[idx] = result;
        }
    }
    per_index
}

/// Maps the instance's open tasks to rows and fills their hours.
pub(super) async fn fill_task_hours(
    inst: &Instance,
    http: &Http,
    tasks: Vec<crate::models::MineTask>,
    today: chrono::NaiveDate,
    mapping: &BoardMapping,
) -> Vec<MineTableRow> {
    let hours = fetch_task_hours(inst, http, &tasks, today).await;
    tasks
        .into_iter()
        .zip(hours)
        .map(|(task, hours)| MineTableRow {
            logged_week: hours.logged_week,
            logged_total: hours.logged_total,
            ..mine_task_to_row(task, mapping)
        })
        .collect()
}

/// One instance's open tasks, as listed.
pub(crate) type InstanceTasks = (Instance, Vec<crate::models::MineTask>);

/// Lists every target instance's open tasks concurrently, in target order; an
/// instance whose list fails is left out.
pub(crate) async fn collect_mine_task_lists(
    targets: &[Instance],
    http: &Http,
) -> Vec<InstanceTasks> {
    let timer = std::time::Instant::now();
    let mut set: tokio::task::JoinSet<(usize, Option<InstanceTasks>)> = tokio::task::JoinSet::new();
    for (idx, inst) in targets.iter().enumerate() {
        let inst = inst.clone();
        let client = ActiveCollabClient::new(inst.clone(), http.clone());
        set.spawn(async move {
            let listed = client.fetch_open_tasks().await.ok().map(|t| (inst, t));
            (idx, listed)
        });
    }
    let mut per_index: Vec<Option<InstanceTasks>> = vec![None; targets.len()];
    while let Some(joined) = set.join_next().await {
        if let Ok((idx, listed)) = joined {
            per_index[idx] = listed;
        }
    }
    crate::timing::record("mine_list_load", timer.elapsed());
    per_index.into_iter().flatten().collect()
}

/// The rows of the listed tasks placed with `mapping`, without hours.
pub(crate) fn mine_rows_of(lists: &[InstanceTasks], mapping: &BoardMapping) -> Vec<MineTableRow> {
    lists
        .iter()
        .flat_map(|(_, tasks)| tasks.iter().cloned())
        .map(|task| mine_task_to_row(task, mapping))
        .collect()
}

/// The hours of every listed task, fetched concurrently per instance, in
/// list order.
pub(crate) async fn collect_mine_hours(lists: &[InstanceTasks], http: &Http) -> Vec<MineHours> {
    let timer = std::time::Instant::now();
    let today = chrono::Local::now().date_naive();
    let mut set: tokio::task::JoinSet<(usize, Vec<MineHours>)> = tokio::task::JoinSet::new();
    for (idx, (inst, tasks)) in lists.iter().enumerate() {
        let (inst, tasks, http) = (inst.clone(), tasks.clone(), http.clone());
        set.spawn(async move { (idx, fetch_task_hours(&inst, &http, &tasks, today).await) });
    }
    let mut per_index: Vec<Vec<MineHours>> = vec![Vec::new(); lists.len()];
    while let Some(joined) = set.join_next().await {
        if let Ok((idx, hours)) = joined {
            per_index[idx] = hours;
        }
    }
    crate::timing::record("mine_hours_load", timer.elapsed());
    per_index.into_iter().flatten().collect()
}

/// Each task's hours, in task order.
///
/// One records request covers the user's week for every task; each task's
/// total comes from its own payload, fetched concurrently. A failed request
/// leaves the affected hours `None` and never drops a task.
async fn fetch_task_hours(
    inst: &Instance,
    http: &Http,
    tasks: &[crate::models::MineTask],
    today: chrono::NaiveDate,
) -> Vec<MineHours> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let (week_start, week_end) = resolve_period(PeriodPreset::Week, today);
    let from = week_start.format("%Y-%m-%d").to_string();
    let to = week_end.format("%Y-%m-%d").to_string();

    let (week, totals) = tokio::join!(
        fetch_week_hours(inst, http, &from, &to),
        fetch_total_hours(inst, http, tasks)
    );

    tasks
        .iter()
        .zip(totals)
        .map(|(task, logged_total)| MineHours {
            instance: task.instance_name.clone(),
            task_id: task.id,
            logged_week: week
                .as_ref()
                .map(|per_task| per_task.get(&task.id).copied().unwrap_or(0.0)),
            logged_total,
        })
        .collect()
}

/// The user's hours per task id over `from..=to`, rounded to 2 decimals;
/// `None` when the user id is missing or the records request fails.
async fn fetch_week_hours(
    inst: &Instance,
    http: &Http,
    from: &str,
    to: &str,
) -> Option<std::collections::HashMap<i64, f64>> {
    let user_id = inst.user_id.filter(|id| *id != 0)?;
    let client = ActiveCollabClient::new(inst.clone(), http.clone());
    let Ok(TimeRecordsOutcome::Ok(report)) =
        client.fetch_time_records_by_date(user_id, from, to).await
    else {
        return None;
    };
    let mut per_task: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
    for record in report
        .time_records
        .iter()
        .filter(|r| r.parent_type == "Task")
    {
        *per_task.entry(record.parent_id).or_insert(0.0) += record.value;
    }
    for hours in per_task.values_mut() {
        *hours = (*hours * 100.0).round() / 100.0;
    }
    Some(per_task)
}

/// Each task's total logged hours (`tracked_time` of its payload), fetched
/// concurrently, in task order; `None` where the request or payload fails.
async fn fetch_total_hours(
    inst: &Instance,
    http: &Http,
    tasks: &[crate::models::MineTask],
) -> Vec<Option<f64>> {
    let mut set: tokio::task::JoinSet<(usize, Option<f64>)> = tokio::task::JoinSet::new();
    for (idx, task) in tasks.iter().enumerate() {
        let project_id = task.project_id;
        let task_id = task.id;
        let client = ActiveCollabClient::new(inst.clone(), http.clone());
        set.spawn(async move {
            let total = match client.fetch_task(project_id, task_id).await {
                Ok((200, Some(payload))) => payload.get("tracked_time").and_then(|v| v.as_f64()),
                _ => None,
            };
            (idx, total)
        });
    }
    let mut totals: Vec<Option<f64>> = vec![None; tasks.len()];
    while let Some(joined) = set.join_next().await {
        if let Ok((idx, total)) = joined {
            totals[idx] = total;
        }
    }
    totals
}

fn placement_of(task: &crate::models::MineTask, mapping: &BoardMapping) -> Placement {
    place(&task.labels, task.task_list.as_deref(), mapping)
}

fn mine_task_to_row(task: crate::models::MineTask, mapping: &BoardMapping) -> MineTableRow {
    let placement = placement_of(&task, mapping);
    let task_number = match task.task_number {
        Some(n) if n != 0 => n,
        _ => task.id,
    };
    MineTableRow {
        instance: task.instance_name,
        project_id: task.project_id,
        task_number,
        task_id: task.id,
        name: task.name,
        due_on: task.due_on,
        project_name: None,
        estimate: task.estimate,
        logged_week: None,
        logged_total: None,
        task_list_id: task.task_list_id,
        task_list: task.task_list,
        labels: task.labels,
        phase: placement.phase,
        flagged: placement.flagged,
    }
}

/// Auth-aware mine aggregation for the CLI path.
///
/// Loads every target's rows with their hours, propagating Unauthorized rather than swallowing it.
/// On 401 from any target instance, returns Err(Unauthorized). Other non-200 errors
/// from an instance are silently skipped (unchanged behaviour for transient failures).
async fn fetch_mine_rows_checked(
    targets: &[Instance],
    http: &Http,
    filter: &MineFilter,
) -> anyhow::Result<Vec<MineTableRow>> {
    fetch_mine_rows_checked_on(targets, http, chrono::Local::now().date_naive(), filter).await
}

pub(super) async fn fetch_mine_rows_checked_on(
    targets: &[Instance],
    http: &Http,
    today: chrono::NaiveDate,
    filter: &MineFilter,
) -> anyhow::Result<Vec<MineTableRow>> {
    let timer = std::time::Instant::now();

    let mut rows = Vec::new();
    for result in fan_out_instance_rows(targets, http, today, filter).await {
        match result {
            Ok(instance_rows) => rows.extend(instance_rows),
            Err(e) if e.is::<Unauthorized>() => return Err(e),
            Err(_) => {}
        }
    }

    crate::timing::record("mine_list_load", timer.elapsed());
    Ok(rows)
}

/// The outcome of `mine_core` for the caller to act on.
pub(crate) enum MineOutcome {
    /// The caller should launch the interactive mine TUI for these instances.
    /// No rows are pre-fetched; the TUI reads its own snapshot and revalidates.
    TuiLaunch { targets: Vec<Instance> },
    /// All work completed inside mine_core; caller returns this exit code.
    Done(i32),
}

/// Parity: Python cmd_mine (testable core).
///
/// Loads instances, applies optional instance filter, then:
///   - `json=true`: fetches rows, emits JSON, returns `Done(0)`.
///   - `is_tty=true, json=false`, no `filter`: returns `TuiLaunch` so the caller
///     opens the interactive mine TUI.  Rows are NOT pre-fetched here — the TUI
///     reads its own snapshot and revalidates via `Cmd::LoadMineTasks`.
///   - otherwise (a TTY with an active `filter`, which the TUI cannot apply):
///     fetches the kept rows and writes the plain-text table, returns `Done(0)`.
///
/// `filter` is applied before the per-task hour requests; `count` and
/// `buckets` of the JSON reflect only the kept tasks.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn mine_core(
    repo: &InstanceRepository<'_>,
    http: &Http,
    instance_filter: Option<&str>,
    filter: &MineFilter,
    json: bool,
    is_tty: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> MineOutcome {
    let targets = match load_targets(repo, instance_filter) {
        Ok(targets) => targets,
        Err(failure) => {
            writeln!(err, "{}", failure.message).ok();
            return MineOutcome::Done(failure.code);
        }
    };

    if is_tty && !json && !filter.is_active() {
        return MineOutcome::TuiLaunch { targets };
    }

    let rows = match fetch_mine_rows_checked(&targets, http, filter).await {
        Ok(r) => r,
        Err(e) if e.is::<Unauthorized>() => {
            writeln!(err, "{}", presenter::reauth_message()).ok();
            return MineOutcome::Done(1);
        }
        Err(e) => {
            writeln!(err, "Error fetching tasks: {e}").ok();
            return MineOutcome::Done(1);
        }
    };

    if json {
        let line = serde_json::to_string(&agent_json::mine_object(&rows))
            .unwrap_or_else(|_| String::from("{}"));
        writeln!(out, "{line}").ok();
        return MineOutcome::Done(0);
    }

    write_mine_table(&rows, filter.is_active(), out);
    MineOutcome::Done(0)
}

/// Writes the plain-text table, or the empty-result message; a filtered empty
/// result says the filter matched nothing rather than that no task exists.
fn write_mine_table(rows: &[MineTableRow], filtered: bool, out: &mut dyn Write) {
    if !rows.is_empty() {
        writeln!(out, "{}", render::render_mine_table(rows)).ok();
        return;
    }
    let message = if filtered {
        t("No open tasks assigned to you match the filter.")
    } else {
        t("No open tasks assigned to you.")
    };
    writeln!(out, "{message}").ok();
}
