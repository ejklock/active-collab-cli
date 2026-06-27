use super::*;
use crate::render::MineTableRow;
use crate::tui::model::{DetailLoad, Header};
use crossterm::event::KeyModifiers;
use serde_json::json;
use std::collections::HashMap;

fn empty_header() -> Header {
    Header::from_instances(&[], None)
}

fn make_groups(count: usize) -> Vec<ProjectGroup> {
    (0..count)
        .map(|i| ProjectGroup {
            project_id: i as i64,
            project_name: format!("Project {i}"),
            instance: "inst".into(),
            tasks: vec![TaskRow {
                task_id: i as i64,
                task_number: i as i64,
                name: format!("Task {i}"),
                instance: "inst".into(),
                project_id: i as i64,
            }],
        })
        .collect()
}

fn make_tasks(count: usize) -> Vec<TaskRow> {
    (0..count)
        .map(|i| TaskRow {
            task_id: i as i64,
            task_number: i as i64,
            name: format!("Task {i}"),
            instance: "inst".into(),
            project_id: 0,
        })
        .collect()
}

fn projects_model(count: usize) -> Model {
    Model {
        stack: vec![Screen::Projects {
            groups: make_groups(count),
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn tasks_model(count: usize) -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(2),
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Tasks {
                project_name: "Project 0".into(),
                tasks: make_tasks(count),
                selected: 0,
                loading: false,
                revalidating: false,
            },
        ],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn loading_projects_model() -> Model {
    Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: true,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

#[test]
fn select_on_projects_pushes_tasks_screen() {
    let m = projects_model(3);
    let (m, _cmds) = update(m, Msg::Select);
    assert_eq!(m.stack.len(), 2);
    assert!(matches!(m.stack.last(), Some(Screen::Tasks { .. })));
    assert!(!m.should_quit);
}

#[test]
fn select_increases_stack_depth_from_one_to_two() {
    let m = projects_model(1);
    assert_eq!(m.stack.len(), 1);
    let (m, _) = update(m, Msg::Select);
    assert_eq!(m.stack.len(), 2);
}

#[test]
fn back_on_tasks_screen_pops_to_projects() {
    let m = tasks_model(3);
    assert_eq!(m.stack.len(), 2);
    let (m, _) = update(m, Msg::Back);
    assert_eq!(m.stack.len(), 1);
    assert!(matches!(m.stack.last(), Some(Screen::Projects { .. })));
    assert!(!m.should_quit);
}

#[test]
fn back_at_root_sets_should_quit() {
    let m = projects_model(3);
    assert_eq!(m.stack.len(), 1);
    let (m, _) = update(m, Msg::Back);
    assert!(m.should_quit);
}

#[test]
fn quit_sets_should_quit() {
    let m = projects_model(3);
    let (m, _) = update(m, Msg::Quit);
    assert!(m.should_quit);
}

#[test]
fn up_at_row_zero_stays_zero_projects() {
    let m = projects_model(3);
    let (m, _) = update(m, Msg::Up);
    let sel = m.stack.last().unwrap().selected();
    assert_eq!(sel, 0);
}

#[test]
fn down_at_last_row_clamps_projects() {
    let m = projects_model(3);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    let sel_before = m.stack.last().unwrap().selected();
    let (m, _) = update(m, Msg::Down);
    let sel_after = m.stack.last().unwrap().selected();
    assert_eq!(sel_before, sel_after);
    assert_eq!(sel_after, 2);
}

#[test]
fn up_at_row_zero_stays_zero_tasks() {
    let m = tasks_model(3);
    let (m, _) = update(m, Msg::Up);
    let sel = m.stack.last().unwrap().selected();
    assert_eq!(sel, 0);
}

#[test]
fn down_at_last_row_clamps_tasks() {
    let m = tasks_model(3);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    let sel_before = m.stack.last().unwrap().selected();
    let (m, _) = update(m, Msg::Down);
    let sel_after = m.stack.last().unwrap().selected();
    assert_eq!(sel_before, sel_after);
    assert_eq!(sel_after, 2);
}

#[test]
fn scroll_up_from_zero_clamps_projects() {
    let m = projects_model(3);
    let (m, _) = update(m, Msg::ScrollUp);
    assert_eq!(m.stack.last().unwrap().selected(), 0);
}

#[test]
fn scroll_down_at_last_row_clamps_projects() {
    let m = projects_model(2);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    assert_eq!(m.stack.last().unwrap().selected(), 1);
    assert!(!m.should_quit);
}

// V2b: click with empty hit-map (no targets populated) is a no-op.
#[test]
fn click_with_empty_targets_is_noop_projects() {
    let m = projects_model(3);
    let sel_before = m.stack.last().unwrap().selected();
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: 99,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(m.stack.last().unwrap().selected(), sel_before);
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
}

// V2b: click with empty hit-map (no targets populated) is a no-op on Tasks screen.
#[test]
fn click_with_empty_targets_is_noop_tasks() {
    let m = tasks_model(3);
    let sel_before = m.stack.last().unwrap().selected();
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: 99,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(m.stack.last().unwrap().selected(), sel_before);
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
}

#[test]
fn over_scroll_never_sets_should_quit_projects() {
    let m = projects_model(2);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);

    let m2 = projects_model(2);
    let (m2, _) = update(m2, Msg::Up);
    assert!(!m2.should_quit);
}

#[test]
fn over_scroll_never_sets_should_quit_tasks() {
    let m = tasks_model(2);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);

    let m2 = tasks_model(2);
    let (m2, _) = update(m2, Msg::Up);
    assert!(!m2.should_quit);
}

#[test]
fn empty_list_navigation_never_panics_or_quits_projects() {
    let m = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::Up);
    assert!(!m.should_quit);
    let (m, _) = update(
        m,
        Msg::Click {
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::ScrollDown);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::ScrollUp);
    assert!(!m.should_quit);
}

#[test]
fn empty_list_navigation_never_panics_or_quits_tasks() {
    let m = Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(1),
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: vec![],
                selected: 0,
                loading: false,
                revalidating: false,
            },
        ],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::Up);
    assert!(!m.should_quit);
    let (m, _) = update(
        m,
        Msg::Click {
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(!m.should_quit);
}

#[test]
fn init_browse_emits_load_tasks_by_project_cmd() {
    let (model, cmds) = init_browse(empty_header(), None);
    assert_eq!(cmds, vec![Cmd::LoadTasksByProject]);
    assert!(!model.should_quit);
    assert_eq!(model.stack.len(), 1);
    assert!(matches!(
        model.stack[0],
        Screen::Projects { loading: true, .. }
    ));
}

#[test]
fn loaded_tasks_stores_groups_and_clears_loading() {
    let m = loading_projects_model();
    let groups = make_groups(2);
    let (m, cmds) = update(
        m,
        Msg::LoadedTasksByProject {
            groups: groups.clone(),
            loaded_at: "2026-06-25T14:00:00Z".into(),
        },
    );
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
    if let Some(Screen::Projects {
        groups: ref g,
        loading,
        ..
    }) = m.stack.last()
    {
        assert!(!loading);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].project_name, "Project 0");
    } else {
        panic!("top screen is not Projects");
    }
}

#[test]
fn non_quit_msgs_do_not_set_should_quit() {
    let msgs: Vec<fn() -> Msg> = vec![
        || Msg::Up,
        || Msg::Down,
        || Msg::ScrollUp,
        || Msg::ScrollDown,
        || Msg::Click {
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        },
    ];
    for make_msg in msgs {
        let m = projects_model(3);
        let (m, _) = update(m, make_msg());
        assert!(!m.should_quit, "should_quit must be false for non-Quit msg");
    }
}

#[test]
fn update_returns_model_and_empty_cmds_for_navigation() {
    let m = projects_model(3);
    let (m, cmds) = update(m, Msg::Down);
    assert!(cmds.is_empty());
    assert_eq!(m.stack.last().unwrap().selected(), 1);
}

#[test]
fn tasks_screen_carries_correct_project_name_on_select() {
    let mut m = projects_model(3);
    if let Some(Screen::Projects { selected, .. }) = m.stack.last_mut() {
        *selected = 1;
    }
    let (m, _) = update(m, Msg::Select);
    if let Some(Screen::Tasks { project_name, .. }) = m.stack.last() {
        assert_eq!(project_name, "Project 1");
    } else {
        panic!("expected Tasks screen");
    }
}

#[test]
fn select_on_empty_projects_is_a_noop() {
    let m = Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: false,
            revalidating: false,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m, _) = update(m, Msg::Select);
    assert_eq!(m.stack.len(), 1);
    assert!(!m.should_quit);
}

fn make_tasks_with_project_id(count: usize, project_id: i64) -> Vec<TaskRow> {
    (0..count)
        .map(|i| TaskRow {
            task_id: i as i64,
            task_number: i as i64,
            name: format!("Task {i}"),
            instance: "inst".into(),
            project_id,
        })
        .collect()
}

fn tasks_model_with_project_id(task_count: usize, project_id: i64) -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: vec![ProjectGroup {
                    project_id,
                    project_name: "Test Project".into(),
                    instance: "inst".into(),
                    tasks: make_tasks_with_project_id(task_count, project_id),
                }],
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Tasks {
                project_name: "Test Project".into(),
                tasks: make_tasks_with_project_id(task_count, project_id),
                selected: 0,
                loading: false,
                revalidating: false,
            },
        ],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn detail_model(line_count: usize, offset: usize) -> Model {
    let lines: Vec<String> = (0..line_count).map(|i| format!("Line {i}")).collect();
    Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(1),
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: make_tasks(1),
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Detail {
                instance: "inst".into(),
                project_id: 0,
                task_id: 0,
                task: serde_json::Value::Null,
                comments: vec![],
                user_map: HashMap::new(),
                lines,
                line_styles: vec![],
                assets: vec![],
                offset,
                loading: false,
                rendered_width: 80,
            },
        ],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

fn loading_detail_model() -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(1),
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: make_tasks(1),
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Detail {
                instance: "inst".into(),
                project_id: 10,
                task_id: 99,
                task: serde_json::Value::Null,
                comments: vec![],
                user_map: HashMap::new(),
                lines: vec![],
                line_styles: vec![],
                assets: vec![],
                offset: 0,
                loading: true,
                rendered_width: usize::MAX,
            },
        ],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    }
}

#[test]
fn select_on_tasks_pushes_detail_screen_with_loading_true() {
    let m = tasks_model_with_project_id(3, 42);
    let (m, cmds) = update(m, Msg::Select);
    assert_eq!(m.stack.len(), 3);
    match m.stack.last() {
        Some(Screen::Detail {
            loading,
            project_id,
            task_id,
            instance,
            ..
        }) => {
            assert!(*loading, "Detail screen must start loading");
            assert_eq!(*project_id, 42);
            assert_eq!(*task_id, 0);
            assert_eq!(instance, "inst", "instance name must be threaded through");
        }
        other => panic!("expected Detail screen, got {other:?}"),
    }
    assert!(!m.should_quit);
    assert_eq!(cmds.len(), 1);
    assert!(matches!(
        &cmds[0],
        Cmd::LoadDetail {
            project_id: 42,
            task_id: 0,
            refresh: false,
            ..
        }
    ));
}

#[test]
fn select_on_tasks_emits_exactly_one_load_detail_cmd() {
    let m = tasks_model_with_project_id(1, 7);
    let (_m, cmds) = update(m, Msg::Select);
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd::LoadDetail");
    assert!(matches!(&cmds[0], Cmd::LoadDetail { refresh: false, .. }));
}

fn make_detail_load(task: serde_json::Value, user_map: HashMap<i64, String>) -> DetailLoad {
    DetailLoad {
        task,
        comments: vec![],
        assets: vec![],
        user_map,
        loaded_at: "2026-06-25T14:00:00Z".into(),
    }
}

#[test]
fn loaded_detail_stores_structured_data_and_clears_loading() {
    let m = loading_detail_model();
    let task = json!({ "name": "Test Task", "id": 99 });
    let load = make_detail_load(task.clone(), HashMap::new());
    let (m, cmds) = update(m, Msg::LoadedDetail(load));
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
    match m.stack.last() {
        Some(Screen::Detail {
            task: stored_task,
            loading,
            rendered_width,
            lines,
            ..
        }) => {
            assert!(!*loading, "loading must be cleared after LoadedDetail");
            assert_eq!(stored_task, &task, "task JSON must be stored");
            assert_eq!(
                *rendered_width,
                usize::MAX,
                "rendered_width must be MAX (cache invalidated)"
            );
            assert!(lines.is_empty(), "lines cache must be empty after load");
        }
        other => panic!("expected Detail screen, got {other:?}"),
    }
}

#[test]
fn back_on_detail_pops_to_tasks() {
    let m = detail_model(5, 0);
    assert_eq!(m.stack.len(), 3);
    let (m, _) = update(m, Msg::Back);
    assert_eq!(m.stack.len(), 2);
    assert!(matches!(m.stack.last(), Some(Screen::Tasks { .. })));
    assert!(!m.should_quit);
}

#[test]
fn detail_scroll_down_increments_offset() {
    let m = detail_model(5, 0);
    let (m, _) = update(m, Msg::Down);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 1),
        _ => panic!("expected Detail"),
    }
}

#[test]
fn detail_scroll_up_decrements_offset() {
    let m = detail_model(5, 2);
    let (m, _) = update(m, Msg::Up);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 1),
        _ => panic!("expected Detail"),
    }
}

#[test]
fn detail_scroll_up_at_zero_stays_zero() {
    let m = detail_model(5, 0);
    let (m, _) = update(m, Msg::Up);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 0),
        _ => panic!("expected Detail"),
    }
    assert!(!m.should_quit);
}

#[test]
fn detail_scroll_down_at_last_line_clamps() {
    let m = detail_model(3, 2);
    let (m, _) = update(m, Msg::Down);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 2),
        _ => panic!("expected Detail"),
    }
    assert!(!m.should_quit);
}

#[test]
fn detail_scroll_mouse_up_down_behaves_like_key() {
    let m = detail_model(5, 1);
    let (m, _) = update(m, Msg::ScrollUp);
    let offset_after_up = match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => *offset,
        _ => panic!("expected Detail"),
    };
    assert_eq!(offset_after_up, 0);

    let m2 = detail_model(5, 1);
    let (m2, _) = update(m2, Msg::ScrollDown);
    let offset_after_down = match m2.stack.last() {
        Some(Screen::Detail { offset, .. }) => *offset,
        _ => panic!("expected Detail"),
    };
    assert_eq!(offset_after_down, 2);
}

#[test]
fn detail_page_up_decrements_by_page_size() {
    let m = detail_model(30, 15);
    let (m, _) = update(m, Msg::PageUp);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 15 - PAGE_SIZE),
        _ => panic!("expected Detail"),
    }
}

#[test]
fn detail_page_up_clamps_at_zero() {
    let m = detail_model(30, 3);
    let (m, _) = update(m, Msg::PageUp);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 0),
        _ => panic!("expected Detail"),
    }
    assert!(!m.should_quit);
}

#[test]
fn detail_page_down_increments_by_page_size() {
    let m = detail_model(30, 0);
    let (m, _) = update(m, Msg::PageDown);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, PAGE_SIZE),
        _ => panic!("expected Detail"),
    }
}

#[test]
fn detail_page_down_clamps_at_max() {
    let m = detail_model(15, 10);
    let (m, _) = update(m, Msg::PageDown);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 14),
        _ => panic!("expected Detail"),
    }
    assert!(!m.should_quit);
}

#[test]
fn detail_over_scroll_never_sets_should_quit_or_panics() {
    let m = detail_model(3, 0);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);

    let m2 = detail_model(3, 0);
    let (m2, _) = update(m2, Msg::Up);
    assert!(!m2.should_quit);

    let m3 = detail_model(3, 0);
    let (m3, _) = update(m3, Msg::PageDown);
    let (m3, _) = update(m3, Msg::PageDown);
    assert!(!m3.should_quit);

    let m4 = detail_model(3, 0);
    let (m4, _) = update(m4, Msg::PageUp);
    assert!(!m4.should_quit);
}

#[test]
fn detail_click_does_not_change_state_or_quit() {
    let m = detail_model(5, 2);
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: 100,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(!m.should_quit);
    assert!(cmds.is_empty());
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => assert_eq!(*offset, 2),
        _ => panic!("expected Detail"),
    }
}

// R6c: AC1 — single-flight guard drops refresh while loading is true
#[test]
fn refresh_while_detail_loading_is_dropped() {
    let m = loading_detail_model();
    let depth_before = m.stack.len();
    let (m, cmds) = update(m, Msg::Refresh);
    assert!(cmds.is_empty(), "refresh-in-flight must produce no Cmd");
    assert_eq!(m.stack.len(), depth_before, "stack depth must not change");
    match m.stack.last() {
        Some(Screen::Detail {
            loading, offset, ..
        }) => {
            assert!(*loading, "loading flag must remain true");
            assert_eq!(*offset, 0, "offset must stay at 0");
        }
        _ => panic!("expected Detail screen"),
    }
    assert!(!m.should_quit);
}

#[test]
fn refresh_while_projects_loading_is_dropped() {
    let m = loading_projects_model();
    let (m, cmds) = update(m, Msg::Refresh);
    assert!(cmds.is_empty(), "refresh-in-flight must produce no Cmd");
    match m.stack.last() {
        Some(Screen::Projects { loading, .. }) => assert!(*loading),
        _ => panic!("expected Projects screen"),
    }
    assert!(!m.should_quit);
}

// R6c: AC2 — refresh when idle emits the correct Cmd and sets loading=true + offset=0
#[test]
fn refresh_on_idle_detail_emits_load_detail_refresh_true() {
    let m = detail_model(10, 5);
    let (m, cmds) = update(m, Msg::Refresh);
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd");
    assert!(
        matches!(&cmds[0], Cmd::LoadDetail { refresh: true, .. }),
        "cmd must be LoadDetail with refresh:true, got: {:?}",
        &cmds[0]
    );
    match m.stack.last() {
        Some(Screen::Detail {
            loading, offset, ..
        }) => {
            assert!(*loading, "loading must be set to true");
            assert_eq!(*offset, 0, "offset must reset to 0");
        }
        _ => panic!("expected Detail screen"),
    }
    assert!(!m.should_quit);
}

#[test]
fn refresh_on_idle_projects_emits_load_tasks_by_project() {
    let m = projects_model(3);
    let (m, cmds) = update(m, Msg::Refresh);
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd");
    assert_eq!(cmds[0], Cmd::LoadTasksByProject);
    match m.stack.last() {
        Some(Screen::Projects { loading, .. }) => assert!(*loading, "loading must be true"),
        _ => panic!("expected Projects screen"),
    }
    assert!(!m.should_quit);
}

#[test]
fn refresh_on_idle_tasks_emits_load_tasks_by_project() {
    let m = tasks_model(3);
    let (m, cmds) = update(m, Msg::Refresh);
    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd");
    assert_eq!(cmds[0], Cmd::LoadTasksByProject);
    match m.stack.last() {
        Some(Screen::Tasks { loading, .. }) => assert!(*loading, "loading must be true"),
        _ => panic!("expected Tasks screen"),
    }
    assert!(!m.should_quit);
}

// R6c: AC3 — cross-screen safety matrix: Detail mouse/scroll/click-below-last never quit
#[test]
fn detail_wheel_scroll_never_sets_should_quit() {
    let m = detail_model(3, 2);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    assert!(!m.should_quit);

    let m2 = detail_model(3, 0);
    let (m2, _) = update(m2, Msg::ScrollUp);
    assert!(!m2.should_quit);
}

#[test]
fn detail_click_below_last_row_does_not_quit() {
    let m = detail_model(3, 0);
    let (m, _) = update(
        m,
        Msg::Click {
            column: 0,
            row: 255,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(!m.should_quit);
}

#[test]
fn projects_mouse_scroll_and_click_beyond_bounds_never_quit() {
    let m = projects_model(2);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    assert!(!m.should_quit);
    let m2 = projects_model(2);
    let (m2, _) = update(
        m2,
        Msg::Click {
            column: 0,
            row: 255,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(!m2.should_quit);
}

#[test]
fn tasks_mouse_scroll_and_click_beyond_bounds_never_quit() {
    let m = tasks_model(2);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    let (m, _) = update(m, Msg::ScrollDown);
    assert!(!m.should_quit);
    let m2 = tasks_model(2);
    let (m2, _) = update(
        m2,
        Msg::Click {
            column: 0,
            row: 255,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(!m2.should_quit);
}

fn make_asset(name: &str, url: &str) -> Asset {
    Asset {
        name: name.into(),
        url: url.into(),
    }
}

#[test]
fn select_on_tasks_threads_instance_into_load_detail_cmd() {
    let m = Model {
        stack: vec![
            Screen::Projects {
                groups: vec![ProjectGroup {
                    project_id: 10,
                    project_name: "P".into(),
                    instance: "second-inst".into(),
                    tasks: vec![TaskRow {
                        task_id: 5,
                        task_number: 1,
                        name: "T".into(),
                        instance: "second-inst".into(),
                        project_id: 10,
                    }],
                }],
                selected: 0,
                loading: false,
                revalidating: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: vec![TaskRow {
                    task_id: 5,
                    task_number: 1,
                    name: "T".into(),
                    instance: "second-inst".into(),
                    project_id: 10,
                }],
                selected: 0,
                loading: false,
                revalidating: false,
            },
        ],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m, cmds) = update(m, Msg::Select);
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::LoadDetail { instance, .. } => {
            assert_eq!(
                instance, "second-inst",
                "instance must come from the selected TaskRow"
            );
        }
        other => panic!("expected LoadDetail, got {other:?}"),
    }
    match m.stack.last() {
        Some(Screen::Detail { instance, .. }) => {
            assert_eq!(instance, "second-inst");
        }
        other => panic!("expected Detail screen, got {other:?}"),
    }
}

#[test]
fn loaded_detail_stores_assets_on_screen() {
    let m = loading_detail_model();
    let assets = vec![make_asset("img.png", "https://example.com/img.png")];
    let load = DetailLoad {
        task: serde_json::Value::Null,
        comments: vec![],
        assets: assets.clone(),
        user_map: HashMap::new(),
        loaded_at: "2026-06-25T14:00:00Z".into(),
    };
    let (m, _) = update(m, Msg::LoadedDetail(load));
    match m.stack.last() {
        Some(Screen::Detail { assets: stored, .. }) => {
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].url, "https://example.com/img.png");
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

fn make_mine_row(task_id: i64, task_number: i64, project_id: i64, instance: &str) -> MineTableRow {
    MineTableRow {
        instance: instance.into(),
        project_id,
        task_number,
        task_id,
        name: format!("Task {task_id}"),
    }
}

// S3-A1: Select on mine screen pushes Detail with THAT row's project_id/task_id/instance
#[test]
fn mine_select_pushes_detail_with_row_project_id_and_instance() {
    let rows = vec![
        make_mine_row(101, 1, 10, "inst-alpha"),
        make_mine_row(202, 2, 20, "inst-beta"),
    ];
    let m = init_mine(empty_header(), Some(rows)).0;

    // Verify initial state: Tasks screen, row 0 selected
    assert_eq!(m.stack.len(), 1);
    assert!(matches!(
        m.stack.last(),
        Some(Screen::Tasks { selected: 0, .. })
    ));

    // Move to row 1 (inst-beta, project 20, task 202), then Select
    let (m, _) = update(m, Msg::Down);
    let (m, cmds) = update(m, Msg::Select);

    assert_eq!(cmds.len(), 1, "must emit exactly one Cmd::LoadDetail");
    match &cmds[0] {
        Cmd::LoadDetail {
            instance,
            project_id,
            task_id,
            refresh,
        } => {
            assert_eq!(instance, "inst-beta", "must use row's instance, not first");
            assert_eq!(
                *project_id, 20,
                "must use row's project_id, not a stack-walk result"
            );
            assert_eq!(*task_id, 202);
            assert!(!*refresh);
        }
        other => panic!("expected Cmd::LoadDetail, got {other:?}"),
    }

    assert_eq!(m.stack.len(), 2);
    match m.stack.last() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            loading,
            ..
        }) => {
            assert_eq!(instance, "inst-beta");
            assert_eq!(*project_id, 20);
            assert_eq!(*task_id, 202);
            assert!(*loading);
        }
        other => panic!("expected Detail screen, got {other:?}"),
    }
}

// S3-A1 variant: first row (inst-alpha, project 10) also resolves from row, not stack
#[test]
fn mine_select_first_row_uses_its_own_project_id_and_instance() {
    let rows = vec![
        make_mine_row(101, 1, 10, "inst-alpha"),
        make_mine_row(202, 2, 20, "inst-beta"),
    ];
    let m = init_mine(empty_header(), Some(rows)).0;

    let (_m, cmds) = update(m, Msg::Select);

    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::LoadDetail {
            instance,
            project_id,
            task_id,
            ..
        } => {
            assert_eq!(instance, "inst-alpha");
            assert_eq!(*project_id, 10);
            assert_eq!(*task_id, 101);
        }
        other => panic!("expected Cmd::LoadDetail, got {other:?}"),
    }
}

// S3-A2 / V2b: a click on a row with a matching hit-map target drills into that row's Detail
// directly (no separate Select needed), resolving the correct index from click_targets.
#[test]
fn mine_click_with_target_drills_into_clicked_rows_detail() {
    use crate::tui::model::ClickTarget;
    let rows = vec![
        make_mine_row(101, 1, 10, "inst-alpha"),
        make_mine_row(202, 2, 20, "inst-beta"),
        make_mine_row(303, 3, 30, "inst-gamma"),
    ];
    let mut m = init_mine(empty_header(), Some(rows)).0;

    // Simulate the shell writing a hit-map for row index 2 at terminal y=4.
    m.set_click_targets(vec![
        ClickTarget {
            y_start: 2,
            y_end: 3,
            index: 0,
        },
        ClickTarget {
            y_start: 3,
            y_end: 4,
            index: 1,
        },
        ClickTarget {
            y_start: 4,
            y_end: 5,
            index: 2,
        },
    ]);

    // Click at y=4 → index 2 → inst-gamma, project 30, task 303
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: 4,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert_eq!(cmds.len(), 1, "click must emit one Cmd::LoadDetail");
    match &cmds[0] {
        Cmd::LoadDetail {
            instance,
            project_id,
            task_id,
            ..
        } => {
            assert_eq!(instance, "inst-gamma", "must use clicked row's instance");
            assert_eq!(*project_id, 30, "must use clicked row's project_id");
            assert_eq!(*task_id, 303);
        }
        other => panic!("expected Cmd::LoadDetail, got {other:?}"),
    }
    match m.stack.last() {
        Some(Screen::Detail {
            instance,
            project_id,
            task_id,
            ..
        }) => {
            assert_eq!(instance, "inst-gamma");
            assert_eq!(*project_id, 30);
            assert_eq!(*task_id, 303);
        }
        other => panic!("expected Detail screen, got {other:?}"),
    }
}

// S3-A2 variant / V2b: click in empty space below all targets is a no-op.
#[test]
fn mine_click_below_last_target_is_noop() {
    use crate::tui::model::ClickTarget;
    let rows = vec![
        make_mine_row(101, 1, 10, "inst-alpha"),
        make_mine_row(202, 2, 20, "inst-beta"),
    ];
    let mut m = init_mine(empty_header(), Some(rows)).0;
    let sel_before = m.stack.last().unwrap().selected();

    m.set_click_targets(vec![
        ClickTarget {
            y_start: 2,
            y_end: 3,
            index: 0,
        },
        ClickTarget {
            y_start: 3,
            y_end: 4,
            index: 1,
        },
    ]);

    // Click at y=10 is below all targets → no-op
    let (m, cmds) = update(
        m,
        Msg::Click {
            column: 0,
            row: 10,
            modifiers: KeyModifiers::NONE,
        },
    );
    assert!(cmds.is_empty(), "click below targets must emit no cmd");
    assert_eq!(m.stack.last().unwrap().selected(), sel_before);
    assert!(!m.should_quit);
}

// Verify mine_model produces a single Tasks screen at loading:false with rows mapped
#[test]
fn mine_model_produces_tasks_screen_with_all_rows_loaded() {
    let rows = vec![
        make_mine_row(10, 1, 5, "inst-a"),
        make_mine_row(20, 2, 6, "inst-b"),
    ];
    let m = init_mine(empty_header(), Some(rows)).0;

    assert_eq!(m.stack.len(), 1);
    assert!(!m.should_quit);
    match m.stack.last() {
        Some(Screen::Tasks {
            tasks,
            loading,
            selected,
            project_name,
            ..
        }) => {
            assert!(!*loading, "mine model must NOT start loading");
            assert_eq!(*selected, 0);
            assert_eq!(tasks.len(), 2);
            assert_eq!(tasks[0].task_id, 10);
            assert_eq!(tasks[0].project_id, 5);
            assert_eq!(tasks[0].instance, "inst-a");
            assert_eq!(tasks[1].task_id, 20);
            assert_eq!(tasks[1].project_id, 6);
            assert_eq!(tasks[1].instance, "inst-b");
            assert!(
                project_name.contains("My Tasks"),
                "project_name must be My Tasks, got: {project_name}"
            );
        }
        other => panic!("expected Tasks screen, got {other:?}"),
    }
}

// P2-A1 / P2-A3: reflow_detail builds line cache at inner_width; is memoized
#[test]
fn reflow_detail_builds_lines_and_is_memoized() {
    let task = json!({
        "name": "My Task",
        "id": 1,
        "project_id": 10,
        "is_completed": false
    });
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 10,
            task_id: 1,
            task,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    // First reflow at width 80: lines must be populated, rendered_width updated
    m.reflow_detail(80);
    let lines_after_first = match m.stack.last() {
        Some(Screen::Detail {
            lines,
            rendered_width,
            ..
        }) => {
            assert_eq!(*rendered_width, 80, "rendered_width must be updated");
            assert!(!lines.is_empty(), "lines must be built after reflow");
            lines.clone()
        }
        _ => panic!("expected Detail"),
    };

    // Second reflow at same width: lines must be identical (memoized, no rebuild)
    m.reflow_detail(80);
    match m.stack.last() {
        Some(Screen::Detail { lines, .. }) => {
            assert_eq!(
                lines, &lines_after_first,
                "lines must not change on a same-width reflow"
            );
        }
        _ => panic!("expected Detail"),
    }

    // Reflow at a different width: rendered_width must update and cache must remain valid
    m.reflow_detail(40);
    match m.stack.last() {
        Some(Screen::Detail {
            lines,
            rendered_width,
            ..
        }) => {
            assert_eq!(
                *rendered_width, 40,
                "rendered_width must update to new width"
            );
            assert!(
                !lines.is_empty(),
                "lines must still be present after second reflow"
            );
            // Every line must fit within the new width
            for line in lines.iter() {
                assert!(
                    line.chars().count() <= 40,
                    "line after reflow at 40 must fit 40 chars: {:?}",
                    line
                );
            }
        }
        _ => panic!("expected Detail"),
    }
}

// P2-A1: every line produced by reflow_detail fits inner_width
#[test]
fn reflow_detail_lines_fit_inner_width() {
    let task = json!({
        "name": "A task with a long description that should be wrapped",
        "id": 42,
        "project_id": 5,
        "is_completed": false,
        "body": "<p>This is a body paragraph with enough words to wrap at a narrow width.</p>"
    });
    let comment = json!({
        "created_by_name": "Alice",
        "created_on": 1700000000,
        "body": "<p>A comment with sufficient length to demonstrate wrapping at narrow terminal.</p>"
    });
    let inner_width: usize = 40;
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 5,
            task_id: 42,
            task,
            comments: vec![comment],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    m.reflow_detail(inner_width);
    match m.stack.last() {
        Some(Screen::Detail { lines, .. }) => {
            assert!(!lines.is_empty(), "lines must produce at least one line");
            for line in lines.iter() {
                assert!(
                    line.chars().count() <= inner_width,
                    "line exceeds inner_width={}: {:?}",
                    inner_width,
                    line
                );
            }
        }
        _ => panic!("expected Detail"),
    }
}

// P2-A1: resize reflows — different width produces different rendered_width
#[test]
fn reflow_detail_rebuilds_on_width_change() {
    let task = json!({ "name": "T", "id": 1, "project_id": 1, "is_completed": false });
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec![],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    m.reflow_detail(80);
    let rw_80 = match m.stack.last() {
        Some(Screen::Detail { rendered_width, .. }) => *rendered_width,
        _ => panic!("expected Detail"),
    };
    assert_eq!(rw_80, 80);

    m.reflow_detail(60);
    let rw_60 = match m.stack.last() {
        Some(Screen::Detail { rendered_width, .. }) => *rendered_width,
        _ => panic!("expected Detail"),
    };
    assert_eq!(rw_60, 60);
}

// P2-A3: reflow_detail is a no-op while loading
#[test]
fn reflow_detail_is_noop_while_loading() {
    let task = json!({ "name": "T", "id": 1, "project_id": 1, "is_completed": false });
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task,
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
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    m.reflow_detail(80);
    match m.stack.last() {
        Some(Screen::Detail {
            lines,
            rendered_width,
            ..
        }) => {
            assert!(lines.is_empty(), "must not build cache while loading");
            assert_eq!(
                *rendered_width,
                usize::MAX,
                "rendered_width must stay MAX while loading"
            );
        }
        _ => panic!("expected Detail"),
    }
}

// P2-A3: reflow_detail clamps offset when content shortens
#[test]
fn reflow_detail_clamps_offset_when_content_shortens() {
    let task = json!({
        "name": "T", "id": 1, "project_id": 1, "is_completed": false,
        "body": "<p>Short body.</p>"
    });
    let mut m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
            line_styles: vec![],
            assets: vec![],
            offset: 4,
            loading: false,
            rendered_width: usize::MAX,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    m.reflow_detail(80);
    match m.stack.last() {
        Some(Screen::Detail { offset, lines, .. }) => {
            let max_offset = lines.len().saturating_sub(1);
            assert!(
                *offset <= max_offset,
                "offset={} must be clamped to max_offset={}",
                offset,
                max_offset
            );
        }
        _ => panic!("expected Detail"),
    }
}

// P2-A2: UserMapResolved updates user_map and invalidates the render cache
#[test]
fn user_map_resolved_updates_map_and_invalidates_cache() {
    let task = json!({
        "name": "Task",
        "id": 1,
        "project_id": 1,
        "is_completed": false,
        "assignee_id": 42
    });
    let m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 1,
            task_id: 1,
            task,
            comments: vec![],
            user_map: HashMap::new(),
            lines: vec!["old content".into()],
            line_styles: vec![],
            assets: vec![],
            offset: 0,
            loading: false,
            rendered_width: 80,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let mut new_map = HashMap::new();
    new_map.insert(42i64, "Alice".to_string());
    let (m, cmds) = update(m, Msg::UserMapResolved(new_map.clone()));
    assert!(cmds.is_empty());

    match m.stack.last() {
        Some(Screen::Detail {
            user_map,
            lines,
            rendered_width,
            ..
        }) => {
            assert_eq!(
                user_map.get(&42),
                Some(&"Alice".to_string()),
                "user_map must be updated"
            );
            assert!(lines.is_empty(), "lines cache must be invalidated");
            assert_eq!(
                *rendered_width,
                usize::MAX,
                "rendered_width must be reset to MAX"
            );
        }
        _ => panic!("expected Detail"),
    }
}

// P2-A2: progressive paint — LoadedDetail with empty user_map, then UserMapResolved
// with a name; after reflow, the assignee line contains the name.
#[test]
fn progressive_paint_assignee_fills_in_after_user_map_resolved() {
    let task = json!({
        "name": "Task",
        "id": 7,
        "project_id": 3,
        "is_completed": false,
        "assignee_id": 99
    });

    // Phase 1: LoadedDetail with empty user_map
    let load = DetailLoad {
        task: task.clone(),
        comments: vec![],
        assets: vec![],
        user_map: HashMap::new(),
        loaded_at: "2026-06-25T14:00:00Z".into(),
    };
    let m = Model {
        stack: vec![Screen::Detail {
            instance: "inst".into(),
            project_id: 3,
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
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };
    let (m, _) = update(m, Msg::LoadedDetail(load));

    // Reflow at width 80: assignee shows as "(99)" because user_map is empty
    let mut m = m;
    m.reflow_detail(80);
    let lines_phase1 = match m.stack.last() {
        Some(Screen::Detail { lines, .. }) => lines.clone(),
        _ => panic!("expected Detail"),
    };
    let phase1_assignee = lines_phase1
        .iter()
        .find(|l| l.contains("(99)"))
        .cloned()
        .unwrap_or_default();
    assert!(
        phase1_assignee.contains("(99)"),
        "phase-1 must show raw id: {:?}",
        lines_phase1
    );

    // Phase 2: UserMapResolved with Alice
    let mut resolved_map = HashMap::new();
    resolved_map.insert(99i64, "Alice".to_string());
    let (mut m, _) = update(m, Msg::UserMapResolved(resolved_map));

    // Reflow at same width: cache must be rebuilt with the new name
    m.reflow_detail(80);
    let lines_phase2 = match m.stack.last() {
        Some(Screen::Detail { lines, .. }) => lines.clone(),
        _ => panic!("expected Detail"),
    };
    assert!(
        lines_phase2.iter().any(|l| l.contains("Alice")),
        "phase-2 lines must contain assignee name 'Alice': {:?}",
        lines_phase2
    );
}

// P2-A3: reflow_detail is a no-op on non-Detail screens
#[test]
fn reflow_detail_is_noop_on_projects_screen() {
    let mut m = projects_model(2);
    m.reflow_detail(80);
    match m.stack.last() {
        Some(Screen::Projects { .. }) => {}
        other => panic!("expected Projects, got {other:?}"),
    }
    assert!(!m.should_quit);
}

// U5b-A1: HeaderNameResolved fills model.header.name from None to Some and header_line shows it
#[test]
fn header_name_resolved_fills_name_and_header_line_shows_it() {
    use crate::store::instances::Instance;
    let inst = Instance {
        name: "acme".into(),
        base_url: "https://acme.example.com".into(),
        email: "user@acme.example.com".into(),
        token: "tok".into(),
        user_id: Some(7),
    };
    let header = Header::from_instances(&[inst], None);
    assert!(header.name.is_none(), "header.name must start as None");

    let m = Model {
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
    let stack_len_before = m.stack.len();

    let (m, cmds) = update(m, Msg::HeaderNameResolved("Alice".into()));

    assert!(cmds.is_empty(), "HeaderNameResolved must emit no Cmds");
    assert_eq!(
        m.header.name,
        Some("Alice".to_string()),
        "header.name must become Some(Alice)"
    );
    assert!(
        m.header.header_line().contains("Alice"),
        "header_line must contain the resolved name; got: {}",
        m.header.header_line()
    );
    assert_eq!(
        m.stack.len(),
        stack_len_before,
        "screen stack must be unchanged"
    );
    assert!(!m.should_quit);
}

// U6c-A2: single global offset scrolls entire content; no Tab/focus cycling
#[test]
fn detail_global_scroll_offset_advances_through_all_content() {
    let lines: Vec<String> = (0..20).map(|i| format!("line {i}")).collect();
    let m = Model {
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
            rendered_width: 80,
        }],
        should_quit: false,
        header: empty_header(),
        viewport: (0, 0),
        click_targets: vec![],
        last_loaded: None,
        selection: None,
        copied_feedback: false,
    };

    let (m, _) = update(m, Msg::Down);
    let (m, _) = update(m, Msg::Down);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(
                *offset, 2,
                "two Down presses must advance single offset to 2"
            );
        }
        _ => panic!("expected Detail"),
    }

    let (m, _) = update(m, Msg::Up);
    match m.stack.last() {
        Some(Screen::Detail { offset, .. }) => {
            assert_eq!(*offset, 1, "Up must decrement single offset");
        }
        _ => panic!("expected Detail"),
    }
}
