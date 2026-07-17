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
    /// Fetch and decode the image bytes for an opened image-viewer asset.
    /// Handled entirely by the shell (ADR 0065); the pure Model never sees bytes.
    LoadImage {
        asset: ImageAssetRef,
    },
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
/// fetches bytes from (slice 0059). Carries no protocol/bytes state (ADR 0065).
#[derive(Debug, Clone, PartialEq)]
pub struct ImageAssetRef {
    pub url: String,
    pub label: String,
}

/// Lifecycle phase of the image-viewer overlay.
///
/// The pure Model tracks only this lifecycle; the decoded bytes and the
/// `ratatui-image` `StatefulProtocol` live in a shell-owned side table,
/// never here (ADR 0065).
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
/// its ancestors `DetailOverlay`/`Screen`) drop that derive (ADR 0064).
#[derive(Debug, Clone)]
pub struct Compose {
    pub kind: ComposeKind,
    pub editor: TextArea<'static>,
    pub status: ComposeStatus,
}

/// The active modal overlay on the Detail read view.
///
/// Compose and the delete prompt are mutually exclusive by construction — only one overlay
/// at a time. The combined state (both active) cannot be constructed with this enum,
/// replacing the previous two independent `Option` fields (ADR 0047).
///
/// `Compose` is intentionally larger than `ConfirmDelete`: it carries a `TextArea` with
/// caret/selection/undo history (ADR 0064). Construction happens only on open/edit, never
/// per-keystroke, so boxing would add indirection with no practical benefit — same
/// rationale as `Screen`'s `large_enum_variant` allow below.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum DetailOverlay {
    None,
    Compose(Compose),
    ConfirmDelete {
        comment_id: i64,
    },
    ImageViewer {
        asset: ImageAssetRef,
        status: ImageStatus,
    },
}

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
        /// Per-card heights (rows) computed by reflow_tasks; mirrors Detail's `lines` cache.
        card_heights: Vec<u16>,
        /// Prefix-sum y-offsets: card_offsets[i] is the cumulative row start of card i;
        /// card_offsets[n] is the total row count. u32 avoids the u16 saturation ceiling.
        card_offsets: Vec<u32>,
        /// Width (card_inner_w) at which the cache was last built; usize::MAX means not-yet-built.
        rendered_width: usize,
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
        /// Mutually exclusive by construction — only one overlay at a time (ADR 0047).
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
            tasks, selected, ..
        } => {
            let row = tasks.get(*selected)?;
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
}

/// All messages the update function understands.
pub enum Msg {
    Up,
    Down,
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
    /// shell (events.rs) converts the crossterm `KeyEvent` to `tui_textarea::Input`;
    /// `update()` applies it via `TextArea::input` (ADR 0064).
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
    /// Sent only by the shell wiring landing in slice 0059 (ADR 0065); this
    /// slice's `update_image` handles the transition but nothing produces the
    /// message yet, so tests construct it directly.
    #[allow(dead_code)]
    ImageLoaded,
    /// The shell failed to fetch or decode the open viewer's image.
    ///
    /// See `ImageLoaded` above — same slice-0059 wiring boundary.
    #[allow(dead_code)]
    ImageLoadErr(String),
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

    fn top_mut(&mut self) -> Option<&mut Screen> {
        self.stack.last_mut()
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
    /// The cache depends only on `tasks` and `card_inner_w`, never on `selected`.
    pub fn reflow_tasks(&mut self, card_inner_w: usize) {
        let Some(Screen::Tasks {
            tasks,
            loading,
            card_heights,
            card_offsets,
            rendered_width,
            ..
        }) = self.top_mut()
        else {
            return;
        };

        if *loading || *rendered_width == card_inner_w {
            return;
        }

        *card_heights = tasks
            .iter()
            .map(|t| crate::tui::task_layout::card_height(t, card_inner_w))
            .collect();

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
    match msg {
        m @ (Msg::Up
        | Msg::ScrollUp
        | Msg::Down
        | Msg::ScrollDown
        | Msg::PageUp
        | Msg::PageDown) => update_scroll(model, m),
        m @ (Msg::Click { .. } | Msg::Drag { .. } | Msg::MouseUp { .. }) => {
            update_pointer(model, m)
        }
        m @ (Msg::Select | Msg::Back | Msg::Quit | Msg::Refresh) => update_navigation(model, m),
        m @ (Msg::LoadedTasksByProject { .. }
        | Msg::LoadedMineTasks { .. }
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
        Msg::FocusNextComment => (handle_focus_next(model), vec![]),
        Msg::FocusPrevComment => (handle_focus_prev(model), vec![]),
        Msg::ConfirmDeleteComment => handle_confirm_delete(model),
        Msg::CancelDeleteComment => (handle_cancel_delete(model), vec![]),
        m @ (Msg::ImageLoaded | Msg::ImageLoadErr(_)) => update_image(model, m),
    }
}

/// Apply the shell's image-fetch outcome to an open `ImageViewer` overlay.
///
/// No-op when the overlay is not `ImageViewer` (e.g. the user already closed
/// it before the fetch settled).
fn update_image(mut model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    let new_status = match msg {
        Msg::ImageLoaded => Some(ImageStatus::Ready),
        Msg::ImageLoadErr(reason) => Some(ImageStatus::Error(reason)),
        _ => None,
    };
    let Some(new_status) = new_status else {
        return (model, vec![]);
    };
    if let Some(Screen::Detail {
        ref mut overlay, ..
    }) = model.top_mut()
    {
        if let Some(status) = overlay.image_viewer_status_mut() {
            *status = new_status;
        }
    }
    (model, vec![])
}

fn update_scroll(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
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
        Msg::Back => (handle_back(model), vec![]),
        Msg::Quit => (handle_quit(model), vec![]),
        Msg::Refresh => handle_refresh(model),
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
        Msg::LoadedDetail(load) => (handle_loaded_detail(model, load), vec![]),
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
        Msg::ComposeCancel => (handle_compose_cancel(model), vec![]),
        Msg::CommentMutationOk => handle_comment_mutation_ok(model),
        Msg::CommentMutationErr(msg) => (handle_comment_mutation_err(model, msg), vec![]),
        Msg::AuthExpired => (handle_auth_expired(model), vec![]),
        _ => (model, vec![]),
    }
}

fn handle_up(model: Model) -> Model {
    match model.top() {
        Some(Screen::Detail { .. }) => handle_focus_prev(model),
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
        Some(_) => (handle_cancel_delete(model), vec![]),
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
                    card_heights: Vec::new(),
                    card_offsets: Vec::new(),
                    rendered_width: usize::MAX,
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
            return handle_cancel_delete(model);
        }
        if overlay.is_image_viewer() {
            return handle_close_image_viewer(model);
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

fn handle_quit(mut model: Model) -> Model {
    // When the image viewer is open, q closes it instead of quitting the app.
    if let Some(Screen::Detail { overlay, .. }) = model.top() {
        if overlay.is_image_viewer() {
            return handle_close_image_viewer(model);
        }
    }
    model.should_quit = true;
    model
}

/// Set `overlay = None`, closing the image viewer (Esc/q).
fn handle_close_image_viewer(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::None;
        *rendered_width = usize::MAX;
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
        ..
    }) = model.top_mut()
    {
        *t = rows_to_task_rows(rows);
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
        })
        .collect()
}

fn handle_loaded_detail(mut model: Model, load: DetailLoad) -> Model {
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
    model
}

/// Set auth_error on the Detail screen without disturbing the compose buffer.
///
/// Called when a comment mutation returns HTTP 401. The user keeps their draft.
fn handle_auth_expired(mut model: Model) -> Model {
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

fn handle_compose_cancel(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::None;
        *rendered_width = usize::MAX;
    }
    model
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

/// Dismiss the confirm prompt without deleting.
fn handle_cancel_delete(mut model: Model) -> Model {
    if let Some(Screen::Detail {
        ref mut overlay,
        ref mut rendered_width,
        ..
    }) = model.top_mut()
    {
        *overlay = DetailOverlay::None;
        *rendered_width = usize::MAX;
    }
    model
}

fn handle_comment_mutation_ok(mut model: Model) -> (Model, Vec<Cmd>) {
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
pub fn init_mine(header: Header, seed: Option<Vec<MineTableRow>>) -> (Model, Vec<Cmd>) {
    let (tasks, loading, revalidating) = match seed {
        Some(rows) if !rows.is_empty() => (rows_to_task_rows(rows), false, true),
        _ => (vec![], true, false),
    };
    let model = Model {
        stack: vec![Screen::Tasks {
            project_name: t("My Tasks"),
            tasks,
            selected: 0,
            loading,
            revalidating,
            card_heights: Vec::new(),
            card_offsets: Vec::new(),
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        modal_button_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    (model, vec![Cmd::LoadMineTasks])
}

const NEAR_DUE_WINDOW_DAYS: i64 = 3;

/// Map an optional `YYYY-MM-DD` due date to a Brazilian-Portuguese label and style.
///
/// Pure and total: no side effects, no I/O. Accepts an injected `today` so callers
/// remain deterministic in tests.
pub fn relative_due(due_on: Option<&str>, today: chrono::NaiveDate) -> (String, DueStyle) {
    use chrono::NaiveDate;

    let Some(s) = due_on else {
        return (t("due_none"), DueStyle::None);
    };

    let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") else {
        return (t("due_none"), DueStyle::None);
    };

    let delta = (date - today).num_days();

    match delta {
        0 => (t("due_today"), DueStyle::Near),
        1 => (t("due_tomorrow"), DueStyle::Near),
        2..=NEAR_DUE_WINDOW_DAYS => {
            let day_word = day_word(delta);
            (
                format!("{} {} {}", t("due_in"), delta, day_word),
                DueStyle::Near,
            )
        }
        d if d > NEAR_DUE_WINDOW_DAYS => {
            let day_word = day_word(d);
            (
                format!("{} {} {}", t("due_in"), d, day_word),
                DueStyle::Normal,
            )
        }
        -1 => (
            format!("{} 1 {}", t("due_overdue"), t("due_day")),
            DueStyle::Overdue,
        ),
        d => {
            let abs_d = d.unsigned_abs() as i64;
            (
                format!("{} {} {}", t("due_overdue"), abs_d, t("due_days")),
                DueStyle::Overdue,
            )
        }
    }
}

fn day_word(n: i64) -> String {
    if n == 1 {
        t("due_day")
    } else {
        t("due_days")
    }
}

#[cfg(test)]
#[path = "../../tests/unit/app.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/model.rs"]
mod model_tests;
