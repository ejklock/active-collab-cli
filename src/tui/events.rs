use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};

use super::model::Msg;

pub fn map_browse_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    match key.code {
        KeyCode::Char('q') => Some(Msg::Quit),
        KeyCode::Esc | KeyCode::Char('b') => Some(Msg::Back),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        KeyCode::Char('r') => Some(Msg::Refresh),
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::PageUp => Some(Msg::PageUp),
        KeyCode::PageDown => Some(Msg::PageDown),
        KeyCode::Enter => Some(Msg::Select),
        KeyCode::Char('d') => Some(Msg::TogglePendingDownload),
        KeyCode::Char(c) if c.is_ascii_digit() => Some(Msg::AssetOpen(c)),
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
