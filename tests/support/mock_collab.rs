//! The fictitious ActiveCollab every replay and end-to-end test mounts: one
//! seed and one set of route shapes, so a shape fixed here is fixed for the
//! terminal walkthroughs and the automated tests alike.
//!
//! Included with `#[path]` by the `mock_collab` example and by integration
//! tests; none of the data is copied from a real instance.

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const USER_ID: i64 = 42;
const USER_NAME: &str = "Ada Lovelace";
/// 2024-03-15 09:30 UTC, fixed so replayed output never depends on the clock.
const COMMENT_CREATED_ON: i64 = 1_710_495_000;
const TOKEN_HEADER: &str = "x-angie-authapitoken";
const FIRST_TASK_LIST: i64 = 4;
const CREATED_COMMENT_ID: i64 = 555;
const CREATED_TIME_RECORD_ID: i64 = 777;

/// The text of the one comment a task with attachments carries.
pub const COMMENT_TEXT: &str = "The file is attached.";
/// The task lists of every seeded project, as `(id, name)`.
pub const TASK_LISTS: [(i64, &str); 3] = [(4, "To do"), (5, "In progress"), (6, "Review")];

/// Where an open append-only request log lives; each request adds one line.
pub type RequestLog = Arc<Mutex<std::fs::File>>;

/// One time record, dated relative to the Monday of the current week so the
/// seed holds on any day of the year.
#[derive(Clone)]
pub struct SeedRecord {
    id: i64,
    days_from_week_start: i64,
    hours: f64,
}

/// Where a comment attachment is served from.
#[derive(Clone, Copy)]
pub enum AttachmentHost {
    /// The instance's own address.
    Instance,
    /// The same loopback server under another host name, so the instance
    /// origin check treats it as a foreign host.
    Foreign,
}

/// One file attached to a task's comment.
#[derive(Clone)]
pub struct SeedAttachment {
    pub name: &'static str,
    host: AttachmentHost,
    /// What the file route serves; `None` makes it answer HTTP 500.
    pub bytes: Option<&'static [u8]>,
}

#[derive(Clone)]
pub struct SeedTask {
    pub id: i64,
    pub project_id: i64,
    number: i64,
    pub name: &'static str,
    estimate: Option<f64>,
    records: Vec<SeedRecord>,
    pub attachments: Vec<SeedAttachment>,
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

/// The data every route serves.
#[derive(Clone)]
pub struct Fixture {
    /// The user the instance's token belongs to; the open tasks and time
    /// records are served under this id.
    pub user_id: i64,
    pub tasks: Vec<SeedTask>,
}

impl Fixture {
    pub fn seeded() -> Fixture {
        let record = |id, days_from_week_start, hours| SeedRecord {
            id,
            days_from_week_start,
            hours,
        };
        let file = |name, host, bytes| SeedAttachment { name, host, bytes };
        Fixture {
            user_id: USER_ID,
            tasks: vec![
                SeedTask {
                    id: 101,
                    project_id: 10,
                    number: 7,
                    name: "Draft the quarterly report",
                    estimate: Some(8.0),
                    records: vec![record(1, 0, 2.5), record(2, -1, 1.5), record(3, -9, 2.0)],
                    attachments: vec![file(
                        "report.txt",
                        AttachmentHost::Instance,
                        Some(b"Quarterly report attachment served by the mock ActiveCollab.\n"),
                    )],
                },
                SeedTask {
                    id: 102,
                    project_id: 10,
                    number: 8,
                    name: "Review the vendor contract",
                    estimate: Some(4.0),
                    records: vec![record(4, -10, 3.0)],
                    attachments: vec![file(
                        "foreign.txt",
                        AttachmentHost::Foreign,
                        Some(b"Attachment served from a host other than the instance.\n"),
                    )],
                },
                SeedTask {
                    id: 103,
                    project_id: 11,
                    number: 3,
                    name: "Plan the team offsite",
                    estimate: None,
                    records: Vec::new(),
                    attachments: vec![
                        file(
                            "agenda.txt",
                            AttachmentHost::Instance,
                            Some(b"Offsite agenda served by the mock ActiveCollab.\n"),
                        ),
                        file("venue-quote.txt", AttachmentHost::Instance, None),
                    ],
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
    user_id: i64,
    task: SeedTask,
    base_url: String,
    placement: Placement,
}

impl TaskDetail {
    fn comments(&self) -> Vec<Value> {
        if self.task.attachments.is_empty() {
            return Vec::new();
        }
        let attachments: Vec<Value> = self
            .task
            .attachments
            .iter()
            .map(|file| {
                let base = match file.host {
                    AttachmentHost::Instance => self.base_url.clone(),
                    AttachmentHost::Foreign => self.base_url.replacen("127.0.0.1", "localhost", 1),
                };
                json!({ "name": file.name, "url": format!("{base}/files/{}", file.name) })
            })
            .collect();
        vec![json!({
            "id": self.task.id * 10,
            "body": format!("<p>{COMMENT_TEXT}</p>"),
            "created_by_id": self.user_id,
            "created_by_name": USER_NAME,
            "created_on": COMMENT_CREATED_ON,
            "attachments": attachments,
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
                "assignee_id": self.user_id,
            },
            "tracked_time": task.total_hours(),
            "comments": self.comments(),
        }))
    }
}

/// The dependencies of the task at `index` in the seed: the next seeded task
/// is its parent and the previous one its child.
struct Dependencies {
    tasks: Vec<SeedTask>,
    index: usize,
}

impl Dependencies {
    fn entry(task: &SeedTask) -> Value {
        json!({
            "id": task.id,
            "class": "Task",
            "project_id": task.project_id,
            "task_number": task.number,
            "name": task.name,
            "is_completed": false,
            "is_trashed": false,
            "task_list_id": FIRST_TASK_LIST,
            "labels": [{ "id": 7, "name": "STATUS:GO LIVE", "color": "#FBBB75" }],
        })
    }
}

impl Respond for Dependencies {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let parents: Vec<Value> = self
            .tasks
            .get(self.index + 1)
            .map(Self::entry)
            .into_iter()
            .collect();
        let children: Vec<Value> = self
            .index
            .checked_sub(1)
            .and_then(|previous| self.tasks.get(previous))
            .map(Self::entry)
            .into_iter()
            .collect();
        ResponseTemplate::new(200)
            .set_body_json(json!({ "parents": parents, "children": children }))
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

struct JobTypes;

impl Respond for JobTypes {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .set_body_json(json!([{ "id": 3, "name": "Development", "is_default": true }]))
    }
}

/// Answers a write with the created record's id.
struct Created(i64);

impl Respond for Created {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({ "single": { "id": self.0 } }))
    }
}

/// Serves an attachment's bytes, or HTTP 500 when the file has none.
struct AttachmentFile(Option<&'static [u8]>);

impl Respond for AttachmentFile {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        match self.0 {
            Some(bytes) => ResponseTemplate::new(200).set_body_bytes(bytes),
            None => ResponseTemplate::new(500),
        }
    }
}

struct Logged<R> {
    inner: R,
    log: Option<RequestLog>,
    unmatched: bool,
}

impl<R: Respond> Respond for Logged<R> {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        if let Some(log) = &self.log {
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
            let _ = log.lock().unwrap().write_all(line.as_bytes());
        }
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
    log: &Option<RequestLog>,
) {
    Mock::given(method(verb))
        .and(path(route))
        .respond_with(Logged {
            inner,
            log: log.clone(),
            unmatched: false,
        })
        .mount(server)
        .await;
}

/// Mounts every seeded route on `server`. With a `log`, each request is
/// appended to it and a request no route serves is marked `unmatched` and
/// answered 404.
pub async fn mount_routes(server: &MockServer, fixture: &Fixture, log: Option<&RequestLog>) {
    let log = log.cloned();
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
        &log,
    )
    .await;
    if log.is_some() {
        Mock::given(any())
            .respond_with(Logged {
                inner: NotFound,
                log: log.clone(),
                unmatched: true,
            })
            .with_priority(u8::MAX)
            .mount(server)
            .await;
    }
    mount(
        server,
        "GET",
        format!("/api/v1/users/{}/tasks", fixture.user_id),
        OpenTasks(fixture.clone()),
        &log,
    )
    .await;
    mount(
        server,
        "GET",
        format!(
            "/api/v1/users/{}/time-records/filtered-by-date",
            fixture.user_id
        ),
        TimeRecordsByDate(fixture.clone()),
        &log,
    )
    .await;
    mount(
        server,
        "GET",
        "/api/v1/job-types".to_string(),
        JobTypes,
        &log,
    )
    .await;
    for (index, task) in fixture.tasks.iter().enumerate() {
        let dependencies = Dependencies {
            tasks: fixture.tasks.clone(),
            index,
        };
        mount(
            server,
            "GET",
            format!("/api/v1/dependencies/tasks/{}", task.id),
            dependencies,
            &log,
        )
        .await;
        let task_route = format!("/api/v1/projects/{}/tasks/{}", task.project_id, task.id);
        let detail = TaskDetail {
            user_id: fixture.user_id,
            task: task.clone(),
            base_url: server.uri(),
            placement: Arc::clone(&placement),
        };
        mount(server, "GET", task_route.clone(), detail, &log).await;
        let mover = MoveTask {
            task_id: task.id,
            placement: Arc::clone(&placement),
        };
        mount(server, "PUT", task_route, mover, &log).await;
        mount(
            server,
            "POST",
            format!("/api/v1/comments/task/{}", task.id),
            Created(CREATED_COMMENT_ID),
            &log,
        )
        .await;
        mount(
            server,
            "PUT",
            format!("/api/v1/complete/task/{}", task.id),
            Created(task.id),
            &log,
        )
        .await;
        for file in &task.attachments {
            mount(
                server,
                "GET",
                format!("/files/{}", file.name),
                AttachmentFile(file.bytes),
                &log,
            )
            .await;
        }
    }
    let mut projects: Vec<i64> = fixture.tasks.iter().map(|task| task.project_id).collect();
    projects.sort_unstable();
    projects.dedup();
    for project_id in projects {
        let route = format!("/api/v1/projects/{project_id}/task-lists");
        mount(server, "GET", route, TaskLists, &log).await;
        mount(
            server,
            "POST",
            format!("/api/v1/projects/{project_id}/time-records"),
            Created(CREATED_TIME_RECORD_ID),
            &log,
        )
        .await;
    }
}
