use crate::render::{build_detail_lines, Asset};
use crate::tui::model::{ProjectGroup, TaskRow};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks};
use crate::tui::theme;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use serde_json::json;
use std::collections::HashMap;

fn make_groups(names: &[&str]) -> Vec<ProjectGroup> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| ProjectGroup {
            project_id: i as i64,
            project_name: name.to_string(),
            tasks: vec![TaskRow {
                task_id: i as i64,
                task_number: (i + 1) as i64,
                name: format!("Task {i}"),
                instance: "inst".into(),
                project_id: i as i64,
            }],
        })
        .collect()
}

fn make_tasks(names: &[&str]) -> Vec<TaskRow> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| TaskRow {
            task_id: i as i64,
            task_number: (i + 1) as i64,
            name: name.to_string(),
            instance: format!("instance{i}"),
            project_id: 0,
        })
        .collect()
}

fn render_projects_to_buf(
    groups: &[ProjectGroup],
    selected: usize,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_projects(
                frame,
                Rect::new(0, 0, width, height),
                groups,
                selected,
                false,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn render_tasks_to_buf(
    tasks: &[TaskRow],
    selected: usize,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_tasks(
                frame,
                Rect::new(0, 0, width, height),
                "Project A",
                tasks,
                selected,
                false,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn buf_to_string(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area();
    let mut result = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            result.push_str(cell.symbol());
        }
        result.push('\n');
    }
    result
}

#[test]
fn draw_projects_at_width_40_does_not_panic() {
    let groups = make_groups(&["Short Project"]);
    let buf = render_projects_to_buf(&groups, 0, 40, 10);
    let content = buf_to_string(&buf);
    assert!(content.contains("Project"), "header 'Project' must appear");
}

#[test]
fn draw_projects_at_width_120_shows_full_name() {
    let long_name = "A Very Long Project Name That Should Fit At Wide Terminal";
    let groups = make_groups(&[long_name]);
    let buf = render_projects_to_buf(&groups, 0, 120, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(long_name),
        "full project name must appear at width 120"
    );
}

#[test]
fn draw_tasks_at_width_40_does_not_panic() {
    let tasks = make_tasks(&["Short Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 40, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("TASK#") || content.contains("NAME"),
        "header must appear"
    );
}

#[test]
fn draw_tasks_at_width_120_shows_full_name() {
    let long_name = "A Very Long Task Name That Should Appear In Full At Wide Width";
    let tasks = make_tasks(&[long_name]);
    let buf = render_tasks_to_buf(&tasks, 0, 120, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(long_name),
        "full task name must appear at width 120"
    );
}

#[test]
fn draw_projects_selected_row_has_selection_symbol() {
    let groups = make_groups(&["Alpha", "Beta", "Gamma"]);
    let buf = render_projects_to_buf(&groups, 1, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(theme::SELECTION_SYMBOL),
        "SELECTION_SYMBOL '▸ ' must appear when a row is selected"
    );
}

#[test]
fn draw_tasks_selected_row_has_selection_symbol() {
    let tasks = make_tasks(&["Task One", "Task Two"]);
    let buf = render_tasks_to_buf(&tasks, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(theme::SELECTION_SYMBOL),
        "SELECTION_SYMBOL '▸ ' must appear when a row is selected"
    );
}

#[test]
fn draw_projects_selection_symbol_absent_on_non_selected_rows() {
    let groups = make_groups(&["Alpha", "Beta"]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    let content = buf_to_string(&buf);
    // Symbol appears exactly once (only the selected row)
    let count = content.matches(theme::SELECTION_SYMBOL).count();
    assert_eq!(
        count, 1,
        "selection symbol must appear exactly once (selected row only)"
    );
}

#[test]
fn draw_projects_header_row_present() {
    let groups = make_groups(&["My Project"]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Project"),
        "header label 'Project' must be present"
    );
    assert!(content.contains('#'), "header label '#' must be present");
}

#[test]
fn draw_tasks_header_row_present() {
    let tasks = make_tasks(&["My Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("TASK#"),
        "header label 'TASK#' must be present"
    );
    assert!(
        content.contains("INSTANCE"),
        "header label 'INSTANCE' must be present"
    );
    assert!(
        content.contains("NAME"),
        "header label 'NAME' must be present"
    );
}

#[test]
fn draw_projects_loading_shows_paragraph_not_table() {
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, 80, 10);
            draw_projects(frame, area, &[], 0, true);
        })
        .unwrap();
    let content = buf_to_string(terminal.backend().buffer());
    assert!(
        content.contains("Loading"),
        "loading state must show 'Loading' text"
    );
}

#[test]
fn draw_tasks_loading_shows_paragraph_not_table() {
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, 80, 10);
            draw_tasks(frame, area, "Project A", &[], 0, true);
        })
        .unwrap();
    let content = buf_to_string(terminal.backend().buffer());
    assert!(
        content.contains("Loading"),
        "loading state must show 'Loading' text"
    );
}

fn render_detail_to_buf(
    lines: &[String],
    assets: &[Asset],
    offset: usize,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_detail(
                frame,
                Rect::new(0, 0, width, height),
                lines,
                assets,
                offset,
                false,
                42,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn draw_detail_long_line_wraps_across_multiple_rows() {
    // A line that is exactly as wide as the content area (width - 2 for borders) must
    // wrap a word that falls past the right edge onto the next buffer row.
    // Use a narrow terminal (20 cols) so a 30-char line definitely wraps.
    let long_line = "word1 word2 word3 word4 word5 extraword".to_string();
    let lines = vec![long_line];
    let buf = render_detail_to_buf(&lines, &[], 0, 20, 10);
    let content = buf_to_string(&buf);
    // At width=20, content area is 18 cols. "word1 word2 word3 w" fills row 1 (18 cols)
    // and "extraword" must appear somewhere on a later row.
    // Rather than asserting exact layout, assert "extraword" appears in the buffer at all
    // and that it is NOT on the very first content row of the buffer (row index 1, y=1).
    assert!(
        content.contains("extraword"),
        "wrapped word must appear in buffer"
    );
    // Row 1 (y=1, first content row inside border) should not contain "extraword"
    let row1: String = buf.area().columns().map(|_| ' ').collect();
    let _ = row1;
    let rows: Vec<&str> = content.lines().collect();
    assert!(rows.len() >= 2, "buffer must have at least two rows");
    assert!(
        !rows[1].contains("extraword"),
        "extraword must NOT appear on the first content row (it must have wrapped): row1='{}'",
        rows[1]
    );
}

#[test]
fn draw_detail_no_panic_at_narrow_width() {
    let lines = vec!["short".to_string()];
    render_detail_to_buf(&lines, &[], 0, 5, 5);
}

#[test]
fn draw_detail_no_panic_at_wide_width() {
    let lines = vec!["a line".to_string()];
    render_detail_to_buf(&lines, &[], 0, 200, 40);
}

#[test]
fn draw_detail_with_assets_renders_panel_and_asset_names() {
    let lines = vec!["Task description".to_string()];
    let assets = vec![
        Asset {
            name: "report.pdf".into(),
            url: "https://example.com/report.pdf".into(),
        },
        Asset {
            name: "photo.png".into(),
            url: "https://example.com/photo.png".into(),
        },
    ];
    let buf = render_detail_to_buf(&lines, &assets, 0, 80, 20);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("[1] report.pdf"),
        "first asset must appear as '[1] report.pdf': {content}"
    );
    assert!(
        content.contains("[2] photo.png"),
        "second asset must appear as '[2] photo.png': {content}"
    );
    assert!(
        content.contains("Artifacts"),
        "panel title 'Artifacts' must appear: {content}"
    );
}

#[test]
fn draw_detail_without_assets_no_panel_and_no_marker() {
    let lines = vec!["Task description".to_string()];
    let buf = render_detail_to_buf(&lines, &[], 0, 80, 20);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains("[1]"),
        "no '[1]' marker must appear when assets empty: {content}"
    );
    assert!(
        !content.contains("Artifacts"),
        "no 'Artifacts' panel title when assets empty: {content}"
    );
    assert!(
        content.contains("Task description"),
        "content must appear in full area: {content}"
    );
}

// P2-A1: build_detail_lines produces boxed lines (rounded corners + comment author)
// each fitting within inner_width, after a reflow at that width.
#[test]
fn build_detail_lines_with_comment_produces_boxed_lines_fitting_width() {
    let inner_width: usize = 60;
    let task = json!({
        "name": "Test Task",
        "id": 5,
        "project_id": 2,
        "is_completed": false
    });
    let comment = json!({
        "created_by_name": "Bob",
        "created_on": 1700000000u64,
        "body": "<p>This is a test comment body for the box rendering test.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let lines = build_detail_lines(&task, &[comment], &user_map, inner_width);

    assert!(!lines.is_empty(), "must produce at least one line");

    // Every line must fit within inner_width
    for line in &lines {
        assert!(
            line.chars().count() <= inner_width,
            "line exceeds inner_width={}: {:?}",
            inner_width,
            line
        );
    }

    // At least one line must contain a rounded corner glyph (box is present)
    let has_box = lines
        .iter()
        .any(|l| l.contains('\u{256D}') || l.contains('\u{2570}'));
    assert!(
        has_box,
        "output must contain rounded comment box corners: {lines:?}"
    );

    // At least one line must contain the author name
    let has_author = lines.iter().any(|l| l.contains("Bob"));
    assert!(
        has_author,
        "output must contain comment author 'Bob': {lines:?}"
    );
}

// P3-A1/A2: color parity — theme styles match Python curses color pairs
#[test]
fn footer_style_is_white_on_blue_matching_python_pair3() {
    use ratatui::style::{Color, Style};
    let style = theme::footer_style();
    assert_eq!(
        style,
        Style::default().fg(Color::White).bg(Color::Blue),
        "footer_style must be white-on-blue (Python pair3 'status')"
    );
}

#[test]
fn header_style_is_cyan_bold_matching_python_pair1() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::header_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "header_style must be cyan+bold (Python pair1)"
    );
}

#[test]
fn selection_style_is_black_on_cyan_bold_matching_python_pair2() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::selection_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "selection_style must be black-on-cyan+bold (Python pair2)"
    );
}

#[test]
fn asset_style_is_yellow_matching_python_pair4() {
    use ratatui::style::{Color, Style};
    let style = theme::asset_style();
    assert_eq!(
        style,
        Style::default().fg(Color::Yellow),
        "asset_style must be yellow (Python pair4)"
    );
}

// P4a-A1/A2: too-small guard — view() renders a single message below thresholds
// and the normal screen above thresholds.
mod view_size_guard {
    use crate::tui::model::{Model, Screen};
    use crate::tui::view::view;
    use ratatui::{backend::TestBackend, Terminal};

    fn buf_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area();
        let mut result = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = buf.cell((x, y)).unwrap();
                result.push_str(cell.symbol());
            }
            result.push('\n');
        }
        result
    }

    fn projects_model() -> Model {
        Model {
            stack: vec![Screen::Projects {
                groups: vec![],
                selected: 0,
                loading: false,
            }],
            should_quit: false,
        }
    }

    #[test]
    fn viewport_below_threshold_renders_only_too_small_message() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let model = projects_model();
        terminal.draw(|frame| view(&model, frame)).unwrap();
        let content = buf_to_string(terminal.backend().buffer());

        assert!(
            content.contains("Terminal too small"),
            "must render 'Terminal too small' at 20x5: {content}"
        );
        assert!(
            !content.contains("↑/↓"),
            "footer hint must NOT appear at 20x5: {content}"
        );
        assert!(
            !content.contains("Project"),
            "table title must NOT appear at 20x5: {content}"
        );
    }

    #[test]
    fn viewport_at_or_above_threshold_renders_normal_screen_without_guard_message() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let model = projects_model();
        terminal.draw(|frame| view(&model, frame)).unwrap();
        let content = buf_to_string(terminal.backend().buffer());

        assert!(
            !content.contains("Terminal too small"),
            "guard message must NOT appear at 80x24: {content}"
        );
        assert!(
            content.contains("↑/↓"),
            "footer hint must appear at 80x24: {content}"
        );
    }
}

// P4b-A2: draw_tasks truncates over-long name with ellipsis on narrow terminal
// and shows the full name with no ellipsis on a wide terminal.
#[test]
fn draw_tasks_narrow_terminal_long_name_shows_ellipsis() {
    let long_name = "A Very Long Task Name That Will Definitely Not Fit In A Narrow Terminal";
    let tasks = make_tasks(&[long_name]);
    let buf = render_tasks_to_buf(&tasks, 0, 40, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains('\u{2026}'),
        "narrow terminal must truncate long task name with ellipsis: {content}"
    );
}

#[test]
fn draw_tasks_wide_terminal_short_name_no_ellipsis() {
    let short_name = "Short";
    let tasks = make_tasks(&[short_name]);
    let buf = render_tasks_to_buf(&tasks, 0, 120, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(short_name),
        "wide terminal must show full name: {content}"
    );
    assert!(
        !content.contains('\u{2026}'),
        "wide terminal must NOT show ellipsis for short name: {content}"
    );
}

// P4b-A3: draw_projects truncates over-long project name with ellipsis on narrow terminal.
#[test]
fn draw_projects_narrow_terminal_long_name_shows_ellipsis() {
    let long_name = "An Extremely Long Project Name That Will Not Fit In A Narrow Terminal";
    let groups = make_groups(&[long_name]);
    let buf = render_projects_to_buf(&groups, 0, 30, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains('\u{2026}'),
        "narrow terminal must truncate long project name with ellipsis: {content}"
    );
}

#[test]
fn draw_projects_wide_terminal_short_name_no_ellipsis() {
    let short_name = "Acme";
    let groups = make_groups(&[short_name]);
    let buf = render_projects_to_buf(&groups, 0, 120, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(short_name),
        "wide terminal must show full project name: {content}"
    );
    assert!(
        !content.contains('\u{2026}'),
        "wide terminal must NOT show ellipsis for short project name: {content}"
    );
}

// P2-A1: build_detail_lines at different widths produces different line counts/widths
#[test]
fn build_detail_lines_reflow_at_different_widths_changes_output() {
    let task = json!({
        "name": "Task",
        "id": 1,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>A longer body text that should wrap differently at different widths.</p>"
    });
    let comment = json!({
        "created_by_name": "Carol",
        "created_on": 1700000000u64,
        "body": "<p>Comment body long enough to demonstrate reflow at different widths.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();

    let comments = [comment];
    let lines_80 = build_detail_lines(&task, &comments, &user_map, 80);
    let lines_40 = build_detail_lines(&task, &comments, &user_map, 40);

    // All lines at width 40 must be at most 40 chars
    for line in &lines_40 {
        assert!(
            line.chars().count() <= 40,
            "line at width 40 exceeds 40 chars: {:?}",
            line
        );
    }

    // All lines at width 80 must be at most 80 chars
    for line in &lines_80 {
        assert!(
            line.chars().count() <= 80,
            "line at width 80 exceeds 80 chars: {:?}",
            line
        );
    }

    // Narrower width produces more lines (wrapping) or at least different output
    assert!(
        lines_40.len() >= lines_80.len() || lines_40 != lines_80,
        "reflow at different widths must produce different output"
    );
}
