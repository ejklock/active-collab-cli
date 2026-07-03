use super::presenter;
use crate::agent_json;
use crate::client::{ActiveCollabClient, Unauthorized};
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

    let mut set: tokio::task::JoinSet<(usize, Vec<crate::models::MineTask>)> =
        tokio::task::JoinSet::new();

    for (idx, inst) in targets.iter().enumerate() {
        let client = ActiveCollabClient::new(inst.clone(), http.clone());
        set.spawn(async move {
            let tasks = client.fetch_open_tasks().await.unwrap_or_default();
            (idx, tasks)
        });
    }

    let mut per_index: Vec<Vec<crate::models::MineTask>> =
        (0..targets.len()).map(|_| Vec::new()).collect();

    while let Some(joined) = set.join_next().await {
        if let Ok((idx, tasks)) = joined {
            per_index[idx] = tasks;
        }
    }

    let rows = per_index
        .into_iter()
        .flat_map(|tasks| tasks.into_iter().map(mine_task_to_row))
        .collect();

    crate::timing::record("mine_list_load", t.elapsed());
    rows
}

fn mine_task_to_row(task: crate::models::MineTask) -> MineTableRow {
    let task_number = match task.task_number {
        Some(n) if n != 0 => n,
        _ => task.id,
    };
    MineTableRow {
        instance: task.instance_name,
        project_id: task.project_id.unwrap_or(0),
        task_number,
        task_id: task.id,
        name: task.name,
        due_on: task.due_on,
        project_name: None,
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
    let timer = std::time::Instant::now();

    let mut set: tokio::task::JoinSet<(usize, anyhow::Result<Vec<crate::models::MineTask>>)> =
        tokio::task::JoinSet::new();

    for (idx, inst) in targets.iter().enumerate() {
        let client = ActiveCollabClient::new(inst.clone(), http.clone());
        set.spawn(async move { (idx, client.fetch_open_tasks().await) });
    }

    let mut per_index: Vec<Vec<crate::models::MineTask>> =
        (0..targets.len()).map(|_| Vec::new()).collect();

    while let Some(joined) = set.join_next().await {
        if let Ok((idx, result)) = joined {
            match result {
                Ok(tasks) => per_index[idx] = tasks,
                Err(e) if e.is::<Unauthorized>() => return Err(e),
                Err(_) => {}
            }
        }
    }

    let rows = per_index
        .into_iter()
        .flat_map(|tasks| tasks.into_iter().map(mine_task_to_row))
        .collect();

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
