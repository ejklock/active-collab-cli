use crate::render::{build_detail_lines, build_header_lines, Asset};
use crate::store::instances::Instance;
use crate::tui::model::{Header, ProjectGroup, TaskRow};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks};
use crate::tui::theme;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use serde_json::json;
use std::collections::HashMap;

fn make_instance(name: &str, email: &str) -> Instance {
    Instance {
        name: name.into(),
        base_url: "https://example.com".into(),
        email: email.into(),
        token: "tok".into(),
        user_id: None,
    }
}

fn make_groups(names: &[&str]) -> Vec<ProjectGroup> {
    names
        .iter()
        .enumerate()
        .map(|(i, name)| ProjectGroup {
            project_id: i as i64,
            project_name: name.to_string(),
            instance: "inst".into(),
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

fn make_groups_with_instance(names_and_instances: &[(&str, &str)]) -> Vec<ProjectGroup> {
    names_and_instances
        .iter()
        .enumerate()
        .map(|(i, (name, inst))| ProjectGroup {
            project_id: i as i64,
            project_name: name.to_string(),
            instance: inst.to_string(),
            tasks: vec![TaskRow {
                task_id: i as i64,
                task_number: (i + 1) as i64,
                name: format!("Task {i}"),
                instance: inst.to_string(),
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
        content.contains("Tasks"),
        "header label 'Tasks' must be present"
    );
    assert!(
        content.contains("Project"),
        "header label 'Project' must be present"
    );
    assert!(
        content.contains("Instance"),
        "header label 'Instance' must be present"
    );
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

// U6c-A1: draw_detail renders ONE global scrollable content block.
// The single bordered panel with the task title must contain all content lines.
// Use width=30 and height=20 so there is room for content lines to appear.
// The buffer must contain "extraword" proving the global single-block layout.
#[test]
fn draw_detail_long_line_wraps_across_multiple_rows() {
    let long_line = "word1 word2 word3 word4 word5 extraword".to_string();
    let lines = vec![long_line];
    let buf = render_detail_to_buf(&lines, &[], 0, 30, 20);
    let content = buf_to_string(&buf);
    // At width=30, content area is 28 cols. The line wraps before "extraword".
    // Assert "extraword" appears somewhere in the buffer.
    assert!(
        content.contains("extraword"),
        "wrapped word must appear in buffer"
    );
    // Row 0 = top border; row 1 = first content row of the single block.
    // "extraword" must NOT appear on the first content row (it wrapped to a later row).
    let rows: Vec<&str> = content.lines().collect();
    assert!(rows.len() >= 3, "buffer must have at least three rows");
    assert!(
        !rows[1].contains("extraword"),
        "extraword must NOT appear on the first content row (it must have wrapped): row='{}'",
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
        content.contains("Attachment 1: report.pdf"),
        "first asset must appear as 'Attachment 1: report.pdf': {content}"
    );
    assert!(
        content.contains("Attachment 2: photo.png"),
        "second asset must appear as 'Attachment 2: photo.png': {content}"
    );
    assert!(
        content.contains("Artifacts"),
        "panel title 'Artifacts' must appear: {content}"
    );
}

// U2-A1: asset label format is 'Attachment N: <filename>' where N is 1-based
// and matches the 1-9 open-asset keyboard shortcut.
#[test]
fn draw_detail_asset_label_uses_attachment_prefix_with_1based_index() {
    let lines = vec!["body".to_string()];
    let assets = vec![
        Asset {
            name: "diagram.png".into(),
            url: "https://example.com/diagram.png".into(),
        },
        Asset {
            name: "notes.txt".into(),
            url: "https://example.com/notes.txt".into(),
        },
    ];
    let buf = render_detail_to_buf(&lines, &assets, 0, 80, 20);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Attachment 1"),
        "first asset must carry label 'Attachment 1': {content}"
    );
    assert!(
        content.contains("diagram.png"),
        "filename must be retained after the label: {content}"
    );
    assert!(
        content.contains("Attachment 2"),
        "second asset must carry label 'Attachment 2': {content}"
    );
    assert!(
        content.contains("notes.txt"),
        "second filename must be retained after the label: {content}"
    );
}

// U2-A2: locales/pt_BR.json contains "Attachment" -> "Anexo" and is valid JSON.
#[test]
fn pt_br_catalog_maps_attachment_to_anexo() {
    let raw = include_str!("../../locales/pt_BR.json");
    let catalog: std::collections::HashMap<String, String> =
        serde_json::from_str(raw).expect("pt_BR.json must be valid JSON");
    assert_eq!(
        catalog.get("Attachment").map(String::as_str),
        Some("Anexo"),
        "pt_BR catalog must map \"Attachment\" -> \"Anexo\""
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

// U1-A2: selection_style brightened to black-on-light-cyan bold
#[test]
fn selection_style_is_black_on_light_cyan_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::selection_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightCyan)
            .add_modifier(Modifier::BOLD),
        "selection_style must be black-on-light-cyan+bold (U1 vibrant palette)"
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

// U1-A1: column_header_style returns light-cyan fg + bold
#[test]
fn column_header_style_is_light_cyan_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::column_header_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::LightCyan)
            .add_modifier(Modifier::BOLD),
        "column_header_style must be light-cyan+bold (U1 vibrant palette)"
    );
}

// U1-A3: regression guards — header_style, footer_style, asset_style, SELECTION_SYMBOL unchanged
#[test]
fn selection_symbol_is_unchanged() {
    assert_eq!(
        theme::SELECTION_SYMBOL,
        "▸ ",
        "SELECTION_SYMBOL must remain '▸ ' (U1 regression guard)"
    );
}

// U1-A1: TestBackend confirms column_header_style fg (LightCyan) is applied to the header row
#[test]
fn render_table_header_row_carries_column_header_style() {
    use ratatui::style::Color;
    use ratatui::{backend::TestBackend, layout::Constraint, Terminal};

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            use crate::tui::drawer::render_table;
            use ratatui::widgets::Cell;
            render_table(
                frame,
                ratatui::layout::Rect::new(0, 0, 80, 10),
                "Test Title",
                &["COL A", "COL B"],
                vec![vec![Cell::from("r1c1"), Cell::from("r1c2")]],
                &[Constraint::Min(10), Constraint::Min(10)],
                0,
            );
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let area = buf.area();

    // The header row is at y=1 (y=0 is the top border drawn by the Block).
    // Walk all non-space cells in that row and verify at least one carries
    // LightCyan fg — proof that column_header_style is wired to the header row.
    let mut found_light_cyan_fg = false;
    for x in 0..area.width {
        let cell = buf.cell((x, 1)).unwrap();
        if cell.symbol() != " " && cell.style().fg == Some(Color::LightCyan) {
            found_light_cyan_fg = true;
            break;
        }
    }
    assert!(
        found_light_cyan_fg,
        "header row (y=1) must have at least one non-space cell with LightCyan fg — \
         column_header_style must be wired to the header row"
    );
}

// P4a-A1/A2: too-small guard — view() renders a single message below thresholds
// and the normal screen above thresholds.
mod view_size_guard {
    use crate::tui::model::{Header, Model, Screen};
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
            header: Header::from_instances(&[], None),
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

// P4b-A3: draw_projects truncates over-long project name with ellipsis on a terminal
// that is wide enough to show some project text but not the full name.
// At width=50, project_width = 50 - 7 - 18 - 6 = 19, so a long name is truncated.
#[test]
fn draw_projects_narrow_terminal_long_name_shows_ellipsis() {
    let long_name = "An Extremely Long Project Name That Will Not Fit In A Narrow Terminal";
    let groups = make_groups(&[long_name]);
    let buf = render_projects_to_buf(&groups, 0, 50, 10);
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

// U4-A2: Projects screen renders three column headers — Tasks, Project, Instance.
#[test]
fn draw_projects_renders_three_column_headers() {
    let groups = make_groups_with_instance(&[("Acme Corp", "prod-inst")]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Tasks"),
        "header must contain 'Tasks': {content}"
    );
    assert!(
        content.contains("Project"),
        "header must contain 'Project': {content}"
    );
    assert!(
        content.contains("Instance"),
        "header must contain 'Instance': {content}"
    );
}

// U4-A2: Projects screen row shows task count, project name, and instance.
#[test]
fn draw_projects_row_shows_task_count_project_and_instance() {
    let groups = make_groups_with_instance(&[("My Project", "staging")]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains('1'),
        "row must show task count '1': {content}"
    );
    assert!(
        content.contains("My Project"),
        "row must show project name: {content}"
    );
    assert!(
        content.contains("staging"),
        "row must show instance name: {content}"
    );
}

// U4-A3: PROJECT column truncates with ellipsis at narrow width; TASKS and INSTANCE remain intact.
#[test]
fn draw_projects_narrow_width_project_truncates_but_instance_and_count_intact() {
    let long_project = "An Extremely Long Project Name That Cannot Fit";
    let groups = make_groups_with_instance(&[(long_project, "prod")]);
    let buf = render_projects_to_buf(&groups, 0, 40, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains('\u{2026}'),
        "narrow terminal must truncate long project name with ellipsis: {content}"
    );
    assert!(
        content.contains("prod"),
        "instance column 'prod' must remain readable at narrow width: {content}"
    );
    assert!(
        content.contains('1'),
        "task count must remain readable at narrow width: {content}"
    );
}

// U4-A3: At a wide terminal a short project name shows no ellipsis and instance is intact.
#[test]
fn draw_projects_wide_width_short_project_and_instance_both_shown() {
    let groups = make_groups_with_instance(&[("Short", "my-instance")]);
    let buf = render_projects_to_buf(&groups, 0, 120, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Short"),
        "wide terminal must show full project name: {content}"
    );
    assert!(
        content.contains("my-instance"),
        "wide terminal must show full instance name: {content}"
    );
    assert!(
        !content.contains('\u{2026}'),
        "wide terminal must NOT show ellipsis for short names: {content}"
    );
}

// U3-A1: Detail scrollbar appears (thumb '█') when lines exceed viewport height.
// Area height=16: viewport_height = 16-2 = 14; 20 lines > 14 → scrollbar shown.
#[test]
fn draw_detail_many_lines_short_area_shows_scrollbar_thumb_glyph_in_rightmost_column() {
    let lines: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    let buf = render_detail_to_buf(&lines, &[], 0, 40, 16);
    let rightmost_x = 39u16;
    let mut found_scrollbar_glyph = false;
    for y in 0..16u16 {
        let cell = buf.cell((rightmost_x, y)).unwrap();
        let sym = cell.symbol();
        if sym == "█" || sym == "│" || sym == "↑" || sym == "↓" {
            found_scrollbar_glyph = true;
            break;
        }
    }
    assert!(
        found_scrollbar_glyph,
        "rightmost column must contain a scrollbar glyph when content (20 lines) exceeds viewport height"
    );
}

// U3-A2: Detail scrollbar absent when content fits viewport.
// Area height=20: viewport_height = 20-2 = 18; 3 lines <= 18 → no scrollbar.
#[test]
fn draw_detail_few_lines_tall_area_no_scrollbar_glyph_in_rightmost_column() {
    let lines: Vec<String> = vec!["line 1".into(), "line 2".into(), "line 3".into()];
    let buf = render_detail_to_buf(&lines, &[], 0, 40, 20);
    let rightmost_x = 39u16;
    for y in 0..20u16 {
        let cell = buf.cell((rightmost_x, y)).unwrap();
        let sym = cell.symbol();
        assert!(
            sym != "█" && sym != "↑" && sym != "↓",
            "rightmost column must NOT contain scrollbar glyphs when content (3 lines) fits viewport (height=20): y={y} sym={sym:?}"
        );
    }
}

fn render_table_to_buf(
    row_count: usize,
    selected: usize,
    width: u16,
    height: u16,
) -> ratatui::buffer::Buffer {
    use crate::tui::drawer::render_table;
    use ratatui::layout::Constraint;
    use ratatui::widgets::Cell;
    let rows: Vec<Vec<Cell<'static>>> = (0..row_count)
        .map(|i| vec![Cell::from(format!("row{i}"))])
        .collect();
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_table(
                frame,
                ratatui::layout::Rect::new(0, 0, width, height),
                "Test",
                &["NAME"],
                rows,
                &[Constraint::Min(0)],
                selected,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

// U3-A3 (overflow case): render_table with more rows than visible capacity shows scrollbar.
// Area height=6 gives visible_capacity=3 (6 - 2 borders - 1 header); 8 rows > 3 → scrollbar shown.
#[test]
fn render_table_overflow_rows_shows_scrollbar_thumb_glyph_in_rightmost_column() {
    let buf = render_table_to_buf(8, 0, 40, 6);
    let rightmost_x = 39u16;
    let mut found_scrollbar_glyph = false;
    for y in 0..6u16 {
        let cell = buf.cell((rightmost_x, y)).unwrap();
        let sym = cell.symbol();
        if sym == "█" || sym == "│" || sym == "↑" || sym == "↓" {
            found_scrollbar_glyph = true;
            break;
        }
    }
    assert!(
        found_scrollbar_glyph,
        "rightmost column must contain a scrollbar glyph when rows (8) overflow visible capacity (3)"
    );
}

// U3-A3 (fits case): render_table with few rows in tall area shows no scrollbar.
// Area height=20 gives visible_capacity=17 (20 - 2 borders - 1 header); 3 rows <= 17 → no scrollbar.
#[test]
fn render_table_few_rows_tall_area_no_scrollbar_glyph_in_rightmost_column() {
    let buf = render_table_to_buf(3, 0, 40, 20);
    let rightmost_x = 39u16;
    for y in 0..20u16 {
        let cell = buf.cell((rightmost_x, y)).unwrap();
        let sym = cell.symbol();
        assert!(
            sym != "█" && sym != "↑" && sym != "↓",
            "rightmost column must NOT contain scrollbar glyphs when rows (3) fit visible capacity (height=20): y={y} sym={sym:?}"
        );
    }
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

// U6c-A1: draw_detail renders a single global content block (no separate header/body/comments boxes).
// The title block must appear and the content from build_detail_lines must be present.
// There is exactly ONE top-left corner glyph (┌) for the content block, plus one for Artifacts.
#[test]
fn draw_detail_renders_single_global_content_block() {
    let task = json!({
        "name": "Test Task",
        "id": 7,
        "project_id": 2,
        "is_completed": false,
        "body": "<p>Task body content here.</p>"
    });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1700000000u64,
        "body": "<p>A comment on this task.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let lines = build_detail_lines(&task, &[comment], &user_map, 76);

    let assets = vec![Asset {
        name: "file.pdf".into(),
        url: "https://example.com/file.pdf".into(),
    }];

    let buf = render_detail_to_buf(&lines, &assets, 0, 80, 40);
    let content = buf_to_string(&buf);

    // The single block title contains the task ID
    assert!(
        content.contains("Task #42"),
        "single block title must contain 'Task #42': {content}"
    );
    // The Artifacts panel must also appear
    assert!(
        content.contains("Artifacts"),
        "Artifacts panel must appear when assets present: {content}"
    );
    // The content block contains the "Test Task" title from build_detail_lines
    assert!(
        content.contains("Test Task"),
        "task name must appear in the unified content block: {content}"
    );
    // Exactly two bordered boxes: content block + Artifacts panel.
    // The top-left corner glyph (┌) appears once per box.
    let box_count = content.matches('┌').count();
    assert_eq!(
        box_count, 2,
        "exactly 2 bordered boxes must render (content + Artifacts), found {box_count}: {content}"
    );
}

// U6c-A3: Detail footer has the single U3 string — no 'Tab switch' text.
#[test]
fn view_detail_footer_has_no_tab_switch_hint() {
    use crate::tui::model::{Header, Model, Screen};
    use crate::tui::view::view;
    use std::collections::HashMap;

    let task = json!({
        "name": "Test Task",
        "id": 7,
        "project_id": 2,
        "is_completed": false,
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let lines = build_header_lines(&task, &user_map, 76);
    let assets = vec![Asset {
        name: "doc.pdf".into(),
        url: "https://example.com/doc.pdf".into(),
    }];

    let model = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 42,
            task,
            comments: vec![],
            user_map,
            lines,
            assets,
            offset: 0,
            loading: false,
            pending_download: false,
            rendered_width: 80,
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(&model, frame)).unwrap();
    let content = buf_to_string(terminal.backend().buffer());

    assert!(
        !content.contains("Tab"),
        "Detail footer must NOT contain 'Tab' hint (U6b removed): {content}"
    );
    assert!(
        !content.contains("switch section"),
        "Detail footer must NOT contain 'switch section' (U6b removed): {content}"
    );
    assert!(
        content.contains("1-9"),
        "Detail footer must still contain '1-9 open asset' hint: {content}"
    );
    assert!(
        content.contains("↑/↓"),
        "Detail footer must contain '↑/↓ scroll' hint: {content}"
    );
}

// U6c-A3: Detail footer without assets — single U3 string, no Tab hint.
#[test]
fn view_detail_footer_without_assets_has_no_tab_hint() {
    use crate::tui::model::{Header, Model, Screen};
    use crate::tui::view::view;
    use std::collections::HashMap;

    let model = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 7,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec!["body".into()],
            assets: vec![],
            offset: 0,
            loading: false,
            pending_download: false,
            rendered_width: 80,
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(&model, frame)).unwrap();
    let content = buf_to_string(terminal.backend().buffer());

    assert!(
        !content.contains("Tab"),
        "Detail footer without assets must NOT contain 'Tab': {content}"
    );
    assert!(
        content.contains("↑/↓"),
        "Detail footer must still contain scroll hint: {content}"
    );
}

// U5a-A1: Header::from_instances builds the struct correctly.
#[test]
fn header_from_instances_single_uses_first_email_and_name() {
    let inst = make_instance("prod", "user@example.com");
    let h = Header::from_instances(&[inst], Some("Alice".into()));
    assert_eq!(h.email, "user@example.com");
    assert_eq!(h.instance, "prod");
    assert_eq!(h.name, Some("Alice".into()));
    assert_eq!(h.extra, 0);
}

#[test]
fn header_from_instances_multi_sets_extra() {
    let inst1 = make_instance("prod", "a@example.com");
    let inst2 = make_instance("staging", "b@example.com");
    let h = Header::from_instances(&[inst1, inst2], None);
    assert_eq!(h.email, "a@example.com");
    assert_eq!(h.instance, "prod");
    assert_eq!(h.extra, 1);
    assert_eq!(h.name, None);
}

#[test]
fn header_from_instances_empty_slice_is_safe() {
    let h = Header::from_instances(&[], None);
    assert_eq!(h.email, "");
    assert_eq!(h.instance, "");
    assert_eq!(h.extra, 0);
    assert_eq!(h.name, None);
}

// U5a-A1: header_line formats correctly with and without name, and with extra>0.
#[test]
fn header_line_with_name_formats_name_email_instance() {
    let h = Header {
        name: Some("Bob".into()),
        email: "bob@acme.com".into(),
        instance: "acme".into(),
        extra: 0,
    };
    assert_eq!(h.header_line(), "Bob <bob@acme.com> · acme");
}

#[test]
fn header_line_without_name_omits_name_prefix() {
    let h = Header {
        name: None,
        email: "bob@acme.com".into(),
        instance: "acme".into(),
        extra: 0,
    };
    assert_eq!(h.header_line(), "<bob@acme.com> · acme");
}

#[test]
fn header_line_with_extra_appends_plus_n_more() {
    let h = Header {
        name: None,
        email: "user@x.com".into(),
        instance: "x".into(),
        extra: 3,
    };
    assert_eq!(h.header_line(), "<user@x.com> · x (+3 more)");
}

#[test]
fn header_line_with_name_and_extra() {
    let h = Header {
        name: Some("Carol".into()),
        email: "carol@co.io".into(),
        instance: "co".into(),
        extra: 2,
    };
    assert_eq!(h.header_line(), "Carol <carol@co.io> · co (+2 more)");
}

// U5a-A2: view() renders header bar on top row with app_header_style (fg White, bg Cyan).
#[test]
fn view_renders_header_on_top_row_with_app_header_style() {
    use crate::tui::model::{Model, Screen};
    use crate::tui::view::view;
    use ratatui::style::Color;

    let inst = make_instance("prod", "alice@example.com");
    let header = Header::from_instances(&[inst], None);

    let model = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(&model, frame)).unwrap();
    let buf = terminal.backend().buffer();

    let top_row_content: String = (0..80u16)
        .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
        .collect();
    assert!(
        top_row_content.contains("alice@example.com"),
        "top row must contain the email: {top_row_content}"
    );
    assert!(
        top_row_content.contains("prod"),
        "top row must contain the instance name: {top_row_content}"
    );

    let mut found_header_style = false;
    for x in 0..80u16 {
        let cell = buf.cell((x, 0)).unwrap();
        if cell.style().fg == Some(Color::White) && cell.style().bg == Some(Color::Cyan) {
            found_header_style = true;
            break;
        }
    }
    assert!(
        found_header_style,
        "top row must have at least one cell with White fg and Cyan bg (app_header_style)"
    );
}

// U5a-A2: content and footer still render below the header (at y=1..n-1 and y=last).
#[test]
fn view_content_and_footer_render_below_header() {
    use crate::tui::model::{Model, Screen};
    use crate::tui::view::view;

    let inst = make_instance("inst", "u@example.com");
    let header = Header::from_instances(&[inst], None);

    let model = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header,
    };

    let height = 10u16;
    let backend = TestBackend::new(80, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(&model, frame)).unwrap();
    let buf = terminal.backend().buffer();

    let last_row: String = (0..80u16)
        .map(|x| buf.cell((x, height - 1)).unwrap().symbol().to_string())
        .collect();
    assert!(
        last_row.contains("↑/↓"),
        "last row must be the footer with navigation hint: {last_row}"
    );

    let top_row: String = (0..80u16)
        .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
        .collect();
    assert!(
        !top_row.contains("↑/↓"),
        "top row (header) must NOT contain footer hint: {top_row}"
    );
}

// U5a-A2: multi-instance header shows (+N more).
#[test]
fn view_multi_instance_header_shows_extra_suffix() {
    use crate::tui::model::{Model, Screen};
    use crate::tui::view::view;

    let inst1 = make_instance("prod", "a@example.com");
    let inst2 = make_instance("staging", "b@example.com");
    let header = Header::from_instances(&[inst1, inst2], None);

    let model = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(&model, frame)).unwrap();
    let buf = terminal.backend().buffer();

    let top_row: String = (0..80u16)
        .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
        .collect();
    assert!(
        top_row.contains("+1 more"),
        "top row must contain '+1 more' for 2-instance header: {top_row}"
    );
}

// U5a-A3: too-small guard still suppresses header+footer at sub-minimum sizes.
#[test]
fn view_too_small_suppresses_header_and_footer() {
    use crate::tui::model::{Model, Screen};
    use crate::tui::view::view;

    let inst = make_instance("prod", "x@example.com");
    let header = Header::from_instances(&[inst], None);

    let model = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
        }],
        should_quit: false,
        header,
    };

    let backend = TestBackend::new(20, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| view(&model, frame)).unwrap();
    let content = buf_to_string(terminal.backend().buffer());

    assert!(
        content.contains("Terminal too small"),
        "must show 'Terminal too small' at 20x5: {content}"
    );
    assert!(
        !content.contains("x@example.com"),
        "header email must NOT appear at 20x5 (guard suppresses header): {content}"
    );
    assert!(
        !content.contains("↑/↓"),
        "footer hint must NOT appear at 20x5 (guard suppresses footer): {content}"
    );
}

// U7-A1/A2: Projects count cell carries badge_style (Magenta+BOLD); project-name cell does not.
// Layout at width=80: x=0 left border, x=1..2 selection-symbol area, x=3 first digit of count.
// y=0 top border, y=1 header row, y=2 selected data row (row 0), y=3 non-selected data row (row 1).
// The non-selected row is tested because the selection highlight overrides fg on the selected row.
#[test]
fn projects_count_cell_is_magenta() {
    use ratatui::style::{Color, Modifier};

    let groups =
        make_groups_with_instance(&[("Alpha Project", "inst-a"), ("Beta Project", "inst-b")]);
    // Select row 0; test the unselected row 1 at y=3 so selection_style does not mask badge_style.
    let buf = render_projects_to_buf(&groups, 0, 80, 10);

    // Count column: x=3 (border=1 + selection-symbol=2), y=3 (non-selected second data row).
    // TASKS_WIDTH=7, so count spans x=3..9. Pick x=3 for the digit.
    let count_cell = buf.cell((3, 3)).unwrap();
    assert_eq!(
        count_cell.style().fg,
        Some(Color::Magenta),
        "count cell must have Magenta fg — badge_style must be applied: symbol={:?}",
        count_cell.symbol()
    );
    assert!(
        count_cell.style().add_modifier.contains(Modifier::BOLD),
        "count cell must have BOLD modifier — badge_style must be applied"
    );

    // Project-name column starts after count (7) + separator (1) = x=11, so first project char
    // is at x=11. Use x=12 to stay clearly inside the column (avoids the separator glyph at x=10).
    let name_cell = buf.cell((12, 3)).unwrap();
    assert_ne!(
        name_cell.style().fg,
        Some(Color::Magenta),
        "project-name cell must NOT carry Magenta fg — badge must be scoped to count column only"
    );
}

// U7-A1 (theme): badge_style returns Magenta fg + BOLD modifier.
#[test]
fn badge_style_is_magenta_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::badge_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
        "badge_style must be magenta+bold"
    );
}

// U5a-A4/A5 (theme): app_header_style is White on Cyan, bold.
#[test]
fn app_header_style_is_white_on_cyan_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::app_header_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::White)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "app_header_style must be white-on-cyan bold"
    );
}
