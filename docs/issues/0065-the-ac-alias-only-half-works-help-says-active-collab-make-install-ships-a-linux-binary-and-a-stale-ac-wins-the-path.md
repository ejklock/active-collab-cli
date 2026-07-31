---
type: Issue
title: "The ac alias only half works: help says active-collab, make install ships a Linux binary, and a stale ac wins the PATH"
description: <One sentence — the change and its motivation.>
status: Proposed
timestamp: 2026-07-31T14:59:12Z
---

<!-- OKF frontmatter above carries the tracker metadata (`status`: open | in-progress |
     closed | superseded) that previously lived only in the directory index. Everything
     BELOW the closing `---` is the issue body and MUST stay byte-identical to the
     published tracker body — strip the frontmatter when publishing. -->

## The `ac` alias only half works

Follow-up to [issue 0064](/issues/0064-installers-drop-the-short-ac-command-restore-it-as-an-alias-next-to-active-collab.md),
which put `ac` back next to `active-collab`. Dogfooding the release surfaced three
distinct defects around that alias:

1. **Help and version ignore the name the user typed.** `src/cli.rs` pins
   `#[command(name = "active-collab")]` and `main.rs` hands clap a hardcoded
   `argv[0]`, so `ac --help` prints `Usage: active-collab [COMMAND]` and
   `ac --version` prints `active-collab 0.7.1`. The README and the embedded agent
   skill speak `ac`; the binary contradicts them at every invocation.
2. **`make install` installs an unrunnable binary on a macOS host.** The `binary`
   target builds inside Docker (Linux ELF) and `install` copies it to `$(BINDIR)/ac`.
   On macOS the result dies with `exec format error`. The Makefile already carries a
   comment saying `install-native` exists for this — it just never enforced it.
   Observed in the wild: a `~/.local/bin/ac` from an earlier `make install` shadowed
   the working release binary.
3. **A pre-existing `ac` silently wins the PATH.** Two flavors: a stale copy of this
   CLI (which 0064's guard deliberately refuses to touch, so the upgrade never lands),
   and macOS's own `/usr/sbin/ac` login-accounting utility, which takes over whenever
   the install dir is later in PATH — printing a bare `total 4537.67`.

### Scope

Included:

- `src/cli.rs` + `src/main.rs` — a pure `invoked_name(argv0)` helper plus
  `command_as(program)`, so clap renders help, usage, errors, and `--version` under
  the invoked basename (`.exe` suffix stripped), falling back to `active-collab`.
- `Cargo.toml` — clap's `string` feature, required to set a command name at runtime.
- `Makefile` — an `host-runs-docker-binary` guard that fails `make install` on a
  non-Linux host before spending a release build, pointing at `install-native` and the
  release installer.
- `install.sh` — replace an existing plain `ac` when it identifies itself as this CLI
  (so upgrades land); keep the refusal for anything else, and say so explicitly when
  the file cannot exec on this host. After linking, warn when `ac` still resolves
  elsewhere on PATH.
- `install.ps1` — the same PATH-shadow warning.

Excluded: renaming the crate, the release asset names, or the `[[bin]]` target; the
`active_collab.py` string still present in one i18n message (pre-existing, tracked
separately).

### Acceptance

- AC1 — invoked as `ac`, the binary prints `Usage: ac …` and `ac <version>`; invoked
  as `active-collab`, it prints the long name. (`verify_by: test`)
- AC2 — `invoked_name` strips directories and the `.exe` suffix and falls back to
  `active-collab` for absent/empty/rootless `argv[0]`. (`verify_by: test`)
- AC3 — `make install` on a non-Linux host exits non-zero with the `install-native`
  pointer, and does not build or copy anything. (`verify_by: command`)
- AC4 — `install.sh` replaces a plain `ac` that reports itself as this CLI, and leaves
  a foreign or unrunnable `ac` in place with a warning. (`verify_by: command`)
- AC5 — after linking, `install.sh` warns when `command -v ac` resolves to a different
  path. (`verify_by: command`)
- CC — no superfluous comments; the added why-comments carry non-obvious rationale
  only. (`verify_by: inspection`)

### Plan

1. Add `invoked_name` + `command_as` to `cli.rs`; thread the program name through
   `run()` and the bare-invocation help path in `main.rs`; enable clap's `string`
   feature; unit-test the helper and the rendered usage.
2. Add the Makefile host guard as a fail-fast prerequisite of `install`.
3. Teach `install.sh` the upgrade + shadow checks; mirror the shadow check in
   `install.ps1`.
4. README + CHANGELOG; release as `0.7.2`.
