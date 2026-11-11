//! MCP server over stdio. Two tools, `ac_read` and `ac_write`, select a
//! command core by `operation` and return the core's `--json` output, so the
//! agent contract stays the one in `agent_json`. While the server runs, stdout
//! belongs to the protocol: every core writes to in-memory buffers.

use crate::commands::{
    self, comment_core, current_core, current_git_branch, get_core, mine_core, task_set_core,
    time_log_core, DisplayFlags, InstanceContext, MineOutcome, OwnedReportPeriod, PeriodPreset,
    ReportFormat, ReportRequest,
};
use crate::store::cache::TaskCache;
use crate::store::instances::InstanceRepository;
use rmcp::handler::server::common::{schema_for_empty_input, schema_for_input};
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};
use rmcp::transport::stdio;
use rmcp::{tool, tool_router, ErrorData as McpError, ServiceExt};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::future::Future;
use std::sync::Arc;

/// Serve MCP on stdin/stdout until the client closes the connection.
pub(crate) async fn serve() -> i32 {
    let running = match AcServer.serve(stdio()).await {
        Ok(running) => running,
        Err(e) => {
            eprintln!("Error starting MCP server: {e}");
            return 1;
        }
    };
    match running.waiting().await {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Error serving MCP: {e}");
            1
        }
    }
}

#[derive(Clone, Copy)]
struct AcServer;

#[tool_router(server_handler)]
impl AcServer {
    #[tool(
        description = "Read ActiveCollab data as the `ac ... --json` commands print it. \
                       Select the command with `operation`.",
        input_schema = input_schema::<ReadOperation>(),
        annotations(read_only_hint = true)
    )]
    async fn ac_read(&self, arguments: JsonObject) -> Result<CallToolResult, McpError> {
        let operation: ReadOperation = parse_operation(arguments)?;
        run_isolated(move || run_read(operation)).await
    }

    #[tool(
        description = "Change ActiveCollab data (post a comment, log time, edit a task, export \
                       the hours report). Select the command with `operation`.",
        input_schema = input_schema::<WriteOperation>(),
        annotations(destructive_hint = true)
    )]
    async fn ac_write(&self, arguments: JsonObject) -> Result<CallToolResult, McpError> {
        let operation: WriteOperation = parse_operation(arguments)?;
        let period = operation.report_period()?;
        run_isolated(move || run_write(operation, period)).await
    }
}

/// The tool's input schema: the operation union with its root as an object.
fn input_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    schema_for_input::<T>().unwrap_or_else(|_| schema_for_empty_input())
}

/// Parse the tool arguments by hand: rmcp reports a failed `Parameters`
/// parse as a tool error, while an unknown operation or a missing field is a
/// malformed request, so it is answered as the `invalid_params` protocol error.
fn parse_operation<T: DeserializeOwned>(arguments: JsonObject) -> Result<T, McpError> {
    serde_json::from_value(Value::Object(arguments))
        .map_err(|e| McpError::invalid_params(format!("invalid arguments: {e}"), None))
}

/// `get`/`current` read a task, `mine` lists the open tasks assigned to you.
/// Every operation takes an optional `instance` name. `mine` without
/// `instance` lists every configured instance; `get` and `current` need
/// `instance` only when several instances are configured.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
enum ReadOperation {
    /// Fetch one task with its comments.
    Get {
        /// Task URL or PROJECT_ID/TASK_ID.
        #[serde(rename = "ref")]
        task_ref: String,
        #[serde(default)]
        no_comments: bool,
        /// Ignore the cache and re-fetch.
        #[serde(default)]
        refresh: bool,
        #[serde(default)]
        download_attachments: bool,
        /// Destination directory for `download_attachments`.
        attachments_dir: Option<String>,
        instance: Option<String>,
    },
    /// Fetch the task of a git branch named like feature/PROJECT_ID-TASK_ID.
    Current {
        /// Defaults to the git branch of the server's working directory.
        branch: Option<String>,
        instance: Option<String>,
    },
    /// List open tasks assigned to you, across instances unless one is named.
    Mine { instance: Option<String> },
}

/// `comment`, `log_time` and `set_task` act on one task; `time_report` writes a
/// file into the server's working directory and returns its path. Every
/// operation takes an optional `instance` name.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[schemars(extend("type" = "object"))]
enum WriteOperation {
    /// Post a comment as the logged-in user.
    Comment {
        /// Task URL or PROJECT_ID/TASK_ID.
        #[serde(rename = "ref")]
        task_ref: String,
        body: String,
        /// Post `body` as HTML unchanged instead of encoding plain text.
        #[serde(default)]
        html: bool,
        instance: Option<String>,
    },
    /// Log time against a task.
    LogTime {
        /// Task URL or PROJECT_ID/TASK_ID.
        #[serde(rename = "ref")]
        task_ref: String,
        /// Positive number of hours.
        hours: f64,
        /// YYYY-MM-DD, defaults to today.
        date: Option<String>,
        summary: Option<String>,
        /// Job type id or name, defaults to the instance default.
        job_type: Option<String>,
        instance: Option<String>,
    },
    /// Edit task fields; give at least one of status, assignee, estimate.
    SetTask {
        /// Task URL or PROJECT_ID/TASK_ID.
        #[serde(rename = "ref")]
        task_ref: String,
        /// open or completed.
        status: Option<String>,
        /// A numeric user id or a name.
        assignee: Option<String>,
        /// Hours, zero or positive.
        estimate: Option<f64>,
        instance: Option<String>,
    },
    /// Write your hours report to a file in the server's working directory.
    TimeReport {
        /// A preset period; give it or both `start` and `end`.
        period: Option<ReportPeriodArg>,
        /// DD-MM-YYYY.
        start: Option<String>,
        /// DD-MM-YYYY, inclusive.
        end: Option<String>,
        /// Defaults to xlsx.
        format: Option<ReportFormatArg>,
        instance: Option<String>,
    },
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ReportPeriodArg {
    Month,
    Fortnight,
    Week,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ReportFormatArg {
    Xlsx,
    Csv,
}

impl WriteOperation {
    /// The period of a `time_report` operation; `None` for every other
    /// operation. A missing or contradictory period is an `invalid_params`
    /// protocol error.
    fn report_period(&self) -> Result<Option<OwnedReportPeriod>, McpError> {
        let WriteOperation::TimeReport {
            period, start, end, ..
        } = self
        else {
            return Ok(None);
        };
        match (period, start, end) {
            (Some(preset), None, None) => Ok(Some(OwnedReportPeriod::Preset(match preset {
                ReportPeriodArg::Month => PeriodPreset::Month,
                ReportPeriodArg::Fortnight => PeriodPreset::Fortnight,
                ReportPeriodArg::Week => PeriodPreset::Week,
            }))),
            (None, Some(start), Some(end)) => Ok(Some(OwnedReportPeriod::Range {
                start: start.clone(),
                end: end.clone(),
            })),
            _ => Err(McpError::invalid_params(
                "time_report needs either `period` or both `start` and `end`",
                None,
            )),
        }
    }
}

/// What a command core produced: its exit code and its two writers.
#[derive(Default)]
struct CoreRun {
    code: i32,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl CoreRun {
    /// Exit 0 returns `out`; any other exit is a tool error carrying `err`,
    /// or `out` when the core reports a write failure there as JSON.
    fn into_result(self) -> CallToolResult {
        if self.code == 0 {
            return CallToolResult::success(vec![ContentBlock::text(lossy(self.out))]);
        }
        let message = if self.err.is_empty() {
            lossy(self.out)
        } else {
            lossy(self.err)
        };
        CallToolResult::error(vec![ContentBlock::text(message)])
    }

    fn failed(code: i32, message: &str) -> CoreRun {
        CoreRun {
            code,
            out: Vec::new(),
            err: format!("{message}\n").into_bytes(),
        }
    }
}

fn lossy(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Run a core on a blocking thread. The cores hold a borrowed SQLite
/// connection across awaits, which is not `Send`, so the future is driven to
/// completion by `block_on` on that thread instead of being spawned.
async fn run_isolated<F, Fut>(make: F) -> Result<CallToolResult, McpError>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = CoreRun>,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || handle.block_on(make()))
        .await
        .map(CoreRun::into_result)
        .map_err(|e| McpError::internal_error(format!("tool task failed: {e}"), None))
}

fn open_instance(instance: Option<&str>, run: &mut CoreRun) -> Option<InstanceContext> {
    match commands::setup_instance_client(instance, &mut run.err) {
        Ok(ctx) => Some(ctx),
        Err(code) => {
            run.code = code;
            None
        }
    }
}

fn json_flags() -> DisplayFlags {
    DisplayFlags {
        json: true,
        short: false,
        refresh: false,
        no_comments: false,
        download_attachments: false,
        attachments_dir: None,
    }
}

async fn run_read(operation: ReadOperation) -> CoreRun {
    let mut run = CoreRun::default();
    match operation {
        ReadOperation::Get {
            task_ref,
            no_comments,
            refresh,
            download_attachments,
            attachments_dir,
            instance,
        } => {
            let Some(ctx) = open_instance(instance.as_deref(), &mut run) else {
                return run;
            };
            let cache = TaskCache::new(ctx.store.conn());
            let flags = DisplayFlags {
                refresh,
                no_comments,
                download_attachments,
                attachments_dir,
                ..json_flags()
            };
            run.code = get_core(
                &task_ref,
                &ctx.instance,
                &cache,
                &ctx.client,
                &flags,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        ReadOperation::Current { branch, instance } => {
            let Some(ctx) = open_instance(instance.as_deref(), &mut run) else {
                return run;
            };
            let cache = TaskCache::new(ctx.store.conn());
            let branch = branch.or_else(current_git_branch);
            run.code = current_core(
                branch.as_deref(),
                &ctx.instance,
                &cache,
                &ctx.client,
                &json_flags(),
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        ReadOperation::Mine { instance } => {
            let (store, http) = match commands::open_store_and_http(&mut run.err) {
                Ok(pair) => pair,
                Err(code) => {
                    run.code = code;
                    return run;
                }
            };
            let repo = InstanceRepository::new(store.conn());
            match mine_core(
                &repo,
                &http,
                instance.as_deref(),
                true,
                false,
                &mut run.out,
                &mut run.err,
            )
            .await
            {
                MineOutcome::Done(code) => run.code = code,
                MineOutcome::TuiLaunch { .. } => {
                    return CoreRun::failed(1, "Error: mine tried to open the interactive view.")
                }
            }
        }
    }
    run
}

async fn run_write(operation: WriteOperation, period: Option<OwnedReportPeriod>) -> CoreRun {
    let mut run = CoreRun::default();
    match operation {
        WriteOperation::Comment {
            task_ref,
            body,
            html,
            instance,
        } => {
            let Some(ctx) = open_instance(instance.as_deref(), &mut run) else {
                return run;
            };
            run.code = comment_core(
                Some(&task_ref),
                None,
                &body,
                html,
                &ctx.instance,
                &ctx.client,
                true,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        WriteOperation::LogTime {
            task_ref,
            hours,
            date,
            summary,
            job_type,
            instance,
        } => {
            let Some(ctx) = open_instance(instance.as_deref(), &mut run) else {
                return run;
            };
            run.code = time_log_core(
                Some(&task_ref),
                None,
                hours,
                date.as_deref(),
                summary.as_deref(),
                job_type.as_deref(),
                &ctx.client,
                true,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        WriteOperation::SetTask {
            task_ref,
            status,
            assignee,
            estimate,
            instance,
        } => {
            let Some(ctx) = open_instance(instance.as_deref(), &mut run) else {
                return run;
            };
            run.code = task_set_core(
                Some(&task_ref),
                None,
                status.as_deref(),
                assignee.as_deref(),
                estimate,
                &ctx.client,
                true,
                &mut run.out,
                &mut run.err,
            )
            .await;
        }
        WriteOperation::TimeReport {
            format, instance, ..
        } => {
            let Some(period) = period else {
                return CoreRun::failed(2, "Error: time_report has no period.");
            };
            let Some(ctx) = open_instance(instance.as_deref(), &mut run) else {
                return run;
            };
            let out_dir = match std::env::current_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    return CoreRun::failed(1, &format!("Error reading current directory: {e}"))
                }
            };
            let request = ReportRequest {
                period: period.as_period(),
                format: match format.unwrap_or(ReportFormatArg::Xlsx) {
                    ReportFormatArg::Xlsx => ReportFormat::Xlsx,
                    ReportFormatArg::Csv => ReportFormat::Csv,
                },
                today: chrono::Local::now().date_naive(),
                user_id: ctx.instance.user_id,
                out_dir: &out_dir,
            };
            run.code =
                commands::time_report_core(&request, &ctx.client, &mut run.out, &mut run.err).await;
        }
    }
    run
}
