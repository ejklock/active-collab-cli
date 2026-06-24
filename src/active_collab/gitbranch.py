import subprocess  # nosec B404
from dataclasses import dataclass
from enum import Enum
from typing import Callable

_VALID_TYPES = frozenset({"feature", "fix", "hotfix"})
_DEFAULT_TYPE = "feature"


class BranchStatus(Enum):
    created = "created"
    exists = "exists"
    not_a_repo = "not_a_repo"
    base_missing = "base_missing"
    error = "error"


@dataclass(frozen=True)
class BranchResult:
    status: BranchStatus
    name: str
    message: str = ""


def build_branch_name(
    branch_type: str | None, project_id: object, task_id: object
) -> str:
    """Return '<type>/<project_id>-<task_id>' for the given branch type.

    Defaults to 'feature' when branch_type is None or empty.
    Raises ValueError for unknown types.
    """
    resolved = (branch_type or "").strip() or _DEFAULT_TYPE
    if resolved not in _VALID_TYPES:
        raise ValueError(
            f"Unknown branch type {resolved!r}. "
            f"Must be one of: {sorted(_VALID_TYPES)}"
        )
    return f"{resolved}/{project_id}-{task_id}"


def _branch_exists(name: str, run: Callable) -> bool | None:
    """Check whether a local branch exists.

    Returns True (exists), False (does not exist),
    or None (not a git repo / error).
    """
    result = run(
        ["git", "rev-parse", "--verify", name],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return True
    stderr = (result.stderr or "").lower()
    if "not a git repository" in stderr:
        return None
    return False


def _resolve_base(
    base: str, run: Callable
) -> tuple[str | None, bool | None]:
    """Resolve the base branch ref, falling back master -> main.

    Returns (resolved_ref, is_not_a_repo):
      (ref, None)  — ref exists, proceed
      (None, None) — neither master nor main found (base_missing)
      (None, True) — not a git repository
    """
    candidates = [base] if base not in ("master", "main") else ["master", "main"]
    for candidate in candidates:
        result = run(
            ["git", "rev-parse", "--verify", candidate],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return candidate, None
        stderr = (result.stderr or "").lower()
        if "not a git repository" in stderr:
            return None, True
    return None, None


def create_branch(
    name: str,
    run: Callable = subprocess.run,
    base: str = "master",
) -> BranchResult:
    """Create a local git branch based on master (or main fallback).

    Checks branch existence with `git rev-parse --verify` first.
    Resolves master -> main when master absent; returns base_missing
    when neither exists. Uses `git checkout -b <name> <base>` so the
    new branch always starts at the resolved base ref, not at HEAD.
    Never overwrites (no -B/force).
    Returns a BranchResult — no exceptions for normal control flow.
    """
    exists = _branch_exists(name, run)

    if exists is None:
        return BranchResult(
            status=BranchStatus.not_a_repo,
            name=name,
            message="Not a git repository",
        )

    if exists:
        return BranchResult(
            status=BranchStatus.exists,
            name=name,
            message=f"Branch {name!r} already exists",
        )

    base_ref, not_a_repo = _resolve_base(base, run)

    if not_a_repo:
        return BranchResult(
            status=BranchStatus.not_a_repo,
            name=name,
            message="Not a git repository",
        )

    if base_ref is None:
        return BranchResult(
            status=BranchStatus.base_missing,
            name=name,
            message=(
                "Base branch not found: neither 'master' nor 'main' exists"
            ),
        )

    result = run(
        ["git", "checkout", "-b", name, base_ref],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return BranchResult(status=BranchStatus.created, name=name)

    return BranchResult(
        status=BranchStatus.error,
        name=name,
        message=(result.stderr or result.stdout or "").strip(),
    )
