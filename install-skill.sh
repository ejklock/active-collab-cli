#!/bin/sh
set -eu

usage() {
  cat <<'EOF'
Usage: install-skill.sh --harness <name> [--scope project|global] [--dir <path>] [--force]
       install-skill.sh -h | --help

Installs the ac-json agent-skill thin-pointer stub for one or more agent
harnesses. Each stub only tells the agent to run `ac skill ac-json` to load
the full ActiveCollab --json read contract from the CLI; it carries no
--json schema fields, so it can never drift from the contract.

Options:
  --harness <name>   Harness to install for. One of:
                      claude, codex, opencode, pi, copilot, cursor, all
  --scope <value>    project (default) writes under --dir. global writes
                      each harness's real user-level path under $HOME:
                      claude, pi, and codex only. opencode, copilot, and
                      cursor have no standard user-level skills directory
                      and are unsupported under --scope global.
  --dir <path>       Base directory to install into (default: .). Only
                      valid with --scope project.
  --force            Overwrite an existing target file
  -h, --help         Show this help and exit

When neither --scope nor --dir is given and stdin is a TTY, you are
prompted to choose project or global (default project). A non-TTY run
(e.g. curl | sh) defaults to project with no prompt.
EOF
}

skill_md_body() {
  cat <<'EOF'
---
name: ac-json
description: Read ActiveCollab task data as machine-readable JSON from the `ac` CLI (ac get/current/mine/browse --json). Run `ac skill ac-json` to load the full contract.
---

# ac-json (thin pointer)

The full, authoritative ActiveCollab `--json` read contract is served by the CLI itself.

Run:

    ac skill ac-json

and follow its output. It documents the curated minified JSON schemas for
`ac get`, `ac current`, `ac mine`, and `ac browse` with `--json`, the round-trippable `ref`,
and the cache / `--no-comments` flags.
EOF
}

skill_mdc_body() {
  cat <<'EOF'
---
description: Read ActiveCollab task data as JSON via the `ac` CLI. Run `ac skill ac-json` for the full contract.
globs:
alwaysApply: false
---

# ac-json (thin pointer)

The full ActiveCollab `--json` read contract is served by the CLI. Run `ac skill ac-json`
and follow its output — it documents the `--json` schemas for get/current/mine/browse,
the round-trippable `ref`, and the cache flags.
EOF
}

write_stub() {
  _target="$1"
  _body_fn="$2"

  if [ -f "${_target}" ] && [ "${_force}" -ne 1 ]; then
    echo "exists, skipping: ${_target} (use --force to overwrite)"
    return 0
  fi

  mkdir -p "$(dirname "${_target}")"
  "${_body_fn}" > "${_target}"
  echo "wrote: ${_target}"
}

unsupported_under_global() {
  echo "global scope is not supported for ${1} (no standard user-level skills directory); install per-project instead" >&2
  return 2
}

install_harness_project() {
  case "$1" in
    claude) write_stub "${_dir}/.claude/skills/ac-json/SKILL.md" skill_md_body ;;
    codex) write_stub "${_dir}/.codex/skills/ac-json/SKILL.md" skill_md_body ;;
    opencode) write_stub "${_dir}/.opencode/skills/ac-json/SKILL.md" skill_md_body ;;
    pi) write_stub "${_dir}/.pi/skills/ac-json/SKILL.md" skill_md_body ;;
    copilot) write_stub "${_dir}/.github/skills/ac-json/SKILL.md" skill_md_body ;;
    cursor) write_stub "${_dir}/.cursor/rules/ac-json.mdc" skill_mdc_body ;;
  esac
}

install_harness_global() {
  case "$1" in
    claude) write_stub "${HOME}/.claude/skills/ac-json/SKILL.md" skill_md_body ;;
    pi) write_stub "${HOME}/.pi/agent/skills/ac-json/SKILL.md" skill_md_body ;;
    codex) write_stub "${HOME}/.codex/skills/ac-json/SKILL.md" skill_md_body ;;
    opencode|copilot|cursor) unsupported_under_global "$1" ;;
  esac
}

install_harness() {
  _name="$1"
  if [ "${_scope}" = "global" ]; then
    install_harness_global "${_name}"
  else
    install_harness_project "${_name}"
  fi
}

_harness=""
_scope=""
_dir="."
_dir_explicit=0
_force=0

while [ $# -gt 0 ]; do
  case "$1" in
    --harness)
      if [ $# -lt 2 ]; then
        echo "Error: --harness requires a value" >&2
        usage >&2
        exit 2
      fi
      _harness="$2"
      shift 2
      ;;
    --scope)
      if [ $# -lt 2 ]; then
        echo "Error: --scope requires a value" >&2
        usage >&2
        exit 2
      fi
      _scope="$2"
      shift 2
      ;;
    --dir)
      if [ $# -lt 2 ]; then
        echo "Error: --dir requires a value" >&2
        usage >&2
        exit 2
      fi
      _dir="$2"
      _dir_explicit=1
      shift 2
      ;;
    --force)
      _force=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [ -n "${_scope}" ]; then
  case "${_scope}" in
    project|global) ;;
    *)
      echo "Error: unknown scope: ${_scope}" >&2
      usage >&2
      exit 2
      ;;
  esac
fi

if [ "${_scope}" = "global" ] && [ "${_dir_explicit}" -eq 1 ]; then
  echo "Error: --dir cannot be combined with --scope global" >&2
  exit 2
fi

if [ -z "${_scope}" ]; then
  if [ "${_dir_explicit}" -eq 0 ] && [ -t 0 ]; then
    printf 'Install scope? [project/global] (default project): '
    read -r _scope_answer
    case "${_scope_answer}" in
      ""|project) _scope="project" ;;
      global) _scope="global" ;;
      *)
        echo "Error: unknown scope: ${_scope_answer}" >&2
        usage >&2
        exit 2
        ;;
    esac
  else
    _scope="project"
  fi
fi

if [ -z "${_harness}" ]; then
  echo "Error: --harness is required" >&2
  usage >&2
  exit 2
fi

case "${_harness}" in
  claude|codex|opencode|pi|copilot|cursor|all) ;;
  *)
    echo "Error: unknown harness: ${_harness}" >&2
    usage >&2
    exit 2
    ;;
esac

_count=0
if [ "${_harness}" = "all" ]; then
  for _h in claude codex opencode pi copilot cursor; do
    if install_harness "${_h}"; then
      _count=$((_count + 1))
    fi
  done
else
  install_harness "${_harness}"
  _count=1
fi

echo "Done: processed ${_count} harness target(s)."
