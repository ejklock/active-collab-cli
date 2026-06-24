# active-collab-cli

Command-line tool for fetching ActiveCollab tasks from self-hosted instances. Supports
multi-instance configuration, SQLite-backed token storage, and outputs human-readable or
JSON task views.

---

## Install

### macOS / Linux (curl one-liner)

```sh
curl -fsSL https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.sh | sh
```

### Windows (PowerShell one-liner)

```powershell
irm https://raw.githubusercontent.com/ejklock/active-collab-cli/main/install.ps1 | iex
```

### Manual download

Download the pre-built binary for your platform from the
[Releases page](https://github.com/ejklock/active-collab-cli/releases), place it on your
PATH, and make it executable (`chmod +x active-collab` on Unix).

| Platform | Asset |
|---|---|
| Linux x86\_64 | `active-collab-linux-x86_64` |
| macOS x86\_64 (Intel) | `active-collab-macos-x86_64` |
| macOS arm64 (Apple Silicon) | `active-collab-macos-arm64` |
| Windows x86\_64 | `active-collab-windows-x86_64.exe` |

### From source

```sh
pip install .
```

Requires Python 3.10+. Runtime is stdlib-only on macOS and Linux (`curses` ships
with Python). On Windows, the `browse` TUI pulls in `windows-curses` — installed
automatically as a platform-conditional dependency.

---

## Usage

### Setup — manage instances

```sh
# Register an ActiveCollab instance (interactive wizard prompts for missing fields)
active-collab setup add
active-collab setup add --name collab --url https://collab.example.com --email me@example.com
# Password is always entered hidden via a prompt — never passed as a flag.

# List configured instances (tokens never shown)
active-collab setup list

# Remove an instance and its cached tasks
active-collab setup remove --name collab

# Test connectivity to all (or one) configured instance
active-collab setup test
active-collab setup test --name collab
```

### get — fetch a task by URL or short form

```sh
active-collab get 665/75159
active-collab get https://collab.example.com/projects/665/tasks/75159
```

### current — fetch the task from the current git branch

Branch must match `(feature|hotfix|fix)/PROJECT_ID-TASK_ID` (e.g. `feature/665-75159`).

```sh
active-collab current
```

### mine — list open tasks assigned to you

```sh
active-collab mine
active-collab list          # alias
```

When run in a terminal (TTY), `mine` opens an interactive arrow-key list of your
open tasks aggregated across all configured instances. Select a task to view its
detail, create a git branch, or open/download its assets — the same actions
available in `browse`. When output is piped or redirected (non-TTY), `mine` falls
back to a plain table suitable for scripts.

### browse — interactive TUI

Arrow-key terminal browser for your open tasks. Navigate projects → tasks →
task detail, then create a git branch or open/download the task's assets.

```sh
active-collab browse
active-collab browse --instance collab   # required when >1 instance configured
```

The TUI uses color where the terminal supports it (cyan header, cyan/reverse
selection highlight, styled status bar). On terminals without color support it
falls back to bold/reverse styling automatically.

The task detail view renders in a rounded frame with the task number and name
embedded in the top border. Each comment appears in its own rounded sub-box with
the author and date in the box's top border. The detail view scrolls vertically
when the content exceeds the screen. The whole TUI is responsive: it adapts to
terminal resize events and guards against too-small terminals without crashing.

**Key bindings**

| Screen | Keys |
|---|---|
| Lists (projects / tasks / assets) | `↑`/`↓` or `k`/`j` move · `Enter` select · `q` quit · `b` back |
| Task detail | `↑`/`↓` or `k`/`j` scroll · `PgUp`/`PgDn` page · `c` create branch · `a` assets · `q`/`b` back |
| Branch-type picker | `feature` / `fix` / `hotfix` (default `feature`) |
| Assets | `o` open in browser · `d` download · `q`/`b` back |

- **Create branch** — names the branch `<type>/<project_id>-<task_id>` (e.g.
  `feature/665-75159`, compatible with `current`), branched off `master`. It
  never overwrites an existing branch.
- **Assets** — image, attachment, and link URLs extracted from the task body,
  comments, and attachments. `o` opens the URL in your browser; `d` downloads
  it. The `X-Angie-AuthApiToken` header is attached **only** when the asset
  URL's scheme and host match the configured instance — foreign hosts are
  fetched without credentials.

### Bare-invocation shortcuts

```sh
active-collab 665/75159     # same as: active-collab get 665/75159
active-collab               # same as: active-collab current (when branch matches)
```

### Flags

| Flag | Applies to | Effect |
|---|---|---|
| `--instance NAME` | `get`, `current`, `mine` | Force a specific configured instance (required when >1 configured) |
| `--short` | `get`, `current` | Print `PROJECT/TASK<TAB>name` only; does not call the users API |
| `--no-comments` | `get`, `current` | Omit the comments section |
| `--json` | `get`, `current` | Print raw task JSON (always hits the API, bypasses cache) |
| `--refresh` | `get`, `current` | Bypass the task cache and re-fetch from the API |

---

## Configuration

**Database path:** `~/.config/active-collab/active-collab.db`

Override with the `ACTIVE_COLLAB_DB` environment variable:

```sh
ACTIVE_COLLAB_DB=/custom/path/active-collab.db active-collab get 665/75159
```

---

## Security

- The API token is stored only in the local SQLite database with directory permissions
  `0700` and file permissions `0600`.
- The token is transmitted exclusively via the `X-Angie-AuthApiToken` HTTP header — never
  in a URL, never printed, never passed as a process argument.
- The password is **never stored**. Only the token returned from the issue-token endpoint
  is persisted.

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Task not found / HTTP error / parse error |
| 2 | Usage error, unknown instance, no instances configured, branch mismatch |

---

## Building from source

```sh
pip install pyinstaller
pyinstaller --onefile --name active-collab _entry.py
# Binary is placed in dist/active-collab
```

Where `_entry.py` contains:

```python
from active_collab.cli import main
if __name__ == '__main__':
    raise SystemExit(main())
```

Pre-built binaries for all platforms are produced automatically by the
`.github/workflows/release.yml` GitHub Actions workflow and attached to each tagged
release.
