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

Requires Python 3.10+. No third-party dependencies — stdlib only.

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
