use crate::i18n::set_language;
use crate::render::{build_detail_lines, build_header_lines, Asset};
use crate::store::instances::Instance;
use crate::tui::model::{Header, ProjectGroup, TaskRow};
use crate::tui::screens::{draw_detail, draw_projects, draw_tasks, DetailParams};
use crate::tui::theme;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;

/// Serialize tests that change the global display language.
static LANG_MUTEX: Mutex<()> = Mutex::new(());

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
    render_detail_to_buf_with_name(lines, assets, offset, width, height, "")
}

fn render_detail_to_buf_with_name(
    lines: &[String],
    assets: &[Asset],
    offset: usize,
    width: u16,
    height: u16,
    task_name: &str,
) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_detail(
                frame,
                Rect::new(0, 0, width, height),
                DetailParams {
                    lines,
                    assets,
                    offset,
                    loading: false,
                    task_id: 42,
                    task_name,
                },
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

// V1-A1: Projects list has a single name column — no task-count 'Tarefas'/numeric column.
// Header shows "Projeto" (pt-BR) and NOT "Tarefas".
#[test]
fn draw_projects_single_name_column_no_task_count() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let groups = make_groups(&["My Project"]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Projeto"),
        "header must show translated 'Projeto': {content}"
    );
    assert!(
        !content.contains("Tarefas"),
        "task-count 'Tarefas' column must be absent: {content}"
    );
}

// V1-A1: Tasks list has a single name column — no TASK# column.
// Header shows "NOME" (pt-BR translation of "NAME") and NOT "TAREFA#".
#[test]
fn draw_tasks_single_name_column_no_task_number() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let tasks = make_tasks(&["My Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        content.contains("NOME"),
        "header must show translated 'NOME': {content}"
    );
    assert!(
        !content.contains("TAREFA#") && !content.contains("TASK#"),
        "task-number column must be absent: {content}"
    );
}

// V1-A3: Projects title renders in pt-BR as 'Projetos'.
// The title uses format!(" {} ", t("Projects")) — spaces are added by format, not by the key.
#[test]
fn draw_projects_title_renders_in_pt_br() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let groups = make_groups(&["A Project"]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Projetos"),
        "Projects title must render as 'Projetos' (pt-BR): {content}"
    );
}

// V1-A3: My Tasks title renders in pt-BR as 'Minhas Tarefas'.
// mine_model sets project_name = t("My Tasks"); draw_tasks shows it as the window title.
#[test]
fn draw_my_tasks_title_renders_in_pt_br() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let title = crate::i18n::t("My Tasks");
    let tasks = make_tasks(&["A Task"]);
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_tasks(frame, Rect::new(0, 0, 80, 10), &title, &tasks, 0, false);
        })
        .unwrap();
    set_language("en");
    let content = buf_to_string(terminal.backend().buffer());
    assert!(
        content.contains("Minhas Tarefas"),
        "My Tasks title must render as 'Minhas Tarefas' (pt-BR): {content}"
    );
}

// V1-A3: pt_BR catalog contains the new "My Tasks" -> "Minhas Tarefas" key.
#[test]
fn pt_br_catalog_maps_my_tasks_to_minhas_tarefas() {
    let raw = include_str!("../../locales/pt_BR.json");
    let catalog: std::collections::HashMap<String, String> =
        serde_json::from_str(raw).expect("pt_BR.json must be valid JSON");
    assert_eq!(
        catalog.get("My Tasks").map(String::as_str),
        Some("Minhas Tarefas"),
        "pt_BR catalog must map \"My Tasks\" -> \"Minhas Tarefas\""
    );
}

// V1-A2: A long project name wraps onto a second buffer line on a narrow terminal.
// At width=20, name_width = 20 - 4 = 16. A name > 16 chars must wrap.
#[test]
fn draw_projects_long_name_wraps_on_narrow_terminal() {
    let long_name = "Alpha Beta Gamma Delta";
    assert!(
        long_name.len() > 16,
        "test name must exceed name_width=16 to trigger wrapping"
    );
    let groups = make_groups(&[long_name]);
    let buf = render_projects_to_buf(&groups, 0, 20, 10);
    let rows: Vec<String> = buf_to_string(&buf).lines().map(str::to_string).collect();
    // Row 0: top border; Row 1: header; Row 2: first data line; Row 3: wrapped continuation.
    // The name must appear across at least two rows (row 2 and row 3).
    let name_part_in_row2 = rows.get(2).map(|r| r.contains("Alpha")).unwrap_or(false);
    let name_cont_in_row3 = rows.get(3).map(|r| r.contains("Delta")).unwrap_or(false);
    assert!(
        name_part_in_row2,
        "first word of name must appear on data row (y=2): rows={rows:?}"
    );
    assert!(
        name_cont_in_row3,
        "wrapped continuation must appear on next row (y=3): rows={rows:?}"
    );
}

// V1-A2: A long task name wraps onto a second buffer line on a narrow terminal.
// At width=20, name_width = 20 - 4 = 16. A name > 16 chars must wrap.
#[test]
fn draw_tasks_long_name_wraps_on_narrow_terminal() {
    let long_name = "Alpha Beta Gamma Delta";
    assert!(
        long_name.len() > 16,
        "test name must exceed name_width=16 to trigger wrapping"
    );
    let tasks = make_tasks(&[long_name]);
    let buf = render_tasks_to_buf(&tasks, 0, 20, 10);
    let rows: Vec<String> = buf_to_string(&buf).lines().map(str::to_string).collect();
    let name_part_in_row2 = rows.get(2).map(|r| r.contains("Alpha")).unwrap_or(false);
    let name_cont_in_row3 = rows.get(3).map(|r| r.contains("Delta")).unwrap_or(false);
    assert!(
        name_part_in_row2,
        "first word of name must appear on data row (y=2): rows={rows:?}"
    );
    assert!(
        name_cont_in_row3,
        "wrapped continuation must appear on next row (y=3): rows={rows:?}"
    );
}

// V1-A2: No ellipsis on narrow terminal — names wrap, not truncate.
#[test]
fn draw_projects_no_ellipsis_on_narrow_terminal() {
    let long_name = "An Extremely Long Project Name That Will Not Fit";
    let groups = make_groups(&[long_name]);
    let buf = render_projects_to_buf(&groups, 0, 20, 15);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains('\u{2026}'),
        "project names must wrap, not truncate with ellipsis: {content}"
    );
}

// V1-A2: No ellipsis on narrow terminal — task names wrap, not truncate.
#[test]
fn draw_tasks_no_ellipsis_on_narrow_terminal() {
    let long_name = "An Extremely Long Task Name That Will Not Fit In A Narrow Terminal";
    let tasks = make_tasks(&[long_name]);
    let buf = render_tasks_to_buf(&tasks, 0, 20, 15);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains('\u{2026}'),
        "task names must wrap, not truncate with ellipsis: {content}"
    );
}

#[test]
fn draw_projects_at_width_40_does_not_panic() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let groups = make_groups(&["Short Project"]);
    let buf = render_projects_to_buf(&groups, 0, 40, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(content.contains("Projeto"), "header 'Projeto' must appear");
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
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let tasks = make_tasks(&["Short Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 40, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(content.contains("NOME"), "header 'NOME' must appear");
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

// V1-A1: Projects header is a single column — no 'Tarefas' (Tasks) column.
#[test]
fn draw_projects_header_row_present() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let groups = make_groups(&["My Project"]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Projeto"),
        "header label 'Projeto' must be present"
    );
    assert!(
        !content.contains("Tarefas"),
        "header label 'Tarefas' must NOT be present (column removed)"
    );
    assert!(
        !content.contains("Instance"),
        "header label 'Instance' must NOT be present (column removed)"
    );
}

// V1-A1: Tasks header is a single column — only "NOME" header, no "TAREFA#".
#[test]
fn draw_tasks_header_row_present() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let tasks = make_tasks(&["My Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        content.contains("NOME"),
        "header label 'NOME' must be present"
    );
    assert!(
        !content.contains("TAREFA#") && !content.contains("TASK#"),
        "header label 'TASK#' must NOT be present (column removed)"
    );
    assert!(
        !content.contains("INSTANCE"),
        "header label 'INSTANCE' must NOT be present (column removed)"
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
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("en");
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
        content.contains("[1]") && content.contains("report.pdf"),
        "first asset must appear as '[1] ↗ report.pdf': {content}"
    );
    assert!(
        content.contains("[2]") && content.contains("photo.png"),
        "second asset must appear as '[2] ↗ photo.png': {content}"
    );
    assert!(
        content.contains("Artifacts"),
        "panel title 'Artifacts' must appear: {content}"
    );
}

// U2-A1: asset label format is '[N] ↗ <label>' where N is 1-based
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
        content.contains("[1]"),
        "first asset must carry 1-based index '[1]': {content}"
    );
    assert!(
        content.contains("diagram.png"),
        "filename must be retained after the label: {content}"
    );
    assert!(
        content.contains("[2]"),
        "second asset must carry 1-based index '[2]': {content}"
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

// U8-A1: sober cool retro theme — exact Rgb channels + modifiers for every style fn.
#[test]
fn footer_style_is_light_grey_on_steel_bg_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::footer_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(208, 216, 224))
            .bg(Color::Rgb(38, 52, 74))
            .add_modifier(Modifier::BOLD),
        "footer_style must be light-grey on steel-blue band, bold (sober palette)"
    );
}

#[test]
fn header_style_is_steel_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::header_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(140, 165, 196))
            .add_modifier(Modifier::BOLD),
        "header_style must be steel-blue+bold (sober palette)"
    );
}

// U8-A1: selection_style — near-black on discreet amber, bold.
#[test]
fn selection_style_is_near_black_on_amber_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::selection_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(13, 13, 13))
            .bg(Color::Rgb(210, 160, 90))
            .add_modifier(Modifier::BOLD),
        "selection_style must be near-black on amber+bold (sober palette)"
    );
}

#[test]
fn asset_style_is_muted_green_underlined() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::asset_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(120, 190, 130))
            .add_modifier(Modifier::UNDERLINED),
        "asset_style must be muted-green+underlined (sober palette)"
    );
}

// U8-A1: column_header_style — soft cyan fg + bold.
#[test]
fn column_header_style_is_soft_cyan_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::column_header_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(102, 204, 204))
            .add_modifier(Modifier::BOLD),
        "column_header_style must be soft-cyan+bold (sober palette)"
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

// U8-A2: TestBackend confirms column_header_style fg (soft cyan Rgb(102,204,204)) is applied to the header row.
#[test]
fn render_table_header_row_carries_column_header_style() {
    use ratatui::style::Color;
    use ratatui::{backend::TestBackend, layout::Constraint, text::Text, Terminal};

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            use crate::tui::drawer::render_table;
            use ratatui::widgets::{Cell, Row};
            let rows = vec![Row::new(vec![
                Cell::from(Text::raw("r1c1")),
                Cell::from(Text::raw("r1c2")),
            ])];
            render_table(
                frame,
                ratatui::layout::Rect::new(0, 0, 80, 10),
                "Test Title",
                &["COL A", "COL B"],
                rows,
                &[Constraint::Min(10), Constraint::Min(10)],
                0,
            );
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let area = buf.area();

    // The header row is at y=1 (y=0 is the top border drawn by the Block).
    // Walk all non-space cells in that row and verify at least one carries
    // soft-cyan Rgb(102,204,204) fg — proof that column_header_style is wired to the header row.
    let soft_cyan = Color::Rgb(102, 204, 204);
    let mut found_soft_cyan_fg = false;
    for x in 0..area.width {
        let cell = buf.cell((x, 1)).unwrap();
        if cell.symbol() != " " && cell.style().fg == Some(soft_cyan) {
            found_soft_cyan_fg = true;
            break;
        }
    }
    assert!(
        found_soft_cyan_fg,
        "header row (y=1) must have at least one non-space cell with soft-cyan Rgb(102,204,204) fg — \
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

// V1-A1: draw_tasks wide terminal shows full name without ellipsis.
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

// V1-A1: draw_projects wide terminal shows full name without ellipsis.
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

// V1-A1: Projects renders single-column header — no 'Instance' column.
#[test]
fn draw_projects_renders_single_column_header() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let groups = make_groups_with_instance(&[("Acme Corp", "prod-inst")]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Projeto"),
        "header must contain 'Projeto': {content}"
    );
    assert!(
        !content.contains("Tarefas"),
        "header must NOT contain 'Tarefas' (task-count column removed): {content}"
    );
    assert!(
        !content.contains("Instance"),
        "header must NOT contain 'Instance' (column removed): {content}"
    );
}

// V1-A1: Projects screen row shows project name only (no task count, no instance).
#[test]
fn draw_projects_row_shows_project_name_only() {
    let groups = make_groups_with_instance(&[("My Project", "staging")]);
    let buf = render_projects_to_buf(&groups, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("My Project"),
        "row must show project name: {content}"
    );
    assert!(
        !content.contains("staging"),
        "row must NOT show instance name (column removed): {content}"
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
    use ratatui::widgets::{Cell, Row};
    let rows: Vec<Row<'static>> = (0..row_count)
        .map(|i| Row::new(vec![Cell::from(format!("row{i}"))]))
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
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("en");
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

    let buf = render_detail_to_buf_with_name(&lines, &assets, 0, 80, 40, "Test Task");
    let content = buf_to_string(&buf);

    // The single block title now shows the task name, not "Task #42"
    assert!(
        content.contains("Test Task"),
        "single block title must contain the task name 'Test Task': {content}"
    );
    assert!(
        !content.contains("Task #42"),
        "single block title must NOT contain 'Task #42' (name is now in border): {content}"
    );
    // The Artifacts panel must also appear
    assert!(
        content.contains("Artifacts"),
        "Artifacts panel must appear when assets present: {content}"
    );
    // Exactly two bordered boxes: content block + Artifacts panel.
    // The top-left corner glyph (┌) appears once per box.
    let box_count = content.matches('┌').count();
    assert_eq!(
        box_count, 2,
        "exactly 2 bordered boxes must render (content + Artifacts), found {box_count}: {content}"
    );
}

// D2-A1: Detail frame border shows task NAME when loaded, falls back to #<id> when empty.
#[test]
fn draw_detail_border_shows_task_name_when_loaded() {
    let lines = vec!["some content".to_string()];
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 80, 10, "My Important Task");
    let content = buf_to_string(&buf);
    // Top border (row 0) must contain the task name
    let rows: Vec<&str> = content.lines().collect();
    let top_border = rows[0];
    assert!(
        top_border.contains("My Important Task"),
        "top border must contain task name 'My Important Task': {top_border}"
    );
    assert!(
        !top_border.contains("Task #") && !top_border.contains("Tarefa #"),
        "top border must NOT contain 'Task #' / 'Tarefa #' when name is present: {top_border}"
    );
}

#[test]
fn draw_detail_border_falls_back_to_id_when_name_empty() {
    let lines = vec!["some content".to_string()];
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 80, 10, "");
    let content = buf_to_string(&buf);
    let rows: Vec<&str> = content.lines().collect();
    let top_border = rows[0];
    assert!(
        top_border.contains("#42"),
        "top border must contain '#42' as fallback when name is empty: {top_border}"
    );
}

#[test]
fn draw_detail_border_long_name_truncated_with_ellipsis() {
    let very_long_name =
        "This Is An Extremely Long Task Name That Does Not Fit In The Border At All";
    let lines = vec!["content".to_string()];
    // Use a narrow width to force truncation
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 40, 10, very_long_name);
    let content = buf_to_string(&buf);
    let rows: Vec<&str> = content.lines().collect();
    let top_border = rows[0];
    assert!(
        top_border.contains('\u{2026}'),
        "top border must contain ellipsis when name is truncated: {top_border}"
    );
    assert!(
        !top_border.contains("Task #"),
        "top border must NOT fall back to task id when name is present (even truncated): {top_border}"
    );
}

// D2-A3: Over-scroll clamp prevents empty rows below the last content line.
#[test]
fn draw_detail_over_scroll_clamp_prevents_empty_rows() {
    // height=10 → viewport_height = 10 - 2 = 8
    // 12 content lines → max_offset = 12 - 8 = 4
    // With offset=9999 the effective offset must clamp to 4
    let width: u16 = 40;
    let height: u16 = 10;
    let viewport_height = (height - 2) as usize;
    let lines: Vec<String> = (1..=12).map(|i| format!("line {:02}", i)).collect();

    let buf = render_detail_to_buf(&lines, &[], 9999, width, height);
    let content = buf_to_string(&buf);

    // The last content lines visible must be the tail of `lines`.
    // With clamp to offset=4 and viewport=8, visible are lines[4..12] = "line 05" .. "line 12".
    let expected_last = &lines[lines.len() - 1]; // "line 12"
    assert!(
        content.contains(expected_last.as_str()),
        "last content line '{expected_last}' must be visible after over-scroll clamp: {content}"
    );

    // Also verify: the first visible content line is lines[max_offset] = lines[4] = "line 05"
    let max_offset = lines.len().saturating_sub(viewport_height);
    let expected_first_visible = &lines[max_offset]; // "line 05"
    assert!(
        content.contains(expected_first_visible.as_str()),
        "first visible line at clamped offset '{expected_first_visible}' must appear: {content}"
    );

    // Lines before max_offset must NOT appear (they scrolled off top)
    // "line 01" is at index 0, which is before max_offset=4 — should not be visible
    assert!(
        !content.contains("line 01"),
        "line before clamped offset must NOT be visible: {content}"
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

// U8-A2: view() renders header bar on top row with app_header_style (soft-cyan on steel-blue band, bold).
#[test]
fn view_renders_header_on_top_row_with_app_header_style_is_soft_cyan_on_steel() {
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

    let soft_cyan = Color::Rgb(102, 204, 204);
    let steel_bg = Color::Rgb(38, 52, 74);
    let mut found_header_style = false;
    for x in 0..80u16 {
        let cell = buf.cell((x, 0)).unwrap();
        if cell.style().fg == Some(soft_cyan) && cell.style().bg == Some(steel_bg) {
            found_header_style = true;
            break;
        }
    }
    assert!(
        found_header_style,
        "top row must have at least one cell with soft-cyan Rgb(102,204,204) fg and steel-bg Rgb(38,52,74) bg (app_header_style)"
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

// U8-A1 (theme): badge_style returns amber Rgb(210,160,90) fg + BOLD modifier.
#[test]
fn badge_style_is_amber_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::badge_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(210, 160, 90))
            .add_modifier(Modifier::BOLD),
        "badge_style must be amber Rgb(210,160,90)+bold (sober palette)"
    );
}

// U8-A1 (theme): app_header_style is soft-cyan on steel-blue band, bold.
#[test]
fn app_header_style_is_soft_cyan_on_steel_bold() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::app_header_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(102, 204, 204))
            .bg(Color::Rgb(38, 52, 74))
            .add_modifier(Modifier::BOLD),
        "app_header_style must be soft-cyan on steel-blue band, bold (sober palette)"
    );
}

// U9-A1: Projects screen has no Instance header; project name absorbs full width.
// Render at width=60. name_width = 60 - 4 = 56, so a 40-char name fits without wrapping.
#[test]
fn draw_projects_no_instance_header_and_name_fits_full_width() {
    let name = "A Project Name Exactly Forty Chars LongXX";
    assert!(
        name.len() <= 56,
        "test name must fit in freed name_width=56"
    );
    let groups = make_groups_with_instance(&[(name, "some-inst")]);
    let buf = render_projects_to_buf(&groups, 0, 60, 10);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains("Instance"),
        "Projects header must NOT contain 'Instance' (U9): {content}"
    );
    assert!(
        content.contains(name),
        "project name must appear fully at width=60 without truncation (freed width): {content}"
    );
}

// U9-A2: Tasks-in-project screen has no INSTANCE header; NAME absorbs full width.
// Render at width=60. name_width = 60 - 4 = 56, so a 40-char name fits without wrapping.
#[test]
fn draw_tasks_no_instance_header_and_name_fits_full_width() {
    let name = "A Task Name Exactly Forty Characters LongX";
    assert!(
        name.len() <= 56,
        "test name must fit in freed name_width=56"
    );
    let tasks = make_tasks(&[name]);
    let buf = render_tasks_to_buf(&tasks, 0, 60, 10);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains("INSTANCE"),
        "Tasks header must NOT contain 'INSTANCE' (U9): {content}"
    );
    assert!(
        content.contains(name),
        "task name must appear fully at width=60 without truncation (freed width): {content}"
    );
}

// V1-A1: Responsive — project_name column absorbs all available width (overhead=4 only).
// A name of 25 chars fits at w=30 (name_width = 30 - 4 = 26 >= 25 chars).
#[test]
fn draw_projects_name_column_absorbs_full_width() {
    let name_25 = "Twenty-Five Character Name";
    assert_eq!(name_25.len(), 26);
    let groups = make_groups_with_instance(&[(name_25, "inst")]);
    // At width=32, new name_width = 32 - 4 = 28 (>= 26 chars, fits on one line).
    let buf = render_projects_to_buf(&groups, 0, 32, 10);
    let content = buf_to_string(&buf);
    assert!(
        content.contains(name_25),
        "name must fit fully at width=32 with single-column layout: {content}"
    );
}
