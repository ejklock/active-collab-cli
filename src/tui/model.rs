use crate::board::Phase;
use crate::commands::{validate_range, MineHours, OwnedReportPeriod, PeriodPreset, ReportFormat};
use crate::i18n::t;
use crate::render::{Asset, MineTableRow, StyleRun};
use crate::store::instances::Instance;
use crate::tui::detail_geometry::Selection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tui_textarea::TextArea;

/// Identity bar shown at the top of every screen.
///
/// Built once at startup from the active instance list and the cached user
/// directory; name may be None when user_id is absent or not yet in the cache.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub name: Option<String>,
    pub email: String,
    pub instance: String,
    pub extra: usize,
}

impl Header {
    /// Build a Header from the active instance list and an optional display name.
    ///
    /// The first element is treated as the primary (active) instance.
    /// `extra` is the number of additional instances beyond the first.
    /// An empty slice produces empty strings with `extra = 0`.
    pub fn from_instances(instances: &[Instance], name: Option<String>) -> Header {
        match instances.first() {
            Some(inst) => Header {
                name,
                email: inst.email.clone(),
                instance: inst.name.clone(),
                extra: instances.len().saturating_sub(1),
            },
            None => Header {
                name,
                email: String::new(),
                instance: String::new(),
                extra: 0,
            },
        }
    }

    /// Format the identity line for rendering in the header bar.
    ///
    /// Produces `"NAME <email> · instance"` when name is Some, or
    /// `"<email> · instance"` when name is None.
    /// Appends `" (+N more)"` when extra > 0.
    pub fn header_line(&self) -> String {
        let base = match &self.name {
            Some(n) => format!("{} <{}> · {}", n, self.email, self.instance),
            None => format!("<{}> · {}", self.email, self.instance),
        };
        if self.extra > 0 {
            format!("{} (+{} more)", base, self.extra)
        } else {
            base
        }
    }
}

/// A row in the task list (shared by Projects and Tasks screens).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRow {
    pub task_id: i64,
    pub task_number: i64,
    pub name: String,
    pub instance: String,
    pub project_id: i64,
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub estimate: Option<f64>,
    #[serde(default)]
    pub logged_week: Option<f64>,
    #[serde(default)]
    pub logged_total: Option<f64>,
    /// The board column the task sits in.
    #[serde(default)]
    pub phase: crate::board::Phase,
    /// A status label says the task is waiting or blocked.
    #[serde(default)]
    pub flagged: bool,
    /// Name of the task list the task sits in; `None` when unknown.
    #[serde(default)]
    pub task_list: Option<String>,
}

/// How the mine Tasks screen lays its tasks out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MineView {
    /// Grouped under due-date bucket headers.
    #[default]
    List,
    /// One column per board phase, side by side, cards stacked.
    KanbanHorizontal,
    /// One list grouped by board phase: a bordered box per phase with its cards
    /// stacked inside.
    KanbanVertical,
}

impl MineView {
    /// Every view, in the order the view modal lists them.
    pub const ALL: [MineView; 3] = [
        MineView::List,
        MineView::KanbanHorizontal,
        MineView::KanbanVertical,
    ];

    /// The value stored in the settings.
    pub fn setting(self) -> &'static str {
        match self {
            MineView::List => "list",
            MineView::KanbanHorizontal => "kanban_horizontal",
            MineView::KanbanVertical => "kanban_vertical",
        }
    }

    /// The view a stored setting names; an absent or unknown value is the list,
    /// and the plain `kanban` an earlier version stored is the horizontal one.
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("kanban" | "kanban_horizontal") => MineView::KanbanHorizontal,
            Some("kanban_vertical") => MineView::KanbanVertical,
            _ => MineView::List,
        }
    }

    /// The kanban layout this view draws; `None` for the list.
    fn layout(self) -> Option<KanbanLayout> {
        match self {
            MineView::List => None,
            MineView::KanbanHorizontal => Some(KanbanLayout::Horizontal),
            MineView::KanbanVertical => Some(KanbanLayout::Vertical),
        }
    }
}

/// How the kanban arranges the phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KanbanLayout {
    /// Phases are columns side by side.
    #[default]
    Horizontal,
    /// Phases are sections of one list, top to bottom.
    Vertical,
}

/// Where the kanban's focus sits: the focused phase and, per phase, the
/// selected card (an index into that phase's cards). `column` is the phase
/// index whichever way the phases are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KanbanFocus {
    pub column: usize,
    pub cards: [usize; 4],
    pub layout: KanbanLayout,
}

/// The indices into `tasks` of each column's cards, in `Phase::ALL` order and
/// keeping the order of `tasks`.
pub fn kanban_columns(tasks: &[TaskRow]) -> [Vec<usize>; 4] {
    let mut columns: [Vec<usize>; 4] = Default::default();
    for (index, task) in tasks.iter().enumerate() {
        if let Some(column) = Phase::ALL.iter().position(|p| *p == task.phase) {
            columns[column].push(index);
        }
    }
    columns
}

/// The nearest phase after (`forward`) or before `from` that has cards.
fn populated_phase(columns: &[Vec<usize>; 4], from: usize, forward: bool) -> Option<usize> {
    let populated = |phase: &usize| !columns[*phase].is_empty();
    if forward {
        (from + 1..columns.len()).find(populated)
    } else {
        (0..from).rev().find(populated)
    }
}

/// The card a move into a phase of `len` cards lands on: its first card going
/// forward, its last going back.
fn entering_card(len: usize, forward: bool) -> usize {
    if forward {
        0
    } else {
        len.saturating_sub(1)
    }
}

impl KanbanFocus {
    /// Focus on the first column that holds a card, else the first column.
    fn first_populated(tasks: &[TaskRow]) -> Self {
        let column = kanban_columns(tasks)
            .iter()
            .position(|cards| !cards.is_empty())
            .unwrap_or(0);
        Self {
            column,
            cards: [0; 4],
            layout: KanbanLayout::default(),
        }
    }

    /// The view this focus is drawn in.
    fn view(&self) -> MineView {
        match self.layout {
            KanbanLayout::Horizontal => MineView::KanbanHorizontal,
            KanbanLayout::Vertical => MineView::KanbanVertical,
        }
    }

    /// The index into `tasks` of the focused column's selected card; `None`
    /// when the column is empty.
    pub fn selected_task(&self, tasks: &[TaskRow]) -> Option<usize> {
        kanban_columns(tasks)[self.column]
            .get(self.cards[self.column])
            .copied()
    }

    /// After a reload keep, in every column, the same task (by instance and id)
    /// wherever it moved; the focus follows the selected task when it moved to
    /// another column, else it stays on its column. A task that is gone leaves
    /// the old position clamped into its column.
    fn reselected(mut self, earlier: &[TaskRow], fresh: &[TaskRow]) -> Self {
        let before = kanban_columns(earlier);
        let after = kanban_columns(fresh);
        let followed = before[self.column]
            .get(self.cards[self.column])
            .and_then(|&i| {
                let prior = &earlier[i];
                fresh
                    .iter()
                    .position(|r| r.instance == prior.instance && r.task_id == prior.task_id)
            })
            .and_then(|at| {
                after.iter().enumerate().find_map(|(column, rows)| {
                    rows.iter()
                        .position(|&j| j == at)
                        .map(|card| (column, card))
                })
            });
        for ((card, old), new) in self.cards.iter_mut().zip(&before).zip(&after) {
            let moved = old.get(*card).and_then(|&i| {
                let prior = &earlier[i];
                new.iter().position(|&j| {
                    fresh[j].instance == prior.instance && fresh[j].task_id == prior.task_id
                })
            });
            *card = moved.unwrap_or_else(|| (*card).min(new.len().saturating_sub(1)));
        }
        if let Some((column, card)) = followed {
            self.column = column;
            self.cards[column] = card;
        }
        self
    }

    fn step_column(&mut self, forward: bool) {
        self.column = if forward {
            (self.column + 1).min(Phase::ALL.len() - 1)
        } else {
            self.column.saturating_sub(1)
        };
    }

    /// Move the selection to the next or previous card of the whole list,
    /// crossing from one phase into the next phase that has cards; at the very
    /// first or last card it stays.
    fn step_through(&mut self, tasks: &[TaskRow], forward: bool) {
        let before = self.cards[self.column];
        self.step_card(tasks, forward);
        if self.cards[self.column] != before {
            return;
        }
        let columns = kanban_columns(tasks);
        if let Some(phase) = populated_phase(&columns, self.column, forward) {
            self.column = phase;
            self.cards[phase] = entering_card(columns[phase].len(), forward);
        }
    }

    /// Jump to the first card of the next or previous phase that has cards.
    fn jump_phase(&mut self, tasks: &[TaskRow], forward: bool) {
        if let Some(phase) = populated_phase(&kanban_columns(tasks), self.column, forward) {
            self.column = phase;
            self.cards[phase] = 0;
        }
    }

    fn step_card(&mut self, tasks: &[TaskRow], forward: bool) {
        let last = kanban_columns(tasks)[self.column].len().saturating_sub(1);
        let card = &mut self.cards[self.column];
        *card = if forward {
            (*card + 1).min(last)
        } else {
            card.saturating_sub(1)
        };
    }
}

/// Style classification produced by `relative_due`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DueStyle {
    Overdue,
    Near,
    Normal,
    None,
}

/// A project group returned by the controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroup {
    pub project_id: i64,
    pub project_name: String,
    pub instance: String,
    pub tasks: Vec<TaskRow>,
}

/// A y-range record mapping a rendered terminal row span to a list index.
///
/// Populated by `render_table` each frame and stored in the model so that
/// `handle_click_list` can resolve a click terminal-row → list index without
/// re-running geometry logic outside the render pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ClickTarget {
    /// First terminal row of this list item (inclusive).
    pub y_start: u16,
    /// One past the last terminal row of this list item (exclusive).
    pub y_end: u16,
    /// Zero-based index into the data list.
    pub index: usize,
}

/// A click-target for one of the two confirm-modal buttons.
///
/// Geometry is derived from the modal Rect that `render_modal` returns —
/// single-sourced so the hit-test matches what was drawn. Written by the
/// shell after each draw (via `set_modal_button_targets`) and consumed by
/// `handle_click_detail` on a plain left-click.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalButtonTarget {
    /// Left terminal column (inclusive).
    pub x_start: u16,
    /// One past the last terminal column (exclusive).
    pub x_end: u16,
    /// Terminal row the button occupies.
    pub row: u16,
    /// `true` → confirms the delete; `false` → cancels it.
    pub is_confirm: bool,
}

/// Elm-style effect: what the shell should do after a pure update.
#[derive(Debug, Clone, PartialEq)]
pub enum Cmd {
    LoadTasksByProject,
    /// Fetch the mine task list in the background (SWR revalidation for the mine TUI).
    LoadMineTasks,
    /// Store the chosen mine view so the next launch opens in it.
    SaveMineView(MineView),
    LoadDetail {
        instance: String,
        project_id: i64,
        task_id: i64,
        refresh: bool,
    },
    OpenAsset {
        instance: String,
        url: String,
    },
    /// Copy the given text to the system clipboard (interpreted by the shell via arboard).
    CopyToClipboard(String),
    SubmitComment {
        instance: String,
        project_id: i64,
        task_id: i64,
        body: String,
    },
    /// PUT an existing comment body (edit path).
    UpdateComment {
        instance: String,
        comment_id: i64,
        body: String,
    },
    /// DELETE an existing comment after the user confirms.
    DeleteComment {
        instance: String,
        comment_id: i64,
    },
    /// Export the hours report of the logged-in user of `instance`; the shell
    /// resolves today's date and the output directory.
    ExportHoursReport {
        instance: String,
        period: OwnedReportPeriod,
        format: ReportFormat,
    },
    /// POST a new time record for the task. `summary` carries the optional note; the
    /// job type and record date are resolved by the shell, never by the pure core.
    SubmitTimeLog {
        instance: String,
        project_id: i64,
        task_id: i64,
        hours: f64,
        summary: Option<String>,
    },
    /// PUT one or more task-field writes (completion and/or assignee/estimate) for
    /// the current Detail task. Each field is optional so this single shared effect
    /// covers every task-edit affordance (estimate now; status and assignee once
    /// their own overlays land) without a per-field `Cmd` variant.
    SubmitTaskEdit {
        instance: String,
        project_id: i64,
        task_id: i64,
        completion: Option<bool>,
        assignee_id: Option<i64>,
        estimate: Option<f64>,
        task_list_id: Option<i64>,
    },
    /// Read the task lists of the open Detail task's project for the task-list picker.
    LoadTaskLists {
        instance: String,
        project_id: i64,
        task_id: i64,
    },
    /// Fetch and decode the image bytes for an opened image-viewer asset.
    /// Handled entirely by the shell; the pure Model never sees bytes.
    LoadImage {
        asset: ImageAssetRef,
    },
    /// Download the open task's downloadable assets into the user's downloads
    /// folder; the shell resolves the home directory and runs the download.
    DownloadAttachments {
        instance: String,
        project_id: i64,
        task_id: i64,
        assets: Vec<Asset>,
    },
}

/// How a finished attachment download ended.
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadOutcome {
    /// `saved` of `total` assets were written under `dir`; the per-asset
    /// failures are not an error of the download as a whole.
    Done {
        saved: usize,
        total: usize,
        dir: std::path::PathBuf,
    },
    /// The download could not start or run; carries the translated message.
    Failed(String),
}

/// The attachment download as the open Detail screen shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadState {
    Running,
    /// The task has no downloadable asset, so no download was started.
    NoAttachments,
    Finished(DownloadOutcome),
}

/// What kind of comment compose operation is in progress.
#[derive(Debug, Clone, PartialEq)]
pub enum ComposeKind {
    New,
    /// Editing an existing comment (PUT path). Carries the comment id being edited.
    Edit {
        comment_id: i64,
    },
}

/// Current lifecycle phase of the compose area.
#[derive(Debug, Clone, PartialEq)]
pub enum ComposeStatus {
    Editing,
    Submitting,
    Error(String),
}

/// The image asset the viewer overlay is displaying.
///
/// `label` is the display filename (used for the modal title and the
/// loading/error placeholder text); `url` is the source location the shell
/// fetches bytes from. Carries no protocol/bytes state.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageAssetRef {
    pub url: String,
    pub label: String,
}

/// Lifecycle phase of the image-viewer overlay.
///
/// The pure Model tracks only this lifecycle; the decoded bytes and the
/// `ratatui-image` `StatefulProtocol` live in a shell-owned side table,
/// never here.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageStatus {
    Loading,
    Ready,
    Error(String),
}

/// Transient state for the in-progress comment compose area.
///
/// `editor` carries caret position, selection, and undo/redo history alongside the
/// text — `tui_textarea::TextArea` does not implement `PartialEq`, so `Compose` (and
/// its ancestors `DetailOverlay`/`Screen`) drop that derive.
#[derive(Debug, Clone)]
pub struct Compose {
    pub kind: ComposeKind,
    pub editor: TextArea<'static>,
    pub status: ComposeStatus,
}

/// Which field of the log-time modal currently has focus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogTimeField {
    Hours,
    Summary,
}

/// Current lifecycle phase of the log-time modal.
#[derive(Debug, Clone, PartialEq)]
pub enum LogTimeStatus {
    Editing,
    Submitting,
    Error(String),
}

/// Transient state for the in-progress log-time modal.
///
/// Plain `String` buffers (not a `TextArea`) since each field is a single line with
/// no caret/undo needs, so the whole form stays `PartialEq` unlike `Compose`.
#[derive(Debug, Clone, PartialEq)]
pub struct LogTimeForm {
    pub hours: String,
    pub summary: String,
    pub field: LogTimeField,
    pub status: LogTimeStatus,
}

impl LogTimeForm {
    fn focused_buffer_mut(&mut self) -> &mut String {
        match self.field {
            LogTimeField::Hours => &mut self.hours,
            LogTimeField::Summary => &mut self.summary,
        }
    }
}

/// The period choices of the hours-report modal, in list order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportPeriodChoice {
    Monthly,
    Biweekly,
    Weekly,
    Custom,
}

impl ReportPeriodChoice {
    fn next(self) -> Self {
        match self {
            ReportPeriodChoice::Monthly => ReportPeriodChoice::Biweekly,
            ReportPeriodChoice::Biweekly => ReportPeriodChoice::Weekly,
            ReportPeriodChoice::Weekly | ReportPeriodChoice::Custom => ReportPeriodChoice::Custom,
        }
    }

    fn prev(self) -> Self {
        match self {
            ReportPeriodChoice::Monthly | ReportPeriodChoice::Biweekly => {
                ReportPeriodChoice::Monthly
            }
            ReportPeriodChoice::Weekly => ReportPeriodChoice::Biweekly,
            ReportPeriodChoice::Custom => ReportPeriodChoice::Weekly,
        }
    }

    /// The named period behind a preset choice; `None` for the typed range.
    pub(crate) fn preset(self) -> Option<PeriodPreset> {
        match self {
            ReportPeriodChoice::Monthly => Some(PeriodPreset::Month),
            ReportPeriodChoice::Biweekly => Some(PeriodPreset::Fortnight),
            ReportPeriodChoice::Weekly => Some(PeriodPreset::Week),
            ReportPeriodChoice::Custom => None,
        }
    }
}

/// Which part of the hours-report modal has focus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportField {
    Period,
    Start,
    End,
}

/// Lifecycle phase of the hours-report modal.
#[derive(Debug, Clone, PartialEq)]
pub enum ReportStatus {
    Editing,
    Exporting,
    Done { path: String, total: String },
    Error(String),
}

/// Maximum length of a typed `DD-MM-YYYY` date.
const REPORT_DATE_MAX_CHARS: usize = 10;

/// Transient state of the hours-report modal.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportForm {
    pub instance: String,
    pub period: ReportPeriodChoice,
    pub format: ReportFormat,
    pub start: String,
    pub end: String,
    pub focus: ReportField,
    pub status: ReportStatus,
}

impl ReportForm {
    fn new(instance: String) -> Self {
        ReportForm {
            instance,
            period: ReportPeriodChoice::Monthly,
            format: ReportFormat::Xlsx,
            start: String::new(),
            end: String::new(),
            focus: ReportField::Period,
            status: ReportStatus::Editing,
        }
    }

    fn date_buffer_mut(&mut self) -> Option<&mut String> {
        match self.focus {
            ReportField::Period => None,
            ReportField::Start => Some(&mut self.start),
            ReportField::End => Some(&mut self.end),
        }
    }

    fn toggle_format(&mut self) {
        self.format = match self.format {
            ReportFormat::Xlsx => ReportFormat::Csv,
            ReportFormat::Csv => ReportFormat::Xlsx,
        };
    }

    fn move_period(&mut self, period: ReportPeriodChoice) {
        self.period = period;
        if period != ReportPeriodChoice::Custom {
            self.focus = ReportField::Period;
        }
    }

    fn cycle_focus(&mut self) {
        if self.period != ReportPeriodChoice::Custom {
            return;
        }
        self.focus = match self.focus {
            ReportField::Period => ReportField::Start,
            ReportField::Start => ReportField::End,
            ReportField::End => ReportField::Period,
        };
    }

    fn type_char(&mut self, c: char) {
        match self.date_buffer_mut() {
            Some(buffer) => {
                if (c.is_ascii_digit() || c == '-') && buffer.len() < REPORT_DATE_MAX_CHARS {
                    buffer.push(c);
                }
            }
            None => match c {
                'j' => self.move_period(self.period.next()),
                'k' => self.move_period(self.period.prev()),
                'f' => self.toggle_format(),
                _ => {}
            },
        }
    }

    /// The export this form asks for, or the validation message of a bad range.
    fn request(&self) -> Result<(OwnedReportPeriod, ReportFormat), String> {
        let period = match self.period.preset() {
            Some(preset) => OwnedReportPeriod::Preset(preset),
            None => {
                validate_range(&self.start, &self.end)?;
                OwnedReportPeriod::Range {
                    start: self.start.clone(),
                    end: self.end.clone(),
                }
            }
        };
        Ok((period, self.format))
    }
}

/// Current lifecycle phase of a task-field edit modal (the estimate form and any
/// future status/assignee affordances that share the same `Cmd::SubmitTaskEdit`
/// effect).
#[derive(Debug, Clone, PartialEq)]
pub enum EditStatus {
    Editing,
    Submitting,
    Error(String),
}

/// Transient state for the in-progress estimate-edit modal.
///
/// A plain `String` buffer (not a `TextArea`) since the field is a single line with
/// no caret/undo needs, so the whole form stays `PartialEq` unlike `Compose`.
#[derive(Debug, Clone, PartialEq)]
pub struct EstimateForm {
    pub value: String,
    pub status: EditStatus,
}

/// The active modal overlay on the Detail read view.
///
/// Compose, the log-time form, the estimate-edit form, the delete prompt, and the
/// image viewer are mutually exclusive by construction — only one overlay at a
/// time. The combined state (more than one active) cannot be constructed with this
/// enum, replacing the previous independent `Option` fields.
///
/// `Compose` is intentionally larger than the other variants: it carries a `TextArea`
/// with caret/selection/undo history. Construction happens only on
/// open/edit, never per-keystroke, so boxing would add indirection with no practical
/// benefit — same rationale as `Screen`'s `large_enum_variant` allow below.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum DetailOverlay {
    None,
    Compose(Compose),
    LogTime(LogTimeForm),
    EstimateEdit(EstimateForm),
    /// The status-toggle confirm prompt. `completed_target` is the completion value
    /// the confirm would submit — the opposite of the task's current `is_completed`
    /// at the moment the overlay opened.
    StatusConfirm {
        completed_target: bool,
        status: EditStatus,
    },
    /// The assignee-picker modal. `candidates` holds id/name pairs built from the
    /// Detail screen's cached user directory, sorted case-insensitively by name.
    /// `filter` is the typed search buffer, applied as a case-insensitive substring
    /// match on the name only. `selected` is the index of the highlighted row within
    /// the FILTERED candidate list, not `candidates` itself.
    AssigneePicker {
        candidates: Vec<(i64, String)>,
        filter: String,
        selected: usize,
        status: EditStatus,
    },
    TaskListPicker(TaskListPicker),
    /// The detail was opened by `l` on the mine screen: it shows nothing yet, and
    /// the task list picker opens once the task has loaded.
    TaskListPickerPending,
    ConfirmDelete {
        comment_id: i64,
    },
    ImageViewer {
        asset: ImageAssetRef,
        status: ImageStatus,
    },
}

/// Whether the task-list picker has its lists yet.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskListLoad {
    Loading,
    Failed(String),
    Ready,
}

/// The task-list picker modal: the project's lists in server order, the task's
/// current list, the typed filter, the highlighted row within the filtered lists,
/// the write status, the lists read state and the last write error.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskListPicker {
    pub candidates: Vec<(i64, String)>,
    pub current_id: Option<i64>,
    pub filter: String,
    pub selected: usize,
    pub status: EditStatus,
    pub load: TaskListLoad,
    pub error: Option<String>,
}

/// Borrowed view into an active `AssigneePicker` overlay: its full candidate list,
/// the filter buffer, the highlighted index (within the FILTERED list), and its
/// lifecycle status.
pub type AssigneePickerView<'a> = (&'a [(i64, String)], &'a str, usize, &'a EditStatus);

/// Mutable counterpart of [`AssigneePickerView`].
pub type AssigneePickerViewMut<'a> = (
    &'a mut Vec<(i64, String)>,
    &'a mut String,
    &'a mut usize,
    &'a mut EditStatus,
);

impl DetailOverlay {
    pub fn compose(&self) -> Option<&Compose> {
        match self {
            DetailOverlay::Compose(c) => Some(c),
            _ => Option::None,
        }
    }

    pub fn compose_mut(&mut self) -> Option<&mut Compose> {
        match self {
            DetailOverlay::Compose(c) => Some(c),
            _ => Option::None,
        }
    }

    pub fn log_time(&self) -> Option<&LogTimeForm> {
        match self {
            DetailOverlay::LogTime(f) => Some(f),
            _ => Option::None,
        }
    }

    pub fn log_time_mut(&mut self) -> Option<&mut LogTimeForm> {
        match self {
            DetailOverlay::LogTime(f) => Some(f),
            _ => Option::None,
        }
    }

    pub fn estimate_edit(&self) -> Option<&EstimateForm> {
        match self {
            DetailOverlay::EstimateEdit(f) => Some(f),
            _ => Option::None,
        }
    }

    pub fn estimate_edit_mut(&mut self) -> Option<&mut EstimateForm> {
        match self {
            DetailOverlay::EstimateEdit(f) => Some(f),
            _ => Option::None,
        }
    }

    pub fn status_confirm(&self) -> Option<(bool, &EditStatus)> {
        match self {
            DetailOverlay::StatusConfirm {
                completed_target,
                status,
            } => Some((*completed_target, status)),
            _ => Option::None,
        }
    }

    pub fn status_confirm_mut(&mut self) -> Option<(&mut bool, &mut EditStatus)> {
        match self {
            DetailOverlay::StatusConfirm {
                completed_target,
                status,
            } => Some((completed_target, status)),
            _ => Option::None,
        }
    }

    pub fn assignee_picker(&self) -> Option<AssigneePickerView<'_>> {
        match self {
            DetailOverlay::AssigneePicker {
                candidates,
                filter,
                selected,
                status,
            } => Some((candidates.as_slice(), filter.as_str(), *selected, status)),
            _ => Option::None,
        }
    }

    pub fn assignee_picker_mut(&mut self) -> Option<AssigneePickerViewMut<'_>> {
        match self {
            DetailOverlay::AssigneePicker {
                candidates,
                filter,
                selected,
                status,
            } => Some((candidates, filter, selected, status)),
            _ => Option::None,
        }
    }

    pub fn confirm_delete_id(&self) -> Option<i64> {
        match self {
            DetailOverlay::ConfirmDelete { comment_id } => Some(*comment_id),
            _ => Option::None,
        }
    }

    pub fn image_viewer(&self) -> Option<(&ImageAssetRef, &ImageStatus)> {
        match self {
            DetailOverlay::ImageViewer { asset, status } => Some((asset, status)),
            _ => Option::None,
        }
    }

    pub fn image_viewer_status_mut(&mut self) -> Option<&mut ImageStatus> {
        match self {
            DetailOverlay::ImageViewer { status, .. } => Some(status),
            _ => Option::None,
        }
    }

    pub fn is_compose(&self) -> bool {
        matches!(self, DetailOverlay::Compose(_))
    }

    pub fn is_log_time(&self) -> bool {
        matches!(self, DetailOverlay::LogTime(_))
    }

    pub fn is_estimate_edit(&self) -> bool {
        matches!(self, DetailOverlay::EstimateEdit(_))
    }

    pub fn is_status_confirm(&self) -> bool {
        matches!(self, DetailOverlay::StatusConfirm { .. })
    }

    pub fn task_list_picker(&self) -> Option<&TaskListPicker> {
        match self {
            DetailOverlay::TaskListPicker(picker) => Some(picker),
            _ => None,
        }
    }

    fn task_list_picker_mut(&mut self) -> Option<&mut TaskListPicker> {
        match self {
            DetailOverlay::TaskListPicker(picker) => Some(picker),
            _ => None,
        }
    }

    /// The task-list picker's navigable state, available once its lists have loaded.
    pub fn task_list_picker_view(&self) -> Option<AssigneePickerView<'_>> {
        let picker = self.task_list_picker()?;
        (picker.load == TaskListLoad::Ready).then_some((
            picker.candidates.as_slice(),
            picker.filter.as_str(),
            picker.selected,
            &picker.status,
        ))
    }

    fn task_list_picker_view_mut(&mut self) -> Option<AssigneePickerViewMut<'_>> {
        let picker = self.task_list_picker_mut()?;
        (picker.load == TaskListLoad::Ready).then_some((
            &mut picker.candidates,
            &mut picker.filter,
            &mut picker.selected,
            &mut picker.status,
        ))
    }

    pub fn is_assignee_picker(&self) -> bool {
        matches!(self, DetailOverlay::AssigneePicker { .. })
    }

    pub fn is_confirm(&self) -> bool {
        matches!(self, DetailOverlay::ConfirmDelete { .. })
    }

    pub fn is_image_viewer(&self) -> bool {
        matches!(self, DetailOverlay::ImageViewer { .. })
    }
}

/// Structured payload sent by spawn_load_detail phase 1.
///
/// Carries the raw task/comments/assets + whatever user_map was available
/// (cached or empty). The shell calls reflow_detail before each draw, so
/// the pure layer never touches terminal width.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailLoad {
    pub task: Value,
    pub comments: Vec<Value>,
    pub assets: Vec<Asset>,
    pub user_map: HashMap<i64, String>,
    /// ISO wall-clock string (YYYY-MM-DDTHH:MM:SSZ) stamped by the shell when the load completes.
    pub loaded_at: String,
    /// The logged-in user's id for the instance that owns this task. Used to gate the
    /// `[editar]` affordance to comments authored by the current user.
    pub current_user_id: Option<i64>,
    /// True when the fetch returned HTTP 401; the TUI sets auth_error on the Detail screen.
    pub unauthorized: bool,
}

/// A screen on the navigation stack.
///
/// The Detail variant is intentionally large — it holds the full render cache
/// for the open task. The stack is always shallow (≤ 3 elements) so heap
/// boxing of the large variant would add indirection with no practical benefit.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Screen {
    Projects {
        groups: Vec<ProjectGroup>,
        selected: usize,
        loading: bool,
        /// True while an in-flight revalidation fetch is running on a warm-seeded list.
        /// Distinct from `loading`: loading blanks the list; revalidating keeps it painted.
        revalidating: bool,
    },
    Tasks {
        project_name: String,
        tasks: Vec<TaskRow>,
        selected: usize,
        loading: bool,
        /// True while an in-flight revalidation fetch is running on a warm-seeded mine list.
        /// Distinct from `loading`: revalidating keeps rows painted while the refresh runs.
        revalidating: bool,
        /// True after a task opened from the mine screen was edited successfully. Back
        /// consumes it by reloading the mine screen, whether or not a reload is in flight.
        stale_after_edit: bool,
        /// Per-slot heights (rows) computed by reflow_tasks; a slot is a card plus, for the
        /// first card of a due-date bucket, the bucket header row above it. Mirrors Detail's
        /// `lines` cache.
        card_heights: Vec<u16>,
        /// Prefix-sum y-offsets: card_offsets[i] is the row where slot i starts (the bucket
        /// header row for the first card of a bucket); card_offsets[n] is the total row count. u32 avoids the u16 saturation ceiling.
        card_offsets: Vec<u32>,
        /// Width (card_inner_w) at which the cache was last built; usize::MAX means not-yet-built.
        rendered_width: usize,
        /// `Some(today)` groups the tasks under due-date bucket headers, which use this
        /// date; `None` is a plain list. The tasks are kept in bucket order when set.
        due_today: Option<chrono::NaiveDate>,
        /// `Some` shows the tasks as a kanban of board phases with this focus; `None`
        /// shows the list. Only the mine screen (`due_today` set) ever holds `Some`.
        kanban: Option<KanbanFocus>,
        /// `Some` while the view modal is open: the highlighted choice. Only the mine
        /// screen (`due_today` set) ever holds `Some`.
        view_modal: Option<MineView>,
    },
    Detail {
        instance: String,
        project_id: i64,
        task_id: i64,
        /// Structured task JSON — source of truth for reflow.
        task: Value,
        /// Structured comments — source of truth for reflow.
        comments: Vec<Value>,
        /// User id→name map, updated on phase-2 resolution.
        user_map: HashMap<i64, String>,
        /// Memoized line cache rebuilt by reflow_detail.
        lines: Vec<String>,
        /// Parallel emphasis-style channel, index-aligned with `lines`.
        /// Each element holds the `StyleRun`s for the corresponding line.
        /// Populated exclusively by reflow_detail; initialized empty at construction.
        line_styles: Vec<Vec<StyleRun>>,
        assets: Vec<Asset>,
        offset: usize,
        loading: bool,
        /// Width at which the cache was last built; usize::MAX means "not yet built".
        rendered_width: usize,
        /// The active modal overlay: compose area or delete-confirm prompt.
        /// Mutually exclusive by construction — only one overlay at a time.
        overlay: DetailOverlay,
        /// Logged-in user's id for this instance; used to gate [editar]/[excluir] affordances.
        current_user_id: Option<i64>,
        /// All clickable affordance spans rebuilt by reflow_detail.
        ///
        /// Each entry carries its `AffordanceKind` (Edit/Delete/Confirm/Cancel) so a
        /// single linear scan over this vec replaces the four parallel affordance vecs
        /// that were here before. Hit-tested by `affordance_at` in model.rs.
        affordances: Vec<crate::render::LocalAffordance>,
        /// Index of the currently-focused comment card; None when the thread has no comments.
        focused_comment: Option<usize>,
        /// Per-card global line ranges `(start_line, line_count)`, parallel to `comments`.
        /// Rebuilt by reflow_detail on the same rendered_width invalidation as `lines`.
        comment_spans: Vec<(usize, usize)>,
        /// Set when a 401 response is received from a detail load or comment mutation.
        /// Cleared on a subsequent successful (200) detail load.
        auth_error: bool,
        /// The attachment download of this task; kept across a reload and gone
        /// with the screen.
        download: Option<DownloadState>,
    },
}

impl Screen {
    fn row_count(&self) -> usize {
        match self {
            Screen::Projects { groups, .. } => groups.len(),
            Screen::Tasks { tasks, .. } => tasks.len(),
            Screen::Detail { .. } => 0,
        }
    }

    fn selected(&self) -> usize {
        match self {
            Screen::Projects { selected, .. } => *selected,
            Screen::Tasks { selected, .. } => *selected,
            Screen::Detail { .. } => 0,
        }
    }

    fn set_selected(&mut self, value: usize) {
        match self {
            Screen::Projects { selected, .. } => *selected = value,
            Screen::Tasks { selected, .. } => *selected = value,
            Screen::Detail { .. } => {}
        }
    }

    fn is_loading(&self) -> bool {
        match self {
            Screen::Projects { loading, .. } => *loading,
            Screen::Tasks { loading, .. } => *loading,
            Screen::Detail { loading, .. } => *loading,
        }
    }
}

/// Page size for Detail screen scroll (PageUp/PageDown).
pub const PAGE_SIZE: usize = 10;

/// True maximum scroll offset for the Detail screen.
///
/// Assets are now inline in `lines` (no separate fixed panel), so the text
/// viewport spans the full content area: `viewport_rows - DETAIL_CHROME_ROWS`.
/// Reads only its arguments — no terminal, time, or async sources — so it is
/// safe to call from the pure TEA update loop.
///
/// The `assets` and `viewport_cols` parameters are retained for call-site
/// compatibility; Rust does not warn on unused fn params.
///
/// When the viewport is too small to show any text rows the function clamps
/// `text_viewport_height` to 1 (guaranteeing max = lines_len - 1), which is
/// the least surprising bound and keeps model-only tests (viewport=(0,0))
/// consistent with render behaviour.
pub fn detail_max_offset(
    viewport_rows: u16,
    viewport_cols: u16,
    lines_len: usize,
    assets: &[Asset],
) -> usize {
    let _ = (viewport_cols, assets);
    let text_viewport_height = crate::tui::detail_geometry::content_height_clamped(viewport_rows);
    lines_len.saturating_sub(text_viewport_height)
}

fn clamp_offset(offset: &mut usize, len: usize) {
    let max = len.saturating_sub(1);
    if *offset > max {
        *offset = max;
    }
}

enum SelectAction {
    PushTasks {
        project_name: String,
        tasks: Vec<TaskRow>,
    },
    PushDetail {
        instance: String,
        project_id: i64,
        task_id: i64,
    },
}

/// Determine the action to take when the user presses Select on the top screen.
///
/// Returns None when no action applies (e.g. empty list, Detail screen active).
fn select_action(stack: &[Screen]) -> Option<SelectAction> {
    match stack.last()? {
        Screen::Projects {
            groups, selected, ..
        } => {
            let group = groups.get(*selected)?;
            Some(SelectAction::PushTasks {
                project_name: group.project_name.clone(),
                tasks: group.tasks.clone(),
            })
        }
        Screen::Tasks {
            tasks,
            selected,
            kanban,
            ..
        } => {
            let index = match kanban {
                Some(focus) => focus.selected_task(tasks)?,
                None => *selected,
            };
            let row = tasks.get(index)?;
            Some(SelectAction::PushDetail {
                instance: row.instance.clone(),
                project_id: row.project_id,
                task_id: row.task_id,
            })
        }
        Screen::Detail { .. } => None,
    }
}

/// Application model — the single source of truth for the pure TEA layer.
pub struct Model {
    pub stack: Vec<Screen>,
    pub should_quit: bool,
    pub header: Header,
    /// Terminal viewport size (cols, rows) written by the shell each frame before drawing.
    pub viewport: (u16, u16),
    /// Hit-map from the last render pass, written by the shell after `terminal.draw`.
    ///
    /// Each entry records the terminal y-range and list index of a visible data row.
    /// Empty until the first draw; stale during the frame that produces it (acceptable
    /// because clicks arrive in the following event loop iteration).
    pub click_targets: Vec<ClickTarget>,
    /// Hit-map for the two confirm-modal buttons, set by the shell after each draw.
    ///
    /// Geometry is derived from the modal Rect returned by `render_modal` — single-sourced
    /// so the hit-test always matches what was drawn. Empty when no confirm modal is open.
    pub modal_button_targets: Vec<ModalButtonTarget>,
    /// ISO wall-clock string (YYYY-MM-DDTHH:MM:SSZ) of when the currently-displayed data was loaded.
    /// None until the first load completes; stamped exclusively by the shell via Msg payloads.
    pub last_loaded: Option<String>,
    /// Active text selection driven by mouse press/drag; cleared on plain click or new navigation.
    pub selection: Option<Selection>,
    /// Set by the shell after a successful clipboard write; cleared at the start of the next selection.
    pub copied_feedback: bool,
    /// The hours-report modal; model-level because it opens from every screen.
    pub report: Option<ReportForm>,
}

/// All messages the update function understands.
pub enum Msg {
    Up,
    Down,
    /// `v`: open the view modal on the mine screen.
    ViewModalOpen,
    /// ↑ in the view modal: highlight the previous view.
    ViewModalUp,
    /// ↓ in the view modal: highlight the next view.
    ViewModalDown,
    /// Enter in the view modal: show the highlighted view and close.
    ViewModalApply,
    /// Esc in the view modal: close without changing the view.
    ViewModalClose,
    /// ←: move the kanban's focus one column left, or to the previous phase in
    /// the vertical kanban.
    ColumnLeft,
    /// →: move the kanban's focus one column right, or to the next phase in the
    /// vertical kanban.
    ColumnRight,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    /// Left mouse button pressed (Down event).
    ///
    /// Modifiers are carried as plain data; the pure update discriminates
    /// between selection (unmodified) and activation (Ctrl/Cmd/Super) without
    /// touching the terminal.
    Click {
        column: u16,
        row: u16,
        modifiers: crossterm::event::KeyModifiers,
    },
    /// Left mouse button dragged (moved while held).
    Drag {
        column: u16,
        row: u16,
        modifiers: crossterm::event::KeyModifiers,
    },
    /// Left mouse button released.
    MouseUp {
        column: u16,
        row: u16,
        modifiers: crossterm::event::KeyModifiers,
    },
    Select,
    Back,
    Quit,
    Refresh,
    LoadedTasksByProject {
        groups: Vec<ProjectGroup>,
        /// ISO wall-clock string stamped by the shell when the load completed.
        loaded_at: String,
    },
    /// SWR revalidation completed for the mine task list.
    /// Replaces the current tasks, clears revalidating, and stamps last_loaded.
    LoadedMineTasks {
        rows: Vec<MineTableRow>,
        /// ISO wall-clock string stamped by the shell when the load completed.
        loaded_at: String,
    },
    /// The week and total hours of the listed mine tasks arrived after their rows.
    LoadedMineHours {
        hours: Vec<MineHours>,
    },
    /// Phase-1 load: structured data + cached (or empty) user_map.
    /// The shell calls reflow_detail before drawing to materialise lines.
    LoadedDetail(DetailLoad),
    /// Phase-2: fresh user directory resolved; invalidates the render cache
    /// so the next reflow shows the assignee name.
    UserMapResolved(HashMap<i64, String>),
    AssetActionResult,
    /// Background user directory resolved a display name for the header.
    /// Sets model.header.name when it was previously absent.
    HeaderNameResolved(String),
    /// Open the compose area on the current Detail screen.
    ComposeOpen,
    /// Apply a backend-neutral key input to the compose editor.
    ///
    /// Carries everything but the shell-level Ctrl+S/Esc shortcuts: printable chars,
    /// Enter (newline), caret movement, Home/End, Backspace/Delete, undo/redo. The
    /// shell (events.rs) converts the crossterm `KeyEvent` to `tui_textarea::Input`.
    /// `update()` applies it via `TextArea::input`.
    ComposeInput(tui_textarea::Input),
    /// Submit the current compose buffer as a new comment.
    ComposeSubmit,
    /// Cancel the compose area, discarding the buffer.
    ComposeCancel,
    /// The comment POST succeeded; refresh the detail view.
    CommentMutationOk,
    /// The comment POST failed; preserve the buffer and show an error.
    CommentMutationErr(String),
    /// A 401 response from a comment mutation (create/update/delete).
    /// Sets auth_error on the Detail screen without clearing the compose buffer.
    AuthExpired,
    /// Open the log-time modal on the current Detail screen.
    LogTimeOpen,
    /// Append a printable character to the currently focused log-time field.
    LogTimeChar(char),
    /// Remove the last character from the currently focused log-time field.
    LogTimeBackspace,
    /// Switch focus between the Hours and Summary fields.
    LogTimeToggleField,
    /// Submit the current log-time buffers as a new time record.
    LogTimeSubmit,
    /// Cancel the log-time modal, discarding the buffers.
    LogTimeCancel,
    /// The time-record POST succeeded; refresh the detail view.
    TimeMutationOk,
    /// The time-record POST failed; preserve the buffers and show an error.
    TimeMutationErr(String),
    /// Open the hours-report modal on any screen.
    ReportOpen,
    /// Move the period selection up.
    ReportUp,
    /// Move the period selection down.
    ReportDown,
    /// Toggle the format; ignored while a date field has focus.
    ReportToggleFormat,
    /// Move focus list → Start → End → list (Custom only).
    ReportToggleField,
    /// A typed character; its meaning depends on the focused part.
    ReportChar(char),
    /// Remove the last character of the focused date field.
    ReportBackspace,
    /// Start the export.
    ReportSubmit,
    /// Close the modal from any status.
    ReportClose,
    /// The export finished: the saved path and the formatted total.
    ReportOk {
        path: String,
        total: String,
    },
    /// The export failed with a translated message.
    ReportErr(String),
    /// Open the estimate-edit modal on the current Detail screen.
    EstimateOpen,
    /// Append a printable character to the estimate value buffer.
    EstimateChar(char),
    /// Remove the last character from the estimate value buffer.
    EstimateBackspace,
    /// Submit the current estimate buffer as a task-field write.
    EstimateSubmit,
    /// Cancel the estimate-edit modal, discarding the buffer.
    EstimateCancel,
    /// A task-field write (completion and/or assignee/estimate) succeeded; refresh
    /// the detail view.
    TaskEditOk,
    /// A task-field write failed; preserve the open form's buffer and show an error.
    TaskEditErr(String),
    /// Open the status-confirm modal on the current Detail screen, targeting the
    /// opposite of the task's current completion state.
    StatusToggleOpen,
    /// Confirm the pending status change and submit it as a task-field write.
    StatusToggleConfirm,
    /// Cancel the status-confirm modal without submitting.
    StatusToggleCancel,
    /// Open the assignee-picker modal on the current Detail screen, built from the
    /// already-loaded user directory.
    AssigneePickerOpen,
    /// Move the assignee-picker selection up by one row.
    AssigneePickerUp,
    /// Move the assignee-picker selection down by one row.
    AssigneePickerDown,
    /// Append a printable character to the assignee-picker filter buffer.
    AssigneePickerChar(char),
    /// Remove the last character from the assignee-picker filter buffer.
    AssigneePickerBackspace,
    /// Confirm the highlighted candidate and submit it as a task-field write.
    AssigneePickerSubmit,
    /// Cancel the assignee-picker modal without submitting.
    AssigneePickerCancel,
    /// Open the task-list picker on the current Detail screen and read the lists.
    TaskListPickerOpen,
    /// Move the task-list picker selection up by one row.
    TaskListPickerUp,
    /// Move the task-list picker selection down by one row.
    TaskListPickerDown,
    /// Append a printable character to the task-list picker filter.
    TaskListPickerChar(char),
    /// Remove the last character from the task-list picker filter.
    TaskListPickerBackspace,
    /// Move the task to the highlighted list.
    TaskListPickerSubmit,
    /// Close the task-list picker without a write.
    TaskListPickerCancel,
    /// The task lists read for one Detail task, or the failure text.
    TaskListsLoaded {
        instance: String,
        project_id: i64,
        task_id: i64,
        lists: Result<Vec<(i64, String)>, String>,
    },
    /// Move the comment-card focus cursor forward by one card (j / Down in Detail browse mode).
    FocusNextComment,
    /// Move the comment-card focus cursor backward by one card (k / Up in Detail browse mode).
    FocusPrevComment,
    /// Confirm the pending delete (Enter key in confirm sub-mode).
    ConfirmDeleteComment,
    /// Cancel the pending delete (Esc key in confirm sub-mode).
    CancelDeleteComment,
    /// The shell finished fetching and decoding the open viewer's image.
    ///
    /// Sent only by the shell wiring still to land; this
    /// slice's `update_image` handles the transition but nothing produces the
    /// message yet, so tests construct it directly.
    #[allow(dead_code)]
    ImageLoaded,
    /// The shell failed to fetch or decode the open viewer's image.
    ///
    /// See `ImageLoaded` above — same shell-wiring boundary.
    #[allow(dead_code)]
    ImageLoadErr(String),
    /// Start downloading every downloadable asset of the open task (`D`).
    DownloadAttachments,
    /// The shell finished the download started for this task.
    AttachmentsDownloaded {
        instance: String,
        project_id: i64,
        task_id: i64,
        outcome: DownloadOutcome,
    },
}

impl Model {
    /// Build the initial browse model.
    ///
    /// When `seed` is `Some` with a non-empty list the Projects screen is painted
    /// immediately (`loading=false`, `revalidating=true`) so the user sees content
    /// before the revalidation fetch returns.  A cold start (`None` or empty seed)
    /// falls back to the classic `loading=true` placeholder.
    pub fn browse(header: Header, seed: Option<Vec<ProjectGroup>>) -> Self {
        let (groups, loading, revalidating) = match seed {
            Some(g) if !g.is_empty() => (g, false, true),
            _ => (vec![], true, false),
        };
        Model {
            stack: vec![Screen::Projects {
                groups,
                selected: 0,
                loading,
                revalidating,
            }],
            should_quit: false,
            header,
            viewport: (0, 0),
            click_targets: vec![],
            modal_button_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
            report: None,
        }
    }

    /// Replace the hit-map recorded by the last render pass.
    ///
    /// Called by the shell after `terminal.draw` completes, mirroring the
    /// `viewport` write pattern. Only the shell touches this field.
    pub fn set_click_targets(&mut self, targets: Vec<ClickTarget>) {
        self.click_targets = targets;
    }

    /// Replace the confirm-modal button targets recorded by the last render pass.
    ///
    /// Geometry is single-sourced from the modal Rect `render_modal` returns.
    /// Called by the shell immediately after `set_click_targets`. Empty when no
    /// confirm modal was rendered.
    pub fn set_modal_button_targets(&mut self, targets: Vec<ModalButtonTarget>) {
        self.modal_button_targets = targets;
    }

    pub fn top(&self) -> Option<&Screen> {
        self.stack.last()
    }

    /// The view modal's highlighted choice while it is open on the top screen.
    pub fn view_modal(&self) -> Option<MineView> {
        match self.top() {
            Some(Screen::Tasks { view_modal, .. }) => *view_modal,
            _ => None,
        }
    }

    fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
    }

    /// The Tasks screen wherever it sits in the stack: background loads land
    /// while a Detail screen is open above it.
    pub fn tasks_screen(&self) -> Option<&Screen> {
        self.stack
            .iter()
            .find(|s| matches!(s, Screen::Tasks { .. }))
    }

    fn tasks_screen_mut(&mut self) -> Option<&mut Screen> {
        self.stack
            .iter_mut()
            .find(|s| matches!(s, Screen::Tasks { .. }))
    }

    /// Rebuild the Detail screen's line cache if the width changed.
    ///
    /// Guard clauses ensure this is a no-op when the top screen is not Detail,
    /// is still loading, or the cache is already current for `inner_width`.
    ///
    /// After a rebuild, `offset` is clamped to `lines.len().saturating_sub(1)`
    /// so a resize that shortens content keeps the scroll position in range.
    pub fn reflow_detail(&mut self, inner_width: usize) {
        let Some(Screen::Detail {
            task,
            comments,
            user_map,
            lines,
            line_styles,
            rendered_width,
            offset,
            loading,
            current_user_id,
            affordances,
            comment_spans,
            ..
        }) = self.top_mut()
        else {
            return;
        };

        if *loading || *rendered_width == inner_width {
            return;
        }

        let uid = *current_user_id;
        let content =
            crate::render::build_detail_content(task, comments, user_map, inner_width, uid);
        *lines = content.lines;
        *line_styles = content.line_styles;
        *affordances = content.affordances;
        *comment_spans = content.comment_spans;

        *rendered_width = inner_width;
        clamp_offset(offset, lines.len());
    }

    /// Rebuild the Tasks card-height cache if the card width changed.
    ///
    /// Guard clauses ensure this is a no-op when the top screen is not Tasks,
    /// is still loading, or the cache is already current for `card_inner_w`.
    ///
    /// The cache depends on `tasks`, `card_inner_w` and `due_today` (which adds the bucket
    /// header rows), never on `selected`.
    pub fn reflow_tasks(&mut self, card_inner_w: usize) {
        let Some(Screen::Tasks {
            tasks,
            loading,
            card_heights,
            card_offsets,
            rendered_width,
            due_today,
            ..
        }) = self.top_mut()
        else {
            return;
        };

        if *loading || *rendered_width == card_inner_w {
            return;
        }

        *card_heights = crate::tui::task_layout::slot_heights(tasks, card_inner_w, *due_today);

        let mut offsets = Vec::with_capacity(card_heights.len() + 1);
        let mut acc: u32 = 0;
        for &h in card_heights.iter() {
            offsets.push(acc);
            acc = acc.saturating_add(h as u32);
        }
        offsets.push(acc);
        *card_offsets = offsets;
        *rendered_width = card_inner_w;
    }
}

/// Pure update — returns new model and any effects to run.
///
/// All navigation, selection, and loader logic lives here so it is
/// headlessly unit-testable with no terminal or async runtime.
///
/// Dispatches to family sub-handlers to keep each function within the
/// cyclomatic-complexity budget (≤ 10).
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    if model.view_modal().is_some() && behind_view_modal(&msg) {
        return (model, vec![]);
    }
    update_by_family(model, msg)
}

fn update_by_family(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        m @ (Msg::Up
        | Msg::ScrollUp
        | Msg::Down
        | Msg::ScrollDown
        | Msg::PageUp
        | Msg::PageDown
        | Msg::Click { .. }
        | Msg::Drag { .. }
        | Msg::MouseUp { .. }) => update_input(model, m),
        m @ (Msg::Select
        | Msg::Back
        | Msg::Quit
        | Msg::Refresh
        | Msg::ViewModalOpen
        | Msg::ViewModalUp
        | Msg::ViewModalDown
        | Msg::ViewModalApply
        | Msg::ViewModalClose
        | Msg::ColumnLeft
        | Msg::ColumnRight) => update_navigation(model, m),
        m @ (Msg::LoadedTasksByProject { .. }
        | Msg::LoadedMineTasks { .. }
        | Msg::LoadedMineHours { .. }
        | Msg::LoadedDetail(_)
        | Msg::UserMapResolved(_)
        | Msg::AssetActionResult
        | Msg::HeaderNameResolved(_)) => update_loaded(model, m),
        m @ (Msg::ComposeOpen
        | Msg::ComposeInput(_)
        | Msg::ComposeSubmit
        | Msg::ComposeCancel
        | Msg::CommentMutationOk
        | Msg::CommentMutationErr(_)
        | Msg::AuthExpired) => update_compose(model, m),
        m @ (Msg::LogTimeOpen
        | Msg::LogTimeChar(_)
        | Msg::LogTimeBackspace
        | Msg::LogTimeToggleField
        | Msg::LogTimeSubmit
        | Msg::LogTimeCancel
        | Msg::TimeMutationOk
        | Msg::TimeMutationErr(_)) => update_log_time(model, m),
        m @ (Msg::ReportOpen
        | Msg::ReportUp
        | Msg::ReportDown
        | Msg::ReportToggleFormat
        | Msg::ReportToggleField
        | Msg::ReportChar(_)
        | Msg::ReportBackspace
        | Msg::ReportSubmit
        | Msg::ReportClose
        | Msg::ReportOk { .. }
        | Msg::ReportErr(_)) => update_report(model, m),
        m @ (Msg::EstimateOpen
        | Msg::EstimateChar(_)
        | Msg::EstimateBackspace
        | Msg::EstimateSubmit
        | Msg::EstimateCancel
        | Msg::StatusToggleOpen
        | Msg::StatusToggleConfirm
        | Msg::StatusToggleCancel
        | Msg::AssigneePickerOpen
        | Msg::AssigneePickerUp
        | Msg::AssigneePickerDown
        | Msg::AssigneePickerChar(_)
        | Msg::AssigneePickerBackspace
        | Msg::AssigneePickerSubmit
        | Msg::AssigneePickerCancel
        | Msg::TaskListPickerOpen
        | Msg::TaskListPickerUp
        | Msg::TaskListPickerDown
        | Msg::TaskListPickerChar(_)
        | Msg::TaskListPickerBackspace
        | Msg::TaskListPickerSubmit
        | Msg::TaskListPickerCancel
        | Msg::TaskListsLoaded { .. }
        | Msg::TaskEditOk
        | Msg::TaskEditErr(_)) => update_task_edit(model, m),
        m @ (Msg::FocusNextComment
        | Msg::FocusPrevComment
        | Msg::ConfirmDeleteComment
        | Msg::CancelDeleteComment) => update_comment_focus(model, m),
        m @ (Msg::ImageLoaded
        | Msg::ImageLoadErr(_)
        | Msg::DownloadAttachments
        | Msg::AttachmentsDownloaded { .. }) => update_asset_effects(model, m),
    }
}

/// Route the image-viewer and attachment-download messages to their handlers.
fn update_asset_effects(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::ImageLoaded => (apply_image_status(model, ImageStatus::Ready), vec![]),
        Msg::ImageLoadErr(reason) => (
            apply_image_status(model, ImageStatus::Error(reason)),
            vec![],
        ),
        Msg::DownloadAttachments => handle_download_start(model),
        Msg::AttachmentsDownloaded {
            instance,
            project_id,
            task_id,
            outcome,
        } => (
            handle_download_finished(model, (&instance, project_id, task_id), outcome),
            vec![],
        ),
        _ => (model, vec![]),
    }
}

/// Start the attachment download of the open Detail: one effect carrying every
/// downloadable asset of the task and its comments.
///
/// Does nothing off the Detail screen, while it is still loading, or while a
/// download of the screen already runs; a task without a downloadable asset
/// shows a note instead of starting one.
fn handle_download_start(mut model: Model) -> (Model, Vec<Cmd>) {
    let Some(Screen::Detail {
        instance,
        project_id,
        task_id,
        task,
        comments,
        loading,
        download,
        ..
    }) = model.top_mut()
    else {
        return (model, vec![]);
    };
    if *loading || matches!(download, Some(DownloadState::Running)) {
        return (model, vec![]);
    }
    let assets = crate::controller::downloadable_assets(task, comments);
    if assets.is_empty() {
        *download = Some(DownloadState::NoAttachments);
        return (model, vec![]);
    }
    *download = Some(DownloadState::Running);
    let cmd = Cmd::DownloadAttachments {
        instance: instance.clone(),
        project_id: *project_id,
        task_id: *task_id,
        assets,
    };
    (model, vec![cmd])
}

/// Record a finished download on the Detail it was started for; a result for
/// another task, or after the screen was popped, is dropped.
fn handle_download_finished(
    mut model: Model,
    task_ref: (&str, i64, i64),
    outcome: DownloadOutcome,
) -> Model {
    if let Some(Screen::Detail {
        instance,
        project_id,
        task_id,
        download,
        ..
    }) = model.top_mut()
    {
        if (instance.as_str(), *project_id, *task_id) == task_ref {
            *download = Some(DownloadState::Finished(outcome));
        }
    }
    model
}

/// Apply the shell's image-fetch outcome to an open `ImageViewer` overlay.
///
/// No-op when the overlay is not `ImageViewer` (e.g. the user already closed
/// it before the fetch settled).
fn apply_image_status(mut model: Model, new_status: ImageStatus) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay, ..
    }) = model.top_mut()
    {
        if let Some(status) = overlay.image_viewer_status_mut() {
            *status = new_status;
        }
    }
    model
}

fn update_input(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        m @ (Msg::Click { .. } | Msg::Drag { .. } | Msg::MouseUp { .. }) => {
            update_pointer(model, m)
        }
        m => update_scroll(model, m),
    }
}

fn update_scroll(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    if model.report.is_some() && matches!(msg, Msg::ScrollUp | Msg::ScrollDown) {
        return (model, vec![]);
    }
    match msg {
        Msg::Up => (handle_up(model), vec![]),
        Msg::ScrollUp => (handle_scroll_up(model), vec![]),
        Msg::Down => (handle_down(model), vec![]),
        Msg::ScrollDown => (handle_scroll_down(model), vec![]),
        Msg::PageUp => (handle_page_up(model), vec![]),
        Msg::PageDown => (handle_page_down(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_pointer(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    if model.report.is_some() {
        return (model, vec![]);
    }
    match msg {
        Msg::Click {
            column,
            row,
            modifiers,
        } => handle_click(model, column, row, modifiers),
        Msg::Drag {
            column,
            row,
            modifiers,
        } => handle_drag(model, column, row, modifiers),
        Msg::MouseUp {
            column,
            row,
            modifiers,
        } => handle_mouse_up(model, column, row, modifiers),
        _ => (model, vec![]),
    }
}

fn update_navigation(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::Select => handle_select(model),
        Msg::Back => handle_back_reloading_mine(model),
        Msg::Quit => (handle_quit(model), vec![]),
        Msg::Refresh => handle_refresh(model),
        Msg::ColumnLeft => (move_kanban(model, false, false), vec![]),
        Msg::ColumnRight => (move_kanban(model, false, true), vec![]),
        m => update_view_modal(model, m),
    }
}

fn update_view_modal(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::ViewModalOpen => (open_view_modal(model), vec![]),
        Msg::ViewModalUp => (move_view_modal(model, false), vec![]),
        Msg::ViewModalDown => (move_view_modal(model, true), vec![]),
        Msg::ViewModalApply => apply_view_modal(model),
        Msg::ViewModalClose => (close_view_modal(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_loaded(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::LoadedTasksByProject { groups, loaded_at } => {
            (handle_loaded_tasks(model, groups, loaded_at), vec![])
        }
        Msg::LoadedMineTasks { rows, loaded_at } => {
            (handle_loaded_mine_tasks(model, rows, loaded_at), vec![])
        }
        Msg::LoadedMineHours { hours } => (handle_loaded_mine_hours(model, hours), vec![]),
        Msg::LoadedDetail(load) => handle_loaded_detail(model, load),
        Msg::UserMapResolved(map) => (handle_user_map_resolved(model, map), vec![]),
        Msg::AssetActionResult => (model, vec![]),
        Msg::HeaderNameResolved(name) => (handle_header_name_resolved(model, name), vec![]),
        _ => (model, vec![]),
    }
}

fn update_compose(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::ComposeOpen => (handle_compose_open(model), vec![]),
        Msg::ComposeInput(input) => (handle_compose_input(model, input), vec![]),
        Msg::ComposeSubmit => handle_compose_submit(model),
        Msg::ComposeCancel => (close_detail_overlay(model), vec![]),
        Msg::CommentMutationOk => handle_comment_mutation_ok(model),
        Msg::CommentMutationErr(msg) => (handle_comment_mutation_err(model, msg), vec![]),
        Msg::AuthExpired => (handle_auth_expired(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_log_time(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::LogTimeOpen => (handle_log_time_open(model), vec![]),
        Msg::LogTimeChar(c) => (handle_log_time_char(model, c), vec![]),
        Msg::LogTimeBackspace => (handle_log_time_backspace(model), vec![]),
        Msg::LogTimeToggleField => (handle_log_time_toggle_field(model), vec![]),
        Msg::LogTimeSubmit => handle_log_time_submit(model),
        Msg::LogTimeCancel => (close_detail_overlay(model), vec![]),
        Msg::TimeMutationOk => handle_time_mutation_ok(model),
        Msg::TimeMutationErr(msg) => (handle_time_mutation_err(model, msg), vec![]),
        _ => (model, vec![]),
    }
}

/// The estimate, status and assignee overlays all submit a task edit, so they
/// share its outcome messages.
fn update_task_edit(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        m @ (Msg::StatusToggleOpen | Msg::StatusToggleConfirm | Msg::StatusToggleCancel) => {
            update_status_toggle(model, m)
        }
        m @ (Msg::AssigneePickerOpen
        | Msg::AssigneePickerUp
        | Msg::AssigneePickerDown
        | Msg::AssigneePickerChar(_)
        | Msg::AssigneePickerBackspace
        | Msg::AssigneePickerSubmit
        | Msg::AssigneePickerCancel) => update_assignee_picker(model, m),
        m @ (Msg::TaskListPickerOpen
        | Msg::TaskListPickerUp
        | Msg::TaskListPickerDown
        | Msg::TaskListPickerChar(_)
        | Msg::TaskListPickerBackspace
        | Msg::TaskListPickerSubmit
        | Msg::TaskListPickerCancel
        | Msg::TaskListsLoaded { .. }) => update_task_list_picker(model, m),
        Msg::TaskEditOk => handle_task_edit_ok(model),
        Msg::TaskEditErr(msg) => (handle_task_edit_err(model, msg), vec![]),
        m => update_estimate(model, m),
    }
}

fn update_estimate(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::EstimateOpen => (handle_estimate_open(model), vec![]),
        Msg::EstimateChar(c) => (handle_estimate_char(model, c), vec![]),
        Msg::EstimateBackspace => (handle_estimate_backspace(model), vec![]),
        Msg::EstimateSubmit => handle_estimate_submit(model),
        Msg::EstimateCancel => (close_detail_overlay(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_comment_focus(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::FocusNextComment => (handle_focus_next(model), vec![]),
        Msg::FocusPrevComment => (handle_focus_prev(model), vec![]),
        Msg::ConfirmDeleteComment => handle_confirm_delete(model),
        Msg::CancelDeleteComment => (close_detail_overlay(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_status_toggle(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::StatusToggleOpen => (handle_status_toggle_open(model), vec![]),
        Msg::StatusToggleConfirm => handle_status_toggle_confirm(model),
        Msg::StatusToggleCancel => (close_detail_overlay(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_assignee_picker(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::AssigneePickerOpen => (handle_assignee_picker_open(model), vec![]),
        Msg::AssigneePickerUp => (handle_picker_up(model, ASSIGNEE_PICKER), vec![]),
        Msg::AssigneePickerDown => (handle_picker_down(model, ASSIGNEE_PICKER), vec![]),
        Msg::AssigneePickerChar(c) => (handle_picker_char(model, ASSIGNEE_PICKER, c), vec![]),
        Msg::AssigneePickerBackspace => (handle_picker_backspace(model, ASSIGNEE_PICKER), vec![]),
        Msg::AssigneePickerSubmit => handle_assignee_picker_submit(model),
        Msg::AssigneePickerCancel => (close_detail_overlay(model), vec![]),
        _ => (model, vec![]),
    }
}

fn update_task_list_picker(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::TaskListPickerOpen => handle_task_list_picker_open(model),
        Msg::TaskListPickerUp => (handle_picker_up(model, TASK_LIST_PICKER), vec![]),
        Msg::TaskListPickerDown => (handle_picker_down(model, TASK_LIST_PICKER), vec![]),
        Msg::TaskListPickerChar(c) => (handle_picker_char(model, TASK_LIST_PICKER, c), vec![]),
        Msg::TaskListPickerBackspace => (handle_picker_backspace(model, TASK_LIST_PICKER), vec![]),
        Msg::TaskListPickerSubmit => handle_task_list_picker_submit(model),
        Msg::TaskListPickerCancel => (close_detail_overlay(model), vec![]),
        Msg::TaskListsLoaded {
            instance,
            project_id,
            task_id,
            lists,
        } => (
            handle_task_lists_loaded(model, (&instance, project_id, task_id), lists),
            vec![],
        ),
        _ => (model, vec![]),
    }
}

/// Messages that must not reach the screen behind the open view modal: they
/// would move or reload it while the modal owns the keyboard and the pointer.
fn behind_view_modal(msg: &Msg) -> bool {
    matches!(
        msg,
        Msg::Up
            | Msg::Down
            | Msg::ScrollUp
            | Msg::ScrollDown
            | Msg::PageUp
            | Msg::PageDown
            | Msg::Click { .. }
            | Msg::Drag { .. }
            | Msg::MouseUp { .. }
            | Msg::Select
            | Msg::Back
            | Msg::Refresh
            | Msg::ColumnLeft
            | Msg::ColumnRight
            | Msg::ViewModalOpen
    )
}

/// Open the view modal on the mine screen with the current view highlighted;
/// a no-op on every other screen.
fn open_view_modal(mut model: Model) -> Model {
    if let Some(Screen::Tasks {
        due_today: Some(_),
        kanban,
        view_modal,
        ..
    }) = model.top_mut()
    {
        *view_modal = Some(kanban.as_ref().map_or(MineView::List, KanbanFocus::view));
    }
    model
}

/// Highlight the previous or next view, stopping at the ends of the list.
fn move_view_modal(mut model: Model, forward: bool) -> Model {
    if let Some(Screen::Tasks {
        view_modal: Some(choice),
        ..
    }) = model.top_mut()
    {
        let at = MineView::ALL.iter().position(|v| v == choice).unwrap_or(0);
        let next = if forward {
            (at + 1).min(MineView::ALL.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        *choice = MineView::ALL[next];
    }
    model
}

fn close_view_modal(mut model: Model) -> Model {
    if let Some(Screen::Tasks { view_modal, .. }) = model.top_mut() {
        *view_modal = None;
    }
    model
}

/// Show the highlighted view, close the modal and ask the shell to remember
/// the choice. A kanban keeps the focus it already has, so switching between
/// the two layouts leaves the same card selected.
fn apply_view_modal(mut model: Model) -> (Model, Vec<Cmd>) {
    let Some(Screen::Tasks {
        tasks,
        kanban,
        view_modal,
        ..
    }) = model.top_mut()
    else {
        return (model, vec![]);
    };
    let Some(view) = view_modal.take() else {
        return (model, vec![]);
    };
    *kanban = view.layout().map(|layout| KanbanFocus {
        layout,
        ..kanban.unwrap_or_else(|| KanbanFocus::first_populated(tasks))
    });
    (model, vec![Cmd::SaveMineView(view)])
}

/// Move the kanban's focus one step. ↑/↓ (`along_cards`) step the card within
/// the focused column in the horizontal kanban and through the whole
/// phase-grouped list in the vertical one; ←/→ step the column in the
/// horizontal kanban and jump to the first card of the neighbouring phase that
/// has cards in the vertical one.
fn move_kanban(mut model: Model, along_cards: bool, forward: bool) -> Model {
    if let Some(Screen::Tasks {
        tasks,
        kanban: Some(focus),
        ..
    }) = model.top_mut()
    {
        match (focus.layout, along_cards) {
            (KanbanLayout::Horizontal, true) => focus.step_card(tasks, forward),
            (KanbanLayout::Horizontal, false) => focus.step_column(forward),
            (KanbanLayout::Vertical, true) => focus.step_through(tasks, forward),
            (KanbanLayout::Vertical, false) => focus.jump_phase(tasks, forward),
        }
    }
    model
}

fn handle_up(model: Model) -> Model {
    match model.top() {
        Some(Screen::Detail { .. }) => handle_focus_prev(model),
        Some(Screen::Tasks {
            kanban: Some(_), ..
        }) => move_kanban(model, true, false),
        Some(_) => {
            let mut model = model;
            let sel = model.top().map(|s| s.selected()).unwrap_or(0);
            if let Some(screen) = model.top_mut() {
                screen.set_selected(sel.saturating_sub(1));
            }
            model
        }
        None => model,
    }
}

fn handle_down(model: Model) -> Model {
    let (viewport_cols, viewport_rows) = model.viewport;
    match model.top() {
        Some(Screen::Detail { .. }) => handle_focus_next(model),
        Some(Screen::Tasks {
            kanban: Some(_), ..
        }) => move_kanban(model, true, true),
        Some(screen) => {
            let count = screen.row_count();
            let sel = screen.selected();
            let _ = (viewport_cols, viewport_rows);
            let mut model = model;
            if count > 0 {
                if let Some(screen) = model.top_mut() {
                    screen.set_selected((sel + 1).min(count - 1));
                }
            }
            model
        }
        None => model,
    }
}

fn handle_scroll_up(mut model: Model) -> Model {
    match model.top_mut() {
        Some(Screen::Detail { offset, .. }) => {
            *offset = offset.saturating_sub(1);
        }
        Some(Screen::Tasks {
            kanban: Some(_), ..
        }) => {}
        Some(screen) => {
            let sel = screen.selected();
            screen.set_selected(sel.saturating_sub(1));
        }
        None => {}
    }
    model
}

fn handle_scroll_down(mut model: Model) -> Model {
    let (viewport_cols, viewport_rows) = model.viewport;
    match model.top_mut() {
        Some(Screen::Detail {
            offset,
            lines,
            assets,
            ..
        }) => {
            let max = detail_max_offset(viewport_rows, viewport_cols, lines.len(), assets);
            *offset = (*offset + 1).min(max);
        }
        Some(Screen::Tasks {
            kanban: Some(_), ..
        }) => {}
        Some(screen) => {
            let count = screen.row_count();
            if count > 0 {
                let sel = screen.selected();
                screen.set_selected((sel + 1).min(count - 1));
            }
        }
        None => {}
    }
    model
}

fn handle_page_up(mut model: Model) -> Model {
    if let Some(Screen::Detail { offset, .. }) = model.top_mut() {
        *offset = offset.saturating_sub(PAGE_SIZE);
    }
    model
}

fn handle_page_down(mut model: Model) -> Model {
    let (viewport_cols, viewport_rows) = model.viewport;
    if let Some(Screen::Detail {
        offset,
        lines,
        assets,
        ..
    }) = model.top_mut()
    {
        let max = detail_max_offset(viewport_rows, viewport_cols, lines.len(), assets);
        *offset = (*offset + PAGE_SIZE).min(max);
    }
    model
}

fn handle_focus_next(mut model: Model) -> Model {
    let info = focus_move_info(&model);
    let Some((focused, comment_count, comment_spans, viewport)) = info else {
        return model;
    };
    if comment_count == 0 {
        return model;
    }
    let new_focus = match focused {
        None => 0,
        Some(i) => (i + 1).min(comment_count - 1),
    };
    apply_focus(&mut model, new_focus, &comment_spans, viewport);
    model
}

fn handle_focus_prev(mut model: Model) -> Model {
    let info = focus_move_info(&model);
    let Some((focused, comment_count, comment_spans, viewport)) = info else {
        return model;
    };
    if comment_count == 0 {
        return model;
    }
    let new_focus = match focused {
        None => 0,
        Some(0) => 0,
        Some(i) => i - 1,
    };
    apply_focus(&mut model, new_focus, &comment_spans, viewport);
    model
}

type FocusMoveInfo = Option<(Option<usize>, usize, Vec<(usize, usize)>, (u16, u16))>;

fn focus_move_info(model: &Model) -> FocusMoveInfo {
    match model.top() {
        Some(Screen::Detail {
            comments,
            focused_comment,
            comment_spans,
            ..
        }) => Some((
            *focused_comment,
            comments.len(),
            comment_spans.clone(),
            model.viewport,
        )),
        _ => None,
    }
}

fn apply_focus(
    model: &mut Model,
    new_focus: usize,
    comment_spans: &[(usize, usize)],
    viewport: (u16, u16),
) {
    if let Some(Screen::Detail {
        focused_comment,
        offset,
        lines,
        assets,
        ..
    }) = model.top_mut()
    {
        *focused_comment = Some(new_focus);
        if let Some(&(card_start, card_count)) = comment_spans.get(new_focus) {
            let (viewport_cols, viewport_rows) = viewport;
            let max = detail_max_offset(viewport_rows, viewport_cols, lines.len(), assets);
            *offset = scroll_offset_for_card(*offset, card_start, card_count, viewport_rows, max);
        }
    }
}

/// Derive the scroll offset so that the card at `[card_start, card_start + card_count)`
/// is fully visible within `viewport_rows` (minus chrome).
///
/// - Card below viewport end → scroll down so card's last line is the last visible line.
/// - Card above viewport start → scroll up so card's first line is the first visible line.
/// - Card already fully visible → offset unchanged.
pub(crate) fn scroll_offset_for_card(
    current_offset: usize,
    card_start: usize,
    card_count: usize,
    viewport_rows: u16,
    max_offset: usize,
) -> usize {
    let text_vh = crate::tui::detail_geometry::content_height_clamped(viewport_rows);
    let card_end = card_start + card_count;
    let viewport_end = current_offset + text_vh;

    if card_end > viewport_end {
        card_end.saturating_sub(text_vh).min(max_offset)
    } else if card_start < current_offset {
        card_start.min(max_offset)
    } else {
        current_offset
    }
}

fn handle_click(
    model: Model,
    column: u16,
    row: u16,
    modifiers: crossterm::event::KeyModifiers,
) -> (Model, Vec<Cmd>) {
    match model.top() {
        Some(Screen::Detail { .. }) => handle_click_detail(model, column, row, modifiers),
        Some(_) => handle_click_list(model, row),
        None => (model, vec![]),
    }
}

fn handle_click_list(model: Model, row: u16) -> (Model, Vec<Cmd>) {
    let Some(target) = model
        .click_targets
        .iter()
        .find(|t| row >= t.y_start && row < t.y_end)
        .cloned()
    else {
        return (model, vec![]);
    };

    let mut model = model;
    if let Some(screen) = model.top_mut() {
        screen.set_selected(target.index);
    }
    handle_select(model)
}

fn confirm_modal_is_open(model: &Model) -> bool {
    matches!(
        model.top(),
        Some(Screen::Detail { overlay, .. }) if overlay.is_confirm()
    )
}

fn dispatch_confirm_modal_click(model: Model, column: u16, row: u16) -> (Model, Vec<Cmd>) {
    let hit = model
        .modal_button_targets
        .iter()
        .find(|t| row == t.row && column >= t.x_start && column < t.x_end)
        .cloned();
    match hit {
        Some(btn) if btn.is_confirm => handle_confirm_delete(model),
        Some(_) => (close_detail_overlay(model), vec![]),
        None => (model, vec![]),
    }
}

fn handle_click_detail(
    model: Model,
    column: u16,
    row: u16,
    modifiers: crossterm::event::KeyModifiers,
) -> (Model, Vec<Cmd>) {
    use super::hit_test::resolve_detail_click;
    use crossterm::event::KeyModifiers;
    let has_modifier =
        modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);

    if confirm_modal_is_open(&model) {
        return dispatch_confirm_modal_click(model, column, row);
    }

    if has_modifier {
        match resolve_detail_click(&model, column, row, true) {
            Some(target) => return apply_detail_click_target(model, target),
            None => return (model, vec![]),
        }
    }

    if is_in_body_area(&model, row) {
        let mut model = model;
        model.copied_feedback = false;
        model.selection = Some(Selection {
            anchor: (row, column),
            cursor: (row, column),
        });
        return (model, vec![]);
    }

    // Plain click outside the body (e.g. asset panel): clear selection, no asset action.
    // Reserved for V6 text selection — must not open an asset on plain click.
    let mut model = model;
    model.selection = None;
    (model, vec![])
}

/// Map a resolved `DetailClickTarget` to its TEA effect: updated Model and emitted Cmds.
///
/// Keeps the `handle_click_detail` function within the complexity budget by lifting
/// the four-way dispatch into a dedicated, independently-testable unit.
fn apply_detail_click_target(
    model: Model,
    target: super::hit_test::DetailClickTarget,
) -> (Model, Vec<Cmd>) {
    use super::hit_test::DetailClickTarget;

    match target {
        DetailClickTarget::CommentEdit(id) => (handle_edit_comment_request(model, id), vec![]),
        DetailClickTarget::CommentDelete(id) => (handle_delete_comment_request(model, id), vec![]),
        DetailClickTarget::OpenUrl(url) | DetailClickTarget::OpenAsset(url) => {
            let instance = match model.top() {
                Some(Screen::Detail { instance, .. }) => instance.clone(),
                _ => return (model, vec![]),
            };
            (model, vec![Cmd::OpenAsset { instance, url }])
        }
        DetailClickTarget::ViewImage(asset) => {
            if !matches!(model.top(), Some(Screen::Detail { .. })) {
                return (model, vec![]);
            }
            (
                handle_open_image_viewer(model, asset.clone()),
                vec![Cmd::LoadImage { asset }],
            )
        }
    }
}

/// Set `overlay = ImageViewer { asset, status: Loading }`, opening the viewer.
///
/// Invalidates the render cache like the other overlay-open handlers
/// (`handle_compose_open`, `handle_delete_comment_request`) so it appears on
/// the next reflow. No-op when the top screen is not Detail.
fn handle_open_image_viewer(mut model: Model, asset: ImageAssetRef) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::ImageViewer {
            asset,
            status: ImageStatus::Loading,
        };
        *rendered_width = usize::MAX;
    }
    model
}

/// Return true when `row` falls within the scrollable body text area of the Detail screen.
///
/// Assets are now inline in the scrollable body, so the body spans the full content area;
/// bounds are delegated to `detail_geometry::is_in_content`.
fn is_in_body_area(model: &Model, row: u16) -> bool {
    let Some(Screen::Detail { .. }) = model.top() else {
        return false;
    };
    let (_viewport_cols, viewport_rows) = model.viewport;
    crate::tui::detail_geometry::is_in_content(viewport_rows, row)
}

fn handle_drag(
    mut model: Model,
    column: u16,
    row: u16,
    modifiers: crossterm::event::KeyModifiers,
) -> (Model, Vec<Cmd>) {
    use crossterm::event::KeyModifiers;
    let has_modifier =
        modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);
    if has_modifier {
        return (model, vec![]);
    }
    if let Some(ref mut sel) = model.selection {
        sel.cursor = (row, column);
    }
    (model, vec![])
}

fn handle_mouse_up(
    mut model: Model,
    column: u16,
    row: u16,
    modifiers: crossterm::event::KeyModifiers,
) -> (Model, Vec<Cmd>) {
    use crossterm::event::KeyModifiers;
    let has_modifier =
        modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER);
    if has_modifier {
        return (model, vec![]);
    }

    let sel = match model.selection.take() {
        Some(s) => s,
        None => return (model, vec![]),
    };

    if !sel.is_drag() {
        return (model, vec![]);
    }

    let (_viewport_cols, viewport_rows) = model.viewport;
    let anchor = sel.anchor;
    let Screen::Detail { lines, offset, .. } = model.top().expect("detail screen") else {
        return (model, vec![]);
    };
    let text = crate::tui::detail_geometry::selected_text(*offset, viewport_rows, sel, lines);
    if text.is_empty() {
        return (model, vec![]);
    }

    model.selection = Some(Selection {
        anchor,
        cursor: (row, column),
    });

    (model, vec![Cmd::CopyToClipboard(text)])
}

fn handle_select(model: Model) -> (Model, Vec<Cmd>) {
    // When the delete-confirm modal is open, Enter confirms the delete.
    if let Some(Screen::Detail { overlay, .. }) = model.top() {
        if overlay.is_confirm() {
            return handle_confirm_delete(model);
        }
    }
    let mut model = model;
    let mut cmds = vec![];
    if let Some(action) = select_action(&model.stack) {
        match action {
            SelectAction::PushTasks {
                project_name,
                tasks,
            } => {
                model.stack.push(Screen::Tasks {
                    project_name,
                    tasks,
                    selected: 0,
                    loading: false,
                    revalidating: false,
                    stale_after_edit: false,
                    card_heights: Vec::new(),
                    card_offsets: Vec::new(),
                    rendered_width: usize::MAX,
                    due_today: None,
                    kanban: None,
                    view_modal: None,
                });
            }
            SelectAction::PushDetail {
                instance,
                project_id,
                task_id,
            } => {
                cmds.push(Cmd::LoadDetail {
                    instance: instance.clone(),
                    project_id,
                    task_id,
                    refresh: false,
                });
                model.stack.push(Screen::Detail {
                    instance,
                    project_id,
                    task_id,
                    task: Value::Null,
                    comments: vec![],
                    user_map: HashMap::new(),
                    lines: vec![],
                    line_styles: vec![],
                    assets: vec![],
                    offset: 0,
                    loading: true,
                    rendered_width: usize::MAX,
                    overlay: DetailOverlay::None,
                    current_user_id: None,
                    affordances: vec![],
                    focused_comment: None,
                    comment_spans: vec![],
                    auth_error: false,
                    download: None,
                });
            }
        }
    }
    (model, cmds)
}

fn handle_back(model: Model) -> Model {
    // When the delete-confirm modal is open, Esc cancels the delete (not Back).
    if let Some(Screen::Detail { overlay, .. }) = model.top() {
        if overlay.is_confirm() {
            return close_detail_overlay(model);
        }
        if overlay.is_image_viewer() {
            return close_detail_overlay(model);
        }
    }
    let mut model = model;
    if model.stack.len() <= 1 {
        model.should_quit = true;
    } else {
        model.stack.pop();
    }
    model
}

/// The mine screen directly below a Detail on top of the stack.
fn mine_screen_below_detail(model: &mut Model) -> Option<&mut Screen> {
    let below = model.stack.len().checked_sub(2)?;
    if !matches!(model.stack.last(), Some(Screen::Detail { .. })) {
        return None;
    }
    model.stack.get_mut(below).filter(|s| {
        matches!(
            s,
            Screen::Tasks {
                due_today: Some(_),
                ..
            }
        )
    })
}

/// Back, reloading the mine screen it returns to when an edit marked it stale.
fn handle_back_reloading_mine(model: Model) -> (Model, Vec<Cmd>) {
    let from_detail = matches!(model.top(), Some(Screen::Detail { .. }));
    let depth = model.stack.len();
    let mut model = handle_back(model);
    let popped_detail = from_detail && model.stack.len() < depth;
    if !popped_detail {
        return (model, vec![]);
    }
    if let Some(Screen::Tasks {
        due_today: Some(_),
        stale_after_edit,
        revalidating,
        ..
    }) = model.top_mut()
    {
        if std::mem::take(stale_after_edit) {
            // A reload already in flight may predate the edit; the newer reply wins.
            *revalidating = true;
            return (model, vec![Cmd::LoadMineTasks]);
        }
    }
    (model, vec![])
}

fn handle_quit(mut model: Model) -> Model {
    // When the image viewer is open, q closes it instead of quitting the app.
    if let Some(Screen::Detail { overlay, .. }) = model.top() {
        if overlay.is_image_viewer() {
            return close_detail_overlay(model);
        }
    }
    model.should_quit = true;
    model
}

/// Dismiss whatever overlay the Detail screen shows, without submitting (the image
/// viewer's Esc/q, a picker's cancel).
fn close_detail_overlay(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay, ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::None;
    }
    model
}

fn handle_refresh(mut model: Model) -> (Model, Vec<Cmd>) {
    let mut cmds = vec![];
    if let Some(top) = model.top_mut() {
        if top.is_loading() {
            // Single-flight guard: drop the duplicate refresh while one is in flight.
            return (model, cmds);
        }
        if let Screen::Tasks {
            due_today: Some(_),
            revalidating,
            ..
        } = top
        {
            // The mine rows stay painted while the reload runs; its reply clears the flag.
            if !*revalidating {
                *revalidating = true;
                cmds.push(Cmd::LoadMineTasks);
            }
            return (model, cmds);
        }
        match top {
            Screen::Detail {
                instance,
                project_id,
                task_id,
                ref mut loading,
                ref mut offset,
                ..
            } => {
                *loading = true;
                *offset = 0;
                cmds.push(Cmd::LoadDetail {
                    instance: instance.clone(),
                    project_id: *project_id,
                    task_id: *task_id,
                    refresh: true,
                });
            }
            Screen::Projects {
                ref mut loading, ..
            } => {
                *loading = true;
                cmds.push(Cmd::LoadTasksByProject);
            }
            Screen::Tasks {
                ref mut loading,
                ref mut revalidating,
                ..
            } => {
                *loading = true;
                *revalidating = false;
                cmds.push(Cmd::LoadTasksByProject);
            }
        }
    }
    (model, cmds)
}

fn handle_loaded_tasks(mut model: Model, groups: Vec<ProjectGroup>, loaded_at: String) -> Model {
    if let Some(Screen::Projects {
        groups: ref mut g,
        loading: ref mut l,
        revalidating: ref mut rv,
        ..
    }) = model.top_mut()
    {
        *g = groups;
        *l = false;
        *rv = false;
    }
    model.last_loaded = Some(loaded_at);
    model
}

fn handle_loaded_mine_tasks(mut model: Model, rows: Vec<MineTableRow>, loaded_at: String) -> Model {
    if let Some(Screen::Tasks {
        tasks: ref mut t,
        loading: ref mut l,
        revalidating: ref mut rv,
        card_heights: ref mut ch,
        card_offsets: ref mut co,
        rendered_width: ref mut rw,
        selected: ref mut sel,
        due_today,
        kanban,
        ..
    }) = model.tasks_screen_mut()
    {
        let earlier = std::mem::take(t);
        *t = order_by_bucket(rows_to_task_rows(rows), *due_today);
        keep_earlier_hours(t, &earlier);
        *sel = reselect(*sel, &earlier, t);
        if let Some(focus) = kanban {
            *focus = if earlier.is_empty() {
                KanbanFocus {
                    layout: focus.layout,
                    ..KanbanFocus::first_populated(t)
                }
            } else {
                focus.reselected(&earlier, t)
            };
        }
        *l = false;
        *rv = false;
        // Invalidate card cache so reflow_tasks rebuilds at current width.
        *ch = vec![];
        *co = vec![];
        *rw = usize::MAX;
    }
    model.last_loaded = Some(loaded_at);
    model
}

crate::render::buckets::impl_bucketed_task!(TaskRow);

/// The tasks in due-date bucket order when a grouping date is set, else as given.
fn order_by_bucket(tasks: Vec<TaskRow>, due_today: Option<chrono::NaiveDate>) -> Vec<TaskRow> {
    let Some(today) = due_today else {
        return tasks;
    };
    crate::render::buckets::group_by_bucket(&tasks, today)
        .into_iter()
        .flat_map(|bucket| bucket.items)
        .cloned()
        .collect()
}

/// The selection after a reload: the same task (by instance and id) wherever it
/// moved, else the old index clamped to the new list.
fn reselect(selected: usize, earlier: &[TaskRow], fresh: &[TaskRow]) -> usize {
    let moved = earlier.get(selected).and_then(|prior| {
        fresh
            .iter()
            .position(|r| r.instance == prior.instance && r.task_id == prior.task_id)
    });
    moved.unwrap_or_else(|| selected.min(fresh.len().saturating_sub(1)))
}

/// Rows arrive before their hours: a row without hours shows the hours the
/// same task (by instance and id) showed before the reload.
fn keep_earlier_hours(fresh: &mut [TaskRow], earlier: &[TaskRow]) {
    let by_task: HashMap<(&str, i64), &TaskRow> = earlier
        .iter()
        .map(|r| ((r.instance.as_str(), r.task_id), r))
        .collect();
    for row in fresh {
        if let Some(prior) = by_task.get(&(row.instance.as_str(), row.task_id)) {
            row.logged_week = row.logged_week.or(prior.logged_week);
            row.logged_total = row.logged_total.or(prior.logged_total);
        }
    }
}

/// Sets week and total hours of each listed task named by an entry; an unknown
/// entry value clears it, a row without an entry is untouched.
fn handle_loaded_mine_hours(mut model: Model, hours: Vec<MineHours>) -> Model {
    if let Some(Screen::Tasks {
        tasks: ref mut t,
        card_heights: ref mut ch,
        card_offsets: ref mut co,
        rendered_width: ref mut rw,
        ..
    }) = model.tasks_screen_mut()
    {
        let by_task: HashMap<(&str, i64), &MineHours> = hours
            .iter()
            .map(|h| ((h.instance.as_str(), h.task_id), h))
            .collect();
        for row in t.iter_mut() {
            if let Some(entry) = by_task.get(&(row.instance.as_str(), row.task_id)) {
                row.logged_week = entry.logged_week;
                row.logged_total = entry.logged_total;
            }
        }
        *ch = vec![];
        *co = vec![];
        *rw = usize::MAX;
    }
    model
}

fn rows_to_task_rows(rows: Vec<MineTableRow>) -> Vec<TaskRow> {
    rows.into_iter()
        .map(|r| TaskRow {
            task_id: r.task_id,
            task_number: r.task_number,
            name: r.name,
            instance: r.instance,
            project_id: r.project_id,
            due_on: r.due_on,
            project_name: r.project_name,
            estimate: r.estimate,
            logged_week: r.logged_week,
            logged_total: r.logged_total,
            phase: r.phase,
            flagged: r.flagged,
            task_list: r.task_list,
        })
        .collect()
}

fn handle_loaded_detail(mut model: Model, load: DetailLoad) -> (Model, Vec<Cmd>) {
    let picker_pending = matches!(
        model.top(),
        Some(Screen::Detail {
            overlay: DetailOverlay::TaskListPickerPending,
            ..
        })
    );
    let loaded_ok = !load.task.is_null() && !load.unauthorized;
    if let Some(Screen::Detail {
        task: ref mut t,
        comments: ref mut c,
        assets: ref mut a,
        user_map: ref mut um,
        lines: ref mut ls,
        line_styles: ref mut lss,
        rendered_width: ref mut rw,
        ref mut loading,
        ref mut current_user_id,
        ref mut affordances,
        ref mut overlay,
        ref mut focused_comment,
        ref mut comment_spans,
        ref mut auth_error,
        ..
    }) = model.top_mut()
    {
        *t = load.task;
        *c = load.comments;
        *a = load.assets;
        *um = load.user_map;
        *loading = false;
        *current_user_id = load.current_user_id;
        *auth_error = load.unauthorized;
        // Invalidate line cache so reflow_detail rebuilds at current width.
        *ls = vec![];
        *lss = vec![];
        *rw = usize::MAX;
        *affordances = vec![];
        *overlay = DetailOverlay::None;
        *focused_comment = None;
        *comment_spans = vec![];
    }
    model.last_loaded = Some(load.loaded_at);
    if picker_pending && loaded_ok {
        return handle_task_list_picker_open(model);
    }
    (model, vec![])
}

/// Set auth_error on the Detail screen without disturbing the compose buffer.
///
/// Called when a comment mutation returns HTTP 401. The user keeps their draft.
fn handle_auth_expired(mut model: Model) -> Model {
    if let Some(form) = model
        .report
        .as_mut()
        .filter(|form| form.status == ReportStatus::Exporting)
    {
        form.status = ReportStatus::Error(t(
            "Token invalid or revoked — run `ac setup add` to re-authenticate.",
        ));
    }
    if let Some(Screen::Detail {
        ref mut auth_error, ..
    }) = model.top_mut()
    {
        *auth_error = true;
    }
    model
}

fn handle_user_map_resolved(mut model: Model, map: HashMap<i64, String>) -> Model {
    if let Some(Screen::Detail {
        user_map: ref mut um,
        lines: ref mut ls,
        line_styles: ref mut lss,
        rendered_width: ref mut rw,
        ..
    }) = model.top_mut()
    {
        *um = map;
        // Invalidate line cache so the next reflow shows the updated assignee.
        *ls = vec![];
        *lss = vec![];
        *rw = usize::MAX;
    }
    model
}

fn handle_header_name_resolved(mut model: Model, name: String) -> Model {
    model.header.name = Some(name);
    model
}

fn handle_compose_open(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if !overlay.is_compose() {
            *overlay = DetailOverlay::Compose(Compose {
                kind: ComposeKind::New,
                editor: TextArea::default(),
                status: ComposeStatus::Editing,
            });
            *rendered_width = usize::MAX;
        }
    }
    model
}

/// Apply a backend-neutral key input to the compose editor.
///
/// A pure data operation: `TextArea::input` handles printable chars, newline,
/// caret movement, Home/End, Backspace/Delete, and undo/redo internally.
fn handle_compose_input(mut model: Model, input: tui_textarea::Input) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(cp) = overlay.compose_mut() {
            if cp.status == ComposeStatus::Editing {
                cp.editor.input(input);
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

fn handle_compose_submit(mut model: Model) -> (Model, Vec<Cmd>) {
    let submit_info = extract_compose_submit_info(&model);

    let Some((instance, project_id, task_id, kind, body)) = submit_info else {
        return (model, vec![]);
    };

    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(cp) = overlay.compose_mut() {
            cp.status = ComposeStatus::Submitting;
            *rendered_width = usize::MAX;
        }
    }

    let cmd = match kind {
        ComposeKind::New => Cmd::SubmitComment {
            instance,
            project_id,
            task_id,
            body,
        },
        ComposeKind::Edit { comment_id } => Cmd::UpdateComment {
            instance,
            comment_id,
            body,
        },
    };
    (model, vec![cmd])
}

/// Extract the fields needed to submit a compose buffer, or None when the guard fails.
///
/// Guard: overlay must be Compose, status must be `Editing`, and the joined editor
/// body must be non-empty after trimming (blocks whitespace-only submits).
fn extract_compose_submit_info(model: &Model) -> Option<(String, i64, i64, ComposeKind, String)> {
    match model.top() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            overlay,
            ..
        }) => {
            let cp = overlay.compose()?;
            let body = cp.editor.lines().join("\n");
            if cp.status == ComposeStatus::Editing && !body.trim().is_empty() {
                Some((
                    instance.clone(),
                    *project_id,
                    *task_id,
                    cp.kind.clone(),
                    body,
                ))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Open the compose area pre-filled with the plain-text body of `comment_id`.
///
/// No-op when the top screen is not Detail or the comment is not found. Sets
/// `ComposeKind::Edit{comment_id}` and invalidates the render cache so the compose
/// block appears on the next reflow.
fn handle_edit_comment_request(mut model: Model, comment_id: i64) -> Model {
    let body = match model.top() {
        Some(Screen::Detail { comments, .. }) => comments
            .iter()
            .find(|c| c.get("id").and_then(|v| v.as_i64()) == Some(comment_id))
            .map(|c| {
                c.get("body_plain_text")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        let html = c.get("body").and_then(|v| v.as_str()).unwrap_or("");
                        crate::render::html_to_text(html)
                    })
            }),
        _ => return model,
    };

    let Some(body) = body else {
        return model;
    };

    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::Compose(Compose {
            kind: ComposeKind::Edit { comment_id },
            editor: TextArea::from(body.lines()),
            status: ComposeStatus::Editing,
        });
        *rendered_width = usize::MAX;
    }
    model
}

/// Set `overlay = DetailOverlay::ConfirmDelete { comment_id }` and invalidate the render
/// cache so the confirm prompt appears on the next reflow. Emits no Cmd.
fn handle_delete_comment_request(mut model: Model, comment_id: i64) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::ConfirmDelete { comment_id };
        *rendered_width = usize::MAX;
    }
    model
}

/// Emit `Cmd::DeleteComment` for the pending comment and clear the overlay.
fn handle_confirm_delete(mut model: Model) -> (Model, Vec<Cmd>) {
    let fields = match model.top() {
        Some(Screen::Detail {
            instance, overlay, ..
        }) => overlay.confirm_delete_id().map(|id| (instance.clone(), id)),
        _ => None,
    };

    let Some((instance, comment_id)) = fields else {
        return (model, vec![]);
    };

    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::None;
        *rendered_width = usize::MAX;
    }

    (
        model,
        vec![Cmd::DeleteComment {
            instance,
            comment_id,
        }],
    )
}

/// Clear the active overlay and emit `Cmd::LoadDetail { refresh: true }` for the
/// current Detail screen — the server-truth refresh shared by every write path
/// (comment mutation, time-record mutation) after a successful POST/PUT.
fn refresh_detail_after_write(mut model: Model) -> (Model, Vec<Cmd>) {
    let detail_fields = match model.top() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            ..
        }) => Some((instance.clone(), *project_id, *task_id)),
        _ => None,
    };

    if let Some(Screen::Detail {
        ref mut overlay, ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::None;
    }

    let Some((instance, project_id, task_id)) = detail_fields else {
        return (model, vec![]);
    };

    let cmd = Cmd::LoadDetail {
        instance,
        project_id,
        task_id,
        refresh: true,
    };
    (model, vec![cmd])
}

fn handle_comment_mutation_ok(model: Model) -> (Model, Vec<Cmd>) {
    refresh_detail_after_write(model)
}

fn handle_comment_mutation_err(mut model: Model, msg: String) -> Model {
    if let Some(Screen::Detail {
        overlay: DetailOverlay::Compose(cp),
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        cp.status = ComposeStatus::Error(msg);
        *rendered_width = usize::MAX;
    }
    model
}

/// Open the log-time modal on the current Detail screen with empty buffers.
///
/// No-op when the log-time modal is already open — mirrors `handle_compose_open`.
fn handle_log_time_open(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if !overlay.is_log_time() {
            *overlay = DetailOverlay::LogTime(LogTimeForm {
                hours: String::new(),
                summary: String::new(),
                field: LogTimeField::Hours,
                status: LogTimeStatus::Editing,
            });
            *rendered_width = usize::MAX;
        }
    }
    model
}

/// Append `c` to the currently focused log-time field while the form is `Editing`.
fn handle_log_time_char(mut model: Model, c: char) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.log_time_mut() {
            if form.status == LogTimeStatus::Editing {
                form.focused_buffer_mut().push(c);
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

/// Remove the last character from the currently focused log-time field while
/// the form is `Editing`.
fn handle_log_time_backspace(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.log_time_mut() {
            if form.status == LogTimeStatus::Editing {
                form.focused_buffer_mut().pop();
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

/// Switch the focused log-time field between Hours and Summary.
fn handle_log_time_toggle_field(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.log_time_mut() {
            form.field = match form.field {
                LogTimeField::Hours => LogTimeField::Summary,
                LogTimeField::Summary => LogTimeField::Hours,
            };
            *rendered_width = usize::MAX;
        }
    }
    model
}

/// Extract the fields needed to submit the log-time form, or None when the guard fails.
///
/// Guard: overlay must be `LogTime` and its status must be `Editing`.
fn extract_log_time_submit_info(model: &Model) -> Option<(String, i64, i64, String, String)> {
    match model.top() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            overlay,
            ..
        }) => {
            let form = overlay.log_time()?;
            if form.status != LogTimeStatus::Editing {
                return None;
            }
            Some((
                instance.clone(),
                *project_id,
                *task_id,
                form.hours.clone(),
                form.summary.clone(),
            ))
        }
        _ => None,
    }
}

/// Set the log-time overlay's status to `Error(message)`, when the overlay is active.
fn set_log_time_error(model: &mut Model, message: String) {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.log_time_mut() {
            form.status = LogTimeStatus::Error(message);
            *rendered_width = usize::MAX;
        }
    }
}

/// Parse the hours buffer and, when it is a value greater than zero, set the form to
/// `Submitting` and emit `Cmd::SubmitTimeLog`. An empty, zero, negative, or
/// non-numeric hours buffer sets `Error` instead and emits no `Cmd`. The summary
/// buffer becomes `Some` only when non-empty after trimming.
fn handle_log_time_submit(mut model: Model) -> (Model, Vec<Cmd>) {
    let Some((instance, project_id, task_id, hours_input, summary_input)) =
        extract_log_time_submit_info(&model)
    else {
        return (model, vec![]);
    };

    let parsed_hours = hours_input
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|hours| *hours > 0.0);

    let Some(hours) = parsed_hours else {
        set_log_time_error(&mut model, t("Enter a positive number of hours"));
        return (model, vec![]);
    };

    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.log_time_mut() {
            form.status = LogTimeStatus::Submitting;
            *rendered_width = usize::MAX;
        }
    }

    let summary = {
        let trimmed = summary_input.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };

    (
        model,
        vec![Cmd::SubmitTimeLog {
            instance,
            project_id,
            task_id,
            hours,
            summary,
        }],
    )
}

/// The instance a report export targets: the open Detail task's instance, else
/// the header's primary (first configured) instance.
fn report_instance(model: &Model) -> String {
    match model.top() {
        Some(Screen::Detail { instance, .. }) => instance.clone(),
        _ => model.header.instance.clone(),
    }
}

fn report_submit(form: &mut ReportForm) -> Vec<Cmd> {
    if form.status == ReportStatus::Exporting {
        return vec![];
    }
    match form.request() {
        Ok((period, format)) => {
            form.status = ReportStatus::Exporting;
            vec![Cmd::ExportHoursReport {
                instance: form.instance.clone(),
                period,
                format,
            }]
        }
        Err(message) => {
            form.status = ReportStatus::Error(message);
            vec![]
        }
    }
}

/// Apply a form-editing message to the open report modal; messages that do not
/// edit the form are ignored.
fn report_edit(form: &mut ReportForm, msg: Msg) -> Vec<Cmd> {
    let on_date_field = form.focus != ReportField::Period;
    match msg {
        Msg::ReportUp => form.move_period(form.period.prev()),
        Msg::ReportDown => form.move_period(form.period.next()),
        Msg::ReportToggleFormat if !on_date_field => form.toggle_format(),
        Msg::ReportToggleField => form.cycle_focus(),
        Msg::ReportChar(c) => form.type_char(c),
        Msg::ReportBackspace => {
            if let Some(buffer) = form.date_buffer_mut() {
                buffer.pop();
            }
        }
        Msg::ReportSubmit => return report_submit(form),
        Msg::ReportOk { path, total } => form.status = ReportStatus::Done { path, total },
        Msg::ReportErr(message) => form.status = ReportStatus::Error(message),
        _ => {}
    }
    vec![]
}

fn update_report(mut model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    match msg {
        Msg::ReportOpen => {
            if model.report.is_none() {
                model.report = Some(ReportForm::new(report_instance(&model)));
            }
            return (model, vec![]);
        }
        Msg::ReportClose => {
            model.report = None;
            return (model, vec![]);
        }
        Msg::ReportChar('b')
            if model
                .report
                .as_ref()
                .is_some_and(|form| form.focus == ReportField::Period) =>
        {
            model.report = None;
            return (model, vec![]);
        }
        _ => {}
    }
    let cmds = match model.report.as_mut() {
        Some(form) => report_edit(form, msg),
        None => vec![],
    };
    (model, cmds)
}

fn handle_time_mutation_ok(model: Model) -> (Model, Vec<Cmd>) {
    refresh_detail_after_write(model)
}

fn handle_time_mutation_err(mut model: Model, msg: String) -> Model {
    set_log_time_error(&mut model, msg);
    model
}

/// Read the task's current `estimate` field as a plain decimal string, or an empty
/// string when the field is absent or not numeric.
fn estimate_prefill(task: &Value) -> String {
    match task.get("estimate").and_then(|v| v.as_f64()) {
        Some(hours) if hours == hours.trunc() => (hours as i64).to_string(),
        Some(hours) => hours.to_string(),
        None => String::new(),
    }
}

/// Open the estimate-edit modal on the current Detail screen, prefilled from the
/// task's current estimate when present.
///
/// No-op when the estimate-edit modal is already open — mirrors `handle_log_time_open`.
fn handle_estimate_open(mut model: Model) -> Model {
    let prefill = match model.top() {
        Some(Screen::Detail { task, .. }) => estimate_prefill(task),
        _ => String::new(),
    };
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if !overlay.is_estimate_edit() {
            *overlay = DetailOverlay::EstimateEdit(EstimateForm {
                value: prefill,
                status: EditStatus::Editing,
            });
            *rendered_width = usize::MAX;
        }
    }
    model
}

/// Append `c` to the estimate value buffer while the form is `Editing`.
fn handle_estimate_char(mut model: Model, c: char) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.estimate_edit_mut() {
            if form.status == EditStatus::Editing {
                form.value.push(c);
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

/// Remove the last character from the estimate value buffer while the form is `Editing`.
fn handle_estimate_backspace(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.estimate_edit_mut() {
            if form.status == EditStatus::Editing {
                form.value.pop();
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

/// Extract the fields needed to submit the estimate form, or None when the guard fails.
///
/// Guard: overlay must be `EstimateEdit` and its status must be `Editing`.
fn extract_estimate_submit_info(model: &Model) -> Option<(String, i64, i64, String)> {
    match model.top() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            overlay,
            ..
        }) => {
            let form = overlay.estimate_edit()?;
            if form.status != EditStatus::Editing {
                return None;
            }
            Some((instance.clone(), *project_id, *task_id, form.value.clone()))
        }
        _ => None,
    }
}

/// Report a failed task write on whichever edit overlay is active: the estimate form,
/// the status prompt or the assignee picker show the error status; the task-list picker
/// returns to editing with the error text so another list can be tried. A no-op otherwise.
fn set_task_edit_error(model: &mut Model, message: String) {
    let Some(Screen::Detail {
        overlay,
        rendered_width,
        ..
    }) = model.top_mut()
    else {
        return;
    };
    let shown = match overlay {
        DetailOverlay::EstimateEdit(form) => {
            form.status = EditStatus::Error(message);
            true
        }
        DetailOverlay::StatusConfirm { status, .. }
        | DetailOverlay::AssigneePicker { status, .. } => {
            *status = EditStatus::Error(message);
            true
        }
        DetailOverlay::TaskListPicker(picker) => {
            picker.status = EditStatus::Editing;
            picker.error = Some(message);
            true
        }
        _ => false,
    };
    if shown {
        *rendered_width = usize::MAX;
    }
}

/// Parse the estimate value buffer and, when it is a number >= 0, set the form to
/// `Submitting` and emit `Cmd::SubmitTaskEdit` carrying `estimate: Some(value)` with
/// `completion` and `assignee_id` both `None`. An empty, negative, or non-numeric
/// buffer sets `Error` instead and emits no `Cmd`.
fn handle_estimate_submit(mut model: Model) -> (Model, Vec<Cmd>) {
    let Some((instance, project_id, task_id, value_input)) = extract_estimate_submit_info(&model)
    else {
        return (model, vec![]);
    };

    let parsed_estimate = value_input
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| *value >= 0.0);

    let Some(estimate) = parsed_estimate else {
        set_task_edit_error(&mut model, t("Enter a non-negative number of hours"));
        return (model, vec![]);
    };

    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(form) = overlay.estimate_edit_mut() {
            form.status = EditStatus::Submitting;
            *rendered_width = usize::MAX;
        }
    }

    (
        model,
        vec![Cmd::SubmitTaskEdit {
            instance,
            project_id,
            task_id,
            completion: None,
            assignee_id: None,
            estimate: Some(estimate),
            task_list_id: None,
        }],
    )
}

fn handle_task_edit_ok(mut model: Model) -> (Model, Vec<Cmd>) {
    if let Some(Screen::Tasks {
        stale_after_edit, ..
    }) = mine_screen_below_detail(&mut model)
    {
        *stale_after_edit = true;
    }
    refresh_detail_after_write(model)
}

fn handle_task_edit_err(mut model: Model, msg: String) -> Model {
    set_task_edit_error(&mut model, msg);
    model
}

/// Read the task's current `is_completed` field, defaulting to `false` when absent
/// or not boolean.
fn task_is_completed(task: &Value) -> bool {
    task.get("is_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Open the status-confirm modal on the current Detail screen, targeting the
/// opposite of the task's current `is_completed`.
///
/// No-op when the status-confirm modal is already open — mirrors `handle_estimate_open`.
fn handle_status_toggle_open(mut model: Model) -> Model {
    let completed_target = match model.top() {
        Some(Screen::Detail { task, .. }) => !task_is_completed(task),
        _ => false,
    };
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if !overlay.is_status_confirm() {
            *overlay = DetailOverlay::StatusConfirm {
                completed_target,
                status: EditStatus::Editing,
            };
            *rendered_width = usize::MAX;
        }
    }
    model
}

/// Extract the fields needed to submit the pending status-confirm target, or None
/// when the guard fails.
///
/// Guard: overlay must be `StatusConfirm` and its status must be `Editing`.
fn extract_status_confirm_submit_info(model: &Model) -> Option<(String, i64, i64, bool)> {
    match model.top() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            overlay,
            ..
        }) => {
            let (completed_target, status) = overlay.status_confirm()?;
            if *status != EditStatus::Editing {
                return None;
            }
            Some((instance.clone(), *project_id, *task_id, completed_target))
        }
        _ => None,
    }
}

/// Set the status-confirm overlay to `Submitting` and emit `Cmd::SubmitTaskEdit`
/// carrying `completion: Some(completed_target)` with `assignee_id` and `estimate`
/// both `None`. A no-op (no Cmd) when the overlay is absent or not `Editing`.
fn handle_status_toggle_confirm(mut model: Model) -> (Model, Vec<Cmd>) {
    let Some((instance, project_id, task_id, completed_target)) =
        extract_status_confirm_submit_info(&model)
    else {
        return (model, vec![]);
    };

    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some((_, status)) = overlay.status_confirm_mut() {
            *status = EditStatus::Submitting;
            *rendered_width = usize::MAX;
        }
    }

    (
        model,
        vec![Cmd::SubmitTaskEdit {
            instance,
            project_id,
            task_id,
            completion: Some(completed_target),
            assignee_id: None,
            estimate: None,
            task_list_id: None,
        }],
    )
}

/// Build the assignee-picker candidate list from the Detail screen's cached user
/// directory, sorted case-insensitively by name with the user id as a tiebreaker so
/// the order is deterministic regardless of the map's iteration order.
fn assignee_candidates(user_map: &HashMap<i64, String>) -> Vec<(i64, String)> {
    let mut candidates: Vec<(i64, String)> = user_map
        .iter()
        .map(|(id, name)| (*id, name.clone()))
        .collect();
    candidates.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then(a.0.cmp(&b.0))
    });
    candidates
}

/// Index of `assignee_id` within `candidates`, or 0 when absent or not found.
fn assignee_candidate_index(candidates: &[(i64, String)], assignee_id: Option<i64>) -> usize {
    assignee_id
        .and_then(|id| candidates.iter().position(|(cid, _)| *cid == id))
        .unwrap_or(0)
}

/// Filter `candidates` to those whose name contains `filter` as a case-insensitive
/// substring. Matches on the name only, never the id. An empty filter returns every
/// candidate unchanged.
pub(crate) fn filter_assignee_candidates(
    candidates: &[(i64, String)],
    filter: &str,
) -> Vec<(i64, String)> {
    if filter.is_empty() {
        return candidates.to_vec();
    }
    let needle = filter.to_lowercase();
    candidates
        .iter()
        .filter(|(_, name)| name.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Open the assignee-picker modal on the current Detail screen, with candidates built
/// from the already-loaded user directory and the task's current assignee pre-selected.
///
/// No-op when the assignee-picker modal is already open — mirrors `handle_estimate_open`.
fn handle_assignee_picker_open(mut model: Model) -> Model {
    let built = match model.top() {
        Some(Screen::Detail { user_map, task, .. }) => {
            let candidates = assignee_candidates(user_map);
            let assignee_id = task.get("assignee_id").and_then(|v| v.as_i64());
            let selected = assignee_candidate_index(&candidates, assignee_id);
            Some((candidates, selected))
        }
        _ => None,
    };
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if !overlay.is_assignee_picker() {
            if let Some((candidates, selected)) = built {
                *overlay = DetailOverlay::AssigneePicker {
                    candidates,
                    filter: String::new(),
                    selected,
                    status: EditStatus::Editing,
                };
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

/// The accessors a picker overlay exposes so the assignee and task-list pickers
/// share one navigation core. A view is `None` while the overlay is not open or,
/// for the task-list picker, while its lists have not loaded.
#[derive(Clone, Copy)]
struct PickerKind {
    view: for<'a> fn(&'a DetailOverlay) -> Option<AssigneePickerView<'a>>,
    view_mut: for<'a> fn(&'a mut DetailOverlay) -> Option<AssigneePickerViewMut<'a>>,
}

const ASSIGNEE_PICKER: PickerKind = PickerKind {
    view: DetailOverlay::assignee_picker,
    view_mut: DetailOverlay::assignee_picker_mut,
};

const TASK_LIST_PICKER: PickerKind = PickerKind {
    view: DetailOverlay::task_list_picker_view,
    view_mut: DetailOverlay::task_list_picker_view_mut,
};

/// Run `edit` on a picker's navigable state and repaint when it reports a change.
/// A no-op while the picker is not open or not yet loaded.
fn edit_picker(
    mut model: Model,
    kind: PickerKind,
    edit: impl FnOnce(AssigneePickerViewMut<'_>) -> bool,
) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        if let Some(view) = (kind.view_mut)(overlay) {
            if edit(view) {
                *rendered_width = usize::MAX;
            }
        }
    }
    model
}

/// Move a picker's selection up by one row within the FILTERED candidate list,
/// saturating at the top. No-op while not `Editing` or when no candidate matches
/// the current filter.
fn handle_picker_up(model: Model, kind: PickerKind) -> Model {
    edit_picker(model, kind, |(candidates, filter, selected, status)| {
        let movable = *status == EditStatus::Editing
            && !filter_assignee_candidates(candidates, filter).is_empty();
        if movable {
            *selected = selected.saturating_sub(1);
        }
        movable
    })
}

/// Move a picker's selection down by one row within the FILTERED candidate list,
/// saturating at the bottom. No-op while not `Editing` or when no candidate
/// matches the current filter.
fn handle_picker_down(model: Model, kind: PickerKind) -> Model {
    edit_picker(model, kind, |(candidates, filter, selected, status)| {
        let filtered_len = filter_assignee_candidates(candidates, filter).len();
        let movable = *status == EditStatus::Editing && filtered_len > 0;
        if movable {
            *selected = (*selected + 1).min(filtered_len - 1);
        }
        movable
    })
}

/// Append `c` to a picker's filter buffer while `Editing`, and reset the
/// highlighted row to the first match of the new filter.
fn handle_picker_char(model: Model, kind: PickerKind, c: char) -> Model {
    edit_picker(model, kind, |(_, filter, selected, status)| {
        let typing = *status == EditStatus::Editing;
        if typing {
            filter.push(c);
            *selected = 0;
        }
        typing
    })
}

/// Remove the last character from a picker's filter buffer while `Editing`, and
/// reset the highlighted row to the first match of the new filter. No-op on an
/// already-empty filter.
fn handle_picker_backspace(model: Model, kind: PickerKind) -> Model {
    edit_picker(model, kind, |(_, filter, selected, status)| {
        let erasing = *status == EditStatus::Editing && !filter.is_empty();
        if erasing {
            filter.pop();
            *selected = 0;
        }
        erasing
    })
}

/// Extract the fields needed to submit a picker's highlighted candidate, or None
/// when the guard fails.
///
/// Guard: the picker must be open, its status must be `Editing`, and the FILTERED
/// candidate list must be non-empty.
fn picked_candidate(model: &Model, kind: PickerKind) -> Option<(String, i64, i64, i64)> {
    match model.top() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            overlay,
            ..
        }) => {
            let (candidates, filter, selected, status) = (kind.view)(overlay)?;
            if *status != EditStatus::Editing {
                return None;
            }
            let filtered = filter_assignee_candidates(candidates, filter);
            let (picked_id, _) = filtered.get(selected)?;
            Some((instance.clone(), *project_id, *task_id, *picked_id))
        }
        _ => None,
    }
}

/// Set a picker to `Submitting` once its highlighted candidate is sent.
fn mark_picker_submitting(model: Model, kind: PickerKind) -> Model {
    edit_picker(model, kind, |(_, _, _, status)| {
        *status = EditStatus::Submitting;
        true
    })
}

/// Set the assignee-picker overlay to `Submitting` and emit `Cmd::SubmitTaskEdit`
/// carrying `assignee_id: Some(the highlighted candidate id)` with `completion` and
/// `estimate` both `None`. A no-op (no Cmd) when the overlay is absent, not `Editing`,
/// or the candidate list is empty.
fn handle_assignee_picker_submit(model: Model) -> (Model, Vec<Cmd>) {
    let Some((instance, project_id, task_id, assignee_id)) =
        picked_candidate(&model, ASSIGNEE_PICKER)
    else {
        return (model, vec![]);
    };
    (
        mark_picker_submitting(model, ASSIGNEE_PICKER),
        vec![Cmd::SubmitTaskEdit {
            instance,
            project_id,
            task_id,
            completion: None,
            assignee_id: Some(assignee_id),
            estimate: None,
            task_list_id: None,
        }],
    )
}

/// Open the task-list picker on the Detail screen in its loading state and emit the
/// read of the project's lists. A no-op off the Detail screen or while any overlay
/// (including the picker itself) is already open.
fn handle_task_list_picker_open(mut model: Model) -> (Model, Vec<Cmd>) {
    if matches!(
        model.top(),
        Some(Screen::Tasks {
            due_today: Some(_),
            ..
        })
    ) {
        return open_detail_with_picker(model);
    }
    let Some(Screen::Detail {
        instance,
        project_id,
        task_id,
        task,
        overlay,
        rendered_width,
        ..
    }) = model.top_mut()
    else {
        return (model, vec![]);
    };
    if !matches!(overlay, DetailOverlay::None) {
        return (model, vec![]);
    }
    let cmd = Cmd::LoadTaskLists {
        instance: instance.clone(),
        project_id: *project_id,
        task_id: *task_id,
    };
    *overlay = DetailOverlay::TaskListPicker(TaskListPicker {
        candidates: vec![],
        current_id: task.get("task_list_id").and_then(|v| v.as_i64()),
        filter: String::new(),
        selected: 0,
        status: EditStatus::Editing,
        load: TaskListLoad::Loading,
        error: None,
    });
    *rendered_width = usize::MAX;
    (model, vec![cmd])
}

/// `l` on the mine screen: open the selected task's detail as Enter does, and
/// have its task list picker open once the task has loaded.
fn open_detail_with_picker(model: Model) -> (Model, Vec<Cmd>) {
    let (mut model, cmds) = handle_select(model);
    if !cmds.is_empty() {
        if let Some(Screen::Detail { overlay, .. }) = model.top_mut() {
            *overlay = DetailOverlay::TaskListPickerPending;
        }
    }
    (model, cmds)
}

/// Apply a task-lists read to the open picker when it still waits for the Detail
/// task it was read for; any other result is stale and dropped.
fn handle_task_lists_loaded(
    mut model: Model,
    (loaded_instance, loaded_project, loaded_task): (&str, i64, i64),
    lists: Result<Vec<(i64, String)>, String>,
) -> Model {
    if let Some(Screen::Detail {
        instance,
        project_id,
        task_id,
        overlay,
        rendered_width,
        ..
    }) = model.top_mut()
    {
        let same_task =
            instance == loaded_instance && *project_id == loaded_project && *task_id == loaded_task;
        let waiting = overlay
            .task_list_picker_mut()
            .filter(|picker| same_task && picker.load == TaskListLoad::Loading);
        if let Some(picker) = waiting {
            match lists {
                Ok(candidates) => {
                    picker.selected = assignee_candidate_index(&candidates, picker.current_id);
                    picker.candidates = candidates;
                    picker.load = TaskListLoad::Ready;
                }
                Err(message) => picker.load = TaskListLoad::Failed(message),
            }
            *rendered_width = usize::MAX;
        }
    }
    model
}

/// Move the task to the highlighted list: set the picker to `Submitting` and emit
/// `Cmd::SubmitTaskEdit` carrying only `task_list_id`. Choosing the task's current
/// list closes the picker without a write.
fn handle_task_list_picker_submit(model: Model) -> (Model, Vec<Cmd>) {
    let Some((instance, project_id, task_id, list_id)) = picked_candidate(&model, TASK_LIST_PICKER)
    else {
        return (model, vec![]);
    };
    let current_id = match model.top() {
        Some(Screen::Detail { overlay, .. }) => {
            overlay.task_list_picker().and_then(|p| p.current_id)
        }
        _ => None,
    };
    if current_id == Some(list_id) {
        return (close_detail_overlay(model), vec![]);
    }
    let mut model = mark_picker_submitting(model, TASK_LIST_PICKER);
    if let Some(Screen::Detail { overlay, .. }) = model.top_mut() {
        if let Some(picker) = overlay.task_list_picker_mut() {
            picker.error = None;
        }
    }
    (
        model,
        vec![Cmd::SubmitTaskEdit {
            instance,
            project_id,
            task_id,
            completion: None,
            assignee_id: None,
            estimate: None,
            task_list_id: Some(list_id),
        }],
    )
}

/// Initial browse boot: emits Cmd::LoadTasksByProject and marks loading (or revalidating).
///
/// Accepts an optional seed snapshot so the shell can pass a warm cache entry.
/// A non-empty seed seeds the Projects list immediately while always dispatching
/// Cmd::LoadTasksByProject for revalidation.  A cold start (None or empty) produces
/// the classic loading placeholder.
///
/// Called by the shell once at startup, not on every event — keeps the
/// shell minimal while the effect intent is declared in the pure layer.
pub fn init_browse(header: Header, seed: Option<Vec<ProjectGroup>>) -> (Model, Vec<Cmd>) {
    let model = Model::browse(header, seed);
    (model, vec![Cmd::LoadTasksByProject])
}

/// Build the initial mine model and the init commands for the mine TUI.
///
/// Accepts an optional snapshot seed so the shell can pass a warm cache entry.
/// A non-empty seed paints the task list immediately (`loading=false`,
/// `revalidating=true`) while revalidation runs in the background.
/// A cold start (`None` or empty) falls back to `loading=true`.
///
/// ALWAYS emits `Cmd::LoadMineTasks` so the mine list is revalidated on every
/// entry, regardless of whether a snapshot was present.
#[cfg(test)]
pub fn init_mine(header: Header, seed: Option<Vec<MineTableRow>>) -> (Model, Vec<Cmd>) {
    init_mine_on(header, seed, chrono::Local::now().date_naive())
}

/// `init_mine` with an explicit "today" for the due-date buckets, so tests
/// inject the date.
#[cfg(test)]
pub fn init_mine_on(
    header: Header,
    seed: Option<Vec<MineTableRow>>,
    today: chrono::NaiveDate,
) -> (Model, Vec<Cmd>) {
    init_mine_in_view(header, seed, today, MineView::List)
}

/// `init_mine_on` opening in the given view.
pub fn init_mine_in_view(
    header: Header,
    seed: Option<Vec<MineTableRow>>,
    today: chrono::NaiveDate,
    view: MineView,
) -> (Model, Vec<Cmd>) {
    let (tasks, loading, revalidating) = match seed {
        Some(rows) if !rows.is_empty() => (
            order_by_bucket(rows_to_task_rows(rows), Some(today)),
            false,
            true,
        ),
        _ => (vec![], true, false),
    };
    let kanban = view.layout().map(|layout| KanbanFocus {
        layout,
        ..KanbanFocus::first_populated(&tasks)
    });
    let model = Model {
        stack: vec![Screen::Tasks {
            project_name: t("My Tasks"),
            tasks,
            kanban,
            view_modal: None,
            selected: 0,
            loading,
            revalidating,
            stale_after_edit: false,
            card_heights: Vec::new(),
            card_offsets: Vec::new(),
            rendered_width: usize::MAX,
            due_today: Some(today),
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
        report: None,
    };
    (model, vec![Cmd::LoadMineTasks])
}

const NEAR_DUE_WINDOW_DAYS: i64 = 3;

/// Map an optional `YYYY-MM-DD` due date to a label translated into the active language, plus a style.
///
/// Pure and total: no side effects, no I/O. Accepts an injected `today` so callers
/// remain deterministic in tests.
pub fn relative_due(due_on: Option<&str>, today: chrono::NaiveDate) -> (String, DueStyle) {
    use chrono::NaiveDate;

    let Some(s) = due_on else {
        return (t("no due date"), DueStyle::None);
    };

    let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") else {
        return (t("no due date"), DueStyle::None);
    };

    let delta = (date - today).num_days();

    match delta {
        0 => (t("today"), DueStyle::Near),
        1 => (t("tomorrow"), DueStyle::Near),
        2..=NEAR_DUE_WINDOW_DAYS => {
            let day_word = day_word(delta);
            (
                format!("{} {} {}", t("due in"), delta, day_word),
                DueStyle::Near,
            )
        }
        d if d > NEAR_DUE_WINDOW_DAYS => {
            let day_word = day_word(d);
            (
                format!("{} {} {}", t("due in"), d, day_word),
                DueStyle::Normal,
            )
        }
        -1 => (
            format!("{} 1 {}", t("overdue"), t("day")),
            DueStyle::Overdue,
        ),
        d => {
            let abs_d = d.unsigned_abs() as i64;
            (
                format!("{} {} {}", t("overdue"), abs_d, t("days")),
                DueStyle::Overdue,
            )
        }
    }
}

fn day_word(n: i64) -> String {
    if n == 1 {
        t("day")
    } else {
        t("days")
    }
}

#[cfg(test)]
#[path = "../../tests/unit/app.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/model.rs"]
mod model_tests;
