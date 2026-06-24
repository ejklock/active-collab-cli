import datetime
import html as html_lib
import re
import sys

_BLOCK_TAG_PATTERN = re.compile(r"<(?:br|p|div|li|tr|h[1-6])\b[^>]*>", re.IGNORECASE)
_TAG_PATTERN = re.compile(r"<[^>]+>")
_BLANK_LINES_PATTERN = re.compile(r"\n{3,}")


def html_to_text(html: str) -> str:
    """Strip HTML tags and unescape entities. Block/br tags become newlines."""
    if not html:
        return ""
    text = _BLOCK_TAG_PATTERN.sub("\n", html)
    text = _TAG_PATTERN.sub("", text)
    text = html_lib.unescape(text)
    text = _BLANK_LINES_PATTERN.sub("\n\n", text)
    return text.strip()


def fmt_ts(value: object) -> str:
    """Format a unix int/float timestamp as 'YYYY-MM-DD HH:MM' UTC. Returns '' for None."""
    if value is None:
        return ""
    if isinstance(value, (int, float)):
        dt = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc)
        return dt.strftime("%Y-%m-%d %H:%M")
    return str(value)


def fmt_date(value: object) -> str:
    """Format a unix int/float timestamp as 'YYYY-MM-DD' UTC. Returns '' for None."""
    if value is None:
        return ""
    if isinstance(value, (int, float)):
        dt = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc)
        return dt.strftime("%Y-%m-%d")
    return str(value)


def fmt_hours(value: object) -> str:
    """Format an hours value as an integer string when whole, fractional otherwise."""
    if value is None:
        return "0"
    try:
        f = float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return str(value)
    return str(int(f)) if f == int(f) else str(f)


def render_comments(comments: list) -> None:
    """Print the comments section for a task view."""
    if not comments:
        return
    print(f"\nComments ({len(comments)}):")
    for idx, c in enumerate(comments, 1):
        author = c.get("created_by_name") or c.get("created_by_id") or "(unknown)"
        created = fmt_ts(c.get("created_on"))
        body_text = c.get("body_plain_text") or html_to_text(c.get("body") or "")
        print(f"\n  [{idx}] {author} — {created}")
        for line in body_text.splitlines():
            print(f"  {line}")


def render_meta(task: dict, user_map: dict) -> None:
    """Print assignee, dates, estimate, and logged hours for the task view."""
    assignee_id = task.get("assignee_id")
    if assignee_id is None:
        assignee_label = "(unassigned)"
    else:
        name = user_map.get(assignee_id)
        assignee_label = f"{name} ({assignee_id})" if name else f"({assignee_id})"
    print(f"Assignee:  {assignee_label}")

    start = fmt_date(task.get("start_on"))
    if start:
        print(f"Start:     {start}")

    due = fmt_date(task.get("due_on"))
    if due:
        print(f"Due:       {due}")

    print(f"Estimate:  {fmt_hours(task.get('estimate'))}h")
    print(f"Logged:    {fmt_hours(task.get('tracked_time'))}h")


def render_task(task: dict, comments: list, no_comments: bool, user_map: dict) -> None:
    """Print a human-readable task view."""
    print(f"Task:      {task.get('task_number') or task.get('id')}")
    print(f"Name:      {task.get('name', '')}")
    status_label = "Completed" if task.get("is_completed") else "Open"
    print(f"Status:    {status_label}")
    render_meta(task, user_map)
    print()
    body = html_to_text(task.get("body") or "") or "(no description)"
    print("Description:")
    print(body)

    if no_comments:
        return

    render_comments(comments)


def render_mine_table(tasks: list) -> None:
    """Print the mine/list subcommand table to stdout."""
    print(f"{'INSTANCE':<15} {'PROJECT':<10} {'TASK#':<8} {'TASK_ID':<10} NAME")
    print("-" * 80)
    for t in tasks:
        print(
            f"{t['instance']:<15} {t['project_id']:<10} "
            f"{t['task_number']:<8} {t['task_id']:<10} {t['name']}"
        )


def print_error(message: str) -> None:
    """Print a message to stderr."""
    print(message, file=sys.stderr)
