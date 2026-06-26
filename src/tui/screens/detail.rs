use crate::i18n::t;
use crate::render::Asset;
use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Draw the Detail screen as a single scrollable content block with an optional
/// fixed Artifacts panel below.
///
/// When `assets` is non-empty the area is split vertically into a content chunk
/// (Min(0)) and a fixed panel chunk (Length capped at 8). Otherwise the full
/// area goes to content.
pub fn draw_detail(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    lines: &[String],
    assets: &[Asset],
    offset: usize,
    loading: bool,
    task_id: i64,
) {
    let title = format!(" {} #{} ", t("Task"), task_id);

    if loading {
        let msg = Paragraph::new(t("Loading…"))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(msg, area);
        return;
    }

    if assets.is_empty() {
        render_content(frame, area, lines, offset, title);
    } else {
        let panel_height = (assets.len() as u16 + 2).min(8);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(panel_height)])
            .split(area);
        render_content(frame, chunks[0], lines, offset, title);
        render_assets_panel(frame, chunks[1], assets);
    }
}

fn render_content(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    lines: &[String],
    offset: usize,
    title: String,
) {
    let text: Text = Text::from(
        lines
            .iter()
            .map(|l| Line::from(l.clone()))
            .collect::<Vec<_>>(),
    );

    let block = Block::default().borders(Borders::ALL).title(title);
    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((offset as u16, 0));

    frame.render_widget(paragraph, area);

    let viewport_height = area.height.saturating_sub(2) as usize;
    let total_content = lines.len();
    if total_content > viewport_height {
        let mut scrollbar_state = ScrollbarState::new(total_content).position(offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_assets_panel(frame: &mut Frame, area: ratatui::layout::Rect, assets: &[Asset]) {
    let panel_title = format!(" {} ", t("Artifacts"));
    let rows: Vec<Line> = assets
        .iter()
        .enumerate()
        .map(|(i, asset)| {
            Line::styled(
                format!("{} {}: {}", t("Attachment"), i + 1, asset.name),
                theme::asset_style(),
            )
        })
        .collect();

    let panel = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::ALL)
            .title(panel_title)
            .title_style(theme::header_style()),
    );

    frame.render_widget(panel, area);
}
