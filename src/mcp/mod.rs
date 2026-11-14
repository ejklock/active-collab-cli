//! MCP server over stdio. Two tools, `ac_read` and `ac_write`, select a
//! command core by `operation` and return the core's `--json` output, so the
//! agent contract stays the one in `agent_json`; a third, `ac_download_attachments`,
//! saves a task's attachments to disk. While the server runs, stdout
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

    #[tool(
        description = "Download the attachments and inline images of one task into a local \
                       directory and return the task as `ac get --json` prints it, with \
                       `downloaded_attachments` listing each file's path or error. Writes \
                       files on the server's machine; it changes nothing in ActiveCollab.",
        input_schema = input_schema::<DownloadArguments>(),
        annotations(read_only_hint = false, destructive_hint = false, idempotent_hint = true)
    )]
    async fn ac_download_attachments(
        &self,
        arguments: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let arguments: DownloadArguments = parse_operation(arguments)?;
        run_isolated(move || run_download(arguments)).await
    }
}

/// The tool's input schema. A tagged enum derives as a top-level `oneOf`,
/// which Claude's tool API rejects, so the variants are merged into one flat
/// object (see `flatten_operations`).
fn input_schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
    let mut schema = schema_for_input::<T>().unwrap_or_else(|_| schema_for_empty_input());
    flatten_operations(Arc::make_mut(&mut schema));
    schema
}

/// Rewrite a schema whose root is a `oneOf` of tagged variants into one object:
/// an `operation` string enum plus the union of every variant's properties,
/// each described with the operations that take it. A schema without a
/// `oneOf` is left as it is. Only the published schema changes; the enum's own
/// parse still rejects a field the chosen operation does not take.
fn flatten_operations(schema: &mut JsonObject) {
    let Some(Value::Array(variants)) = schema.remove("oneOf") else {
        return;
    };
    let mut operations: Vec<Value> = Vec::new();
    let mut properties = serde_json::Map::new();
    let mut takers: Vec<(String, Vec<String>)> = Vec::new();
    let mut summaries: Vec<String> = Vec::new();
    for variant in &variants {
        if let Some(name) = variant["properties"]["operation"]["const"].as_str() {
            operations.push(Value::String(name.to_owned()));
            summaries.push(match variant["description"].as_str() {
                Some(doc) => format!("{name}: {doc}"),
                None => name.to_owned(),
            });
            merge_variant(name, variant, &mut properties, &mut takers);
        }
    }
    for (field, labels) in takers {
        if let Some(Value::Object(field_schema)) = properties.get_mut(&field) {
            annotate_takers(field_schema, &labels);
        }
    }
    let mut flat = serde_json::Map::new();
    flat.insert(
        "operation".into(),
        serde_json::json!({
            "type": "string",
            "enum": operations,
            "description": format!("The command to run. {}", summaries.join(" "))
        }),
    );
    flat.extend(properties);
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert("properties".into(), Value::Object(flat));
    schema.insert("required".into(), serde_json::json!(["operation"]));
}

/// Add one variant's fields (all but the `operation` tag) to the merged
/// properties, and `takers` records which operations take it and whether they
/// require it. Operations sharing a field must declare the same schema for it,
/// since only one can be published.
fn merge_variant(
    name: &str,
    variant: &Value,
    properties: &mut serde_json::Map<String, Value>,
    takers: &mut Vec<(String, Vec<String>)>,
) {
    let required: Vec<&str> = variant["required"]
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let fields = variant["properties"].as_object().into_iter().flatten();
    for (field, field_schema) in fields.filter(|(field, _)| *field != "operation") {
        let published = properties
            .entry(field.clone())
            .or_insert_with(|| field_schema.clone());
        assert_eq!(
            published, field_schema,
            "operation {name} declares field {field} differently from an earlier operation"
        );
        let label = if required.contains(&field.as_str()) {
            format!("{name} (required)")
        } else {
            name.to_owned()
        };
        match takers.iter_mut().find(|(taken, _)| taken == field) {
            Some((_, labels)) => labels.push(label),
            None => takers.push((field.clone(), vec![label])),
        }
    }
}

fn annotate_takers(field_schema: &mut serde_json::Map<String, Value>, labels: &[String]) {
    let note = format!("Taken by: {}.", labels.join(", "));
    let text = match field_schema.get("description").and_then(Value::as_str) {
        Some(doc) => format!("{doc} {note}"),
        None => note,
    };
    field_schema.insert("description".into(), Value::String(text));
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
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
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

/// Download the attachments and inline images of one task.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DownloadArguments {
    /// Task URL or PROJECT_ID/TASK_ID.
    #[serde(rename = "ref")]
    task_ref: String,
    /// Destination directory; defaults to a per-task directory under the OS
    /// temp directory.
    attachments_dir: Option<String>,
    /// Needed only when several instances are configured.
    instance: Option<String>,
}

/// `comment`, `log_time` and `set_task` act on one task; `time_report` writes a
/// file into the server's working directory and returns its path. Every
/// operation takes an optional `instance` name.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
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
    /// Edit task fields; give at least one of status, assignee, estimate, list.
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
        /// A numeric task list id or a task list name of the task's project.
        list: Option<String>,
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

async fn run_get(task_ref: &str, instance: Option<&str>, flags: &DisplayFlags) -> CoreRun {
    let mut run = CoreRun::default();
    let Some(ctx) = open_instance(instance, &mut run) else {
        return run;
    };
    let cache = TaskCache::new(ctx.store.conn());
    run.code = get_core(
        task_ref,
        &ctx.instance,
        &cache,
        &ctx.client,
        flags,
        &mut run.out,
        &mut run.err,
    )
    .await;
    run
}

async fn run_download(arguments: DownloadArguments) -> CoreRun {
    let flags = DisplayFlags {
        download_attachments: true,
        attachments_dir: arguments.attachments_dir,
        ..json_flags()
    };
    run_get(&arguments.task_ref, arguments.instance.as_deref(), &flags).await
}

async fn run_read(operation: ReadOperation) -> CoreRun {
    let mut run = CoreRun::default();
    match operation {
        ReadOperation::Get {
            task_ref,
            no_comments,
            refresh,
            instance,
        } => {
            let flags = DisplayFlags {
                refresh,
                no_comments,
                ..json_flags()
            };
            return run_get(&task_ref, instance.as_deref(), &flags).await;
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
            list,
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
                list.as_deref(),
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

#[cfg(test)]
#[path = "../../tests/unit/mcp.rs"]
mod tests;
