use crate::app::{self, Cmd, FlatModel, FlatMsg, Msg, Task};
use crate::controller;
use crate::http::Http;
use crate::i18n::t;
use crate::render::{render_detail_lines, MineTableRow};
use crate::store::instances::Instance;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

struct TerminalGuard {
    cleaned_up: bool,
}

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(TerminalGuard { cleaned_up: false })
    }

    fn restore(&mut self) {
        if !self.cleaned_up {
            self.cleaned_up = true;
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

fn mine_rows_to_tasks(rows: Vec<MineTableRow>) -> Vec<Task> {
    rows.into_iter()
        .map(|row| Task {
            project: row.project_id.to_string(),
            id: row.task_number as u32,
            name: format!("{}  {}", row.instance, row.name),
        })
        .collect()
}

/// Minimal crossterm+ratatui runner that seeds the flat TEA model from real mine rows.
///
/// Kept for the `mine` command TTY path (R5). The browse command uses `run_browse`.
pub fn run_mine(rows: Vec<MineTableRow>) -> i32 {
    let tasks = mine_rows_to_tasks(rows);
    let mut model = FlatModel::with_tasks(tasks);

    let mut guard = match TerminalGuard::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error initialising terminal: {e}");
            return 1;
        }
    };

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            guard.restore();
            eprintln!("Error creating terminal: {e}");
            return 1;
        }
    };

    loop {
        if let Err(e) = terminal.draw(|f| app::view_flat(&model, f)) {
            guard.restore();
            eprintln!("Error drawing frame: {e}");
            return 1;
        }

        let crossterm_event = match event::read() {
            Ok(ev) => ev,
            Err(e) => {
                guard.restore();
                eprintln!("Error reading event: {e}");
                return 1;
            }
        };

        let msg = match crossterm_event {
            Event::Key(key) => map_flat_key_event(key),
            Event::Mouse(mouse) => map_flat_mouse_event(mouse),
            _ => None,
        };

        if let Some(m) = msg {
            model = app::update_flat(model, m);
        }

        if model.should_quit {
            break;
        }
    }

    guard.restore();
    0
}

/// Crossterm + tokio shell that drives the pure browse update over a screen stack.
///
/// Minimal shell: draws, polls events, interprets Cmds by spawning tokio tasks,
/// feeds Msg::Loaded… back via a channel. Business logic lives in pure update.
/// Terminal is ALWAYS restored via TerminalGuard on every exit path including panic.
pub fn run_browse(targets: Vec<Instance>, http: Http, db_path: PathBuf) -> i32 {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error creating tokio runtime: {e}");
            return 1;
        }
    };

    rt.block_on(run_browse_async(targets, http, db_path))
}

async fn run_browse_async(targets: Vec<Instance>, http: Http, db_path: PathBuf) -> i32 {
    let (tx, rx) = mpsc::channel::<Msg>();

    let (mut model, init_cmds) = app::init_browse();

    let mut guard = match TerminalGuard::new() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error initialising terminal: {e}");
            return 1;
        }
    };

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            guard.restore();
            eprintln!("Error creating terminal: {e}");
            return 1;
        }
    };

    dispatch_cmds(init_cmds, &targets, &http, &db_path, tx.clone());

    loop {
        drain_channel(&rx, &mut model, &targets, &http, &db_path, &tx);

        if model.should_quit {
            break;
        }

        if let Err(e) = terminal.draw(|f| app::view(&model, f)) {
            guard.restore();
            eprintln!("Error drawing frame: {e}");
            return 1;
        }

        let crossterm_event = match event::read() {
            Ok(ev) => ev,
            Err(e) => {
                guard.restore();
                eprintln!("Error reading event: {e}");
                return 1;
            }
        };

        let msg_opt = match crossterm_event {
            Event::Key(key) => map_browse_key_event(key),
            Event::Mouse(mouse) => map_browse_mouse_event(mouse),
            _ => None,
        };

        if let Some(msg) = msg_opt {
            let (new_model, cmds) = app::update(model, msg);
            model = new_model;
            dispatch_cmds(cmds, &targets, &http, &db_path, tx.clone());
        }
    }

    guard.restore();
    0
}

fn drain_channel(
    rx: &mpsc::Receiver<Msg>,
    model: &mut app::Model,
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    tx: &mpsc::Sender<Msg>,
) {
    while let Ok(msg) = rx.try_recv() {
        let (new_model, cmds) = app::update(std::mem::replace(model, app::Model::browse()), msg);
        *model = new_model;
        dispatch_cmds(cmds, targets, http, db_path, tx.clone());
    }
}

fn dispatch_cmds(
    cmds: Vec<Cmd>,
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    tx: mpsc::Sender<Msg>,
) {
    for cmd in cmds {
        match cmd {
            Cmd::LoadTasksByProject => {
                let targets = targets.to_vec();
                let http = http.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let groups = controller::tasks_by_project(&targets, &http).await;
                    let _ = tx.send(Msg::LoadedTasksByProject(groups));
                });
            }
            Cmd::LoadDetail {
                instance,
                project_id,
                task_id,
                refresh,
            } => {
                let inst = targets.iter().find(|t| t.name == instance).cloned();
                let http = http.clone();
                let db_path = db_path.to_path_buf();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let (lines, assets) =
                        load_and_render_detail(inst, http, db_path, project_id, task_id, refresh)
                            .await;
                    let _ = tx.send(Msg::LoadedDetail(lines, assets));
                });
            }
            Cmd::OpenAsset { instance, url } => {
                let inst = targets.iter().find(|t| t.name == instance).cloned();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let result = spawn_opener(inst, &url);
                    let _ = tx.send(Msg::AssetActionResult(result));
                });
            }
            Cmd::DownloadAsset {
                instance,
                url,
                name,
            } => {
                let inst = targets.iter().find(|t| t.name == instance).cloned();
                let http = http.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let result = run_download(inst, http, &url, &name).await;
                    let _ = tx.send(Msg::AssetActionResult(result));
                });
            }
        }
    }
}

fn spawn_opener(inst: Option<Instance>, url: &str) -> String {
    let _inst = match inst {
        Some(i) => i,
        None => return t("Error: instance not found"),
    };
    if let Err(e) = controller::open_asset(url) {
        return format!("{} {}", t("Error:"), e);
    }
    let opener = platform_opener();
    match std::process::Command::new(opener).arg(url).spawn() {
        Ok(_) => format!("{} {}", t("Downloaded:"), url),
        Err(e) => format!("{} {}", t("Error:"), e),
    }
}

async fn run_download(inst: Option<Instance>, http: Http, url: &str, name: &str) -> String {
    let inst = match inst {
        Some(i) => i,
        None => return t("Error: instance not found"),
    };
    let dest_dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    let dest_path = dest_dir.join(name);
    match controller::download_asset(&http, &inst, url, &dest_path).await {
        Ok(()) => format!("{} {}", t("Downloaded:"), dest_path.display()),
        Err(e) => format!("{} {}", t("Error:"), e),
    }
}

fn platform_opener() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}

async fn load_and_render_detail(
    inst: Option<Instance>,
    http: Http,
    db_path: PathBuf,
    project_id: i64,
    task_id: i64,
    refresh: bool,
) -> (Vec<String>, Vec<crate::render::Asset>) {
    let inst = match inst {
        Some(i) => i,
        None => return (vec![], vec![]),
    };

    let detail = controller::task_detail(db_path, inst, http, project_id, task_id, refresh).await;
    let lines = render_detail_lines(
        &detail.task,
        &detail.comments,
        &detail.assets,
        &detail.user_map,
    );
    (lines, detail.assets)
}

fn map_browse_key_event(key: crossterm::event::KeyEvent) -> Option<Msg> {
    match key.code {
        KeyCode::Char('q') => Some(Msg::Quit),
        KeyCode::Esc | KeyCode::Char('b') => Some(Msg::Back),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Quit),
        KeyCode::Char('r') => Some(Msg::Refresh),
        KeyCode::Up => Some(Msg::Up),
        KeyCode::Down => Some(Msg::Down),
        KeyCode::PageUp => Some(Msg::PageUp),
        KeyCode::PageDown => Some(Msg::PageDown),
        KeyCode::Enter => Some(Msg::Select),
        KeyCode::Char('d') => Some(Msg::TogglePendingDownload),
        KeyCode::Char(c) if c.is_ascii_digit() => Some(Msg::AssetOpen(c)),
        _ => None,
    }
}

fn map_browse_mouse_event(mouse: crossterm::event::MouseEvent) -> Option<Msg> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(Msg::ScrollUp),
        MouseEventKind::ScrollDown => Some(Msg::ScrollDown),
        MouseEventKind::Down(_) => Some(Msg::Click(mouse.row as usize)),
        _ => None,
    }
}

fn map_flat_key_event(key: crossterm::event::KeyEvent) -> Option<FlatMsg> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(FlatMsg::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FlatMsg::Quit),
        KeyCode::Up => Some(FlatMsg::Up),
        KeyCode::Down => Some(FlatMsg::Down),
        _ => None,
    }
}

fn map_flat_mouse_event(mouse: crossterm::event::MouseEvent) -> Option<FlatMsg> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(FlatMsg::ScrollUp),
        MouseEventKind::ScrollDown => Some(FlatMsg::ScrollDown),
        MouseEventKind::Down(_) => Some(FlatMsg::Click(mouse.row as usize)),
        _ => None,
    }
}
