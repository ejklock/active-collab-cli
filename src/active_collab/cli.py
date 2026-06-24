import argparse
import getpass
import json
import re
import subprocess
import sys

from active_collab import render
from active_collab.client import ActiveCollabClient
from active_collab.config import Config
from active_collab.http import HttpClient
from active_collab.models import Instance
from active_collab.store import InstanceRepository, Store, TaskCache

_TASK_URL_PATTERN = re.compile(r"/projects/(\d+)/tasks/(\d+)")
_BRANCH_PATTERN = re.compile(r"^(feature|hotfix|fix)/(\d+)-(\d+)$")


def _parse_task_ref(ref: str) -> tuple[int, int]:
    """Return (project_id, task_id) from a URL or '665/75159' form. Exit 2 on bad ref."""
    m = _TASK_URL_PATTERN.search(ref)
    if m:
        return int(m.group(1)), int(m.group(2))

    parts = ref.split("/")
    if len(parts) == 2 and parts[0].isdigit() and parts[1].isdigit():
        return int(parts[0]), int(parts[1])

    render.print_error(
        f"Error: cannot parse task ref '{ref}'."
        " Use URL or PROJECT_ID/TASK_ID (e.g. 665/75159)."
    )
    sys.exit(2)


def _parse_branch_ref(branch: str) -> tuple[int, int] | None:
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


def _stdin_is_interactive() -> bool:
    return sys.stdin.isatty()


def _pick_instance(instances: list[Instance], instance_name: str | None) -> Instance:
    """Return the matching Instance or print error + exit(2)."""
    if not instances:
        render.print_error(
            "Error: no instances configured. Run: active_collab.py setup add"
        )
        sys.exit(2)

    if instance_name:
        matches = [i for i in instances if i.name == instance_name]
        if not matches:
            known = ", ".join(i.name for i in instances)
            render.print_error(
                f"Error: instance '{instance_name}' not found. Known: {known}"
            )
            sys.exit(2)
        return matches[0]

    if len(instances) == 1:
        return instances[0]

    names = ", ".join(i.name for i in instances)
    render.print_error(
        f"Error: multiple instances configured ({names}). Use --instance NAME."
    )
    sys.exit(2)


def _resolve_field(value: str | None, label: str, interactive: bool) -> str | None:
    if value:
        return value
    if not interactive:
        return None
    while True:
        try:
            val = input(f"{label}: ").strip()
        except (EOFError, KeyboardInterrupt):
            return None
        if val:
            return val
        print(f"{label} cannot be empty.", file=sys.stderr)


def _load_task(
    cache: TaskCache,
    client: ActiveCollabClient,
    instance_name: str,
    project_id: int,
    task_id: int,
    refresh: bool,
    no_comments: bool,
) -> tuple[dict, list] | None:
    """Return (task_dict, comments_list) from cache or API, or None on HTTP error."""
    if not refresh:
        cached = cache.read(instance_name, project_id, task_id)
        if cached:
            task = cached["fields"]
            comments = task.pop("comments", [])
            return task, comments

    status, payload = client.fetch_task(project_id, task_id)
    if status != 200:
        render.print_error(
            f"Error: task {project_id}/{task_id} not found (HTTP {status})."
        )
        return None

    task = payload.get("single") or {}
    task["tracked_time"] = payload.get("tracked_time")
    comments = [] if no_comments else payload.get("comments", [])
    cache.write(instance_name, project_id, task_id, task, comments)
    return task, comments


def _do_get_task(
    args: argparse.Namespace, project_id: int, task_id: int
) -> int:
    """Shared fetch-and-render logic for both `get` and `current`."""
    config = Config.load()
    store = Store(config)
    repo = InstanceRepository(store.conn)
    instances = repo.load_all()
    inst = _pick_instance(instances, getattr(args, "instance", None))
    http = HttpClient()
    client = ActiveCollabClient(inst, http)

    if getattr(args, "json", False):
        status, payload = client.fetch_task(project_id, task_id)
        if status != 200:
            render.print_error(
                f"Error: task {project_id}/{task_id} not found (HTTP {status})."
            )
            return 1
        print(json.dumps(payload, indent=2, ensure_ascii=False))
        return 0

    cache = TaskCache(store.conn)
    result = _load_task(
        cache,
        client,
        inst.name,
        project_id,
        task_id,
        getattr(args, "refresh", False),
        getattr(args, "no_comments", False),
    )
    if result is None:
        return 1

    task, comments = result

    if getattr(args, "short", False):
        print(f"{project_id}/{task_id}\t{task.get('name', '')}")
        return 0

    user_map = client.fetch_user_map()
    render.render_task(task, comments, getattr(args, "no_comments", False), user_map)
    return 0


def cmd_setup_add(args: argparse.Namespace) -> int:
    """Register a new ActiveCollab instance via the issue-token exchange."""
    interactive = _stdin_is_interactive()

    name = _resolve_field(args.name, "Instance name", interactive)
    url = _resolve_field(args.url, "Base URL (https://...)", interactive)
    email = _resolve_field(args.email, "Email", interactive)

    if not (name and url and email):
        render.print_error("Error: --name, --url and --email are required.")
        return 2

    password = getpass.getpass("Password (input hidden): ")
    if not password:
        render.print_error("Error: password is required.")
        return 2

    base_url = url.rstrip("/")
    http = HttpClient()
    dummy_inst = Instance(name="", base_url=base_url, email=email, token="")
    client = ActiveCollabClient(dummy_inst, http)
    token, response = client.exchange_token(base_url, email, password)
    del password

    if not token:
        detail = response.get("message") or "token exchange failed"
        render.print_error(f"Error: {detail}")
        return 1

    user_id = client.resolve_user_id(base_url, token, email)
    instance = Instance(name=name, base_url=base_url, email=email, token=token, user_id=user_id)
    config = Config.load()
    store = Store(config)
    InstanceRepository(store.conn).save(instance)
    print(f"Instance '{name}' saved.")

    if interactive:
        authed_inst = Instance(name=name, base_url=base_url, email=email, token=token)
        authed_client = ActiveCollabClient(authed_inst, http)
        _run_connectivity_check(authed_client)

    return 0


def _run_connectivity_check(client: ActiveCollabClient) -> None:
    try:
        status, _ = client.test_connectivity()
        if status == 200:
            print("Connectivity: OK")
        else:
            print(f"Connectivity: FAILED (HTTP {status})")
    except ConnectionError as exc:
        print(f"Connectivity: FAILED ({exc})")


def cmd_setup_list(_args: argparse.Namespace) -> int:
    config = Config.load()
    store = Store(config)
    rows = InstanceRepository(store.conn).list_for_display()

    if not rows:
        print("No instances configured. Run: active_collab.py setup add")
        return 0

    print(f"{'NAME':<20} {'URL':<40} {'EMAIL':<30} {'USER_ID'}")
    print("-" * 100)
    for name, base_url, email, user_id in rows:
        print(f"{name:<20} {base_url:<40} {email:<30} {user_id or ''}")
    return 0


def cmd_setup_remove(args: argparse.Namespace) -> int:
    config = Config.load()
    store = Store(config)
    repo = InstanceRepository(store.conn)
    cache = TaskCache(store.conn)
    deleted = repo.delete(args.name)
    cache.delete_for_instance(args.name)

    if deleted == 0:
        render.print_error(f"Error: instance '{args.name}' not found.")
        return 2
    print(f"Instance '{args.name}' removed.")
    return 0


def cmd_setup_test(args: argparse.Namespace) -> int:
    config = Config.load()
    store = Store(config)
    repo = InstanceRepository(store.conn)
    http = HttpClient()

    if args.name:
        rows = repo.find_by_name(args.name)
        if not rows:
            render.print_error(f"Error: instance '{args.name}' not found.")
            return 2
    else:
        rows = repo.list_connectivity()

    exit_code = 0
    for name, base_url, token in rows:
        inst = Instance(name=name, base_url=base_url, email="", token=token)
        client = ActiveCollabClient(inst, http)
        try:
            status, _ = client.test_connectivity()
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
        render.print_error(
            "Error: not in a git repository or HEAD is detached."
        )
        return 2

    ids = _parse_branch_ref(branch)
    if not ids:
        render.print_error(
            f"Error: branch '{branch}' does not match expected pattern "
            f"(feature|hotfix|fix)/PROJECT_ID-TASK_ID (e.g. feature/665-75159)."
        )
        return 2

    project_id, task_id = ids
    return _do_get_task(args, project_id, task_id)


def cmd_mine(args: argparse.Namespace) -> int:
    config = Config.load()
    store = Store(config)
    repo = InstanceRepository(store.conn)
    instances = repo.load_all()
    http = HttpClient()

    if not instances:
        render.print_error(
            "Error: no instances configured. Run: active_collab.py setup add"
        )
        return 2

    target = instances
    if getattr(args, "instance", None):
        matches = [i for i in instances if i.name == args.instance]
        if not matches:
            known = ", ".join(i.name for i in instances)
            render.print_error(
                f"Error: instance '{args.instance}' not found. Known: {known}"
            )
            return 2
        target = matches

    all_tasks: list[dict] = []
    for inst in target:
        client = ActiveCollabClient(inst, http)
        for t in client.fetch_open_tasks():
            all_tasks.append(
                {
                    "instance": t.instance_name,
                    "project_id": t.project_id,
                    "task_number": t.task_number or t.id,
                    "task_id": t.id,
                    "name": t.name,
                }
            )

    if not all_tasks:
        print("No open tasks assigned to you.")
        return 0

    render.render_mine_table(all_tasks)
    return 0


def _add_display_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("--instance", help="Force a named instance.")
    p.add_argument("--short", action="store_true", help="Print PROJECT/TASK<TAB>name only.")
    p.add_argument("--no-comments", action="store_true", dest="no_comments")
    p.add_argument("--json", action="store_true", help="Print raw task JSON.")
    p.add_argument("--refresh", action="store_true", help="Ignore cache and re-fetch.")


def _add_get_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("ref", help="Task URL or PROJECT_ID/TASK_ID (e.g. 665/75159).")
    _add_display_args(p)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="active-collab",
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


_KNOWN_COMMANDS = ("setup", "get", "current", "mine", "list")

_COMMAND_HANDLERS = {
    "get": cmd_get,
    "current": cmd_current,
    "mine": cmd_mine,
    "list": cmd_mine,
}


def _normalize_argv(argv: list) -> list:
    """Expand bare-invocation shortcuts before argparse sees the argv list."""
    if argv and not argv[0].startswith("-") and argv[0] not in _KNOWN_COMMANDS:
        return ["get"] + argv
    if not argv:
        branch = _current_branch()
        if branch and _parse_branch_ref(branch):
            return ["current"]
    return argv


def main(argv: list | None = None) -> int:
    parser = _build_parser()

    if argv is None:
        argv = sys.argv[1:]

    argv = _normalize_argv(argv)
    args = parser.parse_args(argv)

    if args.command == "setup":
        return _dispatch_setup(args, parser)

    handler = _COMMAND_HANDLERS.get(args.command)
    if handler:
        return handler(args)

    parser.print_help()
    return 2
