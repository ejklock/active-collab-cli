//! Pure footer decision: hint selection, transient status, and layout plan.
//!
//! Mirrors the `detail_geometry` / `task_layout` split: this module owns the
//! whole "what does the footer say and how tall is it" question as pure
//! functions over primitives. `view.rs` stays draw-only and calls
//! `footer::plan` once, then renders the resulting `FooterPlan`.

use crate::i18n::t;
use crate::render::{display_width, wrap_text};
use crate::tui::model::{Compose, DownloadOutcome, DownloadState, Screen};

/// Reformat a BRT timestamp `YYYY-MM-DDTHH:MM:SS` into `DD/MM/YYYY HH:MM`.
/// Returns None when the input is too short or cannot be sliced at the expected offsets.
pub(crate) fn format_br_datetime(iso: &str) -> Option<String> {
    // Minimum: "YYYY-MM-DDTHH:MM" = 16 chars.
    if iso.len() < 16 {
        return None;
    }
    let year = iso.get(0..4)?;
    let month = iso.get(5..7)?;
    let day = iso.get(8..10)?;
    let hour = iso.get(11..13)?;
    let minute = iso.get(14..16)?;
    Some(format!("{}/{}/{} {}:{}", day, month, year, hour, minute))
}

/// Footer hint for the given screen.
///
/// When a compose modal is open the modal owns the compose hint.
/// When the confirm-delete modal is open the modal owns its hint; the footer
/// shows no confirm hint (one-home rule). Falls through to own-focused or browse.
pub(crate) fn hint_for_screen(screen: &Screen) -> String {
    match screen {
        Screen::Detail {
            focused_comment,
            comments,
            current_user_id,
            ..
        } => detail_hint(*focused_comment, comments, *current_user_id),
        _ => t("↑/↓ navigate  Enter select  r refresh  H report  Esc/b back  q quit"),
    }
}

/// Derive the instruction hint for the Detail screen. Every Detail overlay owns
/// its own in-modal hint, so the footer shows only the own-comment or browsing hint.
fn detail_hint(
    focused_comment: Option<usize>,
    comments: &[serde_json::Value],
    current_user_id: Option<i64>,
) -> String {
    if is_own_comment_focused(focused_comment, comments, current_user_id) {
        return t("↑/↓ · j/k move · Ctrl+click edit/delete · c new");
    }
    t("↑/↓ · j/k move · c comment · t log time · e estimate · s status · r refresh · D download · a assignee · l list · H report · Esc/b back · q quit")
}

pub(crate) fn is_own_comment_focused(
    focused_comment: Option<usize>,
    comments: &[serde_json::Value],
    current_user_id: Option<i64>,
) -> bool {
    let (Some(idx), Some(uid)) = (focused_comment, current_user_id) else {
        return false;
    };
    comments
        .get(idx)
        .and_then(|c| c.get("created_by_id"))
        .and_then(|v| v.as_i64())
        .map(|cid| cid == uid)
        .unwrap_or(false)
}

/// Derive the transient status string for the Detail footer status row.
///
/// Priority (highest first): auth_error > attachment download > copied_feedback.
/// When `compose` is `Some`, the modal overlay owns the compose hint/status.
/// The footer still shows auth_error or copied_feedback if either is set.
pub(crate) fn detail_status_line(
    compose: Option<&Compose>,
    copied_feedback: bool,
    auth_error: bool,
    download: Option<&DownloadState>,
) -> Option<String> {
    if auth_error {
        return Some(t(
            "Token invalid or revoked — run `ac setup add` to re-authenticate.",
        ));
    }
    if let Some(state) = download {
        return Some(download_status_text(state));
    }
    if compose.is_some() {
        return if copied_feedback {
            Some(t("Copiado ✓"))
        } else {
            None
        };
    }
    if copied_feedback {
        return Some(t("Copiado ✓"));
    }
    None
}

/// The footer status text of an attachment download.
fn download_status_text(state: &DownloadState) -> String {
    match state {
        DownloadState::Running => t("Downloading attachments…"),
        DownloadState::NoAttachments => t("No attachments to download"),
        DownloadState::Finished(DownloadOutcome::Failed(message)) => message.clone(),
        DownloadState::Finished(DownloadOutcome::Done { saved, total, dir }) => {
            t("Downloaded {saved} of {total} attachment(s) to {dir}")
                .replace("{saved}", &saved.to_string())
                .replace("{total}", &total.to_string())
                .replace("{dir}", &dir.display().to_string())
        }
    }
}

/// Number of wrapped lines a text occupies at the given display-column width.
/// Returns at least 1 for non-empty text; returns 1 for empty text.
pub(crate) fn wrapped_height(text: &str, width: usize) -> u16 {
    if text.is_empty() || width == 0 {
        return 1;
    }
    wrap_text(text, width).len().max(1) as u16
}

/// Pre-computed plan for how the footer should be rendered.
pub(crate) struct FooterPlan {
    pub(crate) height: u16,
    /// The full hint string (may be multi-line when stacked).
    pub(crate) hint: String,
    /// Right-side text (timestamp and/or copied indicator), if any.
    pub(crate) right_text: Option<String>,
    /// When true, hint and right cannot share a row; render right below hint.
    pub(crate) stacked: bool,
    pub(crate) right_is_copied: bool,
    /// Thin transient status row rendered below the hint region; collapses when None.
    pub(crate) status_line: Option<String>,
}

impl FooterPlan {
    fn compute(
        hint: &str,
        last_loaded: Option<&str>,
        copied_feedback: bool,
        status_line: Option<String>,
        width: usize,
    ) -> Self {
        let timestamp_text = last_loaded
            .and_then(format_br_datetime)
            .map(|formatted| format!("{} {}", t("Updated at"), formatted));

        let copied_indicator = if copied_feedback {
            Some(t("footer.copied_indicator"))
        } else {
            None
        };

        let right_segments: Vec<String> = [copied_indicator, timestamp_text]
            .into_iter()
            .flatten()
            .collect();

        let status_height: u16 = if status_line.is_some() { 1 } else { 0 };

        if right_segments.is_empty() {
            return Self {
                height: wrapped_height(hint, width) + status_height,
                hint: hint.to_string(),
                right_text: None,
                stacked: false,
                right_is_copied: false,
                status_line,
            };
        }

        let right_text = right_segments.join("  ");
        let hint_dw = display_width(hint);
        let right_dw = display_width(&right_text);

        if hint_dw + 1 + right_dw <= width {
            Self {
                height: 1 + status_height,
                hint: hint.to_string(),
                right_text: Some(right_text),
                stacked: false,
                right_is_copied: copied_feedback,
                status_line,
            }
        } else {
            let hint_height = wrapped_height(hint, width);
            let right_height = wrapped_height(&right_text, width);
            Self {
                height: hint_height + right_height + status_height,
                hint: hint.to_string(),
                right_text: Some(right_text),
                stacked: true,
                right_is_copied: copied_feedback,
                status_line,
            }
        }
    }
}

/// Compose the whole footer decision for the given screen + footer state + width.
///
/// Single deep entry: selects the hint, derives the transient status
/// line (the `Screen::Detail` destructuring lives here, not in `view()`), and
/// computes the layout plan.
pub(crate) fn plan(
    screen: &Screen,
    last_loaded: Option<&str>,
    copied_feedback: bool,
    width: usize,
) -> FooterPlan {
    let hint = hint_for_screen(screen);
    let status_line = if let Screen::Detail {
        overlay,
        auth_error,
        download,
        ..
    } = screen
    {
        detail_status_line(
            overlay.compose(),
            copied_feedback,
            *auth_error,
            download.as_ref(),
        )
    } else {
        None
    };
    FooterPlan::compute(&hint, last_loaded, copied_feedback, status_line, width)
}
