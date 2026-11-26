use super::{presenter, resolve};
use crate::agent_json;
use crate::client::{ActiveCollabClient, TaskWriteOutcome, UploadOutcome};
use crate::i18n::{fill, t};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// Where an upload reports each file as it starts and learns that its caller
/// gave up. The CLI uses `silent`; the MCP server hands in its progress
/// channel and the flag its task cancel sets.
pub(crate) struct UploadReporter {
    messages: Option<UnboundedSender<String>>,
    cancelled: Arc<AtomicBool>,
}

impl UploadReporter {
    pub(crate) fn silent() -> Self {
        Self {
            messages: None,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn new(messages: UnboundedSender<String>, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            messages: Some(messages),
            cancelled,
        }
    }

    fn report(&self, message: String) {
        if let Some(messages) = &self.messages {
            let _ = messages.send(message);
        }
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// The files of one write, uploaded and ready to attach.
pub(crate) struct Uploaded {
    pub(crate) codes: Vec<String>,
    pub(crate) names: Vec<String>,
}

/// Why an upload batch stopped. Nothing is attached after any of these.
#[derive(Debug)]
pub(crate) enum UploadFailure {
    /// A path is missing, unreadable or not a regular file; found before any upload.
    Unreadable {
        path: String,
        reason: String,
    },
    Unauthorized,
    Rejected {
        name: String,
        status: u16,
    },
    NoCode {
        name: String,
    },
    Transport {
        name: String,
        reason: String,
    },
    Cancelled,
}

impl UploadFailure {
    /// 2 for a caller mistake (a bad path), 1 for a runtime failure.
    pub(crate) fn exit_code(&self) -> i32 {
        match self {
            UploadFailure::Unreadable { .. } => 2,
            UploadFailure::Unauthorized
            | UploadFailure::Rejected { .. }
            | UploadFailure::NoCode { .. }
            | UploadFailure::Transport { .. }
            | UploadFailure::Cancelled => 1,
        }
    }

    pub(crate) fn message(&self) -> String {
        match self {
            UploadFailure::Unreadable { path, reason } => fill(
                &t("cannot attach {path}: {reason}"),
                &[("{path}", path), ("{reason}", reason)],
            ),
            UploadFailure::Unauthorized => presenter::reauth_message(),
            UploadFailure::Rejected { name, status } => fill(
                &t("upload of {name} failed with HTTP {status}; nothing was posted"),
                &[("{name}", name), ("{status}", &status.to_string())],
            ),
            UploadFailure::NoCode { name } => fill(
                &t("upload of {name} returned no file code; nothing was posted"),
                &[("{name}", name)],
            ),
            UploadFailure::Transport { name, reason } => fill(
                &t("upload of {name} failed: {reason}; nothing was posted"),
                &[("{name}", name), ("{reason}", reason)],
            ),
            UploadFailure::Cancelled => t("upload cancelled; nothing was posted"),
        }
    }
}

/// Check every path up front, so a typo in the last one fails before the
/// first byte leaves the machine.
fn check_readable(paths: &[PathBuf]) -> Result<Vec<String>, UploadFailure> {
    paths
        .iter()
        .map(|path| {
            let unreadable = |reason: String| UploadFailure::Unreadable {
                path: path.display().to_string(),
                reason,
            };
            let metadata = std::fs::metadata(path).map_err(|e| unreadable(e.to_string()))?;
            if !metadata.is_file() {
                return Err(unreadable(t("not a regular file")));
            }
            std::fs::File::open(path).map_err(|e| unreadable(e.to_string()))?;
            Ok(display_name(path))
        })
        .collect()
}

/// Expand a leading `~` or `~/` to `home`; any other path, or no known
/// home, stays as typed. A shell does this for the CLI; an MCP agent or the
/// TUI path field passes the path unexpanded.
pub(crate) fn expand_home(path: &Path, home: Option<&Path>) -> PathBuf {
    match (path.strip_prefix("~"), home) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Upload `paths` one by one, all or nothing: the first failure stops the
/// batch. A cancel is honored before each file and once more after the
/// last, so a cancelled batch is never attached. No paths upload nothing.
/// A leading `~/` resolves against the user's home directory.
pub(crate) async fn upload_files(
    client: &ActiveCollabClient,
    paths: &[PathBuf],
    reporter: &UploadReporter,
) -> Result<Uploaded, UploadFailure> {
    upload_files_from_home(client, paths, reporter, dirs::home_dir().as_deref()).await
}

async fn upload_files_from_home(
    client: &ActiveCollabClient,
    paths: &[PathBuf],
    reporter: &UploadReporter,
    home: Option<&Path>,
) -> Result<Uploaded, UploadFailure> {
    let paths: Vec<PathBuf> = paths.iter().map(|path| expand_home(path, home)).collect();
    let names = check_readable(&paths)?;
    let total = paths.len();
    let mut codes = Vec::with_capacity(total);
    for (index, (path, name)) in paths.iter().zip(&names).enumerate() {
        if reporter.is_cancelled() {
            return Err(UploadFailure::Cancelled);
        }
        reporter.report(format!("uploading {}/{total} {name}", index + 1));
        let outcome = client
            .upload_file(path)
            .await
            .map_err(|e| UploadFailure::Transport {
                name: name.clone(),
                reason: e.to_string(),
            })?;
        match outcome {
            UploadOutcome::Uploaded(code) => codes.push(code),
            UploadOutcome::NoCode => return Err(UploadFailure::NoCode { name: name.clone() }),
            UploadOutcome::Unauthorized => return Err(UploadFailure::Unauthorized),
            UploadOutcome::Failed(status) => {
                return Err(UploadFailure::Rejected {
                    name: name.clone(),
                    status,
                })
            }
        }
    }
    if reporter.is_cancelled() {
        return Err(UploadFailure::Cancelled);
    }
    Ok(Uploaded { codes, names })
}

/// Attach local files directly to a task. Exit code: 0 success, 2 usage
/// error, 1 runtime failure.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn task_attach_core(
    task_ref: Option<&str>,
    branch: Option<&str>,
    attachments: &[PathBuf],
    reporter: &UploadReporter,
    client: &ActiveCollabClient,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if attachments.is_empty() {
        presenter::write_comment_failure(&t("no file to attach"), json, out, err);
        return 2;
    }
    let (project_id, task_id) = match resolve::resolve_task_ref_for_comment(task_ref, branch, err) {
        Ok(ids) => ids,
        Err(code) => return code,
    };
    let uploaded = match upload_files(client, attachments, reporter).await {
        Ok(uploaded) => uploaded,
        Err(failure) => {
            presenter::write_comment_failure(&failure.message(), json, out, err);
            return failure.exit_code();
        }
    };
    let reason = match client
        .attach_to_task(project_id, task_id, &uploaded.codes)
        .await
    {
        Ok(TaskWriteOutcome::Ok(_)) => {
            write_task_attach_success(task_id, project_id, &uploaded.names, json, out);
            return 0;
        }
        Ok(TaskWriteOutcome::Unauthorized) => presenter::reauth_message(),
        Ok(TaskWriteOutcome::Failed(status)) => fill(
            &t("HTTP {status} attaching files to task {project_id}/{task_id}"),
            &[
                ("{status}", &status.to_string()),
                ("{project_id}", &project_id.to_string()),
                ("{task_id}", &task_id.to_string()),
            ],
        ),
        Err(e) => e.to_string(),
    };
    presenter::write_comment_failure(&reason, json, out, err);
    1
}

fn write_task_attach_success(
    task_id: i64,
    project_id: i64,
    names: &[String],
    json: bool,
    out: &mut dyn Write,
) {
    if json {
        writeln!(
            out,
            "{}",
            agent_json::task_attach_result(task_id, project_id, names)
        )
        .ok();
        return;
    }
    writeln!(
        out,
        "{}",
        fill(
            &t("Attached {count} file(s) to task {project_id}/{task_id}: {names}"),
            &[
                ("{count}", &names.len().to_string()),
                ("{project_id}", &project_id.to_string()),
                ("{task_id}", &task_id.to_string()),
                ("{names}", &names.join(", ")),
            ],
        )
    )
    .ok();
}

#[cfg(test)]
#[path = "../../tests/unit/attach.rs"]
mod tests;
