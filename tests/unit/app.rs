use super::*;

fn make_groups(count: usize) -> Vec<ProjectGroup> {
    (0..count)
        .map(|i| ProjectGroup {
            project_id: i as i64,
            project_name: format!("Project {i}"),
            tasks: vec![TaskRow {
                task_id: i as i64,
                task_number: i as i64,
                name: format!("Task {i}"),
                instance: "inst".into(),
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
        })
        .collect()
}

fn projects_model(count: usize) -> Model {
    Model {
        stack: vec![Screen::Projects {
            groups: make_groups(count),
            selected: 0,
            loading: false,
        }],
        should_quit: false,
    }
}

fn tasks_model(count: usize) -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(2),
                selected: 0,
                loading: false,
            },
            Screen::Tasks {
                project_name: "Project 0".into(),
                tasks: make_tasks(count),
                selected: 0,
                loading: false,
            },
        ],
        should_quit: false,
    }
}

fn loading_projects_model() -> Model {
    Model {
        stack: vec![Screen::Projects {
            groups: vec![],
            selected: 0,
            loading: true,
        }],
        should_quit: false,
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

#[test]
fn click_clamps_to_last_row_projects() {
    let m = projects_model(3);
    let (m, _) = update(m, Msg::Click(99));
    assert_eq!(m.stack.last().unwrap().selected(), 2);
    assert!(!m.should_quit);
}

#[test]
fn click_clamps_to_last_row_tasks() {
    let m = tasks_model(3);
    let (m, _) = update(m, Msg::Click(99));
    assert_eq!(m.stack.last().unwrap().selected(), 2);
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
        }],
        should_quit: false,
    };
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::Up);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::Click(0));
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
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: vec![],
                selected: 0,
                loading: false,
            },
        ],
        should_quit: false,
    };
    let (m, _) = update(m, Msg::Down);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::Up);
    assert!(!m.should_quit);
    let (m, _) = update(m, Msg::Click(0));
    assert!(!m.should_quit);
}

#[test]
fn init_browse_emits_load_tasks_by_project_cmd() {
    let (model, cmds) = init_browse();
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
    let (m, cmds) = update(m, Msg::LoadedTasksByProject(groups.clone()));
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
        || Msg::Click(0),
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
        }],
        should_quit: false,
    };
    let (m, _) = update(m, Msg::Select);
    assert_eq!(m.stack.len(), 1);
    assert!(!m.should_quit);
}

#[test]
fn flat_model_tests_preserved() {
    let tasks = vec![
        Task {
            project: "P1".into(),
            id: 1,
            name: "Task A".into(),
        },
        Task {
            project: "P2".into(),
            id: 2,
            name: "Task B".into(),
        },
    ];
    let m = FlatModel::with_tasks(tasks);
    assert_eq!(m.tasks.len(), 2);
    assert_eq!(m.selected, 0);
    assert!(!m.should_quit);

    let m = update_flat(m, FlatMsg::Down);
    assert_eq!(m.selected, 1);
    let m = update_flat(m, FlatMsg::Down);
    assert_eq!(m.selected, 1);

    let m2 = FlatModel::with_tasks(vec![]);
    let m2 = update_flat(m2, FlatMsg::Down);
    assert!(!m2.should_quit);

    let m3 = FlatModel::with_tasks(vec![Task {
        project: "P".into(),
        id: 1,
        name: "T".into(),
    }]);
    let m3 = update_flat(m3, FlatMsg::Quit);
    assert!(m3.should_quit);
}

fn tasks_model_with_project_id(task_count: usize, project_id: i64) -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: vec![ProjectGroup {
                    project_id,
                    project_name: "Test Project".into(),
                    tasks: make_tasks(task_count),
                }],
                selected: 0,
                loading: false,
            },
            Screen::Tasks {
                project_name: "Test Project".into(),
                tasks: make_tasks(task_count),
                selected: 0,
                loading: false,
            },
        ],
        should_quit: false,
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
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: make_tasks(1),
                selected: 0,
                loading: false,
            },
            Screen::Detail {
                instance: "inst".into(),
                project_id: 0,
                task_id: 0,
                lines,
                assets: vec![],
                offset,
                loading: false,
                pending_download: false,
            },
        ],
        should_quit: false,
    }
}

fn loading_detail_model() -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(1),
                selected: 0,
                loading: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: make_tasks(1),
                selected: 0,
                loading: false,
            },
            Screen::Detail {
                instance: "inst".into(),
                project_id: 10,
                task_id: 99,
                lines: vec![],
                assets: vec![],
                offset: 0,
                loading: true,
                pending_download: false,
            },
        ],
        should_quit: false,
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

#[test]
fn loaded_detail_stores_lines_and_clears_loading() {
    let m = loading_detail_model();
    let lines = vec!["Line 1".to_string(), "Line 2".to_string()];
    let (m, cmds) = update(m, Msg::LoadedDetail(lines.clone(), vec![]));
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
    match m.stack.last() {
        Some(Screen::Detail {
            lines: l, loading, ..
        }) => {
            assert!(!*loading, "loading must be cleared after LoadedDetail");
            assert_eq!(*l, lines);
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
    let (m, cmds) = update(m, Msg::Click(100));
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
    let (m, _) = update(m, Msg::Click(999));
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
    let (m2, _) = update(m2, Msg::Click(999));
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
    let (m2, _) = update(m2, Msg::Click(999));
    assert!(!m2.should_quit);
}

fn detail_model_with_assets(assets: Vec<Asset>, instance: &str) -> Model {
    Model {
        stack: vec![
            Screen::Projects {
                groups: make_groups(1),
                selected: 0,
                loading: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: vec![TaskRow {
                    task_id: 1,
                    task_number: 1,
                    name: "T".into(),
                    instance: instance.into(),
                }],
                selected: 0,
                loading: false,
            },
            Screen::Detail {
                instance: instance.into(),
                project_id: 0,
                task_id: 1,
                lines: vec![],
                assets,
                offset: 0,
                loading: false,
                pending_download: false,
            },
        ],
        should_quit: false,
    }
}

fn make_asset(name: &str, url: &str) -> Asset {
    Asset {
        name: name.into(),
        url: url.into(),
    }
}

#[test]
fn asset_open_digit_1_emits_open_asset_cmd_with_correct_url() {
    let assets = vec![
        make_asset("file1.pdf", "https://acme.example.com/file1.pdf"),
        make_asset("file2.pdf", "https://acme.example.com/file2.pdf"),
    ];
    let m = detail_model_with_assets(assets, "acme");
    let (_m, cmds) = update(m, Msg::AssetOpen('1'));
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::OpenAsset { instance, url } => {
            assert_eq!(instance, "acme");
            assert_eq!(url, "https://acme.example.com/file1.pdf");
        }
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

#[test]
fn asset_open_digit_2_selects_second_asset() {
    let assets = vec![
        make_asset("a.pdf", "https://host.com/a.pdf"),
        make_asset("b.pdf", "https://host.com/b.pdf"),
    ];
    let m = detail_model_with_assets(assets, "inst-b");
    let (_m, cmds) = update(m, Msg::AssetOpen('2'));
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::OpenAsset { url, .. } => assert_eq!(url, "https://host.com/b.pdf"),
        other => panic!("expected OpenAsset, got {other:?}"),
    }
}

#[test]
fn asset_open_out_of_range_digit_emits_no_cmd() {
    let assets = vec![make_asset("only.pdf", "https://host.com/only.pdf")];
    let m = detail_model_with_assets(assets, "inst");
    let (_m, cmds) = update(m, Msg::AssetOpen('9'));
    assert!(cmds.is_empty(), "out-of-range digit must produce no cmd");
}

#[test]
fn asset_open_digit_0_emits_no_cmd() {
    let assets = vec![make_asset("a.pdf", "https://host.com/a.pdf")];
    let m = detail_model_with_assets(assets, "inst");
    let (_m, cmds) = update(m, Msg::AssetOpen('0'));
    assert!(cmds.is_empty(), "digit 0 must produce no cmd");
}

#[test]
fn asset_download_emits_download_cmd_with_selected_instance() {
    let assets = vec![make_asset(
        "report.pdf",
        "https://acme.example.com/report.pdf",
    )];
    let m = detail_model_with_assets(assets, "acme-inst");
    let (m, _) = update(m, Msg::TogglePendingDownload);
    let (_m, cmds) = update(m, Msg::AssetOpen('1'));
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::DownloadAsset {
            instance,
            url,
            name,
        } => {
            assert_eq!(instance, "acme-inst");
            assert_eq!(url, "https://acme.example.com/report.pdf");
            assert_eq!(name, "report.pdf");
        }
        other => panic!("expected DownloadAsset, got {other:?}"),
    }
}

#[test]
fn asset_open_carries_selected_tasks_instance_not_first_instance() {
    let m = Model {
        stack: vec![
            Screen::Projects {
                groups: vec![ProjectGroup {
                    project_id: 1,
                    project_name: "P".into(),
                    tasks: vec![
                        TaskRow {
                            task_id: 10,
                            task_number: 1,
                            name: "task on inst1".into(),
                            instance: "inst1".into(),
                        },
                        TaskRow {
                            task_id: 20,
                            task_number: 2,
                            name: "task on inst2".into(),
                            instance: "inst2".into(),
                        },
                    ],
                }],
                selected: 0,
                loading: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: vec![
                    TaskRow {
                        task_id: 10,
                        task_number: 1,
                        name: "task on inst1".into(),
                        instance: "inst1".into(),
                    },
                    TaskRow {
                        task_id: 20,
                        task_number: 2,
                        name: "task on inst2".into(),
                        instance: "inst2".into(),
                    },
                ],
                selected: 1,
                loading: false,
            },
            Screen::Detail {
                instance: "inst2".into(),
                project_id: 1,
                task_id: 20,
                lines: vec![],
                assets: vec![make_asset("x.pdf", "https://inst2.example.com/x.pdf")],
                offset: 0,
                loading: false,
                pending_download: false,
            },
        ],
        should_quit: false,
    };
    let (_m, cmds) = update(m, Msg::AssetOpen('1'));
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Cmd::OpenAsset { instance, .. } => {
            assert_eq!(
                instance, "inst2",
                "must use selected task's instance, not first"
            );
        }
        other => panic!("expected OpenAsset, got {other:?}"),
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
                    tasks: vec![TaskRow {
                        task_id: 5,
                        task_number: 1,
                        name: "T".into(),
                        instance: "second-inst".into(),
                    }],
                }],
                selected: 0,
                loading: false,
            },
            Screen::Tasks {
                project_name: "P".into(),
                tasks: vec![TaskRow {
                    task_id: 5,
                    task_number: 1,
                    name: "T".into(),
                    instance: "second-inst".into(),
                }],
                selected: 0,
                loading: false,
            },
        ],
        should_quit: false,
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
    let (m, _) = update(m, Msg::LoadedDetail(vec!["line".into()], assets.clone()));
    match m.stack.last() {
        Some(Screen::Detail { assets: stored, .. }) => {
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].url, "https://example.com/img.png");
        }
        other => panic!("expected Detail, got {other:?}"),
    }
}

#[test]
fn asset_action_on_non_detail_screen_is_noop() {
    let m = projects_model(2);
    let (m, cmds) = update(m, Msg::AssetOpen('1'));
    assert!(cmds.is_empty());
    assert!(!m.should_quit);
}
