use crate::i18n::set_language;
use crate::render::{build_detail_content, build_header_lines, Asset, StyleRun};
use crate::richtext::RichStyle;
use crate::store::instances::Instance;
use crate::tui::model::{Header, ProjectGroup, TaskRow};
use crate::tui::screens::{
    asset_panel_render_height, draw_detail, draw_projects, draw_tasks, DetailParams,
};
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
                due_on: None,
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
                due_on: None,
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
            due_on: None,
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
            let mut targets = vec![];
            draw_projects(
                frame,
                Rect::new(0, 0, width, height),
                groups,
                selected,
                false,
                false,
                &mut targets,
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
            let mut targets = vec![];
            draw_tasks(
                frame,
                Rect::new(0, 0, width, height),
                "Project A",
                tasks,
                selected,
                false,
                false,
                &mut targets,
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
    let empty_styles: Vec<Vec<crate::render::StyleRun>> = vec![vec![]; lines.len()];
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_detail(
                frame,
                Rect::new(0, 0, width, height),
                DetailParams {
                    lines,
                    line_styles: &empty_styles,
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

// D2a-AC1: Tasks list renders NO 'NOME' column header; each task is a bordered card.
// The card content line begins with '#<task_number>' and includes the task name.
#[test]
fn draw_tasks_card_layout_no_nome_header_has_bordered_card() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("pt_BR");
    let tasks = make_tasks(&["My Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 80, 10);
    set_language("en");
    let content = buf_to_string(&buf);
    assert!(
        !content.contains("NOME"),
        "NOME column header must be absent in card layout: {content}"
    );
    assert!(
        !content.contains("TAREFA#") && !content.contains("TASK#"),
        "task-number column must be absent: {content}"
    );
    assert!(
        content.contains("#1"),
        "card content must start with '#<task_number>': {content}"
    );
    assert!(
        content.contains("My Task"),
        "card content must contain the task name: {content}"
    );
    // Card uses rounded-corner box chars
    assert!(
        content.contains('\u{256D}') || content.contains('\u{2570}'),
        "card must use rounded-box border chars: {content}"
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
            let mut targets = vec![];
            draw_tasks(
                frame,
                Rect::new(0, 0, 80, 10),
                &title,
                &tasks,
                0,
                false,
                false,
                &mut targets,
            );
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

// D2a-AC2: A long task name wraps inside the card and the card grows in height.
// At width=20, outer block takes 2 cols (borders), card borders take 2 more, HPAD takes 2 more,
// leaving card_inner_w = 20 - 2 - 2 - 2 = 14.
// "#1  Alpha Beta Gamma Delta" (len 26) wraps at 14 cols across at least 2 content rows.
// Card layout: row 0 = outer top border; rows 1+ are card rows (top border, content, bottom border).
#[test]
fn draw_tasks_long_name_wraps_inside_card() {
    let long_name = "Alpha Beta Gamma Delta";
    let tasks = make_tasks(&[long_name]);
    let buf = render_tasks_to_buf(&tasks, 0, 20, 15);
    let content = buf_to_string(&buf);
    assert!(
        content.contains("Alpha"),
        "first part of name must appear: {content}"
    );
    assert!(
        content.contains("Delta"),
        "wrapped tail of name must appear: {content}"
    );
    assert!(
        !content.contains('\u{2026}'),
        "name must wrap, not truncate with ellipsis: {content}"
    );
    let rows: Vec<String> = content.lines().map(str::to_string).collect();
    let has_alpha = rows.iter().any(|r| r.contains("Alpha"));
    let has_delta = rows.iter().any(|r| r.contains("Delta"));
    assert!(
        has_alpha && has_delta,
        "both name parts must appear in buffer"
    );
    // The card must occupy more rows than a single-line card (height > 3).
    // Count rows between first and last box char row.
    let box_rows: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.contains('\u{256D}') || r.contains('\u{2570}') || r.contains('\u{2502}'))
        .map(|(i, _)| i)
        .collect();
    assert!(
        box_rows.len() >= 4,
        "card must have at least 4 rows (top border + 2 content + bottom border): rows={rows:?}"
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
    let tasks = make_tasks(&["Short Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 40, 10);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains("NOME"),
        "NOME header must NOT appear in card layout: {content}"
    );
    assert!(
        content.contains("#1"),
        "card must render task number '#1': {content}"
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

// D2a-AC4: The selected card's border and content rows all carry the selection style.
// All buffer rows belonging to the selected card (top border, content, bottom border)
// must have at least one cell with the amber selection background.
#[test]
fn draw_tasks_selected_card_all_rows_carry_selection_style() {
    use ratatui::style::Color;
    let tasks = make_tasks(&["Task One", "Task Two"]);
    let backend = TestBackend::new(80, 15);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut captured_targets: Vec<crate::tui::model::ClickTarget> = vec![];
    terminal
        .draw(|frame| {
            draw_tasks(
                frame,
                Rect::new(0, 0, 80, 15),
                "Project A",
                &tasks,
                0,
                false,
                false,
                &mut captured_targets,
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer();
    let amber = Color::Rgb(210, 160, 90);
    // Find the click target for card 0 (the selected one)
    let target = captured_targets
        .iter()
        .find(|t| t.index == 0)
        .expect("click target for card 0 must be recorded");
    // Every row in the selected card's range must have at least one amber-bg cell
    for y in target.y_start..target.y_end {
        let has_amber = (0..80u16).any(|x| {
            buf.cell((x, y))
                .map(|c| c.style().bg == Some(amber))
                .unwrap_or(false)
        });
        assert!(
            has_amber,
            "selected card row y={y} must have amber-bg selection style (D2a-AC4)"
        );
    }
}

// D2a-AC3: A click on a non-first row of a card resolves to that task's index.
// Card for task 0 occupies at least 3 rows (top border, content, bottom border).
// Clicking the second row (y_start+1) must still resolve to index 0.
#[test]
fn draw_tasks_click_on_non_first_card_row_resolves_to_task_index() {
    let tasks = make_tasks(&["Task Alpha", "Task Beta"]);
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut captured_targets: Vec<crate::tui::model::ClickTarget> = vec![];
    terminal
        .draw(|frame| {
            draw_tasks(
                frame,
                Rect::new(0, 0, 80, 20),
                "Project A",
                &tasks,
                0,
                false,
                false,
                &mut captured_targets,
            );
        })
        .unwrap();
    let target0 = captured_targets
        .iter()
        .find(|t| t.index == 0)
        .expect("click target for card 0 must be recorded");
    assert!(
        target0.y_end > target0.y_start + 1,
        "card 0 must span more than 1 row (y_start={}, y_end={})",
        target0.y_start,
        target0.y_end
    );
    // The non-first row (y_start + 1) must still be within the card's y-range.
    let click_y = target0.y_start + 1;
    let resolved = captured_targets
        .iter()
        .find(|t| click_y >= t.y_start && click_y < t.y_end)
        .map(|t| t.index);
    assert_eq!(
        resolved,
        Some(0),
        "clicking non-first row y={click_y} must resolve to card index 0 (D2a-AC3)"
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

// D2a-AC1: Tasks screen has no column header row; each task is a bordered card.
#[test]
fn draw_tasks_no_header_row_only_cards() {
    let tasks = make_tasks(&["My Task"]);
    let buf = render_tasks_to_buf(&tasks, 0, 80, 10);
    let content = buf_to_string(&buf);
    assert!(
        !content.contains("NOME"),
        "NOME column header must NOT appear in card layout: {content}"
    );
    assert!(
        !content.contains("TAREFA#") && !content.contains("TASK#"),
        "TASK# column header must NOT appear: {content}"
    );
    assert!(
        !content.contains("INSTANCE"),
        "INSTANCE column header must NOT appear: {content}"
    );
    assert!(
        content.contains("#1") && content.contains("My Task"),
        "card must show '#1' task number and task name: {content}"
    );
}

#[test]
fn draw_projects_loading_shows_paragraph_not_table() {
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, 80, 10);
            let mut targets = vec![];
            draw_projects(frame, area, &[], 0, true, false, &mut targets);
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
            let mut targets = vec![];
            draw_tasks(frame, area, "Project A", &[], 0, true, false, &mut targets);
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

// P2-A1: build_detail_content produces boxed lines (rounded corners + comment author)
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
    let lines = build_detail_content(&task, &[comment], &user_map, inner_width).lines;

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
            let mut targets = vec![];
            render_table(
                frame,
                ratatui::layout::Rect::new(0, 0, 80, 10),
                "Test Title",
                &["COL A", "COL B"],
                rows,
                &[Constraint::Min(10), Constraint::Min(10)],
                0,
                &[1],
                &mut targets,
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
                revalidating: false,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        }
    }

    #[test]
    fn viewport_below_threshold_renders_only_too_small_message() {
        use crate::i18n::set_language;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let model = projects_model();
        terminal
            .draw(|frame| view(&model, frame, &mut vec![]))
            .unwrap();
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
        use crate::i18n::set_language;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let model = projects_model();
        terminal
            .draw(|frame| view(&model, frame, &mut vec![]))
            .unwrap();
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
    let row_heights: Vec<u16> = vec![1u16; row_count];
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let mut targets = vec![];
            render_table(
                frame,
                ratatui::layout::Rect::new(0, 0, width, height),
                "Test",
                &["NAME"],
                rows,
                &[Constraint::Min(0)],
                selected,
                &row_heights,
                &mut targets,
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

// P2-A1: build_detail_content at different widths produces different line counts/widths
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
    let lines_80 = build_detail_content(&task, &comments, &user_map, 80).lines;
    let lines_40 = build_detail_content(&task, &comments, &user_map, 40).lines;

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
// The title block must appear and the content from build_detail_content must be present.
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
    let lines = build_detail_content(&task, &[comment], &user_map, 76).lines;

    let assets = vec![Asset {
        name: "file.pdf".into(),
        url: "https://example.com/file.pdf".into(),
    }];

    let buf = render_detail_to_buf_with_name(&lines, &assets, 0, 80, 40, "Test Task");
    let content = buf_to_string(&buf);

    // The task name appears via the Título meta row inside the Details panel.
    assert!(
        content.contains("Test Task"),
        "content must contain the task name 'Test Task' (via Título row): {content}"
    );
    assert!(
        !content.contains("Task #42"),
        "content must NOT contain 'Task #42': {content}"
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

// D1a-A1: the Details panel contains a Title row directly after the Task row.
// The title appears exactly once (via the row), not as a separate floating header.
#[test]
fn draw_detail_title_row_present_after_task_row_in_details_panel() {
    let _guard = LANG_MUTEX.lock().unwrap();
    set_language("en");
    let task = json!({
        "name": "OSV-Scanner",
        "id": 71583,
        "project_id": 725,
        "project_name": "Base",
        "is_completed": false
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let detail = build_detail_content(&task, &[], &user_map, 76);
    let buf = render_detail_to_buf_with_name(&detail.lines, &[], 0, 80, 30, "OSV-Scanner");
    let content = buf_to_string(&buf);
    set_language("en");

    assert!(
        content.contains("Title"),
        "Details panel must contain a 'Title' row: {content}"
    );
    assert!(
        content.contains("OSV-Scanner"),
        "Title row must carry the task name: {content}"
    );
    let task_pos = content.find("Task").expect("Task row must be present");
    let title_pos = content.find("Title").expect("Title row must be present");
    assert!(
        task_pos < title_pos,
        "Title row must appear after Task row: task_pos={task_pos} title_pos={title_pos}"
    );
}

// D1a-A2: the task name must NOT appear as a standalone bold header line above the Details
// panel. The content block renders only the `lines` passed in (the Título meta row inside the
// Details panel carries the name via build_detail_content, not via a separate header).
#[test]
fn draw_detail_task_name_not_rendered_as_loose_header() {
    let task_name = "My Important Task";
    let lines = vec!["some content".to_string()];
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 80, 10, task_name);
    let content = buf_to_string(&buf);
    let rows: Vec<&str> = content.lines().collect();
    // The name must NOT appear as a bold header row separate from the passed-in lines.
    // Since the lines only contain "some content", the task_name must not appear at all
    // (it is NOT injected by draw_detail any more).
    assert!(
        !content.contains(task_name),
        "task_name must NOT be rendered as a loose header by draw_detail: {content}"
    );
    // The Block border (row 0) must also not contain the name.
    let top_border = rows[0];
    assert!(
        !top_border.contains(task_name),
        "Block border must NOT contain the task name: {top_border}"
    );
}

// W2-A1: when task name is empty, no placeholder appears in the border (border is clean).
// The loading fallback path still uses the id — but non-loading with empty name shows empty border.
#[test]
fn draw_detail_empty_task_name_shows_no_name_in_border() {
    let lines = vec!["some content".to_string()];
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 80, 10, "");
    let content = buf_to_string(&buf);
    let rows: Vec<&str> = content.lines().collect();
    let top_border = rows[0];
    // Empty name → border title is empty; no id fallback in non-loading state.
    assert!(
        !top_border.contains('#'),
        "Block border must NOT contain '#id' fallback for non-loading empty name: {top_border}"
    );
}

// D1a-A2: a long task_name must not be injected as a loose header even for very long names.
// The passed-in `lines` are rendered as-is with no extra bold header rows.
#[test]
fn draw_detail_long_task_name_not_injected_as_header() {
    let very_long_name =
        "This Is An Extremely Long Task Name That Does Not Fit In The Border At All";
    let lines = vec!["content".to_string()];
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 40, 20, very_long_name);
    let content = buf_to_string(&buf);
    let rows: Vec<&str> = content.lines().collect();
    let top_border = rows[0];
    // The name must NOT appear in the buffer (it is not injected by draw_detail).
    assert!(
        !content.contains("This Is An"),
        "long task_name must NOT be injected as a header by draw_detail: {content}"
    );
    // The Block border must not contain the name either.
    assert!(
        !top_border.contains("This Is"),
        "Block border must NOT contain the task name: {top_border}"
    );
    // The body content must still render.
    assert!(
        content.contains("content"),
        "body content must still render: {content}"
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
            line_styles: vec![],
            assets,
            offset: 0,
            loading: false,
            rendered_width: 80,
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view(&model, frame, &mut vec![]))
        .unwrap();
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
        !content.contains("1-9"),
        "Detail footer must NOT contain '1-9 open asset' hint (numeric scheme removed): {content}"
    );
    assert!(
        content.contains("Ctrl") || content.contains("Cmd") || content.contains("click"),
        "Detail footer must contain Ctrl/Cmd+click model hint: {content}"
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
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: 80,
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view(&model, frame, &mut vec![]))
        .unwrap();
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
            revalidating: false,
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view(&model, frame, &mut vec![]))
        .unwrap();
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
            revalidating: false,
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let height = 10u16;
    let backend = TestBackend::new(80, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view(&model, frame, &mut vec![]))
        .unwrap();
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
            revalidating: false,
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view(&model, frame, &mut vec![]))
        .unwrap();
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
    let _guard = LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    set_language("en");
    use crate::tui::model::{Model, Screen};
    use crate::tui::view::view;

    let inst = make_instance("prod", "x@example.com");
    let header = Header::from_instances(&[inst], None);

    let model = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header,
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let backend = TestBackend::new(20, 5);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| view(&model, frame, &mut vec![]))
        .unwrap();
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

// V5-D3-A1: URL in description renders inline and the URL cells carry link_style
// (muted-green + underline). Surrounding text keeps default style.
#[test]
fn draw_detail_url_in_description_body_has_link_style() {
    use crate::render::build_detail_content;
    use ratatui::style::{Color, Modifier};

    let task = json!({
        "id": 1,
        "name": "Task With Link",
        "body": "<p>Visit https://example.com/docs for more info.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let width: u16 = 80;
    let content = build_detail_content(&task, &[], &user_map, (width - 2) as usize);
    let lines = content.lines;

    let joined = lines.join("\n");
    assert!(
        joined.contains("https://example.com/docs"),
        "inline URL must appear in lines: {joined}"
    );

    let buf = render_detail_to_buf(&lines, &[], 0, width, 30);

    let muted_green = Color::Rgb(120, 190, 130);
    let underline = Modifier::UNDERLINED;

    let mut found_link_cell = false;
    let mut found_normal_cell = false;
    let area = buf.area();

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            let sym = cell.symbol();
            // Any cell that is part of the URL must carry link_style
            if cell.style().fg == Some(muted_green) && cell.style().add_modifier.contains(underline)
            {
                found_link_cell = true;
            }
            // "V" in "Visit" must not carry link style
            if sym == "V" && cell.style().fg != Some(muted_green) {
                found_normal_cell = true;
            }
        }
    }

    assert!(
        found_link_cell,
        "inline URL cells must carry muted-green+underline link_style"
    );
    assert!(
        found_normal_cell,
        "Non-URL text cells (e.g. 'V' in 'Visit') must NOT carry link_style"
    );
}

// V5-D3-A2: URL in a comment body renders inline; the URL cells carry link_style
// (muted-green + underline).
#[test]
fn draw_detail_url_in_comment_body_has_link_style() {
    use crate::render::build_detail_content;
    use ratatui::style::{Color, Modifier};

    let task = json!({
        "id": 1,
        "name": "Task",
        "body": "<p>Description</p>"
    });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1614556800i64,
        "body_plain_text": "See https://docs.example.com/guide for reference"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let width: u16 = 80;
    let content = build_detail_content(&task, &[comment], &user_map, (width - 2) as usize);
    let lines = content.lines;

    let joined = lines.join("\n");
    assert!(
        joined.contains("https://docs.example.com/guide"),
        "inline URL must appear in comment lines: {joined}"
    );

    let buf = render_detail_to_buf(&lines, &[], 0, width, 40);

    let muted_green = Color::Rgb(120, 190, 130);
    let underline = Modifier::UNDERLINED;

    let mut found_link_cell = false;
    let area = buf.area();

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            if cell.style().fg == Some(muted_green) && cell.style().add_modifier.contains(underline)
            {
                found_link_cell = true;
                break;
            }
        }
        if found_link_cell {
            break;
        }
    }

    assert!(
        found_link_cell,
        "inline URL cells in Comment body must carry muted-green+underline link_style"
    );
}

// D3-A2: Border chars │ are never styled as links; a no-URL line renders without link_style.
#[test]
fn draw_detail_border_and_no_url_lines_have_default_style() {
    use ratatui::style::{Color, Modifier};

    let task = json!({
        "id": 1,
        "name": "Task",
        "body": "<p>No links here at all.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let width: u16 = 80;
    let lines = build_detail_content(&task, &[], &user_map, (width - 2) as usize).lines;

    let buf = render_detail_to_buf(&lines, &[], 0, width, 20);

    let muted_green = Color::Rgb(120, 190, 130);
    let underline = Modifier::UNDERLINED;
    let area = buf.area();

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            let sym = cell.symbol();
            // │ border chars must never carry link_style
            if sym == "\u{2502}" {
                assert!(
                    cell.style().fg != Some(muted_green),
                    "│ border at ({x},{y}) must NOT carry link fg color"
                );
                assert!(
                    !cell.style().add_modifier.contains(underline),
                    "│ border at ({x},{y}) must NOT carry underline modifier"
                );
            }
            // No cell should carry link style in a no-URL description
            assert!(
                !(cell.style().fg == Some(muted_green)
                    && cell.style().add_modifier.contains(underline)),
                "cell ({x},{y}) sym={sym:?} must NOT carry link_style when no URL present"
            );
        }
    }
}

// D3-A1: link_style fn returns muted green + underline.
#[test]
fn link_style_is_muted_green_underlined() {
    use ratatui::style::{Color, Modifier, Style};
    let style = theme::link_style();
    assert_eq!(
        style,
        Style::default()
            .fg(Color::Rgb(120, 190, 130))
            .add_modifier(Modifier::UNDERLINED),
        "link_style must be muted-green+underlined matching asset_style color"
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

// V2b click-target tests — verifies the renderer-recorded hit-map drives drill-in correctly.
mod v2b_click_targets {
    use crate::tui::model::{update, Header, Model, Msg, ProjectGroup, Screen, TaskRow};
    use crate::tui::view::view;
    use crossterm::event::KeyModifiers;
    use ratatui::{backend::TestBackend, Terminal};

    fn empty_header() -> Header {
        Header::from_instances(&[], None)
    }

    fn make_task(id: i64, name: &str) -> TaskRow {
        TaskRow {
            task_id: id,
            task_number: id,
            name: name.to_string(),
            instance: "inst".into(),
            project_id: 0,
            due_on: None,
        }
    }

    fn render_and_capture(model: &mut Model, width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut targets = vec![];
        terminal
            .draw(|frame| view(model, frame, &mut targets))
            .unwrap();
        model.set_click_targets(targets);
    }

    // V2b-A1 projects case: a long project name wraps across 2 terminal rows.
    // Clicking anywhere within those 2 rows drills into that project's tasks at the correct index.
    // At width=30 the name_width is 30-4=26; "Alpha Beta Gamma Delta Extra" wraps.
    // Terminal layout: row0=app_header, row1=top_border, row2=column_header,
    // row3=first_data_row, row4=wrap_continuation. Clicking y=4 must resolve to index 0.
    #[test]
    fn click_on_wrapped_projects_row_drills_into_correct_project() {
        let long_name = "Alpha Beta Gamma Delta Extra";
        assert!(long_name.len() > 26, "name must wrap at name_width=26");
        let groups = vec![
            ProjectGroup {
                project_id: 0,
                project_name: long_name.to_string(),
                instance: "inst".into(),
                tasks: vec![make_task(10, "Task A")],
            },
            ProjectGroup {
                project_id: 1,
                project_name: "ShortB".to_string(),
                instance: "inst".into(),
                tasks: vec![make_task(20, "Task B")],
            },
        ];

        let mut model = Model {
            stack: vec![Screen::Projects {
                groups,
                selected: 0,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header: empty_header(),
            viewport: (30, 15),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };

        render_and_capture(&mut model, 30, 15);

        let targets = model.click_targets.clone();
        assert!(
            !targets.is_empty(),
            "hit-map must be populated after render"
        );

        // The first target (index=0) covers the wrapped row, so its y_end > y_start+1.
        let first_target = targets
            .iter()
            .find(|t| t.index == 0)
            .expect("index 0 must exist");
        assert!(
            first_target.y_end > first_target.y_start + 1,
            "wrapped row must span more than one terminal line: {first_target:?}"
        );

        // Click on the continuation line (y_start + 1, inside the wrap) must resolve to index 0.
        let click_y = first_target.y_start + 1;
        let (model_after, cmds) = update(
            model,
            Msg::Click {
                column: 5,
                row: click_y,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(cmds.is_empty(), "PushTasks must not emit async cmds");
        match model_after.top() {
            Some(Screen::Tasks {
                project_name,
                tasks,
                ..
            }) => {
                assert_eq!(
                    project_name, long_name,
                    "clicked project (index 0) must push Tasks for '{long_name}'"
                );
                assert_eq!(tasks.len(), 1, "project 0 has 1 task");
                assert_eq!(tasks[0].task_id, 10, "task_id must match project 0's task");
            }
            other => panic!("expected Tasks screen, got {other:?}"),
        }
    }

    // V2b-A1 (tasks → detail): clicking a task row pushes a Detail screen (same as Enter).
    // Uses a 1-line-height row so the geometry is unambiguous.
    #[test]
    fn click_on_tasks_row_pushes_detail_screen() {
        let tasks = vec![
            make_task(100, "First Task"),
            make_task(200, "Second Task"),
            make_task(300, "Third Task"),
        ];

        let mut model = Model {
            stack: vec![Screen::Tasks {
                project_name: "Proj".into(),
                tasks,
                selected: 0,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header: empty_header(),
            viewport: (80, 15),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };

        render_and_capture(&mut model, 80, 15);

        let targets = model.click_targets.clone();
        assert!(
            !targets.is_empty(),
            "hit-map must be populated after render"
        );

        // Click on the second task (index 1).
        let target = targets
            .iter()
            .find(|t| t.index == 1)
            .expect("index 1 must exist");
        let click_y = target.y_start;

        let (model_after, cmds) = update(
            model,
            Msg::Click {
                column: 5,
                row: click_y,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(cmds.len(), 1, "clicking a task must emit LoadDetail");
        match model_after.top() {
            Some(Screen::Detail {
                task_id, loading, ..
            }) => {
                assert_eq!(
                    *task_id, 200,
                    "Detail must open task with task_id=200 (index 1)"
                );
                assert!(*loading, "Detail must start in loading state");
            }
            other => panic!("expected Detail screen, got {other:?}"),
        }
    }

    // V2b-A2 (empty space no-op): clicking below the last visible row is a no-op.
    // Renders 2 rows in a tall terminal; any y below the second row's y_end is a no-op.
    #[test]
    fn click_below_last_row_is_noop() {
        let groups = vec![
            ProjectGroup {
                project_id: 0,
                project_name: "Alpha".into(),
                instance: "i".into(),
                tasks: vec![make_task(1, "T1")],
            },
            ProjectGroup {
                project_id: 1,
                project_name: "Beta".into(),
                instance: "i".into(),
                tasks: vec![make_task(2, "T2")],
            },
        ];

        let mut model = Model {
            stack: vec![Screen::Projects {
                groups,
                selected: 0,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header: empty_header(),
            viewport: (80, 20),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };

        render_and_capture(&mut model, 80, 20);

        let targets = model.click_targets.clone();
        assert!(!targets.is_empty(), "hit-map must be populated");

        // Find a y that is beyond all recorded targets.
        let max_y_end = targets.iter().map(|t| t.y_end).max().unwrap_or(0);
        let below_y = max_y_end + 2;

        let stack_depth_before = model.stack.len();
        let selected_before = match model.top() {
            Some(Screen::Projects { selected, .. }) => *selected,
            _ => panic!("expected Projects"),
        };

        let (model_after, cmds) = update(
            model,
            Msg::Click {
                column: 5,
                row: below_y,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(cmds.is_empty(), "click below all rows must emit no cmds");
        assert_eq!(
            model_after.stack.len(),
            stack_depth_before,
            "click below all rows must not push a new screen"
        );
        match model_after.top() {
            Some(Screen::Projects { selected, .. }) => {
                assert_eq!(
                    *selected, selected_before,
                    "click below all rows must not change selection"
                );
            }
            other => panic!("expected Projects, got {other:?}"),
        }
    }

    // V2b-A2 (scroll offset): after the list has scrolled (offset>0), the hit-map
    // records the correct visible rows so clicks resolve to the right (scrolled) index.
    // Uses a narrow height to force scrolling.
    #[test]
    fn click_resolves_correct_index_after_scroll() {
        let tasks: Vec<TaskRow> = (0..10)
            .map(|i| make_task(i as i64, &format!("Task {i:02}")))
            .collect();

        // height=6 → 1 header bar + 1 top border + 1 col header + 3 data rows = 6.
        // ratatui will scroll to keep the selected row visible.
        // Select row 7 so the widget scrolls, offset > 0.
        let mut model = Model {
            stack: vec![Screen::Tasks {
                project_name: "Proj".into(),
                tasks,
                selected: 7,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header: empty_header(),
            viewport: (80, 6),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };

        render_and_capture(&mut model, 80, 6);

        let targets = model.click_targets.clone();
        assert!(
            !targets.is_empty(),
            "hit-map must be populated after scroll"
        );

        // Every target index must be >= the scroll offset (i.e. not a hidden row).
        let min_idx = targets.iter().map(|t| t.index).min().unwrap_or(0);
        assert!(
            min_idx > 0,
            "after scrolling to row 7, all visible targets must have index > 0 (got min_idx={min_idx})"
        );

        // Clicking the first visible target's y_start must drill into THAT task (not index 0).
        let first_visible = targets.iter().min_by_key(|t| t.y_start).unwrap().clone();
        let expected_task_id = first_visible.index as i64;
        let click_y = first_visible.y_start;

        let (model_after, cmds) = update(
            model,
            Msg::Click {
                column: 5,
                row: click_y,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(cmds.len(), 1, "must emit LoadDetail cmd");
        match model_after.top() {
            Some(Screen::Detail { task_id, .. }) => {
                assert_eq!(
                    *task_id, expected_task_id,
                    "Detail must open the task at the clicked (scrolled) index={}",
                    first_visible.index
                );
            }
            other => panic!("expected Detail screen, got {other:?}"),
        }
    }
}

mod footer_refresh_hint {
    use crate::i18n::set_language;
    use crate::tui::model::{DetailLoad, Header, Model, Msg, ProjectGroup, Screen, TaskRow};
    use crate::tui::view::view;
    use ratatui::{backend::TestBackend, Terminal};
    use std::collections::HashMap;

    fn buf_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf.cell((x, y)).unwrap().symbol());
            }
            out.push('\n');
        }
        out
    }

    fn projects_model_with_last_loaded(last_loaded: Option<String>) -> Model {
        Model {
            stack: vec![Screen::Projects {
                groups: vec![ProjectGroup {
                    project_id: 0,
                    project_name: "A Project".into(),
                    instance: "inst".into(),
                    tasks: vec![TaskRow {
                        task_id: 0,
                        task_number: 1,
                        name: "Task 0".into(),
                        instance: "inst".into(),
                        project_id: 0,
                        due_on: None,
                    }],
                }],
                selected: 0,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded,
            selection: None,
            copied_feedback: false,
        }
    }

    fn render_model(model: &Model) -> String {
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| view(model, frame, &mut vec![]))
            .unwrap();
        buf_to_string(terminal.backend().buffer())
    }

    // R1-A1: footer on a Projects screen (last_loaded=None) shows 'r refresh' (en)
    // and does NOT show any 'Updated at' text.
    #[test]
    fn projects_footer_shows_refresh_token_in_en_when_last_loaded_none() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = projects_model_with_last_loaded(None);
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("r refresh"),
            "footer must contain 'r refresh' (en) when last_loaded=None: {content}"
        );
        assert!(
            !content.contains("Updated at"),
            "footer must NOT contain 'Updated at' when last_loaded=None: {content}"
        );
    }

    // R1-A1: footer on a Projects screen (last_loaded=None) shows 'r atualizar' in pt_BR.
    #[test]
    fn projects_footer_shows_refresh_token_in_pt_br_when_last_loaded_none() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("pt_BR");
        let model = projects_model_with_last_loaded(None);
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("r atualizar"),
            "footer must contain 'r atualizar' (pt_BR) when last_loaded=None: {content}"
        );
        assert!(
            !content.contains("Atualizado"),
            "footer must NOT contain 'Atualizado' when last_loaded=None: {content}"
        );
    }

    // R1-A1: footer on a Detail screen without assets shows 'r refresh'.
    #[test]
    fn detail_footer_without_assets_shows_refresh_token() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = Model {
            stack: vec![Screen::Detail {
                instance: "inst".into(),
                project_id: 1,
                task_id: 7,
                task: serde_json::Value::Null,
                comments: vec![],
                user_map: HashMap::new(),
                lines: vec!["body".into()],
                line_styles: vec![],
                assets: vec![],
                offset: 0,
                loading: false,
                rendered_width: 80,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("r refresh"),
            "Detail footer (no assets) must contain 'r refresh': {content}"
        );
    }

    // R1-A1: footer on a Detail screen with assets shows 'r refresh'.
    #[test]
    fn detail_footer_with_assets_shows_refresh_token() {
        use crate::render::Asset;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = Model {
            stack: vec![Screen::Detail {
                instance: "inst".into(),
                project_id: 1,
                task_id: 7,
                task: serde_json::Value::Null,
                comments: vec![],
                user_map: HashMap::new(),
                lines: vec!["body".into()],
                line_styles: vec![],
                assets: vec![Asset {
                    name: "doc.pdf".into(),
                    url: "https://example.com/doc.pdf".into(),
                }],
                offset: 0,
                loading: false,
                rendered_width: 80,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("r refresh"),
            "Detail footer (with assets) must contain 'r refresh': {content}"
        );
    }

    // R1b-A1: after a LoadedTasksByProject msg with BRT loaded_at, footer shows date+time DD/MM/YYYY HH:MM.
    #[test]
    fn footer_shows_date_and_time_after_load_msg_en() {
        use crate::tui::model::update;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = projects_model_with_last_loaded(None);
        let (model, _) = update(
            model,
            Msg::LoadedTasksByProject {
                groups: vec![ProjectGroup {
                    project_id: 0,
                    project_name: "A Project".into(),
                    instance: "inst".into(),
                    tasks: vec![],
                }],
                loaded_at: "2026-06-25T11:07:03".into(),
            },
        );
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("Updated at"),
            "footer must contain 'Updated at' after load: {content}"
        );
        assert!(
            content.contains("25/06/2026 11:07"),
            "footer must show date+time '25/06/2026 11:07' (DD/MM/YYYY HH:MM): {content}"
        );
    }

    // R1b-A1: in pt_BR the timestamp shows 'Atualizado em DD/MM/YYYY HH:MM'.
    #[test]
    fn footer_shows_atualizado_em_date_time_in_pt_br() {
        use crate::tui::model::update;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("pt_BR");
        let model = projects_model_with_last_loaded(None);
        let (model, _) = update(
            model,
            Msg::LoadedTasksByProject {
                groups: vec![],
                loaded_at: "2026-06-25T09:30:00".into(),
            },
        );
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("Atualizado em"),
            "footer must contain 'Atualizado em' (pt_BR) after load: {content}"
        );
        assert!(
            content.contains("25/06/2026 09:30"),
            "footer must show date+time '25/06/2026 09:30' in pt_BR: {content}"
        );
    }

    // R1-A2: when last_loaded is None, no 'Updated at' or timestamp appears in the footer.
    #[test]
    fn footer_no_timestamp_text_when_last_loaded_none() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = projects_model_with_last_loaded(None);
        let content = render_model(&model);
        set_language("en");
        assert!(
            !content.contains("Updated at"),
            "footer must NOT contain 'Updated at' when last_loaded=None: {content}"
        );
        assert!(
            !content.contains("Atualizado"),
            "footer must NOT contain 'Atualizado' when last_loaded=None: {content}"
        );
    }

    // R1b-A1: dispatching two loads in sequence updates the footer date+time.
    #[test]
    fn footer_datetime_updates_after_second_load() {
        use crate::tui::model::update;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");

        let model = projects_model_with_last_loaded(None);

        let (model, _) = update(
            model,
            Msg::LoadedTasksByProject {
                groups: vec![],
                loaded_at: "2026-06-25T10:00:00".into(),
            },
        );
        let content_first = render_model(&model);

        let (model, _) = update(
            model,
            Msg::LoadedTasksByProject {
                groups: vec![],
                loaded_at: "2026-06-25T11:45:00".into(),
            },
        );
        let content_second = render_model(&model);
        set_language("en");

        assert!(
            content_first.contains("25/06/2026 10:00"),
            "first load must show '25/06/2026 10:00': {content_first}"
        );
        assert!(
            content_second.contains("25/06/2026 11:45"),
            "second load must update footer to '25/06/2026 11:45': {content_second}"
        );
        assert!(
            !content_second.contains("10:00"),
            "second footer must NOT show the old time '10:00': {content_second}"
        );
    }

    // R1b-A2: format_br_datetime converts BRT ISO string to DD/MM/YYYY HH:MM.
    #[test]
    fn format_br_datetime_valid_produces_dd_mm_yyyy_hhmm() {
        use crate::tui::view::format_br_datetime;
        assert_eq!(
            format_br_datetime("2026-06-25T11:07:03"),
            Some("25/06/2026 11:07".to_string()),
            "valid BRT ISO must produce DD/MM/YYYY HH:MM"
        );
    }

    // R1b-A2: format_br_datetime returns None for a string shorter than 16 chars.
    #[test]
    fn format_br_datetime_short_input_returns_none() {
        use crate::tui::view::format_br_datetime;
        assert_eq!(
            format_br_datetime("2026-06"),
            None,
            "input shorter than 16 chars must return None"
        );
        assert_eq!(format_br_datetime(""), None, "empty input must return None");
    }

    // R1b-A2: format_br_datetime handles minimum-length input (exactly 16 chars).
    #[test]
    fn format_br_datetime_minimum_length_input_produces_result() {
        use crate::tui::view::format_br_datetime;
        assert_eq!(
            format_br_datetime("2026-06-25T11:07"),
            Some("25/06/2026 11:07".to_string()),
            "exactly 16-char input must produce DD/MM/YYYY HH:MM"
        );
    }

    // R1b-A2: now_brt_iso returns a no-Z timestamp in YYYY-MM-DDTHH:MM:SS format.
    #[test]
    fn now_brt_iso_returns_no_z_timestamp() {
        let ts = crate::store::now_brt_iso();
        assert!(
            !ts.ends_with('Z'),
            "now_brt_iso must NOT end with 'Z' (it is not UTC): {ts}"
        );
        assert_eq!(
            ts.len(),
            19,
            "now_brt_iso must be 19 chars (YYYY-MM-DDTHH:MM:SS): {ts}"
        );
        assert_eq!(&ts[4..5], "-", "separator at index 4 must be '-': {ts}");
        assert_eq!(&ts[7..8], "-", "separator at index 7 must be '-': {ts}");
        assert_eq!(&ts[10..11], "T", "separator at index 10 must be 'T': {ts}");
        assert_eq!(&ts[13..14], ":", "separator at index 13 must be ':': {ts}");
        assert_eq!(&ts[16..17], ":", "separator at index 16 must be ':': {ts}");
    }

    // V5-A2: scrollbar thumb position at max_offset vs at offset 0.
    // Render Detail (draw_detail directly, not view) so area equals the full passed rect.
    // Empty task_name so the body viewport = area.height - 2 (no name header row).
    // With width=40, height=22, 50 lines, no assets:
    //   render_content viewport_height = 22-2 = 20
    //   max_offset = 50 - 20 = 30
    // At offset=30 the thumb must be in the bottom half of the rightmost column.
    // At offset=0 the thumb must NOT be in the bottom half.
    #[test]
    fn scrollbar_thumb_reaches_bottom_at_max_offset_and_not_at_offset_zero() {
        use crate::tui::screens::detail::draw_detail;
        use crate::tui::screens::DetailParams;

        let width: u16 = 40;
        let height: u16 = 22;
        let lines: Vec<String> = (1..=50).map(|i| format!("line {i:02}")).collect();
        // Empty task_name → no name header → body viewport = height - 2 = 20.
        let viewport_height = (height - 2) as usize;
        let max_offset = lines.len().saturating_sub(viewport_height);

        let render = |offset: usize| -> ratatui::buffer::Buffer {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|frame| {
                    draw_detail(
                        frame,
                        ratatui::layout::Rect::new(0, 0, width, height),
                        DetailParams {
                            lines: &lines,
                            line_styles: &[],
                            assets: &[],
                            offset,
                            loading: false,
                            task_id: 1,
                            task_name: "",
                        },
                    );
                })
                .unwrap();
            terminal.backend().buffer().clone()
        };

        let buf_max = render(max_offset);
        let buf_top = render(0);

        let rightmost_x = width - 1;
        // The scrollbar track occupies the inner rows of the rightmost column
        // (ratatui renders ↑/↓ arrows at the very top/bottom of the widget area).
        // Check the bottom quarter of the area for the thumb glyph.
        let bottom_start = height * 3 / 4;

        let thumb_in_bottom_at_max = (bottom_start..height).any(|y| {
            buf_max
                .cell((rightmost_x, y))
                .map(|c| c.symbol() == "█")
                .unwrap_or(false)
        });
        let thumb_in_bottom_at_top = (bottom_start..height).any(|y| {
            buf_top
                .cell((rightmost_x, y))
                .map(|c| c.symbol() == "█")
                .unwrap_or(false)
        });

        assert!(
            thumb_in_bottom_at_max,
            "scrollbar thumb must appear in the bottom quarter of the rightmost column when offset=max_offset={max_offset}"
        );
        assert!(
            !thumb_in_bottom_at_top,
            "scrollbar thumb must NOT be in the bottom quarter when offset=0 (50 lines, vh={viewport_height})"
        );
    }

    // R1b-A1: LoadedDetail msg also stamps last_loaded and appears in footer as date+time.
    #[test]
    fn footer_shows_date_time_after_loaded_detail_msg() {
        use crate::tui::model::update;
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = Model {
            stack: vec![Screen::Detail {
                instance: "inst".into(),
                project_id: 1,
                task_id: 7,
                task: serde_json::Value::Null,
                comments: vec![],
                user_map: HashMap::new(),
                lines: vec![],
                line_styles: vec![],
                assets: vec![],
                offset: 0,
                loading: true,
                rendered_width: usize::MAX,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback: false,
        };
        let load = DetailLoad {
            task: serde_json::json!({ "name": "T", "id": 7, "project_id": 1 }),
            comments: vec![],
            assets: vec![],
            user_map: HashMap::new(),
            loaded_at: "2026-06-25T16:22:00".into(),
        };
        let (model, _) = update(model, Msg::LoadedDetail(load));
        let content = render_model(&model);
        set_language("en");
        assert!(
            content.contains("25/06/2026 16:22"),
            "footer must show '25/06/2026 16:22' (DD/MM/YYYY HH:MM) after LoadedDetail: {content}"
        );
        assert!(
            content.contains("Updated at"),
            "footer must contain 'Updated at' after LoadedDetail: {content}"
        );
    }
}

// --- V4a: TestBackend render — comment with long URL shows label, not raw URL ---

// V5-A3: a Detail screen whose comment holds a long URL renders the URL inline.
// The URL cells carry link_style (muted-green + underline). The full URL appears
// in the rendered output (possibly wrapped across lines).
#[test]
fn draw_detail_comment_with_long_url_renders_inline_with_link_style() {
    use crate::render::build_detail_content;
    use ratatui::style::{Color, Modifier};

    let long_url = "https://very-long-domain.example.com/path/to/resource?param=value&other=thing";
    let task = json!({
        "id": 1,
        "name": "Link Label Test",
        "body": "<p>Task description without URL.</p>"
    });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1614556800i64,
        "body_plain_text": format!("Check {long_url} for the spec.")
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let width: u16 = 80;
    let inner_width = (width - 2) as usize;

    let content = build_detail_content(&task, &[comment], &user_map, inner_width);
    let lines = content.lines;

    let joined = lines.join("\n");
    assert!(
        joined.contains("https://"),
        "inline URL must appear in rendered lines: {joined}"
    );

    let buf = render_detail_to_buf(&lines, &[], 0, width, 40);
    let rendered = buf_to_string(&buf);

    assert!(
        rendered.contains("https://"),
        "inline URL must appear in TestBackend output: {rendered}"
    );

    let muted_green = Color::Rgb(120, 190, 130);
    let underline = Modifier::UNDERLINED;
    let area = buf.area();
    let mut found_link_cell = false;

    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            if cell.style().fg == Some(muted_green) && cell.style().add_modifier.contains(underline)
            {
                found_link_cell = true;
                break;
            }
        }
        if found_link_cell {
            break;
        }
    }

    assert!(
        found_link_cell,
        "inline URL cells must carry muted-green+underline link_style"
    );
}

// --- V6: footer indicator and highlight rendering tests ---

mod v6_view {
    use crate::i18n::set_language;
    use crate::tui::model::{Header, Model, Screen, Selection};
    use crate::tui::view::view;
    use ratatui::{backend::TestBackend, Terminal};

    fn buf_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf.cell((x, y)).unwrap().symbol());
            }
            out.push('\n');
        }
        out
    }

    fn render_view(model: &Model) -> String {
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| view(model, frame, &mut vec![]))
            .unwrap();
        buf_to_string(terminal.backend().buffer())
    }

    fn projects_model(copied_feedback: bool) -> Model {
        Model {
            stack: vec![Screen::Projects {
                groups: vec![],
                selected: 0,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection: None,
            copied_feedback,
        }
    }

    fn detail_model_with_selection(lines: Vec<String>, sel: Option<Selection>) -> Model {
        use std::collections::HashMap;
        Model {
            stack: vec![Screen::Detail {
                instance: "inst".into(),
                project_id: 1,
                task_id: 1,
                task: serde_json::Value::Null,
                comments: vec![],
                user_map: HashMap::new(),
                lines,
                line_styles: vec![],
                assets: vec![],
                offset: 0,
                loading: false,
                rendered_width: 120,
            }],
            should_quit: false,
            header: Header::from_instances(&[], None),
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded: None,
            selection: sel,
            copied_feedback: false,
        }
    }

    // V6-A5 (Sc6): V3 selection indicator ('SELEÇÃO') is gone — no longer shown.
    #[test]
    fn v3_selection_indicator_not_shown_in_footer() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("pt_BR");
        let model = projects_model(false);
        let content = render_view(&model);
        set_language("en");
        assert!(
            !content.contains("SELEÇÃO"),
            "V3 'SELEÇÃO' indicator must NOT appear (V3 retired): {content}"
        );
    }

    // V6-A2: footer shows 'COPIADO' indicator when copied_feedback=true (pt_BR).
    #[test]
    fn footer_shows_copied_indicator_when_copied_feedback_true() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("pt_BR");
        let model = projects_model(true);
        let content = render_view(&model);
        set_language("en");
        assert!(
            content.contains("COPIADO"),
            "footer must show 'COPIADO' when copied_feedback=true: {content}"
        );
    }

    // V6-A2: footer omits 'COPIADO' indicator when copied_feedback=false.
    #[test]
    fn footer_omits_copied_indicator_when_copied_feedback_false() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("pt_BR");
        let model = projects_model(false);
        let content = render_view(&model);
        set_language("en");
        assert!(
            !content.contains("COPIADO"),
            "footer must NOT show 'COPIADO' when copied_feedback=false: {content}"
        );
    }

    // V6-A5 (Sc6): 's selection' hint no longer appears in footer — V3 removed.
    #[test]
    fn footer_hint_does_not_contain_s_selection_after_v3_removed() {
        let _guard = super::LANG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        set_language("en");
        let model = projects_model(false);
        let content = render_view(&model);
        set_language("en");
        assert!(
            !content.contains("s selection"),
            "V3 's selection' hint must NOT appear in footer (V3 retired): {content}"
        );
    }

    // V6-A1 (Sc1 drawn feedback): Selected cells render with REVERSED modifier.
    // Use a small viewport so we can precisely control which cells are selected.
    #[test]
    fn selected_cells_render_with_reversed_modifier() {
        use ratatui::style::Modifier;

        let lines = vec!["hello world".to_string()];
        let sel = Some(Selection {
            anchor: (2, 1),
            cursor: (2, 5),
        });
        let model = detail_model_with_selection(lines, sel);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| view(&model, frame, &mut vec![]))
            .unwrap();
        let buf = terminal.backend().buffer();

        // Cells in column range [1..=5] on row 2 must carry the REVERSED modifier.
        let mut found_reversed = false;
        for col in 1u16..=5 {
            let cell = buf.cell((col, 2)).unwrap();
            if cell.style().add_modifier.contains(Modifier::REVERSED) {
                found_reversed = true;
                break;
            }
        }
        assert!(
            found_reversed,
            "at least one cell in the selection range must carry REVERSED modifier (V6 highlight)"
        );
    }

    // V6-A1 (Sc1 drawn feedback): Cells outside the selection span do NOT carry REVERSED.
    #[test]
    fn cells_outside_selection_do_not_carry_reversed_modifier() {
        use ratatui::style::Modifier;

        let lines = vec!["hello world".to_string()];
        let sel = Some(Selection {
            anchor: (2, 2),
            cursor: (2, 4),
        });
        let model = detail_model_with_selection(lines, sel);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| view(&model, frame, &mut vec![]))
            .unwrap();
        let buf = terminal.backend().buffer();

        // Column 0 on row 2 is outside [2..=4] — must not carry REVERSED.
        let cell_before = buf.cell((0u16, 2u16)).unwrap();
        assert!(
            !cell_before
                .style()
                .add_modifier
                .contains(Modifier::REVERSED),
            "cell before selection span must NOT carry REVERSED modifier"
        );
    }
}

// --- W2: Detail chrome responsiveness — task name header + asset row wrapping ---

// D1a-A2: draw_detail does not inject the task name into the content rows at all.
// Only the content lines passed in are rendered; `task_name` is used only for the loading state.
#[test]
fn draw_detail_task_name_not_present_in_content_rows() {
    let name = "Short Task Name";
    let lines = vec!["body content".to_string()];
    let buf = render_detail_to_buf_with_name(&lines, &[], 0, 80, 20, name);
    let content = buf_to_string(&buf);
    // The task_name must NOT appear in the rendered output because draw_detail no longer
    // injects it as a header; it would only appear if it were in the passed-in lines.
    assert!(
        !content.contains(name),
        "task_name must NOT appear in rendered content (not injected by draw_detail): {content}"
    );
    // The body line passed in must still appear.
    assert!(
        content.contains("body content"),
        "passed-in body content must be rendered: {content}"
    );
}

// W2-A3: A long asset label that exceeds the panel inner width wraps to multiple
// lines with a hanging indent, and the full label text is present (no clip).
// Width=40 → panel_inner=38. The prefix "[1] ↗ " is 8 display cols.
// A label of 35+ chars will overflow 38 cols and wrap.
#[test]
fn draw_detail_long_asset_label_wraps_with_hanging_indent_no_clip() {
    let long_label = "very-long-filename-that-does-not-fit.pdf";
    assert!(
        long_label.len() > 30,
        "label must be longer than the available width"
    );
    let assets = vec![Asset {
        name: long_label.into(),
        url: "https://example.com/file.pdf".into(),
    }];
    let buf = render_detail_to_buf_with_name(&["body".to_string()], &assets, 0, 40, 20, "Task");
    let content = buf_to_string(&buf);
    // Beginning of the label must appear in the buffer on the first asset row.
    assert!(
        content.contains("very-long-filename"),
        "beginning of long asset label must appear in buffer: {content}"
    );
    // End fragments of the label must appear on continuation rows (wrapped, not clipped).
    // The label wraps across rows so we check for a fragment from the tail.
    assert!(
        content.contains("fit.pdf"),
        "tail fragment of long asset label must appear in buffer (no clip): {content}"
    );
    // The [1] prefix must appear (first asset marker).
    assert!(
        content.contains("[1]"),
        "'[1]' asset marker must appear: {content}"
    );
    // The hanging indent must appear: a continuation line starts with spaces equal to prefix width.
    // "[1] ↗ " has display_width 8; continuation lines start with 8 spaces.
    assert!(
        content.contains("        "),
        "continuation line must start with hanging indent spaces: {content}"
    );
}

// W2-A3: When a single asset's label wraps, the panel is taller than the single-row
// minimum. At width=32: panel_inner=30; content_width=30-2*PANEL_HPAD=28.
// "[1] ↗ " prefix is 8 display cols; label_width=28-8=20.
// "thirty-char-label-padded-xyz.pdf" (32 chars) wraps to >=2 content rows.
// Spaced height = rows + 0 separators + 2 vpad + 2 borders >= 6.
#[test]
fn draw_detail_wrapped_asset_panel_is_taller_than_minimum() {
    let label_30 = "thirty-char-label-padded-xyz.pdf";
    assert_eq!(label_30.len(), 32, "sanity: label must be 32 chars");
    let assets = vec![Asset {
        name: label_30.into(),
        url: "https://example.com/f.pdf".into(),
    }];
    let buf = render_detail_to_buf_with_name(&["body".to_string()], &assets, 0, 32, 20, "T");
    let content = buf_to_string(&buf);

    assert!(
        content.contains("Artifacts"),
        "Artifacts panel must appear: {content}"
    );
    assert!(
        content.contains("thirty-char"),
        "beginning of asset label must appear: {content}"
    );
    assert!(
        content.contains("xyz.pdf"),
        "end of asset label must appear (no clip): {content}"
    );
}

// D1d-AC1: render_assets_panel renders one blank row between consecutive links,
// one blank interior pad row below the top border, and one above the bottom border.
#[test]
fn draw_detail_asset_panel_has_blank_separator_and_vpad_rows() {
    let viewport_w = 80u16;
    let viewport_h = 30u16;
    let assets = vec![
        Asset {
            name: "alpha.pdf".into(),
            url: "https://example.com/alpha.pdf".into(),
        },
        Asset {
            name: "beta.pdf".into(),
            url: "https://example.com/beta.pdf".into(),
        },
    ];

    let buf = render_detail_to_buf_with_name(
        &["body".to_string()],
        &assets,
        0,
        viewport_w,
        viewport_h,
        "T",
    );

    let inner_width = (viewport_w - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport_h - panel_h;

    // Row immediately inside top border (panel_top + 1) must be blank (top vpad).
    // Exclude the left and right border columns (col 0 and col viewport_w-1).
    let top_vpad: String = (1..viewport_w - 1)
        .map(|x| buf.cell((x, panel_top + 1)).unwrap().symbol().to_string())
        .collect::<String>()
        .trim()
        .to_string();
    assert!(
        top_vpad.is_empty(),
        "row immediately inside top border must be blank (top vpad): {top_vpad:?}"
    );

    // Row immediately inside bottom border (panel_top + panel_h - 2) must be blank (bottom vpad).
    let bottom_vpad_row = panel_top + panel_h - 2;
    let bottom_vpad: String = (1..viewport_w - 1)
        .map(|x| buf.cell((x, bottom_vpad_row)).unwrap().symbol().to_string())
        .collect::<String>()
        .trim()
        .to_string();
    assert!(
        bottom_vpad.is_empty(),
        "row immediately inside bottom border must be blank (bottom vpad): {bottom_vpad:?}"
    );

    // The first asset row is at panel_top + 2 (border + vpad).
    let asset0_row: String = (0..viewport_w)
        .map(|x| buf.cell((x, panel_top + 2)).unwrap().symbol().to_string())
        .collect();
    assert!(
        asset0_row.contains("alpha"),
        "first asset row must contain 'alpha': {asset0_row:?}"
    );

    // The row after asset[0] (panel_top + 3 for 1-row asset) must be blank (separator).
    // Exclude left and right border columns.
    let sep_row: String = (1..viewport_w - 1)
        .map(|x| buf.cell((x, panel_top + 3)).unwrap().symbol().to_string())
        .collect::<String>()
        .trim()
        .to_string();
    assert!(
        sep_row.is_empty(),
        "row between consecutive assets must be blank (separator): {sep_row:?}"
    );

    // The row after the separator (panel_top + 4) must contain asset[1].
    let asset1_row: String = (0..viewport_w)
        .map(|x| buf.cell((x, panel_top + 4)).unwrap().symbol().to_string())
        .collect();
    assert!(
        asset1_row.contains("beta"),
        "second asset row must contain 'beta': {asset1_row:?}"
    );
}

// D1d-AC2: Each link row is inset from the left border by PANEL_HPAD (1 space).
#[test]
fn draw_detail_asset_rows_inset_by_hpad() {
    let viewport_w = 80u16;
    let viewport_h = 30u16;
    let assets = vec![Asset {
        name: "file.pdf".into(),
        url: "https://example.com/file.pdf".into(),
    }];

    let buf = render_detail_to_buf_with_name(
        &["body".to_string()],
        &assets,
        0,
        viewport_w,
        viewport_h,
        "T",
    );

    let inner_width = (viewport_w - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport_h - panel_h;

    // Asset row is at panel_top + 2 (border + vpad).
    let asset_row_abs = panel_top + 2;

    // Column 0 of the buffer is the left border character (│).
    // Column 1 must be a space (PANEL_HPAD=1).
    // Column 2 must be '[' (start of "[1] ↗ label").
    let col0 = buf.cell((0, asset_row_abs)).unwrap().symbol().to_string();
    let col1 = buf.cell((1, asset_row_abs)).unwrap().symbol().to_string();
    let col2 = buf.cell((2, asset_row_abs)).unwrap().symbol().to_string();

    assert!(
        col0.contains('│') || col0.contains('|'),
        "column 0 of asset row must be left border: got {col0:?}"
    );
    assert_eq!(
        col1, " ",
        "column 1 of asset row must be HPAD space: got {col1:?}"
    );
    assert_eq!(
        col2, "[",
        "column 2 of asset row must be '[' (start of [1] prefix): got {col2:?}"
    );
}

// D1d-AC3: A task with four assets whose labels each fit on one line shows all
// four [n] rows — no clipping (ASSET_PANEL_MAX_ROWS=14 clears the spaced 4-link
// card = 4 rows + 3 separators + 2 vpad + 2 borders = 11).
#[test]
fn draw_detail_four_assets_all_visible_no_clip() {
    let assets: Vec<Asset> = (1..=4)
        .map(|i| Asset {
            name: format!("doc{i}.pdf"),
            url: format!("https://example.com/doc{i}.pdf"),
        })
        .collect();

    // Use a generous viewport so all 11 rows fit.
    let viewport_w = 80u16;
    let viewport_h = 40u16;
    let buf = render_detail_to_buf_with_name(
        &["body".to_string()],
        &assets,
        0,
        viewport_w,
        viewport_h,
        "T",
    );
    let content = buf_to_string(&buf);

    for i in 1..=4 {
        assert!(
            content.contains(&format!("[{i}]")),
            "link [{i}] must appear — ASSET_PANEL_MAX_ROWS must clear the 4-link spaced card: {content}"
        );
    }
    // Verify all four filenames appear (label not clipped).
    for i in 1..=4 {
        assert!(
            content.contains(&format!("doc{i}.pdf")),
            "doc{i}.pdf must appear in panel: {content}"
        );
    }
}

// D1a-A2: At a wide width (120 cols), draw_detail still does not inject the task name.
// Asset rows do appear. The name only appears via the content lines (Título row), not as a header.
#[test]
fn draw_detail_wide_width_assets_render_no_injected_name() {
    let name = "Short Name";
    let assets = vec![Asset {
        name: "report.pdf".into(),
        url: "https://example.com/report.pdf".into(),
    }];
    let buf = render_detail_to_buf_with_name(&["body".to_string()], &assets, 0, 120, 30, name);
    let content = buf_to_string(&buf);

    // report.pdf asset label fits on one row — verify it appears somewhere.
    assert!(
        content.contains("report.pdf"),
        "asset label must appear in buffer at wide width: {content}"
    );
    // No ellipsis at wide width.
    assert!(
        !content.contains('\u{2026}'),
        "no ellipsis expected at wide width: {content}"
    );
    // task_name must NOT appear as an injected header (only body line "body" is in lines).
    assert!(
        !content.contains(name),
        "task_name must NOT be injected by draw_detail: {content}"
    );
}

// W2-A2: The over-scroll clamp (D2) still works with the new name header.
// Passes a very large offset; the body must still show the last content lines.
#[test]
fn draw_detail_over_scroll_clamp_still_works_with_name_header() {
    let name = "My Task";
    let lines: Vec<String> = (1..=15).map(|i| format!("body line {i:02}")).collect();
    let buf = render_detail_to_buf_with_name(&lines, &[], 9999, 80, 20, name);
    let content = buf_to_string(&buf);
    // Last content line must be visible (clamp worked).
    assert!(
        content.contains("body line 15"),
        "last body line must be visible after over-scroll clamp: {content}"
    );
}

// --- R3b inline-emphasis style runs ----------------------------------------

fn style_run_matches(run: &StyleRun, expected_style: RichStyle) -> bool {
    run.style == expected_style
}

fn find_style_run_for_text<'a>(
    lines: &'a [String],
    line_styles: &'a [Vec<StyleRun>],
    needle: &str,
    expected_style: RichStyle,
) -> Option<(usize, &'a StyleRun)> {
    lines.iter().enumerate().find_map(|(i, line)| {
        if !line.contains(needle) {
            return None;
        }
        let run = line_styles
            .get(i)?
            .iter()
            .find(|r| style_run_matches(r, expected_style))?;
        Some((i, run))
    })
}

// R3b-A1: <strong> produces a Bold StyleRun on the line containing its text.
#[test]
fn strong_tag_produces_bold_style_run() {
    let task = json!({
        "name": "Bold test",
        "id": 1,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Before <strong>bold word</strong> after.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    assert_eq!(
        content.lines.len(),
        content.line_styles.len(),
        "lines and line_styles must be index-aligned"
    );

    let found = find_style_run_for_text(
        &content.lines,
        &content.line_styles,
        "bold word",
        RichStyle::Bold,
    );
    assert!(
        found.is_some(),
        "<strong> content must have a Bold StyleRun on the line containing 'bold word': {:#?}",
        content.lines
    );
}

// R3b-A1: <b> produces a Bold StyleRun (alias for <strong>).
#[test]
fn b_tag_produces_bold_style_run() {
    let task = json!({
        "name": "B tag test",
        "id": 2,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Before <b>bolded</b> after.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    let found = find_style_run_for_text(
        &content.lines,
        &content.line_styles,
        "bolded",
        RichStyle::Bold,
    );
    assert!(
        found.is_some(),
        "<b> content must have a Bold StyleRun on the line containing 'bolded': {:#?}",
        content.lines
    );
}

// R3b-A1: <em> produces an Italic StyleRun.
#[test]
fn em_tag_produces_italic_style_run() {
    let task = json!({
        "name": "Em test",
        "id": 3,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>See <em>italic text</em> here.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    let found = find_style_run_for_text(
        &content.lines,
        &content.line_styles,
        "italic text",
        RichStyle::Italic,
    );
    assert!(
        found.is_some(),
        "<em> content must have an Italic StyleRun on the line containing 'italic text': {:#?}",
        content.lines
    );
}

// R3b-A1: <i> produces an Italic StyleRun (alias for <em>).
#[test]
fn i_tag_produces_italic_style_run() {
    let task = json!({
        "name": "I tag test",
        "id": 4,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Text <i>slanted</i> end.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    let found = find_style_run_for_text(
        &content.lines,
        &content.line_styles,
        "slanted",
        RichStyle::Italic,
    );
    assert!(
        found.is_some(),
        "<i> content must have an Italic StyleRun on the line containing 'slanted': {:#?}",
        content.lines
    );
}

// R3b-A1: <code> produces a Code StyleRun.
#[test]
fn code_tag_produces_code_style_run() {
    let task = json!({
        "name": "Code test",
        "id": 5,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Run <code>cargo test</code> now.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    let found = find_style_run_for_text(
        &content.lines,
        &content.line_styles,
        "cargo test",
        RichStyle::Code,
    );
    assert!(
        found.is_some(),
        "<code> content must have a Code StyleRun on the line containing 'cargo test': {:#?}",
        content.lines
    );
}

// R3b-A2: <h1> through <h6> produce Bold StyleRuns covering the heading line.
#[test]
fn heading_tags_produce_bold_style_runs() {
    for tag in &["h1", "h2", "h3", "h4", "h5", "h6"] {
        let html_body = format!("<{tag}>Section Title</{tag}>");
        let task = json!({
            "name": "Heading test",
            "id": 10,
            "project_id": 1,
            "is_completed": false,
            "body": html_body
        });
        let user_map: HashMap<i64, String> = HashMap::new();
        let content = build_detail_content(&task, &[], &user_map, 80);

        let found = find_style_run_for_text(
            &content.lines,
            &content.line_styles,
            "Section Title",
            RichStyle::Bold,
        );
        assert!(
            found.is_some(),
            "<{tag}> heading must produce a Bold StyleRun on the heading line: {:#?}",
            content.lines
        );
    }
}

// R3b-A2: Bold runs from <strong> survive a wrap boundary — text split across
// lines must each carry a Bold StyleRun on the fragment line.
#[test]
fn bold_span_across_wrap_boundary_keeps_style_on_all_fragments() {
    let long_bold = "word ".repeat(30);
    let html_body = format!("<p>Before <strong>{long_bold}</strong> after.</p>");
    let task = json!({
        "name": "Wrap bold test",
        "id": 11,
        "project_id": 1,
        "is_completed": false,
        "body": html_body
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 40);

    let bold_run_count = content
        .line_styles
        .iter()
        .filter(|runs| runs.iter().any(|r| r.style == RichStyle::Bold))
        .count();

    assert!(
        bold_run_count >= 2,
        "a bold span spanning multiple wrapped lines must produce Bold runs on >= 2 lines; got {bold_run_count}: {:#?}",
        content.lines
    );
}

// R3b-A3: line_styles is always index-aligned with lines (no orphan rows).
#[test]
fn line_styles_always_aligned_with_lines() {
    let task = json!({
        "name": "Alignment check",
        "id": 20,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Body with <strong>bold</strong> and <em>italic</em>.</p>"
    });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1700000000u64,
        "body": "<p>Comment with <code>code</code> inline.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[comment], &user_map, 60);

    assert_eq!(
        content.lines.len(),
        content.line_styles.len(),
        "line_styles must be the same length as lines after processing body+comments"
    );
}

// R3b-A3: the structured detail path produces the expected plain-text content for
// a fixture with emphasis HTML in both body and comment. Verifies that the rich-text
// pipeline does not drop, duplicate, or corrupt the plain-text values callers observe.
#[test]
fn build_detail_content_structured_path_preserves_plain_text() {
    let task = json!({
        "name": "Regression check",
        "id": 21,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Some <strong>bold</strong> and <em>italic</em> text.</p>"
    });
    let comment = json!({
        "created_by_name": "Bob",
        "created_on": 1700000000u64,
        "body": "<p>A <code>coded</code> comment.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();

    let content = build_detail_content(&task, &[comment], &user_map, 70);
    let joined = content.lines.join("\n");

    assert!(
        !content.lines.is_empty(),
        "structured path must produce output lines"
    );
    assert!(
        joined.contains("Details"),
        "must include the Details panel: {joined}"
    );
    assert!(
        joined.contains("Description"),
        "must include the Description panel: {joined}"
    );
    assert!(
        joined.contains("bold"),
        "plain text of <strong>bold</strong> must appear in content: {joined}"
    );
    assert!(
        joined.contains("italic"),
        "plain text of <em>italic</em> must appear in content: {joined}"
    );
    assert!(
        joined.contains("Comments"),
        "must include the Comments panel for non-empty comments: {joined}"
    );
    assert!(
        joined.contains("Bob"),
        "comment author must appear in content: {joined}"
    );
    assert!(
        joined.contains("coded"),
        "plain text of <code>coded</code> must appear in comment body: {joined}"
    );
    assert_eq!(
        content.lines.len(),
        content.line_styles.len(),
        "lines and line_styles must remain index-aligned after structured rendering"
    );
}

// R3b-A3: Plain lines (no emphasis HTML) produce empty style-run vecs.
#[test]
fn plain_body_produces_empty_style_runs_per_line() {
    let task = json!({
        "name": "Plain body",
        "id": 22,
        "project_id": 1,
        "is_completed": false,
        "body": "<p>Just plain text here.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    let body_lines_with_style_runs: Vec<&str> = content
        .lines
        .iter()
        .zip(&content.line_styles)
        .filter(|(_, runs)| !runs.is_empty())
        .map(|(l, _)| l.as_str())
        .collect();

    assert!(
        body_lines_with_style_runs.is_empty(),
        "plain HTML body must produce no non-empty style-run vecs; affected lines: {body_lines_with_style_runs:?}"
    );
}

// R3b-A1: StyleRun.start is offset by left chrome (≥ 2 cols) — not column 0.
#[test]
fn style_run_start_is_offset_by_chrome() {
    let task = json!({
        "name": "Chrome offset test",
        "id": 23,
        "project_id": 1,
        "is_completed": false,
        "body": "<p><strong>starts bold</strong> rest of line.</p>"
    });
    let user_map: HashMap<i64, String> = HashMap::new();
    let content = build_detail_content(&task, &[], &user_map, 80);

    let bold_run = content
        .line_styles
        .iter()
        .flat_map(|runs| runs.iter())
        .find(|r| r.style == RichStyle::Bold);

    let run = bold_run.expect("must have at least one Bold run for <strong> at line start");
    assert!(
        run.start >= 2,
        "StyleRun.start must be >= 2 (left chrome = 1 border + 1 hpad), got start={}",
        run.start
    );
}

// S8b-A4: revalidating=true shows ↻ in the Projects border title.
#[test]
fn projects_title_shows_revalidating_indicator_when_revalidating() {
    let groups = vec![ProjectGroup {
        project_id: 1,
        project_name: "Project Alpha".into(),
        instance: "inst".into(),
        tasks: vec![],
    }];
    let width = 40u16;
    let height = 10u16;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let mut targets = vec![];
            draw_projects(
                frame,
                Rect::new(0, 0, width, height),
                &groups,
                0,
                false,
                true,
                &mut targets,
            );
        })
        .unwrap();

    let buf = terminal.backend().buffer().clone();
    let top_row: String = (0..width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect();
    assert!(
        top_row.contains('↻'),
        "Projects title must contain ↻ when revalidating=true; got: {top_row:?}"
    );
}

// S8b-A4 (inverse): revalidating=false does NOT show ↻ in the Projects title.
#[test]
fn projects_title_hides_revalidating_indicator_when_not_revalidating() {
    let groups = vec![ProjectGroup {
        project_id: 1,
        project_name: "Project Alpha".into(),
        instance: "inst".into(),
        tasks: vec![],
    }];
    let width = 40u16;
    let height = 10u16;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            let mut targets = vec![];
            draw_projects(
                frame,
                Rect::new(0, 0, width, height),
                &groups,
                0,
                false,
                false,
                &mut targets,
            );
        })
        .unwrap();

    let buf = terminal.backend().buffer().clone();
    let top_row: String = (0..width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect();
    assert!(
        !top_row.contains('↻'),
        "Projects title must NOT contain ↻ when revalidating=false; got: {top_row:?}"
    );
}

// --- W1: Detail chrome responsiveness — header and footer word-wrap on narrow terminals ---

mod w1_chrome_wrap {
    use crate::tui::model::{Header, Model, Screen};
    use crate::tui::view::view;
    use ratatui::{backend::TestBackend, Terminal};

    fn buf_rows(buf: &ratatui::buffer::Buffer) -> Vec<String> {
        let area = buf.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect()
            })
            .collect()
    }

    fn buf_to_string(buf: &ratatui::buffer::Buffer) -> String {
        buf_rows(buf).join("\n")
    }

    fn render_view_at(model: &Model, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| view(model, frame, &mut vec![]))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn projects_model_with_header(header: Header, last_loaded: Option<String>) -> Model {
        Model {
            stack: vec![Screen::Projects {
                groups: vec![],
                selected: 0,
                loading: false,
                revalidating: false,
            }],
            should_quit: false,
            header,
            viewport: (0, 0),
            click_targets: vec![],
            last_loaded,
            selection: None,
            copied_feedback: false,
        }
    }

    // W1-A1: At a narrow terminal width the user header bar wraps to multiple lines and the
    // full identity ({name} <{email}> · {instance}) is present in the buffer — no ellipsis,
    // no right-clip. (BDR 0012 S1)
    #[test]
    fn narrow_header_wraps_and_full_identity_present_no_clip() {
        // header_line = "Alice <alice@prod.example.com> · production" (44 chars).
        // At width=40:
        //   word "Alice" (5), word "<alice@prod.example.com>" (24) → 5+1+24=30 ≤ 40 → same line
        //   word "·" (1) → 30+1+1=32 ≤ 40 → same line
        //   word "production" (10) → 32+1+10=43 > 40 → wraps to line 2
        // header_height = 2; y=0 has "Alice <alice@prod.example.com> ·", y=1 has "production"
        let header = Header {
            name: Some("Alice".into()),
            email: "alice@prod.example.com".into(),
            instance: "production".into(),
            extra: 0,
        };
        let model = projects_model_with_header(header, None);
        let width = 40u16;
        let height = 20u16;
        let buf = render_view_at(&model, width, height);
        let rows = buf_rows(&buf);
        let content = buf_to_string(&buf);

        // Full identity must be present somewhere across all rows.
        assert!(
            content.contains("Alice"),
            "buffer must contain name 'Alice': {content}"
        );
        assert!(
            content.contains("alice@prod.example.com"),
            "buffer must contain email 'alice@prod.example.com': {content}"
        );
        assert!(
            content.contains("production"),
            "buffer must contain instance 'production': {content}"
        );

        // Row 0 must contain the first part of the identity (name + email).
        let row0 = &rows[0];
        assert!(
            row0.contains("Alice"),
            "y=0 must contain 'Alice' (first wrapped header line): {row0}"
        );

        // Row 1 must contain the continuation ('production' wrapped to next line).
        let row1 = &rows[1];
        assert!(
            row1.contains("production"),
            "y=1 must contain 'production' (second wrapped header line): {row1}"
        );

        // No ellipsis anywhere — content wraps, not truncates.
        assert!(
            !content.contains('\u{2026}'),
            "header must wrap not truncate — no ellipsis in buffer: {content}"
        );
    }

    // W1-A2: At a narrow terminal width the footer (hint + Updated-at timestamp) wraps so
    // both the hint text and the timestamp are fully present in the buffer — nothing clipped.
    // (BDR 0012 S3)
    #[test]
    fn narrow_footer_wraps_hint_and_timestamp_both_present() {
        // At width=40 the hint "↑/↓ navigate  Enter select  r refresh  Esc/b back  q quit  s selection" (72 chars)
        // and timestamp "Updated at 15/01/2024 14:30" (27 chars) do not co-fit on one 40-col line.
        // They are stacked: hint wraps across multiple lines, then timestamp below.
        let header = Header {
            name: None,
            email: "u@example.com".into(),
            instance: "inst".into(),
            extra: 0,
        };
        let model = projects_model_with_header(header, Some("2024-01-15T14:30:00".into()));
        let width = 40u16;
        let height = 30u16;
        let buf = render_view_at(&model, width, height);
        let content = buf_to_string(&buf);

        assert!(
            content.contains("↑/↓"),
            "footer must contain hint text '↑/↓': {content}"
        );
        assert!(
            content.contains("Updated at"),
            "footer must contain 'Updated at' timestamp label: {content}"
        );
        assert!(
            content.contains("15/01/2024"),
            "footer must contain formatted date '15/01/2024': {content}"
        );
        assert!(
            content.contains("14:30"),
            "footer must contain time '14:30': {content}"
        );
    }

    // W1-A3: At a wide terminal the user header bar and footer are each a single line at
    // height 1 (no wrapping, no stray blank line). (BDR 0012 S5)
    #[test]
    fn wide_terminal_header_and_footer_each_single_line() {
        // At width=120, header "Alice <alice@example.com> · prod" (33 chars) fits on one line.
        // Footer hint (≤72 chars) also fits on one line at 120 cols.
        // So: y=0 = header, y=1 = content start, y=height-1 = footer, y=height-2 = content end.
        let header = Header {
            name: Some("Alice".into()),
            email: "alice@example.com".into(),
            instance: "prod".into(),
            extra: 0,
        };
        let model = projects_model_with_header(header, None);
        let width = 120u16;
        let height = 20u16;
        let buf = render_view_at(&model, width, height);
        let rows = buf_rows(&buf);

        // y=0 must contain the full header identity on that row alone.
        let row0 = &rows[0];
        assert!(
            row0.contains("Alice") && row0.contains("alice@example.com") && row0.contains("prod"),
            "y=0 must contain the full header identity at wide width: {row0}"
        );

        // y=1 must NOT contain the header identity text — header stayed on y=0 only.
        let row1 = &rows[1];
        assert!(
            !row1.contains("alice@example.com"),
            "y=1 must NOT contain header email — header must be single-line at width=120: {row1}"
        );

        // y=height-1 must contain the footer hint.
        let last_row = &rows[(height - 1) as usize];
        assert!(
            last_row.contains("↑/↓"),
            "y=height-1 must contain footer hint at wide width: {last_row}"
        );

        // y=height-2 must NOT contain the footer hint — footer stayed on one row.
        let second_last = &rows[(height - 2) as usize];
        assert!(
            !second_last.contains("↑/↓"),
            "y=height-2 must NOT contain footer hint — footer must be single-line at width=120: {second_last}"
        );
    }

    // D1a-A2 / W2: at a narrow width where an asset label wraps, draw_detail still renders
    // the body lines and the Artifacts panel. task_name is NOT injected as a header.
    #[test]
    fn draw_detail_wrapped_asset_shows_body_and_artifacts_no_injected_name() {
        use crate::render::Asset;
        use crate::tui::screens::{draw_detail, DetailParams};

        // At width=20 the asset panel inner is 20-4=16 cols.
        // "[1] ↗ " = 7 cols, label_width = 9 cols.
        // Name "ABCDEFGHIJKLMNOPQRS.pdf" → label "ABCDEFGHIJKLMNOPQRS.pdf" > 9 cols → wraps.
        let long_asset = Asset {
            name: "ABCDEFGHIJKLMNOPQRS.pdf".into(),
            url: "https://example.com/long.pdf".into(),
        };

        let task_name = "My Task";
        let lines = vec!["body text".to_string()];
        let empty_styles: Vec<Vec<crate::render::StyleRun>> = vec![vec![]; lines.len()];
        let backend = TestBackend::new(20, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw_detail(
                    frame,
                    ratatui::layout::Rect::new(0, 0, 20, 20),
                    DetailParams {
                        lines: &lines,
                        line_styles: &empty_styles,
                        assets: &[long_asset],
                        offset: 0,
                        loading: false,
                        task_id: 1,
                        task_name,
                    },
                );
            })
            .unwrap();

        let content = super::buf_to_string(terminal.backend().buffer());

        assert!(
            !content.contains(task_name),
            "task_name must NOT be injected as a header by draw_detail: {content}"
        );
        assert!(
            content.contains("body text"),
            "body line must still render: {content}"
        );
        assert!(
            !content.contains('\u{2026}'),
            "no ellipsis when asset wraps: {content}"
        );
        assert!(
            content.contains("Artifacts"),
            "Artifacts panel title must appear: {content}"
        );
    }

    // W1-A4: A single unbreakable token longer than the available display width hard-breaks at
    // the display-column boundary without overflowing the region. (BDR 0012 S6)
    #[test]
    fn unbreakable_token_hard_breaks_at_display_column_boundary() {
        // email = "noreply-serviceaccount-admin@very-long-subdomain.example.com" (61 chars)
        // header_line = "<noreply-serviceaccount-admin@very-long-subdomain.example.com> · inst" (70 chars)
        // At width=30, the angle-bracketed email token (63 chars) cannot word-wrap, so it
        // hard-splits at column 30. Row 0: first 30 chars; row 1: next 30 chars; row 2: tail.
        // All characters must be present in the buffer — verify by checking row-sized substrings.
        let email = "noreply-serviceaccount-admin@very-long-subdomain.example.com";
        let header = Header {
            name: None,
            email: email.into(),
            instance: "inst".into(),
            extra: 0,
        };
        let model = projects_model_with_header(header, None);
        let width = 30u16;
        let height = 20u16;
        let buf = render_view_at(&model, width, height);
        let rows = buf_rows(&buf);

        // Row 0 must start with the first 30 chars of the bracketed email (hard-break boundary).
        // "<noreply-serviceaccount-admin@" = 30 chars.
        let row0 = &rows[0];
        assert!(
            row0.starts_with("<noreply-serviceaccount-admin@"),
            "y=0 must begin with the first 30 chars of the long token: {row0:?}"
        );

        // Row 1 must contain the continuation of the token (no truncation, no ellipsis).
        // "very-long-subdomain.example.co" = next 30 chars.
        let row1 = &rows[1];
        assert!(
            row1.starts_with("very-long-subdomain.example.co"),
            "y=1 must continue the hard-broken token without truncation: {row1:?}"
        );

        // Row 2 must contain the tail ("m> · inst").
        let row2 = &rows[2];
        assert!(
            row2.starts_with("m>"),
            "y=2 must contain the tail of the hard-broken token: {row2:?}"
        );

        // No ellipsis anywhere — hard-break preserves all characters.
        let content: String = rows.join("\n");
        assert!(
            !content.contains('\u{2026}'),
            "unbreakable token must hard-break, not produce ellipsis: {content}"
        );
    }
}

// AC-BLEED (BDR 0018 Sc.3a): On a rendered asset link row, only the visible '[n] ↗ label'
// token cells carry the link/underline style. The leading PANEL_HPAD pad cell (col 1 after
// the left border) and the trailing cells near the right border carry the default style.
// A cell inside the '[n] ↗ label' token carries the asset/link style (underlined).
#[test]
fn asset_link_row_underline_confined_to_token_not_leading_pad_or_trailing_fill() {
    use ratatui::style::Modifier;

    let viewport_w = 80u16;
    let viewport_h = 30u16;
    let assets = vec![
        Asset {
            name: "alpha.pdf".into(),
            url: "https://example.com/alpha.pdf".into(),
        },
        Asset {
            name: "beta.pdf".into(),
            url: "https://example.com/beta.pdf".into(),
        },
    ];

    let buf = render_detail_to_buf_with_name(
        &["body".to_string()],
        &assets,
        0,
        viewport_w,
        viewport_h,
        "T",
    );

    let inner_width = (viewport_w - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport_h - panel_h;

    // Asset[0] row: panel_top + 1 (top border) + 1 (top vpad) = panel_top + 2.
    let asset_row = panel_top + 2;

    // Col 0 is the left border (│). Col 1 is the PANEL_HPAD space — must be UNSTYLED.
    let pad_cell = buf.cell((1, asset_row)).unwrap();
    assert!(
        !pad_cell.style().add_modifier.contains(Modifier::UNDERLINED),
        "leading HPAD cell (col 1) on asset row must NOT carry UNDERLINED modifier; \
         bleed detected: style={:?}",
        pad_cell.style()
    );

    // The rightmost inner column (col viewport_w-2, before the right border) must also be UNSTYLED
    // when the label text does not reach that far (short name "alpha.pdf").
    let right_cell = buf.cell((viewport_w - 2, asset_row)).unwrap();
    assert!(
        !right_cell
            .style()
            .add_modifier
            .contains(Modifier::UNDERLINED),
        "trailing fill cell near right border (col {}) on asset row must NOT carry UNDERLINED; \
         bleed detected: style={:?}",
        viewport_w - 2,
        right_cell.style()
    );

    // Col 2 is the first char of "[1] ↗ alpha.pdf" — must carry the link/underline style.
    let token_cell = buf.cell((2, asset_row)).unwrap();
    assert!(
        token_cell
            .style()
            .add_modifier
            .contains(Modifier::UNDERLINED),
        "first char of '[n]' token (col 2) must carry UNDERLINED modifier; \
         style={:?}",
        token_cell.style()
    );
}

// AC-MAP (click-mapping from real buffer): clicking the actual rendered row of link [2]
// resolves to asset index 1 in asset_panel_cmd_at.
//
// The test renders >=3 assets via TestBackend, locates the actual screen row that contains
// the '[2]' marker in the buffer, feeds THAT row to the click handler, and asserts it
// resolves to asset index 1. Also asserts [1] row → index 0 and a separator/pad row → None.
// This prevents the test from passing while the live mapping is wrong.
#[test]
fn click_second_asset_row_derived_from_real_buffer_resolves_to_index_1() {
    use crate::tui::model::{update, Header, Msg, Screen};
    use crate::tui::screens::detail::{draw_detail, DetailParams};
    use crossterm::event::KeyModifiers;
    use ratatui::{backend::TestBackend, layout::Rect, Terminal};
    use std::collections::HashMap;

    let viewport_w = 80u16;
    let viewport_h = 30u16;
    let assets = vec![
        Asset {
            name: "first.pdf".into(),
            url: "https://example.com/first.pdf".into(),
        },
        Asset {
            name: "second.pdf".into(),
            url: "https://example.com/second.pdf".into(),
        },
        Asset {
            name: "third.pdf".into(),
            url: "https://example.com/third.pdf".into(),
        },
    ];

    // Render via TestBackend to capture the exact layout.
    let empty_styles: Vec<Vec<crate::render::StyleRun>> = vec![];
    let backend = TestBackend::new(viewport_w, viewport_h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_detail(
                frame,
                Rect::new(0, 0, viewport_w, viewport_h),
                DetailParams {
                    lines: &["body".to_string()],
                    line_styles: &empty_styles,
                    assets: &assets,
                    offset: 0,
                    loading: false,
                    task_id: 1,
                    task_name: "T",
                },
            );
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // Scan all rows to find the rows containing "[1]", "[2]", "[3]".
    let mut row_for = [None::<u16>; 4]; // index 1,2,3 used
    for y in 0..viewport_h {
        let row_str: String = (0..viewport_w)
            .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect();
        for n in 1usize..=3 {
            let marker = format!("[{n}]");
            if row_str.contains(&marker) && row_for[n].is_none() {
                row_for[n] = Some(y);
            }
        }
    }

    let row1 = row_for[1].expect("'[1]' marker must appear in the rendered buffer");
    let row2 = row_for[2].expect("'[2]' marker must appear in the rendered buffer");
    let row3 = row_for[3].expect("'[3]' marker must appear in the rendered buffer");

    // The separator row is between [1] and [2] rows.
    // It must be exactly row1 + 1 (since each short asset occupies 1 row).
    let sep_row = row1 + 1;
    assert_eq!(
        sep_row + 1,
        row2,
        "separator must be at row1+1 and asset[2] at row1+2"
    );

    let make_model = |assets: Vec<Asset>| crate::tui::model::Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task: serde_json::Value::Null,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec!["body".to_string()],
            line_styles: vec![],
            assets,
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: Header::from_instances(&[], None),
        viewport: (viewport_w, viewport_h),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    // Ctrl+click on buffer row containing "[1]" → must open first.pdf (index 0).
    let (_, cmds) = update(
        make_model(assets.clone()),
        Msg::Click {
            column: 5,
            row: row1,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "ctrl+click on '[1]' row must emit one cmd");
    match &cmds[0] {
        crate::tui::model::Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/first.pdf",
                "'[1]' row (buffer row {row1}) must resolve to first.pdf (index 0)"
            );
        }
        other => panic!("expected OpenAsset for '[1]' row, got {other:?}"),
    }

    // Ctrl+click on buffer row containing "[2]" → must open second.pdf (index 1).
    let (_, cmds) = update(
        make_model(assets.clone()),
        Msg::Click {
            column: 5,
            row: row2,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "ctrl+click on '[2]' row must emit one cmd");
    match &cmds[0] {
        crate::tui::model::Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/second.pdf",
                "'[2]' row (buffer row {row2}) must resolve to second.pdf (index 1), \
                 not first.pdf — off-by-one regression guard"
            );
        }
        other => panic!("expected OpenAsset for '[2]' row, got {other:?}"),
    }

    // Ctrl+click on buffer row containing "[3]" → must open third.pdf (index 2).
    let (_, cmds) = update(
        make_model(assets.clone()),
        Msg::Click {
            column: 5,
            row: row3,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(cmds.len(), 1, "ctrl+click on '[3]' row must emit one cmd");
    match &cmds[0] {
        crate::tui::model::Cmd::OpenAsset { url, .. } => {
            assert_eq!(
                url, "https://example.com/third.pdf",
                "'[3]' row (buffer row {row3}) must resolve to third.pdf (index 2)"
            );
        }
        other => panic!("expected OpenAsset for '[3]' row, got {other:?}"),
    }

    // Ctrl+click on the separator row → must resolve to None (no asset).
    let (_, cmds) = update(
        make_model(assets.clone()),
        Msg::Click {
            column: 5,
            row: sep_row,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert!(
        cmds.is_empty(),
        "click on separator row (row {sep_row}) must return no asset (None)"
    );

    // Click on the top vpad row (panel_top + 1) → must resolve to None.
    let inner_width = (viewport_w - 2) as usize;
    let panel_h = asset_panel_render_height(&assets, inner_width);
    let panel_top = viewport_h - panel_h;
    let top_vpad = panel_top + 1;
    let (_, cmds) = update(
        make_model(assets.clone()),
        Msg::Click {
            column: 5,
            row: top_vpad,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(
        cmds.is_empty(),
        "click on top vpad row (row {top_vpad}) must return no asset (None)"
    );
}

// --- D1c: logical_position_in_wrap_group unit tests ---

mod d1c_wrap_group_position {
    use crate::render::{logical_position_in_wrap_group, panel_content_width_pub};

    fn make_box_line(content: &str, content_width: usize) -> String {
        let pad = " ".repeat(content_width.saturating_sub(content.len()));
        format!("\u{2502} {content}{pad} \u{2502}")
    }

    // D1c-A4: logical_position_in_wrap_group resolves the first character of line 0
    // (char_col=2, which is content_col=0) to group_start=0 and logical_col=0.
    //
    // Uses a content of exactly content_width chars to simulate a hard-split line, so
    // group_start remains 0 (the click is on the first/only line of the group).
    #[test]
    fn single_line_group_start_col_maps_to_logical_zero() {
        // frag0: exactly 36 ASCII chars — the first hard-split fragment of a [url] token.
        // "[https://example.com/long-path/to/pa" = 1 + 8 + 11 + 16 = 36 chars.
        let content_width = 36usize;
        let frag0 = "[https://example.com/long-path/to/pa";
        assert_eq!(
            frag0.len(),
            content_width,
            "sanity: frag0 must fill content_width exactly"
        );
        let line = make_box_line(frag0, content_width);
        let lines = vec![line];

        let result = logical_position_in_wrap_group(&lines, 0, 2, content_width);
        let (group_start, logical_col) = result.expect("must resolve for a valid box line");
        assert_eq!(group_start, 0);
        assert_eq!(logical_col, 0, "char_col=2 → content_col=0 → logical_col=0");
    }

    // D1c-A4: clicking at char_col=5 on line 0 (a hard-split line) returns logical_col=3.
    //
    // frag0 fills content_width exactly (hard-split); frag1 is the continuation.
    // char_col=5 → content_col = 5-2 = 3 → logical_col = 3 (no prior fragments).
    #[test]
    fn hard_split_line_zero_char_col_maps_correctly() {
        let content_width = 36usize;
        let frag0 = "[https://example.com/long-path/to/pa"; // exactly 36 chars
        let frag1 = "ge]";
        assert_eq!(
            frag0.len(),
            content_width,
            "frag0 must fill content_width exactly"
        );
        let line0 = make_box_line(frag0, content_width);
        let line1 = make_box_line(frag1, content_width);
        let lines = vec![line0, line1];

        let (group_start, logical_col) =
            logical_position_in_wrap_group(&lines, 0, 5, content_width)
                .expect("must resolve on first line");
        assert_eq!(group_start, 0);
        assert_eq!(
            logical_col, 3,
            "char_col=5 → content_col=3 on line 0 → logical_col=3"
        );
    }

    // D1c-A4: clicking at char_col=4 on line 1 (the continuation) maps to
    // logical_col = content_width (36) + content_col (2) = 38.
    #[test]
    fn continuation_line_click_maps_to_logical_col_across_split() {
        let content_width = 36usize;
        let frag0 = "[https://example.com/long-path/to/pa"; // 36 chars — exactly fills content_width
        let frag1 = "ge]";
        assert_eq!(
            frag0.len(),
            content_width,
            "frag0 must fill content_width exactly"
        );
        let line0 = make_box_line(frag0, content_width);
        let line1 = make_box_line(frag1, content_width);
        let lines = vec![line0, line1];

        // char_col=4 on line 1: content_col = 4-2 = 2; previous line contributes 36 cols.
        let (group_start, logical_col) =
            logical_position_in_wrap_group(&lines, 1, 4, content_width)
                .expect("must resolve on continuation line");
        assert_eq!(group_start, 0, "group must walk back to line 0");
        assert_eq!(
            logical_col,
            content_width + 2,
            "logical_col = frag0.display_width ({content_width}) + content_col (2) = {}",
            content_width + 2
        );
    }

    // D1c-A4: a non-box line (no │ border) returns None.
    #[test]
    fn non_box_line_returns_none() {
        let lines = vec!["plain text without box border".to_string()];
        let result = logical_position_in_wrap_group(&lines, 0, 2, 36);
        assert!(result.is_none(), "non-box line must return None");
    }

    // D1c-A4: panel_content_width_pub is called with inner_width (viewport_cols - 2),
    // and returns inner_width - 4 (removes 2 border cols + 2×HPAD).
    // For viewport_cols=42 → inner_width=40 → content_width=36.
    #[test]
    fn panel_content_width_pub_matches_expected() {
        let inner_width = 40usize; // = viewport_cols(42) - 2
        let expected_content_width = inner_width.saturating_sub(4);
        assert_eq!(
            panel_content_width_pub(inner_width),
            expected_content_width,
            "panel_content_width_pub(inner_width=40) must equal inner_width - 4 = 36"
        );
        assert_eq!(expected_content_width, 36, "sanity: 40 - 4 = 36");
    }
}
