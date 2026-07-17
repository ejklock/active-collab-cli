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
