use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};

use super::model::Msg;

pub fn map_browse_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    match key.code {
        KeyCode::Char('q') => Some(Msg::Quit),
        KeyCode::Esc | KeyCode::Char('b') => Some(Msg::Back),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        KeyCode::Char('c') => Some(Msg::ComposeOpen),
        KeyCode::Char('r') => Some(Msg::Refresh),
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::PageUp => Some(Msg::PageUp),
        KeyCode::PageDown => Some(Msg::PageDown),
        KeyCode::Enter => Some(Msg::Select),
        _ => None,
    }
}

/// Map a key event when compose mode is active.
///
/// Ctrl+C quits; Ctrl+S submits; Esc cancels; Enter inserts a newline;
/// Backspace deletes the last character; a plain printable character appends.
/// All other combinations (including Alt+key) produce None.
pub fn map_compose_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char('c') if ctrl => Some(Msg::Quit),
        KeyCode::Char('s') if ctrl => Some(Msg::ComposeSubmit),
        KeyCode::Esc => Some(Msg::ComposeCancel),
        KeyCode::Enter => Some(Msg::ComposeNewline),
        KeyCode::Backspace => Some(Msg::ComposeBackspace),
        KeyCode::Char(c) if !ctrl && !alt => Some(Msg::ComposeInput(c)),
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
