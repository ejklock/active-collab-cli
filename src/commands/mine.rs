use super::hours_report::{resolve_period, PeriodPreset};
use super::presenter;
use crate::agent_json;
use crate::client::{ActiveCollabClient, TimeRecordsOutcome, Unauthorized};
use crate::http::Http;
use crate::i18n::t;
use crate::render::{self, MineTableRow};
use crate::store::instances::{Instance, InstanceRepository};
use std::io::Write;

/// Parity: Python _render_mine_table (aggregation loop).
///
/// For each target instance, builds a client and fetches open tasks. Maps each
/// MineTask to a MineTableRow using task_number when present (else id as fallback),
/// mirroring Python `t.task_number or t.id`.
///
/// Instance order is preserved deterministically via an indexed JoinSet.
pub(crate) async fn collect_mine_rows(targets: &[Instance], http: &Http) -> Vec<MineTableRow> {
    let t = std::time::Instant::now();
    let today = chrono::Local::now().date_naive();
    let rows = fan_out_instance_rows(targets, http, today)
        .await
        .into_iter()
        .flat_map(Result::unwrap_or_default)
        .collect();
    crate::timing::record("mine_list_load", t.elapsed());
    rows
}

/// Loads every target instance's rows concurrently, one result per target in
/// target order; an instance whose task did not finish yields no rows.
async fn fan_out_instance_rows(
    targets: &[Instance],
    http: &Http,
    today: chrono::NaiveDate,
) -> Vec<anyhow::Result<Vec<MineTableRow>>> {
    let mut set: tokio::task::JoinSet<(usize, anyhow::Result<Vec<MineTableRow>>)> =
        tokio::task::JoinSet::new();

    for (idx, inst) in targets.iter().enumerate() {
        let inst = inst.clone();
        let http = http.clone();
        set.spawn(async move {
            let client = ActiveCollabClient::new(inst.clone(), http.clone());
            let rows = match client.fetch_open_tasks().await {
                Ok(tasks) => Ok(fill_task_hours(&inst, &http, tasks, today).await),
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
///
/// One records request covers the user's week for every task; each task's
/// total comes from its own payload, fetched concurrently. A failed request
/// leaves the affected hours `None` and never drops a task.
pub(super) async fn fill_task_hours(
    inst: &Instance,
    http: &Http,
    tasks: Vec<crate::models::MineTask>,
    today: chrono::NaiveDate,
) -> Vec<MineTableRow> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let (week_start, week_end) = resolve_period(PeriodPreset::Week, today);
    let from = week_start.format("%Y-%m-%d").to_string();
    let to = week_end.format("%Y-%m-%d").to_string();

    let (week, totals) = tokio::join!(
        fetch_week_hours(inst, http, &from, &to),
        fetch_total_hours(inst, http, &tasks)
    );

    tasks
        .into_iter()
        .zip(totals)
        .map(|(task, logged_total)| {
            let logged_week = week
                .as_ref()
                .map(|per_task| per_task.get(&task.id).copied().unwrap_or(0.0));
            MineTableRow {
                logged_week,
                logged_total,
                ..mine_task_to_row(task)
            }
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

fn mine_task_to_row(task: crate::models::MineTask) -> MineTableRow {
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
    }
}

/// Auth-aware mine aggregation for the CLI path.
///
/// Mirrors collect_mine_rows but propagates Unauthorized rather than swallowing it.
/// On 401 from any target instance, returns Err(Unauthorized). Other non-200 errors
/// from an instance are silently skipped (unchanged behaviour for transient failures).
async fn fetch_mine_rows_checked(
    targets: &[Instance],
    http: &Http,
) -> anyhow::Result<Vec<MineTableRow>> {
    fetch_mine_rows_checked_on(targets, http, chrono::Local::now().date_naive()).await
}

pub(super) async fn fetch_mine_rows_checked_on(
    targets: &[Instance],
    http: &Http,
    today: chrono::NaiveDate,
) -> anyhow::Result<Vec<MineTableRow>> {
    let timer = std::time::Instant::now();

    let mut rows = Vec::new();
    for result in fan_out_instance_rows(targets, http, today).await {
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
///   - `is_tty=true, json=false`: returns `TuiLaunch` so the caller opens the
///     interactive mine TUI.  Rows are NOT pre-fetched here — the TUI reads its
///     own snapshot and revalidates via `Cmd::LoadMineTasks`.
///   - `is_tty=false, json=false`: fetches rows, writes the plain-text table,
///     returns `Done(0)`.
pub(crate) async fn mine_core(
    repo: &InstanceRepository<'_>,
    http: &Http,
    instance_filter: Option<&str>,
    json: bool,
    is_tty: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> MineOutcome {
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            writeln!(err, "Error loading instances: {e}").ok();
            return MineOutcome::Done(1);
        }
    };

    if instances.is_empty() {
        writeln!(
            err,
            "{}",
            t("Error: no instances configured. Run: active_collab.py setup add")
        )
        .ok();
        return MineOutcome::Done(2);
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
            return MineOutcome::Done(2);
        }
        matches
    } else {
        instances
    };

    if is_tty && !json {
        return MineOutcome::TuiLaunch { targets };
    }

    let rows = match fetch_mine_rows_checked(&targets, http).await {
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

    if rows.is_empty() {
        writeln!(out, "{}", t("No open tasks assigned to you.")).ok();
        return MineOutcome::Done(0);
    }

    writeln!(out, "{}", render::render_mine_table(&rows)).ok();
    MineOutcome::Done(0)
}
