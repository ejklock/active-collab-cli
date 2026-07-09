use super::*;

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

#[test]
fn bare_ref_prepends_get() {
    let result = normalize_argv(&argv(&["665/75159"]), None);
    assert_eq!(result, argv(&["get", "665/75159"]));
}

#[test]
fn bare_ref_with_flags_prepends_get() {
    let result = normalize_argv(&argv(&["665/75159", "--short"]), None);
    assert_eq!(result, argv(&["get", "665/75159", "--short"]));
}

#[test]
fn known_command_passes_through_unchanged() {
    for cmd in KNOWN_COMMANDS {
        let input = argv(&[cmd]);
        let result = normalize_argv(&input, None);
        assert_eq!(result, input, "command '{cmd}' should pass through");
    }
}

#[test]
fn flag_first_argv_passes_through_unchanged() {
    let input = argv(&["--help"]);
    let result = normalize_argv(&input, None);
    assert_eq!(result, input);
}

#[test]
fn empty_argv_with_matching_branch_returns_current() {
    let result = normalize_argv(&[], Some("feature/665-75159"));
    assert_eq!(result, argv(&["current"]));
}

#[test]
fn empty_argv_with_hotfix_branch_returns_current() {
    let result = normalize_argv(&[], Some("hotfix/1-2"));
    assert_eq!(result, argv(&["current"]));
}

#[test]
fn empty_argv_with_fix_branch_returns_current() {
    let result = normalize_argv(&[], Some("fix/99-1000"));
    assert_eq!(result, argv(&["current"]));
}

#[test]
fn empty_argv_with_non_matching_branch_passes_through() {
    let result = normalize_argv(&[], Some("main"));
    assert_eq!(result, argv(&[]));
}

#[test]
fn empty_argv_with_none_branch_passes_through() {
    let result = normalize_argv(&[], None);
    assert_eq!(result, argv(&[]));
}

#[test]
fn empty_argv_with_detached_head_passes_through() {
    let result = normalize_argv(&[], Some("HEAD"));
    assert_eq!(result, argv(&[]));
}

#[test]
fn branch_pattern_accepts_valid_branches() {
    assert!(branch_matches_task_pattern("feature/665-75159"));
    assert!(branch_matches_task_pattern("hotfix/1-2"));
    assert!(branch_matches_task_pattern("fix/99-1000"));
}

#[test]
fn branch_pattern_rejects_wrong_prefix() {
    assert!(!branch_matches_task_pattern("chore/665-75159"));
    assert!(!branch_matches_task_pattern("main"));
    assert!(!branch_matches_task_pattern("HEAD"));
}

#[test]
fn branch_pattern_rejects_non_digit_ids() {
    assert!(!branch_matches_task_pattern("feature/abc-def"));
    assert!(!branch_matches_task_pattern("feature/1-"));
    assert!(!branch_matches_task_pattern("feature/-2"));
}

#[test]
fn branch_pattern_rejects_missing_dash() {
    assert!(!branch_matches_task_pattern("feature/12345"));
}

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    // Insert program name at position 0 as clap expects.
    let mut all = vec!["active-collab"];
    all.extend_from_slice(args);
    Cli::try_parse_from(all)
}

#[test]
fn parse_setup_add_with_all_flags() {
    let cli = parse(&[
        "setup",
        "add",
        "--name",
        "work",
        "--url",
        "https://example.com",
        "--email",
        "a@b.com",
    ])
    .unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!("expected Setup")
    };
    let SetupCmd::Add(add) = opts.subcommand else {
        panic!("expected Add")
    };
    assert_eq!(add.name.as_deref(), Some("work"));
    assert_eq!(add.url.as_deref(), Some("https://example.com"));
    assert_eq!(add.email.as_deref(), Some("a@b.com"));
}

#[test]
fn parse_setup_add_without_flags_is_ok() {
    let cli = parse(&["setup", "add"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Add(add) = opts.subcommand else {
        panic!()
    };
    assert!(add.name.is_none());
    assert!(add.url.is_none());
    assert!(add.email.is_none());
}

#[test]
fn parse_setup_list() {
    let cli = parse(&["setup", "list"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    assert!(matches!(opts.subcommand, SetupCmd::List));
}

#[test]
fn parse_setup_remove_requires_name() {
    // Without --name: must fail
    let err = parse(&["setup", "remove"]);
    assert!(err.is_err());
    // With --name: must succeed
    let cli = parse(&["setup", "remove", "--name", "work"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Remove(r) = opts.subcommand else {
        panic!()
    };
    assert_eq!(r.name, "work");
}

#[test]
fn parse_setup_test_optional_name() {
    let cli = parse(&["setup", "test"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Test(t) = opts.subcommand else {
        panic!()
    };
    assert!(t.name.is_none());

    let cli2 = parse(&["setup", "test", "--name", "work"]).unwrap();
    let Command::Setup(opts2) = cli2.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Test(t2) = opts2.subcommand else {
        panic!()
    };
    assert_eq!(t2.name.as_deref(), Some("work"));
}

#[test]
fn parse_setup_language_no_arg() {
    let cli = parse(&["setup", "language"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Language(l) = opts.subcommand else {
        panic!()
    };
    assert!(l.code.is_none());
}

#[test]
fn parse_setup_language_with_code() {
    let cli = parse(&["setup", "language", "pt_BR"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Language(l) = opts.subcommand else {
        panic!()
    };
    assert_eq!(l.code.as_deref(), Some("pt_BR"));
}

#[test]
fn parse_setup_theme_no_arg() {
    let cli = parse(&["setup", "theme"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Theme(t) = opts.subcommand else {
        panic!()
    };
    assert!(t.code.is_none());
}

#[test]
fn parse_setup_theme_with_code() {
    let cli = parse(&["setup", "theme", "nord"]).unwrap();
    let Command::Setup(opts) = cli.command.unwrap() else {
        panic!()
    };
    let SetupCmd::Theme(t) = opts.subcommand else {
        panic!()
    };
    assert_eq!(t.code.as_deref(), Some("nord"));
}

#[test]
fn parse_get_with_ref_and_display_flags() {
    let cli = parse(&[
        "get",
        "665/75159",
        "--instance",
        "work",
        "--short",
        "--no-comments",
        "--json",
        "--refresh",
    ])
    .unwrap();
    let Command::Get(g) = cli.command.unwrap() else {
        panic!()
    };
    assert_eq!(g.ref_, "665/75159");
    assert_eq!(g.display.instance.as_deref(), Some("work"));
    assert!(g.display.short);
    assert!(g.display.no_comments);
    assert!(g.display.json);
    assert!(g.display.refresh);
}

#[test]
fn parse_current_with_display_flags() {
    let cli = parse(&["current", "--instance", "work"]).unwrap();
    let Command::Current(d) = cli.command.unwrap() else {
        panic!()
    };
    assert_eq!(d.instance.as_deref(), Some("work"));
}

#[test]
fn parse_mine_no_flags() {
    let cli = parse(&["mine"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Mine(_))));
}

#[test]
fn parse_list_alias_for_mine() {
    let cli = parse(&["list"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Mine(_))));
}

#[test]
fn parse_browse() {
    let cli = parse(&["browse"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Browse(_))));
}

#[test]
fn parse_unknown_subcommand_returns_error() {
    let err = parse(&["unknown-cmd"]);
    assert!(err.is_err());
}

#[test]
fn parse_no_subcommand_yields_none_command() {
    let cli = parse(&[]).unwrap();
    assert!(cli.command.is_none());
}

#[test]
fn parse_missing_required_ref_for_get_returns_error() {
    let err = parse(&["get"]);
    assert!(err.is_err());
}

#[test]
fn bare_no_command_action_tty_yields_run_mine() {
    assert_eq!(bare_no_command_action(true), BareNoCommandAction::RunMine);
}

#[test]
fn bare_no_command_action_non_tty_yields_help_exit2() {
    assert_eq!(
        bare_no_command_action(false),
        BareNoCommandAction::HelpExit2
    );
}

// Verify normalize_argv still takes precedence: a bare ref becomes `get`,
// never reaching the no-command default.
#[test]
fn bare_ref_routes_to_get_before_no_command_default() {
    let result = normalize_argv(&argv(&["665/75159"]), None);
    assert_eq!(result.first().map(String::as_str), Some("get"));
}

// Verify task-branch normalization takes precedence over the no-command default.
#[test]
fn task_branch_routes_to_current_before_no_command_default() {
    let result = normalize_argv(&[], Some("feature/665-75159"));
    assert_eq!(result, argv(&["current"]));
}

// Empty argv with a non-task branch leaves argv empty; the TTY function governs what follows.
#[test]
fn empty_argv_non_task_branch_leaves_argv_empty_for_tty_decision() {
    let result = normalize_argv(&[], Some("main"));
    assert!(result.is_empty());
}

// Explicit subcommands parse successfully regardless of TTY state.
#[test]
fn explicit_mine_subcommand_parses_to_some_command() {
    let cli = parse(&["mine"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Mine(_))));
}

#[test]
fn explicit_get_subcommand_parses_to_some_command() {
    let cli = parse(&["get", "1/2"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Get(_))));
}
