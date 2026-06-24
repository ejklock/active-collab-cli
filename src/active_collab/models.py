from dataclasses import dataclass, field


@dataclass
class Instance:
    """A configured ActiveCollab server connection."""

    name: str
    base_url: str
    email: str
    token: str
    user_id: int | None = None

    @classmethod
    def from_row(cls, row: tuple) -> "Instance":
        """Construct from a DB row (name, base_url, email, token, user_id)."""
        name, base_url, email, token, user_id = row
        return cls(name=name, base_url=base_url, email=email, token=token, user_id=user_id)


@dataclass
class Task:
    """A single ActiveCollab task, unwrapped from the API `single` key."""

    id: int
    name: str
    task_number: int | None = None
    is_completed: bool = False
    is_trashed: bool = False
    assignee_id: int | None = None
    project_id: int | None = None
    body: str = ""
    start_on: int | float | None = None
    due_on: int | float | None = None
    estimate: float | None = None
    tracked_time: float | None = None

    @classmethod
    def from_api(cls, single: dict, tracked_time: float | None = None) -> "Task":
        """Parse the `single` dict from the task endpoint payload.

        Missing optional fields default to safe values; no KeyError is raised.
        tracked_time comes from the top-level payload, not from `single`.
        """
        return cls(
            id=single.get("id", 0),
            name=single.get("name", ""),
            task_number=single.get("task_number"),
            is_completed=bool(single.get("is_completed", False)),
            is_trashed=bool(single.get("is_trashed", False)),
            assignee_id=single.get("assignee_id"),
            project_id=single.get("project_id"),
            body=single.get("body") or "",
            start_on=single.get("start_on"),
            due_on=single.get("due_on"),
            estimate=single.get("estimate"),
            tracked_time=tracked_time,
        )


@dataclass
class Comment:
    """A comment on an ActiveCollab task."""

    id: int
    body: str = ""
    body_plain_text: str = ""
    created_by_name: str = ""
    created_by_id: int | None = None
    created_on: int | float | None = None

    @classmethod
    def from_api(cls, data: dict) -> "Comment":
        """Parse a comment dict from the task payload comments list."""
        return cls(
            id=data.get("id", 0),
            body=data.get("body") or "",
            body_plain_text=data.get("body_plain_text") or "",
            created_by_name=data.get("created_by_name") or "",
            created_by_id=data.get("created_by_id"),
            created_on=data.get("created_on"),
        )


@dataclass
class Project:
    """An ActiveCollab project from the projects list endpoint."""

    id: int
    name: str = ""
    is_trashed: bool = False

    @classmethod
    def from_api(cls, data: dict) -> "Project":
        """Parse a project dict from the projects list payload."""
        return cls(
            id=data.get("id", 0),
            name=data.get("name") or "",
            is_trashed=bool(data.get("is_trashed", False)),
        )


@dataclass
class MineTask:
    """A lightweight task entry from the /users/{id}/tasks endpoint."""

    id: int
    task_number: int | None = None
    name: str = ""
    is_completed: bool = False
    is_trashed: bool = False
    project_id: int | None = None
    instance_name: str = ""
    extra_fields: dict = field(default_factory=dict)

    @classmethod
    def from_api(cls, data: dict, instance_name: str = "") -> "MineTask":
        """Parse a task dict from the mine endpoint payload."""
        return cls(
            id=data.get("id", 0),
            task_number=data.get("task_number"),
            name=data.get("name") or "",
            is_completed=bool(data.get("is_completed", False)),
            is_trashed=bool(data.get("is_trashed", False)),
            project_id=data.get("project_id"),
            instance_name=instance_name,
        )
