use crate::i18n::t;
use crate::render::Asset;
use crate::tui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Draw the Detail screen (task body + scroll + optional assets panel) into `area`.
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
        let panel_height = (assets.len() + 2).min(8) as u16;
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

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .scroll((offset as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_assets_panel(frame: &mut Frame, area: ratatui::layout::Rect, assets: &[Asset]) {
    let panel_title = format!(" {} ", t("Artifacts"));
    let rows: Vec<Line> = assets
        .iter()
        .enumerate()
        .map(|(i, asset)| Line::styled(format!("[{}] {}", i + 1, asset.name), theme::asset_style()))
        .collect();

    let panel = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::ALL)
            .title(panel_title)
            .title_style(theme::header_style()),
    );

    frame.render_widget(panel, area);
}
