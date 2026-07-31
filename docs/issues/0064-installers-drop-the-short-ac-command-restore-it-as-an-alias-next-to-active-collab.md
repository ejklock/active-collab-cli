---
type: Issue
title: Installers drop the short ac command — restore it as an alias next to active-collab
description: <One sentence — the change and its motivation.>
status: Proposed
timestamp: 2026-07-31T14:07:35Z
---

<!-- OKF frontmatter above carries the tracker metadata (`status`: open | in-progress |
     closed | superseded) that previously lived only in the directory index. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## Installers drop the short `ac` command

The release installers name the downloaded binary `active-collab` (`install.sh`
`BIN_NAME`, `install.ps1` `$Dest`), so a user who installs from a GitHub Release
has no `ac` on PATH. Every other surface still speaks `ac`: the crate's `[[bin]]`
target, `make install`, the Docker `ENTRYPOINT`, the README quickstart, and the
embedded agent skill (`ac get`, `ac mine`, `ac current`), which instructs agents
to invoke `ac`. Following the README verbatim after a curl install yields
`command not found: ac`.

Both names must exist after an install: `active-collab` as the descriptive
primary, `ac` as the short alias the docs and the skill depend on.

### Scope

Included:

- `install.sh` — after installing `active-collab`, create an `ac` symlink beside
  it, refusing to clobber a pre-existing non-symlink `ac` on PATH.
- `install.ps1` — write an `ac.cmd` shim next to `active-collab.exe` (Windows
  symlinks need elevation; a `.cmd` forwarder works from cmd and PowerShell).
- `Makefile` — `install`/`install-native` also link `active-collab` → `ac`, and
  `uninstall` removes both, so the source path exposes the same two names.
- `README.md` — document that both commands are installed.

Excluded: renaming the crate, the `[[bin]]` target, or the release assets;
changing the CLI's own help name; any behavior change inside the binary.

### Acceptance

- AC1 — after `install.sh`, both `active-collab` and `ac` resolve on PATH, `ac`
  being a symlink to `active-collab` in the same directory. (`verify_by: command`)
- AC2 — `install.sh` leaves a pre-existing regular file named `ac` untouched and
  warns instead of overwriting it. (`verify_by: inspection`)
- AC3 — after `install.ps1`, `ac` resolves to the installed `active-collab.exe`
  via an `ac.cmd` shim in the same directory. (`verify_by: inspection`)
- AC4 — `make install` exposes both `ac` and `active-collab`; `make uninstall`
  removes both. (`verify_by: inspection`)
- AC5 — the README install section states both command names. (`verify_by: inspection`)

### Plan

1. Add the alias link step to `install.sh`, guarded against clobbering.
2. Add the `ac.cmd` shim to `install.ps1`.
3. Mirror the alias in the `Makefile` install/uninstall targets.
4. Update the README install section and the CHANGELOG; release as `0.7.1`.
