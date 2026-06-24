#!/usr/bin/env python3
"""ActiveCollab task fetcher — multi-instance, SQLite-backed, stdlib only."""

import argparse
import datetime
import getpass
import html as html_lib
import json
import os
import re
import sqlite3
import subprocess
import sys
import urllib.error
import urllib.request

DEFAULT_DB_PATH = os.path.join(
    os.path.expanduser("~"), ".config", "active-collab", "active-collab.db"
)
TASK_CACHE_TTL_HOURS = 24

_TASK_URL_PATTERN = re.compile(r"/projects/(\d+)/tasks/(\d+)")
_BRANCH_PATTERN = re.compile(r"^(feature|hotfix|fix)/(\d+)-(\d+)$")
_TAG_PATTERN = re.compile(r"<[^>]+>")
_BLOCK_TAG_PATTERN = re.compile(r"<(?:br|p|div|li|tr|h[1-6])\b[^>]*>", re.IGNORECASE)
_BLANK_LINES_PATTERN = re.compile(r"\n{3,}")


def http_get(url: str, headers: dict) -> tuple:
    """Return (status_code: int, body: bytes). Never raises on HTTP errors."""
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:  # nosec: B310
            return resp.status, resp.read()
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read()
    except urllib.error.URLError as exc:
        raise ConnectionError(str(exc.reason)) from exc


def http_post(url: str, data: dict, headers: dict) -> tuple:
    """Return (status_code: int, body: bytes) for a JSON POST."""
    payload = json.dumps(data).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json", **headers},
    )
    try:
        with urllib.request.urlopen(req) as resp:  # nosec: B310
            return resp.status, resp.read()
    except urllib.error.HTTPError as exc:
        return exc.code, exc.read()
    except urllib.error.URLError as exc:
        raise ConnectionError(str(exc.reason)) from exc


def _db_path() -> str:
    return os.environ.get("ACTIVE_COLLAB_DB", DEFAULT_DB_PATH)


def _open_db() -> sqlite3.Connection:
    """Open (creating if needed) the SQLite DB with schema applied."""
    path = _db_path()
    parent = os.path.dirname(path)
    if not os.path.isdir(parent):
        os.makedirs(parent, mode=0o700, exist_ok=True)
    conn = sqlite3.connect(path)
    if not _is_mode_600(path):
        os.chmod(path, 0o600)
    # DELETE journal avoids WAL sidecar files; single-process CLI needs no WAL.
    conn.execute("PRAGMA journal_mode=DELETE")
    conn.execute("PRAGMA busy_timeout=5000")
    conn.execute("PRAGMA foreign_keys=ON")
    _init_schema(conn)
    return conn


def _is_mode_600(path: str) -> bool:
    mode = oct(os.stat(path).st_mode)[-3:]
    return mode == "600"


def _init_schema(conn: sqlite3.Connection) -> None:
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS instances (
            name       TEXT PRIMARY KEY,
            base_url   TEXT NOT NULL,
            email      TEXT NOT NULL,
            token      TEXT NOT NULL,
            user_id    INTEGER,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ticket_cache (
            instance   TEXT NOT NULL,
            project_id INTEGER NOT NULL,
            task_id    INTEGER NOT NULL,
            fields_json TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            PRIMARY KEY (instance, project_id, task_id)
        );
    """)
    conn.commit()


def _now_iso() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _build_headers(instance: dict) -> dict:
    return {
        "Accept": "application/json",
        "X-Angie-AuthApiToken": instance["token"],
    }


def _stdin_is_interactive() -> bool:
    return sys.stdin.isatty()


def _prompt_required(label: str) -> str | None:
    while True:
        try:
            value = input(f"{label}: ").strip()
        except (EOFError, KeyboardInterrupt):
            return None
        if value:
            return value
        print(f"{label} cannot be empty.", file=sys.stderr)


def _resolve_field(value: str | None, label: str, interactive: bool) -> str | None:
    if value:
        return value
    if interactive:
        return _prompt_required(label)
    return None


def _exchange_token(base_url: str, email: str, password: str) -> tuple:
    """POST to issue-token endpoint. Return (token: str, raw_response: dict)."""
    url = f"{base_url}/api/v1/issue-token"
    body = {
        "username": email,
        "password": password,
        "client_name": "active-collab-skill",
        "client_vendor": "klock",
    }
    status, raw = http_post(url, body, {})
    if status != 200:
        return None, {}
    data = json.loads(raw)
    if not data.get("is_ok"):
        return None, data
    return data.get("token"), data


def _resolve_user_id(base_url: str, token: str, email: str) -> int | None:
    """Fetch /api/v1/users and return the user_id matching email (case-insensitive)."""
    headers = {"Accept": "application/json", "X-Angie-AuthApiToken": token}
    status, body = http_get(f"{base_url}/api/v1/users", headers)
    if status != 200:
        return None
    data = json.loads(body)
    users = data if isinstance(data, list) else []
    email_lower = email.lower()
    for user in users:
        if (user.get("email") or "").lower() == email_lower:
            return user.get("id")
    return None


def _save_instance(name: str, base_url: str, email: str, token: str, user_id: int | None) -> None:
    with _open_db() as conn:
        conn.execute(
            "INSERT OR REPLACE INTO instances (name, base_url, email, token, user_id, created_at)"
            " VALUES (?, ?, ?, ?, ?, ?)",
            (name, base_url, email, token, user_id, _now_iso()),
        )
        conn.commit()


def _load_all_instances(conn: sqlite3.Connection) -> list:
    rows = conn.execute(
        "SELECT name, base_url, email, token, user_id FROM instances ORDER BY created_at, name"
    ).fetchall()
    return [
        {"name": r[0], "base_url": r[1], "email": r[2], "token": r[3], "user_id": r[4]}
        for r in rows
    ]


def _resolve_instance_by_name(instances: list, name: str) -> dict | None:
    matches = [i for i in instances if i["name"] == name]
    return matches[0] if matches else None


def _pick_instance(conn: sqlite3.Connection, instance_name: str | None) -> dict:
    """Return the matching instance or exit with a diagnostic."""
    instances = _load_all_instances(conn)

    if not instances:
        print(
            "Error: no instances configured. Run: active_collab.py setup add",
            file=sys.stderr,
        )
        sys.exit(2)

    if instance_name:
        inst = _resolve_instance_by_name(instances, instance_name)
        if not inst:
            known = ", ".join(i["name"] for i in instances)
            print(
                f"Error: instance '{instance_name}' not found. Known: {known}",
                file=sys.stderr,
            )
            sys.exit(2)
        return inst

    if len(instances) == 1:
        return instances[0]

    names = ", ".join(i["name"] for i in instances)
    print(
        f"Error: multiple instances configured ({names}). Use --instance NAME.",
        file=sys.stderr,
    )
    sys.exit(2)


def _parse_task_ref(ref: str) -> tuple:
    """Return (project_id, task_id) from a URL or '665/75159' short form. Exit 2 on bad ref."""
    m = _TASK_URL_PATTERN.search(ref)
    if m:
        return int(m.group(1)), int(m.group(2))

    parts = ref.split("/")
    if len(parts) == 2 and parts[0].isdigit() and parts[1].isdigit():
        return int(parts[0]), int(parts[1])

    print(
        f"Error: cannot parse task ref '{ref}'. Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159).",
        file=sys.stderr,
    )
    sys.exit(2)


def _parse_branch_ref(branch: str) -> tuple | None:
    """Return (project_id, task_id) if branch matches expected pattern, else None."""
    m = _BRANCH_PATTERN.match(branch)
    if not m:
        return None
    return int(m.group(2)), int(m.group(3))


def _current_branch() -> str | None:
    """Return the current git branch name, or None if not in a git repo / detached HEAD."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode != 0:
            return None
        branch = result.stdout.strip()
        return branch if branch != "HEAD" else None
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None


def _html_to_text(html: str) -> str:
    """Strip HTML tags and unescape entities. Block/br tags become newlines."""
    if not html:
        return ""
    text = _BLOCK_TAG_PATTERN.sub("\n", html)
    text = _TAG_PATTERN.sub("", text)
    text = html_lib.unescape(text)
    text = _BLANK_LINES_PATTERN.sub("\n\n", text)
    return text.strip()


def _fmt_ts(value) -> str:
    """Format a unix int/float timestamp as 'YYYY-MM-DD HH:MM' UTC. Returns '' for None."""
    if value is None:
        return ""
    if isinstance(value, (int, float)):
        dt = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc)
        return dt.strftime("%Y-%m-%d %H:%M")
    return str(value)


def _fmt_date(value) -> str:
    """Format a unix int/float timestamp as 'YYYY-MM-DD' UTC. Returns '' for None."""
    if value is None:
        return ""
    if isinstance(value, (int, float)):
        dt = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc)
        return dt.strftime("%Y-%m-%d")
    return str(value)


def _fmt_hours(value) -> str:
    """Format an hours value as an integer string when whole, fractional otherwise."""
    if value is None:
        return "0"
    try:
        f = float(value)
    except (TypeError, ValueError):
        return str(value)
    return str(int(f)) if f == int(f) else str(f)


def _fetch_user_map(instance: dict) -> dict:
    """Return {user_id: display_name} from GET /api/v1/users. Returns {} on failure."""
    url = f"{instance['base_url'].rstrip('/')}/api/v1/users"
    status, body = http_get(url, _build_headers(instance))
    if status != 200:
        return {}
    data = json.loads(body)
    if not isinstance(data, list):
        return {}
    result = {}
    for user in data:
        uid = user.get("id")
        if uid is None:
            continue
        name = (
            user.get("display_name")
            or " ".join(filter(None, [
                (user.get("first_name") or "").strip(),
                (user.get("last_name") or "").strip(),
            ]))
            or user.get("email")
            or ""
        )
        result[uid] = name
    return result


def _fetch_task(instance: dict, project_id: int, task_id: int) -> tuple:
    """Return (status_code, full_payload_dict_or_None) from the API."""
    url = f"{instance['base_url'].rstrip('/')}/api/v1/projects/{project_id}/tasks/{task_id}"
    status, body = http_get(url, _build_headers(instance))
    if status == 200:
        return status, json.loads(body)
    return status, None


def _read_task_cache(conn: sqlite3.Connection, instance: str, project_id: int, task_id: int) -> dict | None:
    row = conn.execute(
        "SELECT fields_json, fetched_at FROM ticket_cache"
        " WHERE instance=? AND project_id=? AND task_id=?",
        (instance, project_id, task_id),
    ).fetchone()
    if not row:
        return None
    return {"fields": json.loads(row[0]), "fetched_at": row[1]}


def _write_task_cache(
    conn: sqlite3.Connection,
    instance: str,
    project_id: int,
    task_id: int,
    task: dict,
    comments: list,
) -> None:
    payload = {**task, "comments": comments}
    conn.execute(
        "INSERT OR REPLACE INTO ticket_cache"
        " (instance, project_id, task_id, fields_json, fetched_at)"
        " VALUES (?, ?, ?, ?, ?)",
        (instance, project_id, task_id, json.dumps(payload), _now_iso()),
    )
    conn.commit()


def _render_comments(comments: list) -> None:
    """Print the comments section for a task view."""
    if not comments:
        return
    print(f"\nComments ({len(comments)}):")
    for idx, c in enumerate(comments, 1):
        author = c.get("created_by_name") or c.get("created_by_id") or "(unknown)"
        created = _fmt_ts(c.get("created_on"))
        body_text = c.get("body_plain_text") or _html_to_text(c.get("body") or "")
        print(f"\n  [{idx}] {author} — {created}")
        for line in body_text.splitlines():
            print(f"  {line}")


def _render_meta(task: dict, user_map: dict) -> None:
    """Print assignee, dates, estimate, and logged hours for the task view."""
    assignee_id = task.get("assignee_id")
    if assignee_id is None:
        assignee_label = "(unassigned)"
    else:
        name = user_map.get(assignee_id)
        assignee_label = f"{name} ({assignee_id})" if name else f"({assignee_id})"
    print(f"Assignee:  {assignee_label}")

    start = _fmt_date(task.get("start_on"))
    if start:
        print(f"Start:     {start}")

    due = _fmt_date(task.get("due_on"))
    if due:
        print(f"Due:       {due}")

    print(f"Estimate:  {_fmt_hours(task.get('estimate'))}h")
    print(f"Logged:    {_fmt_hours(task.get('tracked_time'))}h")


def _render_task(task: dict, comments: list, no_comments: bool, user_map: dict) -> None:
    """Print a human-readable task view."""
    print(f"Task:      {task.get('task_number') or task.get('id')}")
    print(f"Name:      {task.get('name', '')}")
    status_label = "Completed" if task.get("is_completed") else "Open"
    print(f"Status:    {status_label}")
    _render_meta(task, user_map)
    print()
    body = _html_to_text(task.get("body") or "") or "(no description)"
    print("Description:")
    print(body)

    if no_comments:
        return

    _render_comments(comments)


def _load_task(
    conn: sqlite3.Connection,
    inst: dict,
    project_id: int,
    task_id: int,
    refresh: bool,
    no_comments: bool,
) -> tuple | None:
    """Return (task_dict, comments_list) from cache or API, or None on HTTP error.

    Owns the full cache-read → fetch → unwrap → cache-write flow so that
    _do_get_task can focus on flag dispatch and rendering.
    """
    if not refresh:
        cached = _read_task_cache(conn, inst["name"], project_id, task_id)
        if cached:
            task = cached["fields"]
            comments = task.pop("comments", [])
            return task, comments

    status, payload = _fetch_task(inst, project_id, task_id)
    if status != 200:
        print(f"Error: task {project_id}/{task_id} not found (HTTP {status}).", file=sys.stderr)
        return None

    task = payload.get("single") or {}
    task["tracked_time"] = payload.get("tracked_time")
    comments = [] if no_comments else payload.get("comments", [])
    _write_task_cache(conn, inst["name"], project_id, task_id, task, comments)
    return task, comments


def _do_get_task(args: argparse.Namespace, project_id: int, task_id: int) -> int:
    """Shared fetch-and-render logic for both `get` and `current`."""
    conn = _open_db()
    inst = _pick_instance(conn, getattr(args, "instance", None))

    if getattr(args, "json", False):
        status, payload = _fetch_task(inst, project_id, task_id)
        if status != 200:
            print(f"Error: task {project_id}/{task_id} not found (HTTP {status}).", file=sys.stderr)
            return 1
        print(json.dumps(payload, indent=2, ensure_ascii=False))
        return 0

    result = _load_task(
        conn, inst, project_id, task_id,
        getattr(args, "refresh", False),
        getattr(args, "no_comments", False),
    )
    if result is None:
        return 1

    task, comments = result

    if getattr(args, "short", False):
        print(f"{project_id}/{task_id}\t{task.get('name', '')}")
        return 0

    user_map = _fetch_user_map(inst)
    _render_task(task, comments, getattr(args, "no_comments", False), user_map)
    return 0


def cmd_setup_add(args: argparse.Namespace) -> int:
    """Register a new ActiveCollab instance via the issue-token exchange."""
    interactive = _stdin_is_interactive()

    name = _resolve_field(args.name, "Instance name", interactive)
    url = _resolve_field(args.url, "Base URL (https://...)", interactive)
    email = _resolve_field(args.email, "Email", interactive)

    if not (name and url and email):
        print("Error: --name, --url and --email are required.", file=sys.stderr)
        return 2

    password = getpass.getpass("Password (input hidden): ")
    if not password:
        print("Error: password is required.", file=sys.stderr)
        return 2

    base_url = url.rstrip("/")
    token, response = _exchange_token(base_url, email, password)
    del password

    if not token:
        detail = response.get("message") or "token exchange failed"
        print(f"Error: {detail}", file=sys.stderr)
        return 1

    user_id = _resolve_user_id(base_url, token, email)
    _save_instance(name, base_url, email, token, user_id)
    print(f"Instance '{name}' saved.")

    if interactive:
        _run_connectivity_check(base_url, token)

    return 0


def _run_connectivity_check(base_url: str, token: str) -> None:
    headers = {"Accept": "application/json", "X-Angie-AuthApiToken": token}
    try:
        status, _ = http_get(f"{base_url}/api/v1/projects", headers)
        if status == 200:
            print("Connectivity: OK")
        else:
            print(f"Connectivity: FAILED (HTTP {status})")
    except ConnectionError as exc:
        print(f"Connectivity: FAILED ({exc})")


def cmd_setup_list(_args: argparse.Namespace) -> int:
    with _open_db() as conn:
        rows = conn.execute(
            "SELECT name, base_url, email, user_id FROM instances ORDER BY created_at, name"
        ).fetchall()

    if not rows:
        print("No instances configured. Run: active_collab.py setup add")
        return 0

    print(f"{'NAME':<20} {'URL':<40} {'EMAIL':<30} {'USER_ID'}")
    print("-" * 100)
    for name, base_url, email, user_id in rows:
        print(f"{name:<20} {base_url:<40} {email:<30} {user_id or ''}")
    return 0


def cmd_setup_remove(args: argparse.Namespace) -> int:
    with _open_db() as conn:
        deleted = conn.execute(
            "DELETE FROM instances WHERE name = ?", (args.name,)
        ).rowcount
        conn.execute("DELETE FROM ticket_cache WHERE instance = ?", (args.name,))
        conn.commit()

    if deleted == 0:
        print(f"Error: instance '{args.name}' not found.", file=sys.stderr)
        return 2
    print(f"Instance '{args.name}' removed.")
    return 0


def cmd_setup_test(args: argparse.Namespace) -> int:
    with _open_db() as conn:
        if args.name:
            rows = conn.execute(
                "SELECT name, base_url, token FROM instances WHERE name = ?", (args.name,)
            ).fetchall()
            if not rows:
                print(f"Error: instance '{args.name}' not found.", file=sys.stderr)
                return 2
        else:
            rows = conn.execute(
                "SELECT name, base_url, token FROM instances ORDER BY created_at, name"
            ).fetchall()

    exit_code = 0
    for name, base_url, token in rows:
        headers = {"Accept": "application/json", "X-Angie-AuthApiToken": token}
        try:
            status, _ = http_get(f"{base_url}/api/v1/projects", headers)
            if status == 200:
                print(f"  {name}: OK ({status})")
            else:
                print(f"  {name}: FAILED (HTTP {status})")
                exit_code = 1
        except ConnectionError as exc:
            print(f"  {name}: FAILED ({exc})")
            exit_code = 1

    return exit_code


def cmd_get(args: argparse.Namespace) -> int:
    project_id, task_id = _parse_task_ref(args.ref)
    return _do_get_task(args, project_id, task_id)


def cmd_current(args: argparse.Namespace) -> int:
    branch = _current_branch()
    if not branch:
        print(
            "Error: not in a git repository or HEAD is detached.",
            file=sys.stderr,
        )
        return 2

    ids = _parse_branch_ref(branch)
    if not ids:
        print(
            f"Error: branch '{branch}' does not match expected pattern "
            f"(feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159).",
            file=sys.stderr,
        )
        return 2

    project_id, task_id = ids
    return _do_get_task(args, project_id, task_id)


def _fetch_open_tasks_for_user(inst: dict) -> list:
    """Fetch open tasks assigned to this user via GET /api/v1/users/{user_id}/tasks."""
    user_id = inst.get("user_id")
    if not user_id:
        return []

    headers = _build_headers(inst)
    base = inst["base_url"].rstrip("/")

    status, body = http_get(f"{base}/api/v1/users/{user_id}/tasks", headers)
    if status != 200:
        return []

    data = json.loads(body)
    raw_tasks = data.get("tasks", []) if isinstance(data, dict) else []

    return [
        {
            "instance": inst["name"],
            "project_id": task.get("project_id"),
            "task_number": task.get("task_number") or task.get("id"),
            "task_id": task.get("id"),
            "name": task.get("name", ""),
        }
        for task in raw_tasks
        if not task.get("is_completed") and not task.get("is_trashed")
    ]


def cmd_mine(args: argparse.Namespace) -> int:
    conn = _open_db()
    instances = _load_all_instances(conn)

    if not instances:
        print("Error: no instances configured. Run: active_collab.py setup add", file=sys.stderr)
        return 2

    target_instances = instances
    if getattr(args, "instance", None):
        inst = _resolve_instance_by_name(instances, args.instance)
        if not inst:
            known = ", ".join(i["name"] for i in instances)
            print(f"Error: instance '{args.instance}' not found. Known: {known}", file=sys.stderr)
            return 2
        target_instances = [inst]

    all_tasks = []
    for inst in target_instances:
        tasks = _fetch_open_tasks_for_user(inst)
        all_tasks.extend(tasks)

    if not all_tasks:
        print("No open tasks assigned to you.")
        return 0

    print(f"{'INSTANCE':<15} {'PROJECT':<10} {'TASK#':<8} {'TASK_ID':<10} NAME")
    print("-" * 80)
    for t in all_tasks:
        print(
            f"{t['instance']:<15} {t['project_id']:<10} "
            f"{t['task_number']:<8} {t['task_id']:<10} {t['name']}"
        )
    return 0


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="active_collab.py",
        description="Fetch ActiveCollab tasks from one or more configured instances.",
    )
    sub = parser.add_subparsers(dest="command")

    setup_p = sub.add_parser("setup", help="Manage instance configuration.")
    setup_sub = setup_p.add_subparsers(dest="setup_command")

    add_p = setup_sub.add_parser("add", help="Register an ActiveCollab instance.")
    add_p.add_argument("--name", required=False, help="Unique name (prompted if omitted, interactive).")
    add_p.add_argument("--url", required=False, help="Base URL, e.g. https://collab.example.com.")
    add_p.add_argument("--email", required=False, help="Email for token exchange.")

    setup_sub.add_parser("list", help="List configured instances (no tokens).")

    remove_p = setup_sub.add_parser("remove", help="Remove an instance.")
    remove_p.add_argument("--name", required=True)

    test_p = setup_sub.add_parser("test", help="Test connectivity.")
    test_p.add_argument("--name", help="Test only this instance.")

    get_p = sub.add_parser("get", help="Fetch and display a task.")
    _add_get_args(get_p)

    current_p = sub.add_parser("current", help="Fetch the task from the current git branch.")
    _add_display_args(current_p)

    mine_p = sub.add_parser("mine", aliases=["list"], help="List open tasks assigned to you.")
    mine_p.add_argument("--instance", help="Limit to this instance.")

    return parser


def _add_get_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("ref", help="Task URL or PROJECT_ID/TASK_ID (e.g. 665/75159).")
    _add_display_args(p)


def _add_display_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("--instance", help="Force a named instance.")
    p.add_argument("--short", action="store_true", help="Print PROJECT/TASK<TAB>name only.")
    p.add_argument("--no-comments", action="store_true", dest="no_comments")
    p.add_argument("--json", action="store_true", help="Print raw task JSON.")
    p.add_argument("--refresh", action="store_true", help="Ignore cache and re-fetch.")


def _dispatch_setup(args: argparse.Namespace, parser: argparse.ArgumentParser) -> int:
    if args.setup_command == "add":
        return cmd_setup_add(args)
    if args.setup_command == "list":
        return cmd_setup_list(args)
    if args.setup_command == "remove":
        return cmd_setup_remove(args)
    if args.setup_command == "test":
        return cmd_setup_test(args)
    parser.parse_args(["setup", "--help"])
    return 2


def main(argv: list | None = None) -> int:
    parser = _build_parser()

    if argv is None:
        argv = sys.argv[1:]

    # Support bare invocation: no subcommand + current branch matches pattern
    if argv and not argv[0].startswith("-") and argv[0] not in (
        "setup", "get", "current", "mine", "list"
    ):
        argv = ["get"] + argv
    elif not argv:
        branch = _current_branch()
        if branch and _parse_branch_ref(branch):
            argv = ["current"]

    args = parser.parse_args(argv)

    if args.command == "setup":
        return _dispatch_setup(args, parser)

    if args.command == "get":
        return cmd_get(args)

    if args.command == "current":
        return cmd_current(args)

    if args.command in ("mine", "list"):
        return cmd_mine(args)

    parser.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
