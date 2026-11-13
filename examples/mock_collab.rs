//! A throwaway ActiveCollab stand-in for terminal replays of `ac`.
//!
//! Usage: `mock_collab <store-path> <ac-binary>`. The store path is resolved
//! once to an absolute path that is checked, handed to `ac` and opened alike.
//!
//! It creates a fresh store at `<store-path>` through `<ac-binary>`, registers
//! one instance pointing at its own loopback address, prints one ready line
//! holding that address, serves the seeded routes and exits when stdin closes.
//! Each request is logged as one line in `requests.log` beside the store,
//! saying whether the instance token came with it; a request no route serves is
//! marked `unmatched` and answered 404.
//! It refuses to run against an existing store, against the owner's default
//! store, or without an executable `ac`, so a replay can never touch the real
//! instance.

use anyhow::{bail, Context, Result};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const INSTANCE_NAME: &str = "mock";
const USER_ID: i64 = 42;
const USER_NAME: &str = "Ada Lovelace";
/// 2024-03-15 09:30 UTC, fixed so replayed output never depends on the clock.
const COMMENT_CREATED_ON: i64 = 1_710_495_000;
const REQUEST_LOG: &str = "requests.log";
const TOKEN_HEADER: &str = "x-angie-authapitoken";
const ATTACHMENT_NAME: &str = "report.txt";
const ATTACHMENT_BYTES: &[u8] = b"Quarterly report attachment served by the mock ActiveCollab.\n";
const FOREIGN_ATTACHMENT_NAME: &str = "foreign.txt";
const FOREIGN_ATTACHMENT_BYTES: &[u8] = b"Attachment served from a host other than the instance.\n";
const TASK_LISTS: [(i64, &str); 3] = [(4, "To do"), (5, "In progress"), (6, "Review")];
const FIRST_TASK_LIST: i64 = 4;

/// One time record, dated relative to the Monday of the current week so the
/// seed holds on any day of the year.
#[derive(Clone)]
struct SeedRecord {
    id: i64,
    days_from_week_start: i64,
    hours: f64,
}

#[derive(Clone)]
struct SeedTask {
    id: i64,
    project_id: i64,
    number: i64,
    name: &'static str,
    estimate: Option<f64>,
    records: Vec<SeedRecord>,
    attachment: Option<AttachmentHost>,
}

/// Where a task's comment attachment is served from.
#[derive(Clone, Copy)]
enum AttachmentHost {
    /// The instance's own address.
    Instance,
    /// The same loopback server under another host name, so the instance
    /// origin check treats it as a foreign host.
    Foreign,
}

impl SeedTask {
    fn total_hours(&self) -> f64 {
        self.records.iter().map(|r| r.hours).sum()
    }

    fn list_entry(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "project_id": self.project_id,
            "task_number": self.number,
            "estimate": self.estimate.unwrap_or(0.0),
            "due_on": null,
            "is_completed": false,
            "is_trashed": false,
        })
    }
}

/// The data every route serves; a flow adds its routes by extending this
/// fixture and `mount_routes`.
#[derive(Clone)]
struct Fixture {
    tasks: Vec<SeedTask>,
}

impl Fixture {
    fn seeded() -> Fixture {
        let record = |id, days_from_week_start, hours| SeedRecord {
            id,
            days_from_week_start,
            hours,
        };
        Fixture {
            tasks: vec![
                SeedTask {
                    id: 101,
                    project_id: 10,
                    number: 7,
                    name: "Draft the quarterly report",
                    estimate: Some(8.0),
                    records: vec![record(1, 0, 2.5), record(2, -1, 1.5), record(3, -9, 2.0)],
                    attachment: Some(AttachmentHost::Instance),
                },
                SeedTask {
                    id: 102,
                    project_id: 10,
                    number: 8,
                    name: "Review the vendor contract",
                    estimate: Some(4.0),
                    records: vec![record(4, -10, 3.0)],
                    attachment: Some(AttachmentHost::Foreign),
                },
                SeedTask {
                    id: 103,
                    project_id: 11,
                    number: 3,
                    name: "Plan the team offsite",
                    estimate: None,
                    records: Vec::new(),
                    attachment: None,
                },
            ],
        }
    }
}

fn week_start(today: NaiveDate) -> NaiveDate {
    today - Duration::days(i64::from(today.weekday().num_days_from_monday()))
}

struct OpenTasks(Fixture);

impl Respond for OpenTasks {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let tasks: Vec<Value> = self.0.tasks.iter().map(SeedTask::list_entry).collect();
        ResponseTemplate::new(200).set_body_json(json!({ "tasks": tasks }))
    }
}

/// Which task list each task sits in; a move rewrites it, so the next read of
/// the task shows the new list.
type Placement = Arc<Mutex<HashMap<i64, i64>>>;

struct TaskDetail {
    task: SeedTask,
    base_url: String,
    placement: Placement,
}

impl TaskDetail {
    fn comments(&self) -> Vec<Value> {
        let Some(host) = self.task.attachment else {
            return Vec::new();
        };
        let (base, name) = match host {
            AttachmentHost::Instance => (self.base_url.clone(), ATTACHMENT_NAME),
            AttachmentHost::Foreign => (
                self.base_url.replacen("127.0.0.1", "localhost", 1),
                FOREIGN_ATTACHMENT_NAME,
            ),
        };
        vec![json!({
            "id": self.task.id * 10,
            "body": "<p>The file is attached.</p>",
            "created_by_id": USER_ID,
            "created_by_name": USER_NAME,
            "created_on": COMMENT_CREATED_ON,
            "attachments": [{ "name": name, "url": format!("{base}/files/{name}") }],
        })]
    }
}

impl Respond for TaskDetail {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let task = &self.task;
        let list_id = self.placement.lock().unwrap().get(&task.id).copied();
        ResponseTemplate::new(200).set_body_json(json!({
            "single": {
                "id": task.id,
                "name": task.name,
                "project_id": task.project_id,
                "task_number": task.number,
                "task_list_id": list_id,
                "assignee_id": USER_ID,
            },
            "tracked_time": task.total_hours(),
            "comments": self.comments(),
        }))
    }
}

struct TaskLists;

impl Respond for TaskLists {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let lists: Vec<Value> = TASK_LISTS
            .iter()
            .map(|(id, name)| json!({ "id": id, "name": name }))
            .collect();
        ResponseTemplate::new(200).set_body_json(lists)
    }
}

struct MoveTask {
    task_id: i64,
    placement: Placement,
}

impl Respond for MoveTask {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
            return ResponseTemplate::new(400);
        };
        let mut placement = self.placement.lock().unwrap();
        if let Some(list_id) = body.get("task_list_id").and_then(Value::as_i64) {
            placement.insert(self.task_id, list_id);
        }
        ResponseTemplate::new(200).set_body_json(json!({
            "single": { "id": self.task_id, "task_list_id": placement.get(&self.task_id) }
        }))
    }
}

struct NotFound;

impl Respond for NotFound {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        ResponseTemplate::new(404)
    }
}

struct UserDirectory;

impl Respond for UserDirectory {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!([{
            "id": USER_ID,
            "first_name": "Ada",
            "last_name": "Lovelace",
            "email": "ada@example.com",
        }]))
    }
}

struct AttachmentFile(&'static [u8]);

impl Respond for AttachmentFile {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_bytes(self.0)
    }
}

struct Logged<R> {
    inner: R,
    log: Arc<Mutex<std::fs::File>>,
    unmatched: bool,
}

impl<R: Respond> Respond for Logged<R> {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let token = if request.headers.contains_key(TOKEN_HEADER) {
            "sent"
        } else {
            "withheld"
        };
        let unmatched = if self.unmatched { " unmatched" } else { "" };
        let line = format!(
            "{} {} instance-token {token}{unmatched}\n",
            request.method,
            request.url.path()
        );
        let _ = self.log.lock().unwrap().write_all(line.as_bytes());
        self.inner.respond(request)
    }
}

/// Serves the records dated inside the requested `from..=to` window, as the
/// real API does.
struct TimeRecordsByDate(Fixture);

impl TimeRecordsByDate {
    fn window(request: &Request) -> Option<(NaiveDate, NaiveDate)> {
        let date_of = |key: &str| {
            request
                .url
                .query_pairs()
                .find(|(name, _)| name == key)
                .and_then(|(_, value)| NaiveDate::parse_from_str(&value, "%Y-%m-%d").ok())
        };
        Some((date_of("from")?, date_of("to")?))
    }
}

impl Respond for TimeRecordsByDate {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let Some((from, to)) = Self::window(request) else {
            return ResponseTemplate::new(400);
        };
        let monday = week_start(Utc::now().date_naive());
        let records: Vec<Value> = self
            .0
            .tasks
            .iter()
            .flat_map(|task| task.records.iter().map(move |record| (task, record)))
            .filter_map(|(task, record)| {
                let date = monday + Duration::days(record.days_from_week_start);
                (from <= date && date <= to).then(|| {
                    json!({
                        "id": record.id,
                        "value": record.hours,
                        "record_date": date.and_time(NaiveTime::MIN).and_utc().timestamp(),
                        "summary": "",
                        "parent_type": "Task",
                        "parent_id": task.id,
                    })
                })
            })
            .collect();
        ResponseTemplate::new(200).set_body_json(json!({ "time_records": records }))
    }
}

async fn mount<R: Respond + 'static>(
    server: &MockServer,
    verb: &str,
    route: String,
    inner: R,
    log: &Arc<Mutex<std::fs::File>>,
) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(Logged {
            inner,
            log: Arc::clone(log),
            unmatched: false,
        })
        .mount(server)
        .await;
}

async fn mount_routes(server: &MockServer, fixture: &Fixture, log: &Arc<Mutex<std::fs::File>>) {
    let placement: Placement = Arc::new(Mutex::new(
        fixture
            .tasks
            .iter()
            .map(|task| (task.id, FIRST_TASK_LIST))
            .collect(),
    ));
    mount(
        server,
        "GET",
        "/api/v1/users".to_string(),
        UserDirectory,
        log,
    )
    .await;
    Mock::given(any())
        .respond_with(Logged {
            inner: NotFound,
            log: Arc::clone(log),
            unmatched: true,
        })
        .with_priority(u8::MAX)
        .mount(server)
        .await;
    mount(
        server,
        "GET",
        format!("/api/v1/users/{USER_ID}/tasks"),
        OpenTasks(fixture.clone()),
        log,
    )
    .await;
    mount(
        server,
        "GET",
        format!("/api/v1/users/{USER_ID}/time-records/filtered-by-date"),
        TimeRecordsByDate(fixture.clone()),
        log,
    )
    .await;
    for task in &fixture.tasks {
        let task_route = format!("/api/v1/projects/{}/tasks/{}", task.project_id, task.id);
        let detail = TaskDetail {
            task: task.clone(),
            base_url: server.uri(),
            placement: Arc::clone(&placement),
        };
        mount(server, "GET", task_route.clone(), detail, log).await;
        let mover = MoveTask {
            task_id: task.id,
            placement: Arc::clone(&placement),
        };
        mount(server, "PUT", task_route, mover, log).await;
    }
    let mut projects: Vec<i64> = fixture.tasks.iter().map(|task| task.project_id).collect();
    projects.sort_unstable();
    projects.dedup();
    for project_id in projects {
        let route = format!("/api/v1/projects/{project_id}/task-lists");
        mount(server, "GET", route, TaskLists, log).await;
    }
    for (name, bytes) in [
        (ATTACHMENT_NAME, ATTACHMENT_BYTES),
        (FOREIGN_ATTACHMENT_NAME, FOREIGN_ATTACHMENT_BYTES),
    ] {
        mount(
            server,
            "GET",
            format!("/files/{name}"),
            AttachmentFile(bytes),
            log,
        )
        .await;
    }
}

/// The store path made absolute with its parent directory resolved, so a
/// symlinked folder or a relative spelling compares equal to the real place.
fn resolved(store: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(store).context("cannot make the store path absolute")?;
    let name = absolute
        .file_name()
        .context("the store path has no file name")?
        .to_owned();
    let parent = absolute
        .parent()
        .context("the store path has no parent folder")?;
    let parent = parent
        .canonicalize()
        .with_context(|| format!("the store folder {} does not exist", parent.display()))?;
    Ok(parent.join(name))
}

/// Judges the resolved store path, the one `ac` and rusqlite open.
fn is_owner_store(store: &Path) -> Result<bool> {
    let Some(home) = dirs::home_dir() else {
        return Ok(false);
    };
    let owner_dir = home.join(".config").join("active-collab");
    let owner_dir = owner_dir.canonicalize().unwrap_or(owner_dir);
    let owner_store = owner_dir.join("active-collab.db");
    Ok(store.parent() == owner_store.parent()
        && match (store.file_name(), owner_store.file_name()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        })
}

fn check_arguments(store: &Path, ac: &Path) -> Result<()> {
    if store.symlink_metadata().is_ok() {
        bail!("refusing to reuse the existing store {}", store.display());
    }
    if is_owner_store(store)? {
        bail!(
            "refusing to use the owner's default store {}",
            store.display()
        );
    }
    let meta = ac
        .metadata()
        .with_context(|| format!("the ac binary {} is missing", ac.display()))?;
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        bail!("the ac binary {} is not executable", ac.display());
    }
    Ok(())
}

/// Creates the store by running `ac` against it with a closed environment, so
/// the binary can only ever see the scratch folder.
fn create_store(store: &Path, ac: &Path) -> Result<()> {
    let scratch = store
        .parent()
        .map(Path::to_path_buf)
        .context("the store path has no parent folder")?;
    let output = Command::new(ac)
        .args(["setup", "list"])
        .env_clear()
        .env("ACTIVE_COLLAB_DB", store)
        .env("ACTIVE_COLLAB_LANG", "en")
        .env("HOME", &scratch)
        .env("TMPDIR", &scratch)
        .current_dir(&scratch)
        .output()
        .context("cannot run the ac binary")?;
    if !output.status.success() {
        bail!(
            "creating the store failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn register_instance(store: &Path, base_url: &str) -> Result<()> {
    let conn = rusqlite::Connection::open(store).context("cannot open the new store")?;
    conn.execute(
        "INSERT INTO instances (name, base_url, email, token, user_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, '2026-01-01T00:00:00Z')",
        rusqlite::params![
            INSTANCE_NAME,
            base_url,
            "mock@example.com",
            "mock-token",
            USER_ID
        ],
    )
    .context("cannot register the mock instance")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(store), Some(ac), None) = (args.next(), args.next(), args.next()) else {
        bail!("usage: mock_collab <store-path> <ac-binary>");
    };
    let store = resolved(Path::new(&store))?;
    let ac = std::path::absolute(&ac).context("cannot make the ac path absolute")?;

    check_arguments(&store, &ac)?;
    create_store(&store, &ac)?;

    let server = MockServer::start().await;
    register_instance(&store, &server.uri())?;
    let log_path = store
        .parent()
        .context("the store path has no parent folder")?
        .join(REQUEST_LOG);
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("cannot create the request log {}", log_path.display()))?;
    mount_routes(&server, &Fixture::seeded(), &Arc::new(Mutex::new(log))).await;

    println!("mock-collab ready {}", server.uri());
    std::io::stdout()
        .flush()
        .context("cannot print the ready line")?;

    tokio::task::spawn_blocking(|| std::io::copy(&mut std::io::stdin(), &mut std::io::sink()))
        .await
        .context("the stdin watcher failed")?
        .context("cannot read stdin")?;
    Ok(())
}
