use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use tui_textarea::Input;

use super::model::Msg;

/// Map focus-movement and scroll keys for browse mode.
///
/// Handles j/k and ↑/↓ (comment focus on the Detail screen, list selection
/// elsewhere) plus PageUp/PageDown (page scroll).
fn map_focus_or_scroll_key(code: KeyCode) -> Option<Msg> {
    match code {
        KeyCode::Char('j') => Some(Msg::FocusNextComment),
        KeyCode::Char('k') => Some(Msg::FocusPrevComment),
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::PageUp => Some(Msg::PageUp),
        KeyCode::PageDown => Some(Msg::PageDown),
        _ => None,
    }
}

/// Map a key event when the delete-confirm modal is open.
///
/// Enter confirms the delete; Esc cancels it. All other keys are consumed
/// so they do not bleed into the browse key map while the modal is open.
pub fn map_confirm_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    match key.code {
        KeyCode::Enter => Some(Msg::ConfirmDeleteComment),
        KeyCode::Esc => Some(Msg::CancelDeleteComment),
        _ => None,
    }
}

pub fn map_browse_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    map_focus_or_scroll_key(key.code).or_else(|| match key.code {
        KeyCode::Char('q') => Some(Msg::Quit),
        KeyCode::Esc | KeyCode::Char('b') => Some(Msg::Back),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        KeyCode::Char('c') => Some(Msg::ComposeOpen),
        KeyCode::Char('t') => Some(Msg::LogTimeOpen),
        KeyCode::Char('e') => Some(Msg::EstimateOpen),
        KeyCode::Char('s') => Some(Msg::StatusToggleOpen),
        KeyCode::Char('a') => Some(Msg::AssigneePickerOpen),
        KeyCode::Char('r') => Some(Msg::Refresh),
        KeyCode::Enter => Some(Msg::Select),
        _ => None,
    })
}

/// Map a key event when compose mode is active.
///
/// Ctrl+S submits and Esc cancels — the only two keys the shell intercepts before
/// they reach the editor. Every other key (printable chars, Enter, caret movement,
/// Backspace/Delete, undo/redo, ...) converts to the backend-neutral
/// `tui_textarea::Input` and is carried by `Msg::ComposeInput` for `update()` to
/// apply via `TextArea::input` (ADR 0064).
pub fn map_compose_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if ctrl => Some(Msg::ComposeSubmit),
        KeyCode::Esc => Some(Msg::ComposeCancel),
        _ => Some(Msg::ComposeInput(Input::from(key))),
    }
}

/// Map a key event when the log-time modal is active.
///
/// Ctrl+S submits and Esc cancels — the same shell-intercepted shortcuts as compose
/// mode. Tab switches the focused field between Hours and Summary; every other
/// printable char and Backspace edit the focused field's plain-text buffer directly
/// (no `tui_textarea` — each field is a single line, so `update()` applies char/backspace
/// Msgs to the buffer itself rather than routing through a generic `Input`).
pub fn map_log_time_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if ctrl => Some(Msg::LogTimeSubmit),
        KeyCode::Esc => Some(Msg::LogTimeCancel),
        KeyCode::Tab => Some(Msg::LogTimeToggleField),
        KeyCode::Backspace => Some(Msg::LogTimeBackspace),
        KeyCode::Char(c) => Some(Msg::LogTimeChar(c)),
        _ => None,
    }
}

/// Map a key event when the estimate-edit modal is active.
///
/// Ctrl+S submits and Esc cancels — the same shell-intercepted shortcuts as the
/// log-time modal. There is no Tab here: a single field has nothing to switch
/// between, unlike log time's Hours/Summary pair.
pub fn map_estimate_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('s') if ctrl => Some(Msg::EstimateSubmit),
        KeyCode::Esc => Some(Msg::EstimateCancel),
        KeyCode::Backspace => Some(Msg::EstimateBackspace),
        KeyCode::Char(c) => Some(Msg::EstimateChar(c)),
        _ => None,
    }
}

/// Map a key event when the status-confirm modal is active.
///
/// Enter or Ctrl+S confirms the pending status change; Esc cancels it. There is no
/// text entry here — the target is fixed by `StatusToggleOpen`, not typed.
pub fn map_status_confirm_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Enter => Some(Msg::StatusToggleConfirm),
        KeyCode::Char('s') if ctrl => Some(Msg::StatusToggleConfirm),
        KeyCode::Esc => Some(Msg::StatusToggleCancel),
        _ => None,
    }
}

/// Map a key event when the assignee-picker modal is active.
///
/// j/Down and k/Up move the selection; Enter or Ctrl+S confirms the highlighted
/// candidate; Esc cancels. There is no text entry — candidates come from the cached
/// user directory, not typed.
pub fn map_assignee_picker_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => Some(Msg::AssigneePickerDown),
        KeyCode::Up | KeyCode::Char('k') => Some(Msg::AssigneePickerUp),
        KeyCode::Enter => Some(Msg::AssigneePickerSubmit),
        KeyCode::Char('s') if ctrl => Some(Msg::AssigneePickerSubmit),
        KeyCode::Esc => Some(Msg::AssigneePickerCancel),
        _ => None,
    }
}

pub fn map_browse_mouse_event(mouse: crossterm::event::MouseEvent) -> Option<Msg> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(Msg::ScrollUp),
        MouseEventKind::ScrollDown => Some(Msg::ScrollDown),
        MouseEventKind::Down(MouseButton::Left) => Some(Msg::Click {
            column: mouse.column,
            row: mouse.row,
            modifiers: mouse.modifiers,
        }),
        MouseEventKind::Drag(MouseButton::Left) => Some(Msg::Drag {
            column: mouse.column,
            row: mouse.row,
            modifiers: mouse.modifiers,
        }),
        MouseEventKind::Up(MouseButton::Left) => Some(Msg::MouseUp {
            column: mouse.column,
            row: mouse.row,
            modifiers: mouse.modifiers,
        }),
        _ => None,
    }
}
