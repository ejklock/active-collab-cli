"""Interactive TUI browser for ActiveCollab tasks.

Provides:
- Pure navigation helpers (clamp_index, move_selection)
- Pure text helpers (_truncate)
- Color initialization (_init_colors, _attr)
- BrowseController — dependency-injected business logic
- MineController — aggregates tasks across multiple instances
- run_browser — thin curses view loop (not unit-tested; needs a terminal)
- run_mine_browser — flat-list TUI for mine/list subcommand
- run — entry point called by cli.py's `browse` subcommand
- run_mine — entry point called by cli.py's `mine` subcommand when TTY
"""

import curses
import json
import subprocess  # nosec B404
import sys
import tempfile
import webbrowser
from collections import OrderedDict
from typing import Callable

from active_collab import assets as assets_mod
from active_collab import gitbranch
from active_collab import render as _render
from active_collab.assets import Asset, extract_asset_urls
from active_collab.client import ActiveCollabClient
from active_collab.config import Config
from active_collab.http import HttpClient
from active_collab.models import Instance, MineTask
from active_collab.render import render_task_to_str
from active_collab.store import InstanceRepository, Store

_BRANCH_TYPES = ("feature", "fix", "hotfix")
_DEFAULT_BRANCH_TYPE = "feature"

# Populated by _init_colors() at TUI startup; keys are role names.
_ATTR: dict[str, int] = {}


def _truncate(text: str, width: int) -> str:
    """Return text clipped to width characters.

    When len(text) <= width, returns text unchanged.
    When len(text) > width, returns text[:width-1] + '…' (a single ellipsis char).
    When width <= 0, returns ''.
    """
    if width <= 0:
        return ""
    if len(text) <= width:
        return text
    return text[: width - 1] + "…"


def _init_colors() -> None:
    """Initialize curses color pairs and cache attrs in _ATTR.

    Safe to call multiple times (idempotent). When the terminal does not
    support colors, returns without touching curses color state so callers
    fall back to the plain A_BOLD/A_REVERSE styling via _attr().
    """
    if not curses.has_colors():
        return
    curses.start_color()
    curses.use_default_colors()

    # Pair IDs: 1=header cyan, 2=selected (black on cyan), 3=status bar, 4=badge yellow
    curses.init_pair(1, curses.COLOR_CYAN, -1)
    curses.init_pair(2, curses.COLOR_BLACK, curses.COLOR_CYAN)
    curses.init_pair(3, curses.COLOR_WHITE, curses.COLOR_BLUE)
    curses.init_pair(4, curses.COLOR_YELLOW, -1)

    _ATTR["header"] = curses.color_pair(1) | curses.A_BOLD
    _ATTR["selected"] = curses.color_pair(2)
    _ATTR["status"] = curses.color_pair(3)
    _ATTR["badge"] = curses.color_pair(4)


def _attr(role: str, fallback: int) -> int:
    """Return the color attr for role, or fallback when colors are unavailable."""
    return _ATTR.get(role, fallback)


def clamp_index(index: int, length: int) -> int:
    """Return index clamped to [0, length-1]; 0 when length is 0."""
    if length == 0:
        return 0
    return max(0, min(index, length - 1))


def move_selection(index: int, delta: int, length: int) -> int:
    """Apply delta to index and clamp to [0, length-1]."""
    return clamp_index(index + delta, length)


class BrowseController:
    """All TUI business logic; fully dependency-injected for unit testing."""

    def __init__(
        self,
        client: ActiveCollabClient,
        http: HttpClient,
        instance: Instance,
        run: Callable = subprocess.run,
        opener: Callable = webbrowser.open,
        download_dir: str | None = None,
    ) -> None:
        self._client = client
        self._http = http
        self._instance = instance
        self._run = run
        self._opener = opener
        self._download_dir = download_dir or tempfile.gettempdir()

    def tasks_by_project(
        self,
    ) -> list[tuple[str, list[MineTask]]]:
        """Return tasks grouped by project, ordered by project name.

        Calls fetch_open_tasks and list_projects; falls back to the
        project_id string when a project name is unavailable.
        """
        open_tasks = self._client.fetch_open_tasks()
        project_map = self._build_project_name_map()

        grouped: OrderedDict[int, list[MineTask]] = OrderedDict()
        for task in open_tasks:
            pid = task.project_id or 0
            grouped.setdefault(pid, []).append(task)

        result: list[tuple[str, list[MineTask]]] = []
        for pid, task_list in grouped.items():
            name = project_map.get(pid) or str(pid)
            result.append((name, task_list))

        return result

    def _build_project_name_map(self) -> dict[int, str]:
        status, body = self._client.list_projects()
        if status != 200:
            return {}
        try:
            data = json.loads(body)
        except (ValueError, TypeError):
            return {}
        projects = data if isinstance(data, list) else []
        return {
            p["id"]: p.get("name") or str(p["id"])
            for p in projects
            if isinstance(p, dict) and p.get("id")
        }

    def task_detail(
        self, project_id: int, task_id: int
    ) -> tuple[dict, list, list[Asset]]:
        """Return (task_dict, comments, assets) for the given task."""
        status, payload = self._client.fetch_task(project_id, task_id)
        if status != 200 or not payload:
            return {}, [], []
        task = payload.get("single") or {}
        task["tracked_time"] = payload.get("tracked_time")
        comments = payload.get("comments") or []
        asset_list = extract_asset_urls(task, comments)
        return task, comments, asset_list

    def fetch_open_tasks(self) -> list[MineTask]:
        """Return open tasks from the underlying client."""
        return self._client.fetch_open_tasks()

    def create_task_branch(
        self,
        branch_type: str,
        project_id: int,
        task_id: int,
    ) -> gitbranch.BranchResult:
        """Build and create a git branch; delegates entirely to gitbranch."""
        name = gitbranch.build_branch_name(branch_type, project_id, task_id)
        return gitbranch.create_branch(name, run=self._run, base="master")

    def open_asset(self, asset: Asset) -> None:
        """Open the asset URL in the browser; never attaches the auth token."""
        self._opener(asset.url)

    def download_asset(self, asset: Asset, dest_dir: str | None = None) -> str:
        """Download the asset; attaches auth token only on host match."""
        directory = dest_dir or self._download_dir
        return assets_mod.download_asset(
            self._http,
            asset,
            self._instance.base_url,
            self._instance.token,
            directory,
        )


class MineController:
    """Aggregates open tasks from multiple instances for the mine TUI."""

    def __init__(
        self,
        instances: list[Instance],
        http: HttpClient,
        run: Callable = subprocess.run,
        opener: Callable = webbrowser.open,
        download_dir: str | None = None,
    ) -> None:
        self._controllers: dict[str, BrowseController] = {
            inst.name: BrowseController(
                client=ActiveCollabClient(inst, http),
                http=http,
                instance=inst,
                run=run,
                opener=opener,
                download_dir=download_dir,
            )
            for inst in instances
        }

    def my_tasks(self) -> list[MineTask]:
        """Return open tasks aggregated across all instances.

        Preserves instance insertion order, then task order within each instance.
        """
        result: list[MineTask] = []
        for ctrl in self._controllers.values():
            result.extend(ctrl.fetch_open_tasks())
        return result

    def controller_for(self, task: MineTask) -> BrowseController:
        """Return the BrowseController bound to the task's instance."""
        return self._controllers[task.instance_name]


def _render_list(
    stdscr: "curses._CursesWindow",
    items: list[str],
    sel: int,
    title: str,
) -> None:
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    stdscr.addstr(0, 0, _truncate(title, w - 1), _attr("header", curses.A_BOLD))
    stdscr.addstr(1, 0, _truncate("─" * w, w - 1))
    for i, item in enumerate(items):
        row = i + 2
        if row >= h - 1:
            break
        if i == sel:
            label = _truncate("▸ " + item, w - 1)
            stdscr.addstr(row, 0, label, _attr("selected", curses.A_REVERSE))
        else:
            label = _truncate("  " + item, w - 1)
            stdscr.addstr(row, 0, label)
    hint = "↑/↓ move  Enter select  q quit  b back"
    stdscr.addstr(h - 1, 0, _truncate(hint, w - 1), _attr("status", curses.A_NORMAL))
    stdscr.refresh()


def _render_detail(
    stdscr: "curses._CursesWindow",
    text: str,
    assets: list[Asset],
) -> None:
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if i >= h - 3:
            break
        stdscr.addstr(i, 0, _truncate(line, w - 1))
    asset_hint = f"  [{len(assets)} asset(s)]" if assets else ""
    hint = "q back  c create-branch  a assets" + asset_hint
    stdscr.addstr(h - 2, 0, _truncate(hint, w - 1), _attr("status", curses.A_NORMAL))
    stdscr.refresh()


def _choose_branch_type(stdscr: "curses._CursesWindow") -> str | None:
    """Show a branch-type picker; return chosen type or None on cancel."""
    items = list(_BRANCH_TYPES)
    sel = 0
    while True:
        _render_list(
            stdscr, items, sel,
            "Branch type (Enter to confirm, q cancel)",
        )
        key = stdscr.getch()
        if key in (curses.KEY_UP, ord("k")):
            sel = move_selection(sel, -1, len(items))
        elif key in (curses.KEY_DOWN, ord("j")):
            sel = move_selection(sel, 1, len(items))
        elif key in (curses.KEY_ENTER, 10, 13):
            return items[sel]
        elif key in (ord("q"), ord("b"), 27):
            return None


def _asset_menu(
    stdscr: "curses._CursesWindow",
    assets: list[Asset],
    controller: BrowseController,
) -> None:
    """Show asset list; handle open/download per item."""
    if not assets:
        return
    sel = 0
    while True:
        labels = [f"[{a.kind}] {a.name}" for a in assets]
        _render_list(
            stdscr, labels, sel,
            "Assets (o open  d download  q back)",
        )
        key = stdscr.getch()
        if key in (curses.KEY_UP, ord("k")):
            sel = move_selection(sel, -1, len(assets))
        elif key in (curses.KEY_DOWN, ord("j")):
            sel = move_selection(sel, 1, len(assets))
        elif key == ord("o"):
            controller.open_asset(assets[sel])
        elif key == ord("d"):
            _handle_download(stdscr, controller, assets[sel])
        elif key in (ord("q"), ord("b"), 27):
            return


def _handle_download(
    stdscr: "curses._CursesWindow",
    controller: BrowseController,
    asset: Asset,
) -> None:
    """Attempt to download asset and show the result or error message."""
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    try:
        path = controller.download_asset(asset)
        stdscr.addstr(0, 0, _truncate(f"Downloaded: {path}", w - 1))
    except RuntimeError as exc:
        stdscr.addstr(0, 0, _truncate(f"Error: {exc}", w - 1))
    stdscr.addstr(1, 0, "Press any key...")
    stdscr.refresh()
    stdscr.getch()


def _show_branch_result(
    stdscr: "curses._CursesWindow",
    result: gitbranch.BranchResult,
) -> None:
    stdscr.erase()
    h, w = stdscr.getmaxyx()
    msg = f"{result.status.value}: {result.name}"
    if result.message:
        msg += f" — {result.message}"
    stdscr.addstr(0, 0, _truncate(msg, w - 1))
    stdscr.addstr(1, 0, "Press any key...")
    stdscr.refresh()
    stdscr.getch()


def _screen_projects(
    stdscr: "curses._CursesWindow",
    project_names: list[str],
    groups: list,
    proj_sel: int,
    task_sel: int,
) -> tuple[int, int, str | None]:
    """Render + handle one keypress on the projects screen.

    Returns (proj_sel, task_sel, next_screen) where next_screen is
    None to signal quit.
    """
    _render_list(stdscr, project_names, proj_sel, "Projects")
    key = stdscr.getch()
    if key in (curses.KEY_UP, ord("k")):
        proj_sel = move_selection(proj_sel, -1, len(project_names))
    elif key in (curses.KEY_DOWN, ord("j")):
        proj_sel = move_selection(proj_sel, 1, len(project_names))
    elif key in (curses.KEY_ENTER, 10, 13) and groups:
        task_sel = 0
        return proj_sel, task_sel, "tasks"
    elif key in (ord("q"), 27):
        return proj_sel, task_sel, None
    return proj_sel, task_sel, "projects"


def _screen_tasks(
    stdscr: "curses._CursesWindow",
    groups: list,
    proj_sel: int,
    task_sel: int,
) -> tuple[int, str | None]:
    """Render + handle one keypress on the tasks screen.

    Returns (task_sel, next_screen) where next_screen is None to
    signal quit.
    """
    _, task_list = groups[proj_sel]
    task_labels = [
        f"#{t.task_number or t.id}  {t.name}" for t in task_list
    ]
    _render_list(stdscr, task_labels, task_sel, "Tasks")
    key = stdscr.getch()
    if key in (curses.KEY_UP, ord("k")):
        task_sel = move_selection(task_sel, -1, len(task_list))
    elif key in (curses.KEY_DOWN, ord("j")):
        task_sel = move_selection(task_sel, 1, len(task_list))
    elif key in (curses.KEY_ENTER, 10, 13) and task_list:
        return task_sel, "detail"
    elif key in (ord("b"), 27):
        return task_sel, "projects"
    elif key in (ord("q"),):
        return task_sel, None
    return task_sel, "tasks"


def _render_and_handle_detail(
    stdscr: "curses._CursesWindow",
    controller: BrowseController,
    project_id: int,
    task: MineTask,
) -> None:
    """Render task detail and handle keypresses in a loop until the user exits.

    Handles: q/b/Esc -> return; c -> branch type picker + create; a -> asset menu.
    """
    task_dict, comments, asset_list = controller.task_detail(project_id, task.id)
    user_map: dict = {}
    detail_text = render_task_to_str(task_dict, comments, False, user_map)
    while True:
        _render_detail(stdscr, detail_text, asset_list)
        key = stdscr.getch()
        if key in (ord("q"), ord("b"), 27):
            return
        if key == ord("c"):
            chosen = _choose_branch_type(stdscr)
            if chosen:
                result = controller.create_task_branch(chosen, project_id, task.id)
                _show_branch_result(stdscr, result)
        elif key == ord("a"):
            _asset_menu(stdscr, asset_list, controller)


def _screen_detail(
    stdscr: "curses._CursesWindow",
    groups: list,
    proj_sel: int,
    task_sel: int,
    controller: BrowseController,
) -> str:
    """Render + handle detail screen for the project-browse flow.

    Returns the next screen name (always a string; no quit from detail).
    Delegates detail rendering and key handling to _render_and_handle_detail,
    then signals the browse loop to return to the tasks screen.
    """
    _, task_list = groups[proj_sel]
    task = task_list[task_sel]
    project_id = task.project_id or 0
    _render_and_handle_detail(stdscr, controller, project_id, task)
    return "tasks"


def run_browser(
    stdscr: "curses._CursesWindow",
    controller: BrowseController,
) -> None:
    """Thin curses view loop; all business logic lives in controller."""
    curses.curs_set(0)
    _init_colors()

    groups = controller.tasks_by_project()
    project_names = [name for name, _ in groups]

    proj_sel = 0
    task_sel = 0
    screen: str | None = "projects"

    while screen is not None:
        if screen == "projects":
            proj_sel, task_sel, screen = _screen_projects(
                stdscr, project_names, groups, proj_sel, task_sel
            )
        elif screen == "tasks":
            task_sel, screen = _screen_tasks(
                stdscr, groups, proj_sel, task_sel
            )
        elif screen == "detail":
            screen = _screen_detail(
                stdscr, groups, proj_sel, task_sel, controller
            )


def _screen_mine_list(
    stdscr: "curses._CursesWindow",
    tasks: list[MineTask],
    sel: int,
) -> tuple[int, str | None]:
    """Render the flat mine-task list and handle one keypress.

    Returns (sel, action) where action is 'detail', None (quit), or 'list' (stay).
    """
    labels = [
        f"[{t.instance_name}] #{t.task_number or t.id}  {t.name}"
        for t in tasks
    ]
    _render_list(stdscr, labels, sel, "My Open Tasks")
    key = stdscr.getch()
    if key in (curses.KEY_UP, ord("k")):
        return move_selection(sel, -1, len(tasks)), "list"
    if key in (curses.KEY_DOWN, ord("j")):
        return move_selection(sel, 1, len(tasks)), "list"
    if key in (curses.KEY_ENTER, 10, 13) and tasks:
        return sel, "detail"
    if key in (ord("q"), 27):
        return sel, None
    return sel, "list"


def run_mine_browser(
    stdscr: "curses._CursesWindow",
    mine_controller: MineController,
) -> None:
    """Curses view loop for the mine flat-list TUI."""
    curses.curs_set(0)
    _init_colors()

    tasks = mine_controller.my_tasks()
    if not tasks:
        stdscr.erase()
        h, w = stdscr.getmaxyx()
        msg = "No open tasks assigned to you."
        stdscr.addstr(0, 0, _truncate(msg, w - 1))
        stdscr.addstr(1, 0, "Press any key to exit...")
        stdscr.refresh()
        stdscr.getch()
        return

    sel = 0
    screen: str | None = "list"
    while screen is not None:
        if screen == "list":
            sel, screen = _screen_mine_list(stdscr, tasks, sel)
        elif screen == "detail":
            task = tasks[sel]
            controller = mine_controller.controller_for(task)
            _render_and_handle_detail(stdscr, controller, task.project_id or 0, task)
            screen = "list"


def run_mine(
    instances: list[Instance],
    http: HttpClient,
    run: Callable = subprocess.run,
    opener: Callable = webbrowser.open,
    download_dir: str | None = None,
) -> int:
    """Launch the interactive mine TUI. Caller must have verified TTY."""
    controller = MineController(instances, http, run=run, opener=opener, download_dir=download_dir)
    curses.wrapper(run_mine_browser, controller)
    return 0


def _resolve_browse_instance(
    instances: list[Instance],
    instance_name: str | None,
) -> tuple[Instance | None, str | None]:
    """Resolve which instance to use for the browse command.

    Returns (instance, None) on success, or (None, error_message) on failure.
    Failure cases: no instances configured, unknown name, multiple instances
    with no name given.
    """
    if not instances:
        return None, "Error: no instances configured. Run: active-collab setup add"
    if instance_name:
        return _resolve_named_instance(instances, instance_name)
    return _resolve_implicit_instance(instances)


def _resolve_named_instance(
    instances: list[Instance],
    name: str,
) -> tuple[Instance | None, str | None]:
    matches = [i for i in instances if i.name == name]
    if not matches:
        known = ", ".join(i.name for i in instances)
        return None, f"Error: instance '{name}' not found. Known: {known}"
    return matches[0], None


def _resolve_implicit_instance(
    instances: list[Instance],
) -> tuple[Instance | None, str | None]:
    if len(instances) == 1:
        return instances[0], None
    names = ", ".join(i.name for i in instances)
    return None, f"Error: multiple instances ({names}). Use --instance NAME."


def run(args: object) -> int:
    """Entry point for the `browse` subcommand; builds deps and launches TUI."""
    if not (sys.stdin.isatty() and sys.stdout.isatty()):
        _render.print_error(
            "Error: 'browse' requires an interactive terminal (TTY)."
        )
        return 2

    config = Config.load()
    store = Store(config)
    repo = InstanceRepository(store.conn)
    instances = repo.load_all()

    inst, error = _resolve_browse_instance(instances, getattr(args, "instance", None))
    if error:
        _render.print_error(error)
        return 2

    http = HttpClient()
    client = ActiveCollabClient(inst, http)
    controller = BrowseController(
        client=client,
        http=http,
        instance=inst,
    )
    curses.wrapper(run_browser, controller)
    return 0
