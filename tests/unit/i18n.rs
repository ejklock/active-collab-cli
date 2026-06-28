use super::*;

#[test]
fn supported_contains_en_and_pt_br() {
    assert!(SUPPORTED.contains(&"en"));
    assert!(SUPPORTED.contains(&"pt_BR"));
    assert_eq!(SUPPORTED.len(), 2);
}

#[test]
fn resolve_env_wins_over_db() {
    let result = resolve_language(Some("pt_BR"), Some("en"));
    assert_eq!(result, "pt_BR");
}

#[test]
fn resolve_db_fallback_when_env_empty() {
    let result = resolve_language(Some(""), Some("pt_BR"));
    assert_eq!(result, "pt_BR");
}

#[test]
fn resolve_db_fallback_when_env_none() {
    let result = resolve_language(None, Some("pt_BR"));
    assert_eq!(result, "pt_BR");
}

#[test]
fn resolve_pt_br_utf8_normalizes_to_pt_br() {
    let result = resolve_language(Some("pt_BR.UTF-8"), None);
    assert_eq!(result, "pt_BR");
}

#[test]
fn resolve_unsupported_locale_falls_through_to_en() {
    let result = resolve_language(Some("zz"), Some("zz"));
    assert_eq!(result, "en");
}

#[test]
fn resolve_both_empty_returns_en() {
    let result = resolve_language(Some(""), Some(""));
    assert_eq!(result, "en");
}

#[test]
fn resolve_both_none_returns_en() {
    let result = resolve_language(None, None);
    assert_eq!(result, "en");
}

#[test]
fn resolve_env_en_falls_through_to_db_pt_br() {
    // Python _normalize_locale("en") returns "" (unrecognized), so a literal
    // "en" env value must fall through and let the db value win.
    let result = resolve_language(Some("en"), Some("pt_BR"));
    assert_eq!(result, "pt_BR");
}

#[test]
fn resolve_env_en_with_no_db_returns_en_terminal_fallback() {
    // "en" is unrecognized by normalize_locale; with no db value the
    // terminal fallback at the end of resolve_language returns "en".
    let result = resolve_language(Some("en"), None);
    assert_eq!(result, "en");
}

#[test]
fn set_and_current_language_round_trip() {
    set_language("pt_BR");
    assert_eq!(current_language(), "pt_BR");
    set_language("en");
    assert_eq!(current_language(), "en");
}

#[test]
fn t_returns_input_unchanged_under_en() {
    set_language("en");
    assert_eq!(t("Hello"), "Hello");
    assert_eq!(
        t("Error: no instances configured."),
        "Error: no instances configured."
    );
    assert_eq!(t(""), "");
    set_language("en");
}

#[test]
fn t_translates_known_keys_under_pt_br() {
    set_language("pt_BR");
    assert_eq!(t("Task"), "Tarefa");
    assert_eq!(t("Title"), "Título");
    assert_eq!(t("Assignee"), "Responsável");
    assert_eq!(t("Open"), "Aberto");
    assert_eq!(t("Completed"), "Concluído");
    assert_eq!(t("Comments"), "Comentários");
    assert_eq!(t("Description"), "Descrição");
    assert_eq!(t("(no description)"), "(sem descrição)");
    assert_eq!(t("(unassigned)"), "(não atribuído)");
    assert_eq!(t("(unknown)"), "(desconhecido)");
    assert_eq!(t("Projects"), "Projetos");
    assert_eq!(t("Tasks"), "Tarefas");
    assert_eq!(t("Settings"), "Configurações");
    assert_eq!(t("Error:"), "Erro:");
    assert_eq!(
        t("No open tasks assigned to you."),
        "Nenhuma tarefa aberta atribuída a você."
    );
    set_language("en");
}

#[test]
fn t_returns_identity_for_unknown_key_under_pt_br() {
    set_language("pt_BR");
    assert_eq!(t("Some unlisted key"), "Some unlisted key");
    assert_eq!(t(""), "");
    set_language("en");
}

#[test]
fn pt_br_json_parses_and_is_nonempty() {
    let catalog = pt_br_catalog();
    assert!(
        !catalog.is_empty(),
        "pt_BR.json must parse to a non-empty map"
    );
}

#[test]
fn pt_br_catalog_completeness_all_oracle_keys_present() {
    // Keys transcribed directly from src/active_collab/i18n.py _CATALOG['pt_BR'].
    // This list is the source of truth — NOT derived from the Rust catalog —
    // so the test fails if any key is dropped from pt_BR.json.
    let oracle_keys: &[&str] = &[
        "Task",
        "Title",
        "Status",
        "Assignee",
        "Start",
        "Due",
        "Estimate",
        "Logged",
        "Description",
        "(no description)",
        "(unassigned)",
        "(unknown)",
        "Comments",
        "Completed",
        "Open",
        "INSTANCE",
        "PROJECT",
        "TASK#",
        "TASK_ID",
        "NAME",
        "No instances configured. Run: active_collab.py setup add",
        "Error: no instances configured. Run: active_collab.py setup add",
        "Error: no instances configured. Run: active-collab setup add",
        "Error: instance '{name}' not found. Known: {known}",
        "Error: multiple instances configured ({names}). Use --instance NAME.",
        "Error: multiple instances ({names}). Use --instance NAME.",
        "Error: cannot parse task ref '{ref}'. Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159).",
        "Error: task {p}/{t} not found (HTTP {status}).",
        "Error: --name, --url and --email are required.",
        "Error: password is required.",
        "Error: {detail}",
        "Error: instance '{name}' not found.",
        "Error: not in a git repository or HEAD is detached.",
        "Error: branch '{branch}' does not match expected pattern (feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159).",
        "No open tasks assigned to you.",
        "Connectivity: OK",
        "Connectivity: FAILED (HTTP {status})",
        "Connectivity: FAILED ({exc})",
        "Instance '{name}' saved.",
        "Instance '{name}' removed.",
        "OK ({status})",
        "FAILED (HTTP {status})",
        "FAILED ({exc})",
        "Projects",
        "Tasks",
        "My Open Tasks",
        "Assets",
        "Branch type (Enter to confirm, q cancel)",
        "Terminal too small",
        "Resize to continue",
        "Press any key...",
        "Press any key to exit...",
        "Downloaded:",
        "Error:",
        "move",
        "select",
        "quit",
        "back",
        "create branch",
        "assets",
        "scroll",
        "page",
        "open",
        "download",
        "refresh",
        "Details",
        "Artifacts",
        "Project",
        "Error: 'browse' requires an interactive terminal (TTY).",
        "Error: unsupported language '{code}'. Supported: {supported}.",
        "Language set to '{code}'.",
        "Current language: {code}",
        "Settings",
        "Fetch ActiveCollab tasks from one or more configured instances.",
        "Manage instance configuration.",
        "Register an ActiveCollab instance.",
        "Unique name (prompted if omitted, interactive).",
        "Base URL, e.g. https://collab.example.com.",
        "Email for token exchange.",
        "List configured instances (no tokens).",
        "Remove an instance.",
        "Test connectivity.",
        "Test only this instance.",
        "Show or set the display language.",
        "Language code to set ({supported}). Omit to show current.",
        "Fetch and display a task.",
        "Fetch the task from the current git branch.",
        "List open tasks assigned to you.",
        "Limit to this instance.",
        "Interactive TUI browser for your tasks.",
        "Force a named instance.",
        "Print PROJECT/TASK<TAB>name only.",
        "Print raw task JSON.",
        "Ignore cache and re-fetch.",
        "Task URL or PROJECT_ID/TASK_ID (e.g. 665/75159).",
        "Language",
        "Active instance",
        "English",
        "Portuguese (Brazil)",
        "settings",
    ];

    let catalog = pt_br_catalog();
    let missing: Vec<&&str> = oracle_keys
        .iter()
        .filter(|k| !catalog.contains_key(**k))
        .collect();

    assert!(
        missing.is_empty(),
        "pt_BR catalog missing keys from Python oracle: {missing:?}"
    );
}
