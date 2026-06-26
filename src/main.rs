mod cli;
mod client;
mod commands;
mod config;
mod controller;
mod http;
mod i18n;
mod models;
mod render;
mod richtext;
mod store;
mod timing;
mod tui;

use clap::{CommandFactory, Parser};
use cli::{bare_no_command_action, BareNoCommandAction, Cli, Command};
use commands::{
    current_core, get_core, mine_core, pick_instance, setup_add, setup_language, setup_list,
    setup_remove, setup_test, DisplayFlags, SetupAddFields,
};
use std::io::IsTerminal;
use std::process;

#[tokio::main]
async fn main() {
    let code = run(std::env::args().skip(1).collect()).await;
    process::exit(code);
}

async fn run(raw_argv: Vec<String>) -> i32 {
    let branch = current_git_branch();
    let argv = cli::normalize_argv(&raw_argv, branch.as_deref());

    let cli_result = Cli::try_parse_from(std::iter::once("active-collab".to_owned()).chain(argv));

    let cli = match cli_result {
        Ok(c) => c,
        Err(e) => {
            // clap writes the error to stderr itself; we honour its exit code.
            e.print().ok();
            // clap usage errors exit 2, other errors exit 1 — preserve the kind.
            return if e.exit_code() == 0 { 0 } else { e.exit_code() };
        }
    };

    let Some(command) = cli.command else {
        let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
        return match bare_no_command_action(is_tty) {
            BareNoCommandAction::RunMine => {
                init_language();
                dispatch(Command::Mine(cli::MineArgs { instance: None })).await
            }
            BareNoCommandAction::HelpExit2 => {
                let mut help_cli = Cli::command();
                help_cli.print_help().ok();
                eprintln!();
                2
            }
        };
    };

    init_language();

    dispatch(command).await
}

/// Read ACTIVE_COLLAB_LANG env + DB `language` setting, then call `i18n::set_language`.
/// Any error reading the DB is silently ignored (falls back to env or "en").
fn init_language() {
    let env_value = std::env::var("ACTIVE_COLLAB_LANG").ok();

    let db_value: Option<String> = (|| -> Option<String> {
        let config = config::load();
        let store = store::Store::open(&config).ok()?;
        store::settings::SettingsRepository::new(store.conn())
            .get("language", None)
            .ok()
            .flatten()
    })();

    let lang = i18n::resolve_language(env_value.as_deref(), db_value.as_deref());
    i18n::set_language(&lang);
}

async fn dispatch(command: Command) -> i32 {
    match command {
        Command::Setup(opts) => dispatch_setup(opts.subcommand).await,
        Command::Get(args) => dispatch_get(args).await,
        Command::Current(args) => dispatch_current(args).await,
        Command::Mine(args) => dispatch_mine(args).await,
        Command::Browse(args) => dispatch_browse(args).await,
    }
}

async fn dispatch_setup(cmd: cli::SetupCmd) -> i32 {
    match cmd {
        cli::SetupCmd::Add(args) => dispatch_setup_add(args).await,
        cli::SetupCmd::List => dispatch_setup_list(),
        cli::SetupCmd::Remove(args) => dispatch_setup_remove(args),
        cli::SetupCmd::Test(args) => dispatch_setup_test(args).await,
        cli::SetupCmd::Language(args) => dispatch_setup_language(args),
    }
}

fn open_store() -> Option<store::Store> {
    let config = config::load();
    match store::Store::open(&config) {
        Ok(s) => Some(s),
        Err(e) => {
            render::print_error(&format!("Error opening database: {e}"));
            None
        }
    }
}

fn dispatch_setup_list() -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    setup_list(&repo, &mut std::io::stdout())
}

fn dispatch_setup_remove(args: cli::RemoveArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let cache = store::cache::TaskCache::new(store.conn());
    setup_remove(
        &repo,
        &cache,
        &args.name,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

fn dispatch_setup_language(args: cli::LanguageArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let settings = store::settings::SettingsRepository::new(store.conn());
    setup_language(
        &settings,
        args.code.as_deref(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

async fn dispatch_setup_test(args: cli::TestArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let http = match http::Http::new() {
        Ok(h) => h,
        Err(e) => {
            render::print_error(&format!("Error building HTTP client: {e}"));
            return 1;
        }
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    setup_test(
        &repo,
        args.name.as_deref(),
        http,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_setup_add(args: cli::SetupAddArgs) -> i32 {
    let interactive = stdin_is_tty();

    let name = resolve_field(args.name, "Instance name", interactive);
    let url = resolve_field(args.url, "Base URL (https://...)", interactive);
    let email = resolve_field(args.email, "Email", interactive);

    let password = if interactive {
        rpassword::prompt_password("Password (input hidden): ").ok()
    } else {
        // Non-interactive: read from stdin line (matches Python getpass fallback)
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).ok();
        let trimmed = line
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_owned();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    };

    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let http = match http::Http::new() {
        Ok(h) => h,
        Err(e) => {
            render::print_error(&format!("Error building HTTP client: {e}"));
            return 1;
        }
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    setup_add(
        SetupAddFields { name, url, email },
        password,
        &repo,
        http,
        interactive,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

/// Resolve a field value: use the flag if provided, else prompt when interactive.
/// Mirrors Python `_resolve_field`.
fn resolve_field(value: Option<String>, label: &str, interactive: bool) -> Option<String> {
    if let Some(v) = value {
        if !v.is_empty() {
            return Some(v);
        }
    }
    if !interactive {
        return None;
    }
    loop {
        print!("{label}: ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {}
        }
        let val = input.trim().to_owned();
        if !val.is_empty() {
            return Some(val);
        }
        eprintln!("{label} cannot be empty.");
    }
}

/// True when stdin is connected to a terminal.
fn stdin_is_tty() -> bool {
    std::io::stdin().is_terminal()
}

async fn dispatch_get(args: cli::GetArgs) -> i32 {
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let http = match http::Http::new() {
        Ok(h) => h,
        Err(e) => {
            render::print_error(&format!("Error building HTTP client: {e}"));
            return 1;
        }
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            render::print_error(&format!("Error loading instances: {e}"));
            return 1;
        }
    };
    let mut err_buf = std::io::stderr();
    let idx = match pick_instance(&instances, args.display.instance.as_deref(), &mut err_buf) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let inst = instances[idx].clone();
    let ac_client = client::ActiveCollabClient::new(inst.clone(), http);
    let cache = store::cache::TaskCache::new(store.conn());
    let flags = DisplayFlags {
        json: args.display.json,
        short: args.display.short,
        refresh: args.display.refresh,
        no_comments: args.display.no_comments,
    };
    get_core(
        &args.ref_,
        &inst,
        &cache,
        &ac_client,
        &flags,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_current(args: cli::DisplayArgs) -> i32 {
    let branch = current_git_branch();
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let http = match http::Http::new() {
        Ok(h) => h,
        Err(e) => {
            render::print_error(&format!("Error building HTTP client: {e}"));
            return 1;
        }
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            render::print_error(&format!("Error loading instances: {e}"));
            return 1;
        }
    };
    let mut err_buf = std::io::stderr();
    let idx = match pick_instance(&instances, args.instance.as_deref(), &mut err_buf) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let inst = instances[idx].clone();
    let ac_client = client::ActiveCollabClient::new(inst.clone(), http);
    let cache = store::cache::TaskCache::new(store.conn());
    let flags = DisplayFlags {
        json: args.json,
        short: args.short,
        refresh: args.refresh,
        no_comments: args.no_comments,
    };
    current_core(
        branch.as_deref(),
        &inst,
        &cache,
        &ac_client,
        &flags,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

async fn dispatch_mine(args: cli::MineArgs) -> i32 {
    let config = config::load();
    let db_path = config.db_path.clone();
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let http = match http::Http::new() {
        Ok(h) => h,
        Err(e) => {
            render::print_error(&format!("Error building HTTP client: {e}"));
            return 1;
        }
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
    type TuiCapture = std::sync::Arc<std::sync::Mutex<Option<MineTuiArgs>>>;
    struct MineTuiArgs {
        targets: Vec<store::instances::Instance>,
        rows: Vec<crate::render::MineTableRow>,
    }

    let captured: TuiCapture = std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured_for_closure = captured.clone();

    let exit_code = mine_core(
        &repo,
        &http.clone(),
        args.instance.as_deref(),
        is_tty,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
        move |targets, rows| {
            if let Ok(mut guard) = captured_for_closure.lock() {
                *guard = Some(MineTuiArgs { targets, rows });
            }
            0
        },
    )
    .await;

    let tui_launch = captured.lock().ok().and_then(|mut g| g.take());
    if let Some(args) = tui_launch {
        return tui::run_mine(args.targets, http, db_path, args.rows).await;
    }
    exit_code
}

async fn dispatch_browse(args: cli::BrowseArgs) -> i32 {
    let config = config::load();
    let db_path = config.db_path.clone();
    let store = match open_store() {
        Some(s) => s,
        None => return 1,
    };
    let http = match http::Http::new() {
        Ok(h) => h,
        Err(e) => {
            render::print_error(&format!("Error building HTTP client: {e}"));
            return 1;
        }
    };
    let repo = store::instances::InstanceRepository::new(store.conn());
    let instances = match repo.load_all() {
        Ok(v) => v,
        Err(e) => {
            render::print_error(&format!("Error loading instances: {e}"));
            return 1;
        }
    };
    if instances.is_empty() {
        render::print_error(&crate::i18n::t(
            "Error: no instances configured. Run: active_collab.py setup add",
        ));
        return 2;
    }
    let targets: Vec<store::instances::Instance> = if let Some(name) = args.instance.as_deref() {
        let matches: Vec<_> = instances
            .iter()
            .filter(|i| i.name == name)
            .cloned()
            .collect();
        if matches.is_empty() {
            let known: Vec<&str> = instances.iter().map(|i| i.name.as_str()).collect();
            render::print_error(&crate::i18n::t(&format!(
                "Error: instance '{name}' not found. Known: {known}",
                name = name,
                known = known.join(", ")
            )));
            return 2;
        }
        matches
    } else {
        instances
    };
    tui::browse(targets, http, db_path).await
}

/// Invoke `git rev-parse --abbrev-ref HEAD` and return the branch name.
/// Returns `None` when not in a git repo, on timeout, or when HEAD is detached.
fn current_git_branch() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}
