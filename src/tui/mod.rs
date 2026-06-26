pub mod drawer;
pub mod events;
pub mod model;
pub mod screens;
pub mod theme;
pub mod view;

pub use model::{init_browse, mine_model, update, ClickTarget, Cmd, DetailLoad, Model, Msg};
pub use view::view;

use crate::controller;
use crate::http::Http;
use crate::render::MineTableRow;
use crate::store::instances::Instance;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::{map_browse_key_event, map_browse_mouse_event};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_stream::StreamExt as _;

/// Heartbeat period: ~10 FPS redraw safety net for future spinners and animations.
/// Chosen to be low enough to avoid idle CPU burn while still providing timely redraws.
const FRAME_PERIOD: Duration = Duration::from_millis(100);

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

fn setup_terminal() -> io::Result<(TerminalGuard, Terminal<CrosstermBackend<io::Stdout>>)> {
    let guard = TerminalGuard::new()?;
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    Ok((guard, terminal))
}

/// Handle a crossterm input event: map to a Msg, run update, and dispatch commands.
fn handle_input_event(
    ev: Event,
    model: Model,
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    tx: &mpsc::UnboundedSender<Msg>,
) -> Model {
    let msg_opt = match ev {
        Event::Key(key) => map_browse_key_event(key),
        Event::Mouse(mouse) => map_browse_mouse_event(mouse),
        _ => None,
    };
    if let Some(msg) = msg_opt {
        let (new_model, cmds) = update(model, msg);
        dispatch_cmds(cmds, targets, http, db_path, tx.clone());
        return new_model;
    }
    model
}

/// Handle a background Msg from the channel: run update and dispatch resulting commands.
fn handle_channel_msg(
    msg: Msg,
    model: Model,
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    tx: &mpsc::UnboundedSender<Msg>,
) -> Model {
    let (new_model, cmds) = update(model, msg);
    dispatch_cmds(cmds, targets, http, db_path, tx.clone());
    new_model
}

/// Async TEA loop driven by tokio::select! over three sources:
///   - crossterm EventStream  (keyboard / mouse input)
///   - tokio::sync::mpsc receiver  (background network results)
///   - frame heartbeat tick  (redraw safety net)
///
/// A Msg arriving on the channel updates the model and repaints on the next
/// loop iteration without requiring any input event — this is the fix for
/// "primeiro load demora".
async fn run_app(
    targets: Vec<Instance>,
    http: Http,
    db_path: PathBuf,
    mut model: Model,
    init_cmds: Vec<Cmd>,
) -> i32 {
    let (tx, mut rx) = mpsc::unbounded_channel::<Msg>();

    let (mut guard, mut terminal) = match setup_terminal() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error initialising terminal: {e}");
            return 1;
        }
    };

    dispatch_cmds(init_cmds, &targets, &http, &db_path, tx.clone());
    spawn_header_name_resolution(&targets, &http, &db_path, &model.header, &tx);

    let mut events = crossterm::event::EventStream::new();
    let mut heartbeat = tokio::time::interval(FRAME_PERIOD);

    loop {
        // Reflow the Detail render cache to the current terminal width before drawing.
        // The Detail block has a 1-col border each side, so inner = terminal width - 2.
        if let Ok(size) = terminal.size() {
            model.viewport = (size.width, size.height);
            model.reflow_detail(size.width.saturating_sub(2) as usize);
        }

        let mut frame_targets: Vec<ClickTarget> = Vec::new();
        if let Err(e) = terminal.draw(|f| view(&model, f, &mut frame_targets)) {
            guard.restore();
            eprintln!("Error drawing frame: {e}");
            return 1;
        }
        model.set_click_targets(frame_targets);

        if model.should_quit {
            break;
        }

        tokio::select! {
            maybe_ev = events.next() => {
                match maybe_ev {
                    Some(Ok(ev)) => {
                        model = handle_input_event(ev, model, &targets, &http, &db_path, &tx);
                    }
                    Some(Err(e)) => {
                        guard.restore();
                        eprintln!("Error reading event: {e}");
                        return 1;
                    }
                    None => break,
                }
            }
            Some(msg) = rx.recv() => {
                model = handle_channel_msg(msg, model, &targets, &http, &db_path, &tx);
            }
            _ = heartbeat.tick() => {}
        }
    }

    guard.restore();
    0
}

/// Run the browse TUI. Async — awaited directly on the main runtime.
pub async fn browse(targets: Vec<Instance>, http: Http, db_path: PathBuf) -> i32 {
    let header = build_header(&targets, &db_path);
    let (model, init_cmds) = init_browse(header);
    run_app(targets, http, db_path, model, init_cmds).await
}

/// Run the mine TUI with rows that are already fetched.
///
/// No init_cmds are emitted — the model is fully seeded from the provided rows.
/// Enter/click on a task opens the Detail screen through the shared TEA core,
/// fetching task detail over the network or cache exactly like browse.
pub async fn run_mine(
    targets: Vec<Instance>,
    http: Http,
    db_path: PathBuf,
    rows: Vec<MineTableRow>,
) -> i32 {
    let header = build_header(&targets, &db_path);
    let model = mine_model(rows, header);
    run_app(targets, http, db_path, model, vec![]).await
}

/// Resolve the cached display name for the first instance's user_id and
/// build the Header value. Pure data threading — no network call.
fn build_header(targets: &[Instance], db_path: &std::path::Path) -> model::Header {
    let name = targets.first().and_then(|inst| {
        inst.user_id.and_then(|id| {
            controller::cached_user_map(db_path, inst).and_then(|m| m.get(&id).cloned())
        })
    });
    model::Header::from_instances(targets, name)
}

struct DetailRequest {
    instance: String,
    project_id: i64,
    task_id: i64,
    refresh: bool,
}

fn dispatch_cmds(
    cmds: Vec<Cmd>,
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    tx: mpsc::UnboundedSender<Msg>,
) {
    for cmd in cmds {
        match cmd {
            Cmd::LoadTasksByProject => {
                let targets = targets.to_vec();
                let http = http.clone();
                let tx = tx.clone();
                let db_path = db_path.to_path_buf();
                tokio::spawn(async move {
                    let groups = controller::tasks_by_project(db_path, &targets, &http).await;
                    let loaded_at = crate::store::now_brt_iso();
                    let _ = tx.send(Msg::LoadedTasksByProject { groups, loaded_at });
                });
            }
            Cmd::LoadDetail {
                instance,
                project_id,
                task_id,
                refresh,
            } => {
                let req = DetailRequest {
                    instance,
                    project_id,
                    task_id,
                    refresh,
                };
                spawn_load_detail(targets, http, db_path, &tx, req);
            }
            Cmd::OpenAsset { instance, url } => {
                spawn_open_asset(targets, &tx, instance, url);
            }
            Cmd::DownloadAsset {
                instance,
                url,
                name,
            } => {
                spawn_download_asset(targets, http, &tx, instance, url, name);
            }
            Cmd::SetMouseCapture(on) => {
                if on {
                    let _ = execute!(io::stdout(), EnableMouseCapture);
                } else {
                    let _ = execute!(io::stdout(), DisableMouseCapture);
                }
            }
        }
    }
}

fn spawn_header_name_resolution(
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    header: &model::Header,
    tx: &mpsc::UnboundedSender<Msg>,
) {
    if header.name.is_some() {
        return;
    }
    let Some(inst) = targets.first().cloned() else {
        return;
    };
    let Some(user_id) = inst.user_id else {
        return;
    };
    let http = http.clone();
    let db_path = db_path.to_path_buf();
    let tx = tx.clone();
    tokio::spawn(async move {
        let map = controller::refresh_user_map(db_path, inst, http).await;
        if let Some(name) = map.get(&user_id).cloned() {
            let _ = tx.send(Msg::HeaderNameResolved(name));
        }
    });
}

fn spawn_load_detail(
    targets: &[Instance],
    http: &Http,
    db_path: &Path,
    tx: &mpsc::UnboundedSender<Msg>,
    req: DetailRequest,
) {
    let inst = targets.iter().find(|t| t.name == req.instance).cloned();
    let http = http.clone();
    let db_path = db_path.to_path_buf();
    let tx = tx.clone();
    let DetailRequest {
        project_id,
        task_id,
        refresh,
        ..
    } = req;
    tokio::spawn(async move {
        let inst = match inst {
            Some(i) => i,
            None => {
                let loaded_at = crate::store::now_brt_iso();
                let _ = tx.send(Msg::LoadedDetail(DetailLoad {
                    task: serde_json::Value::Null,
                    comments: vec![],
                    assets: vec![],
                    user_map: std::collections::HashMap::new(),
                    loaded_at,
                }));
                return;
            }
        };

        // Phase 1: load task content and send immediately with the cached user_map
        // (or empty if none). The shell will call reflow_detail before drawing.
        let cached_map = controller::cached_user_map(&db_path, &inst).unwrap_or_default();
        let core = controller::load_task_core(
            db_path.clone(),
            inst.clone(),
            http.clone(),
            project_id,
            task_id,
            refresh,
        )
        .await;

        let loaded_at = crate::store::now_brt_iso();
        let _ = tx.send(Msg::LoadedDetail(DetailLoad {
            task: core.task.clone(),
            comments: core.comments.clone(),
            assets: core.assets.clone(),
            user_map: cached_map,
            loaded_at,
        }));

        // Phase 2: refresh user directory in the background; send UserMapResolved
        // when the fresh map is available so the assignee name fills in.
        let needs_user_refresh = controller::cached_user_map(&db_path, &inst).is_none() || refresh;
        if needs_user_refresh {
            let fresh_map = controller::refresh_user_map(db_path, inst.clone(), http).await;
            if !fresh_map.is_empty() {
                let _ = tx.send(Msg::UserMapResolved(fresh_map));
            }
        }
    });
}

fn spawn_open_asset(
    targets: &[Instance],
    tx: &mpsc::UnboundedSender<Msg>,
    instance: String,
    url: String,
) {
    let inst = targets.iter().find(|t| t.name == instance).cloned();
    let tx = tx.clone();
    tokio::spawn(async move {
        spawn_opener(inst, &url);
        let _ = tx.send(Msg::AssetActionResult);
    });
}

fn spawn_download_asset(
    targets: &[Instance],
    http: &Http,
    tx: &mpsc::UnboundedSender<Msg>,
    instance: String,
    url: String,
    name: String,
) {
    let inst = targets.iter().find(|t| t.name == instance).cloned();
    let http = http.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        run_download(inst, http, &url, &name).await;
        let _ = tx.send(Msg::AssetActionResult);
    });
}

fn spawn_opener(inst: Option<Instance>, url: &str) -> String {
    let _inst = match inst {
        Some(i) => i,
        None => return crate::i18n::t("Error: instance not found"),
    };
    if let Err(e) = controller::open_asset(url) {
        return format!("{} {}", crate::i18n::t("Error:"), e);
    }
    let opener = platform_opener();
    match std::process::Command::new(opener).arg(url).spawn() {
        Ok(_) => format!("{} {}", crate::i18n::t("Downloaded:"), url),
        Err(e) => format!("{} {}", crate::i18n::t("Error:"), e),
    }
}

async fn run_download(inst: Option<Instance>, http: Http, url: &str, name: &str) -> String {
    let inst = match inst {
        Some(i) => i,
        None => return crate::i18n::t("Error: instance not found"),
    };
    let dest_dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir);
    let dest_path = dest_dir.join(name);
    match controller::download_asset(&http, &inst, url, &dest_path).await {
        Ok(()) => format!("{} {}", crate::i18n::t("Downloaded:"), dest_path.display()),
        Err(e) => format!("{} {}", crate::i18n::t("Error:"), e),
    }
}

fn platform_opener() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    }
}
