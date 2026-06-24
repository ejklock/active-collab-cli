import datetime
import html as html_lib
import re
import sys

_BLOCK_TAG_PATTERN = re.compile(
    r"<(?:br|p|div|li|tr|h[1-6])\b[^>]*>", re.IGNORECASE
)
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
    """Format a unix int/float timestamp as 'YYYY-MM-DD HH:MM' UTC.

    Returns '' for None.
    """
    if value is None:
        return ""
    if isinstance(value, (int, float)):
        dt = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc)
        return dt.strftime("%Y-%m-%d %H:%M")
    return str(value)


def fmt_date(value: object) -> str:
    """Format a unix int/float timestamp as 'YYYY-MM-DD' UTC.

    Returns '' for None.
    """
    if value is None:
        return ""
    if isinstance(value, (int, float)):
        dt = datetime.datetime.fromtimestamp(value, tz=datetime.timezone.utc)
        return dt.strftime("%Y-%m-%d")
    return str(value)


def fmt_hours(value: object) -> str:
    """Format an hours value as integer when whole, fractional otherwise."""
    if value is None:
        return "0"
    try:
        f = float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return str(value)
    return str(int(f)) if f == int(f) else str(f)


def render_comments_to_str(comments: list) -> str:
    """Return the comments section for a task view as a string."""
    if not comments:
        return ""
    lines = [f"\nComments ({len(comments)}):"]
    for idx, c in enumerate(comments, 1):
        author = (
            c.get("created_by_name") or c.get("created_by_id") or "(unknown)"
        )
        created = fmt_ts(c.get("created_on"))
        body_text = (
            c.get("body_plain_text") or html_to_text(c.get("body") or "")
        )
        lines.append(f"\n  [{idx}] {author} — {created}")
        for line in body_text.splitlines():
            lines.append(f"  {line}")
    return "\n".join(lines)


def render_meta_to_str(task: dict, user_map: dict) -> str:
    """Return assignee, dates, estimate, and logged hours as a string."""
    assignee_id = task.get("assignee_id")
    if assignee_id is None:
        assignee_label = "(unassigned)"
    else:
        name = user_map.get(assignee_id)
        assignee_label = (
            f"{name} ({assignee_id})" if name else f"({assignee_id})"
        )

    lines = [f"Assignee:  {assignee_label}"]

    start = fmt_date(task.get("start_on"))
    if start:
        lines.append(f"Start:     {start}")

    due = fmt_date(task.get("due_on"))
    if due:
        lines.append(f"Due:       {due}")

    lines.append(f"Estimate:  {fmt_hours(task.get('estimate'))}h")
    lines.append(f"Logged:    {fmt_hours(task.get('tracked_time'))}h")
    return "\n".join(lines)


def render_task_to_str(
    task: dict, comments: list, no_comments: bool, user_map: dict
) -> str:
    """Return a human-readable task view as a string."""
    lines = [
        f"Task:      {task.get('task_number') or task.get('id')}",
        f"Name:      {task.get('name', '')}",
        f"Status:    {'Completed' if task.get('is_completed') else 'Open'}",
        render_meta_to_str(task, user_map),
        "",
        "Description:",
        html_to_text(task.get("body") or "") or "(no description)",
    ]

    if not no_comments:
        comments_str = render_comments_to_str(comments)
        if comments_str:
            lines.append(comments_str)

    return "\n".join(lines)


def render_comments(comments: list) -> None:
    """Print the comments section for a task view."""
    print(render_comments_to_str(comments), end="")


def render_meta(task: dict, user_map: dict) -> None:
    """Print assignee, dates, estimate, and logged hours for the task view."""
    print(render_meta_to_str(task, user_map))


def render_task(
    task: dict, comments: list, no_comments: bool, user_map: dict
) -> None:
    """Print a human-readable task view."""
    print(render_task_to_str(task, comments, no_comments, user_map))


def render_mine_table(tasks: list) -> None:
    """Print the mine/list subcommand table to stdout."""
    print(
        f"{'INSTANCE':<15} {'PROJECT':<10} {'TASK#':<8} {'TASK_ID':<10} NAME"
    )
    print("-" * 80)
    for t in tasks:
        print(
            f"{t['instance']:<15} {t['project_id']:<10} "
            f"{t['task_number']:<8} {t['task_id']:<10} {t['name']}"
        )


def print_error(message: str) -> None:
    """Print a message to stderr."""
    print(message, file=sys.stderr)
