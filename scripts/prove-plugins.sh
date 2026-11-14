#!/bin/sh
# Proves the ac plugin installs from this checkout with each harness's own CLI:
# install, assert the skill and the MCP server are listed (Pi: skill only, and no
# extension from the package, read from its RPC get_commands), uninstall.
# The Pi extension check sees only extensions that register a command: Pi 1.5.2
# has no command that lists loaded extensions, so one registering only tools or
# hooks passes it. Codex is asserted through its own app-server plugin/read.
# Both the Codex assertion and the Pi extension check need jq.
# Every harness command runs with its home and config variables pointing into a
# temporary directory, so no real home is read or written.
# A harness whose CLI is not on PATH is skipped. Overrides: PROVE_ROOT (the tree
# to install from), CLAUDE_BIN, CODEX_BIN, OPENCODE_BIN, PI_BIN (the real binary,
# not a wrapper that adds --profile).
set -u

root="${PROVE_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
claude_bin="${CLAUDE_BIN:-claude}"
codex_bin="${CODEX_BIN:-codex}"
opencode_bin="${OPENCODE_BIN:-opencode}"
pi_bin="${PI_BIN:-pi}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
trap 'exit 130' INT TERM

expect() {
  # expect <label> <pattern> <text>
  if ! printf '%s\n' "$3" | grep -q -e "$2"; then
    echo "  missing ${1}: expected /${2}/ in:" >&2
    printf '%s\n' "$3" | sed 's/^/    /' >&2
    return 1
  fi
}

expect_absent() {
  if printf '%s\n' "$3" | grep -q -e "$2"; then
    echo "  still present ${1}: /${2}/ in:" >&2
    printf '%s\n' "$3" | sed 's/^/    /' >&2
    return 1
  fi
}

strip_ansi() {
  sed 's/\x1b\[[0-9;]*m//g'
}

prove_claude() {
  home="${work}/claude"
  mkdir -p "${home}"
  export HOME="${home}" CLAUDE_CONFIG_DIR="${home}/claude" XDG_CONFIG_HOME="${home}/xdg-config" \
    XDG_DATA_HOME="${home}/xdg-data" XDG_CACHE_HOME="${home}/xdg-cache"
  "${claude_bin}" plugin marketplace add "${root}" >/dev/null 2>&1 || return 1
  "${claude_bin}" plugin install active-collab@active-collab >/dev/null 2>&1 || return 1
  details="$("${claude_bin}" plugin details active-collab@active-collab 2>&1)"
  status=0
  expect "claude skill" 'Skills (1)  active-collab' "${details}" || status=1
  expect "claude mcp server" 'MCP servers (1)  active-collab' "${details}" || status=1
  "${claude_bin}" plugin uninstall active-collab@active-collab >/dev/null 2>&1 || status=1
  "${claude_bin}" plugin marketplace remove active-collab >/dev/null 2>&1 || status=1
  left="$("${claude_bin}" plugin list 2>&1)"
  expect_absent "claude plugin after uninstall" 'active-collab@active-collab' "${left}" || status=1
  return "${status}"
}

codex_plugin_read() {
  # Prints the plugin/read response of codex app-server (JSON-RPC over stdio) for the plugin.
  responses="$( { printf '%s\n' \
    '{"id":1,"method":"initialize","params":{"clientInfo":{"name":"prove-plugins","version":"1"}}}' \
    '{"method":"initialized"}' \
    '{"id":2,"method":"plugin/read","params":{"marketplacePath":"'"${root}"'/.agents/plugins/marketplace.json","pluginName":"active-collab"}}'
    sleep 8; } | timeout 30 "${codex_bin}" app-server 2>/dev/null)" || return 1
  printf '%s\n' "${responses}" | jq -c -R 'fromjson? | select(.id == 2 and .result != null)' | grep .
}

prove_codex() {
  home="${work}/codex"
  mkdir -p "${home}/codex"
  export HOME="${home}" CODEX_HOME="${home}/codex" XDG_CONFIG_HOME="${home}/xdg-config" \
    XDG_DATA_HOME="${home}/xdg-data" XDG_CACHE_HOME="${home}/xdg-cache"
  "${codex_bin}" plugin marketplace add "${root}" >/dev/null 2>&1 || return 1
  "${codex_bin}" plugin add active-collab@active-collab >/dev/null 2>&1 || return 1
  status=0
  read_out="$(codex_plugin_read)" || {
    echo "  codex app-server did not answer plugin/read (it is experimental; codex cannot be proved here)" >&2
    return 1
  }
  skills="$(printf '%s\n' "${read_out}" | jq -r '.result.plugin.skills[]?.name' 2>/dev/null)"
  expect "codex skill" '^active-collab:active-collab$' "${skills}" || status=1
  servers="$(printf '%s\n' "${read_out}" | jq -r '.result.plugin.mcpServers[]?' 2>/dev/null)"
  expect "codex plugin mcp server" '^active-collab$' "${servers}" || status=1
  mcp="$("${codex_bin}" mcp list 2>&1)"
  expect "codex mcp server" '^active-collab  *ac  *mcp' "${mcp}" || status=1
  "${codex_bin}" plugin remove active-collab@active-collab >/dev/null 2>&1 || status=1
  "${codex_bin}" plugin marketplace remove active-collab >/dev/null 2>&1 || status=1
  mcp="$("${codex_bin}" mcp list 2>&1)"
  expect_absent "codex mcp server after remove" '^active-collab  ' "${mcp}" || status=1
  return "${status}"
}

prove_opencode() {
  home="${work}/opencode"
  mkdir -p "${home}/project"
  export HOME="${home}" XDG_CONFIG_HOME="${home}/xdg-config" XDG_DATA_HOME="${home}/xdg-data" \
    XDG_CACHE_HOME="${home}/xdg-cache" XDG_STATE_HOME="${home}/xdg-state"
  cd "${home}/project" || return 1
  printf '{"plugin":["file://%s/packages/active-collab-agent"]}\n' "${root}" >opencode.json
  status=0
  skills="$("${opencode_bin}" debug skill 2>&1)"
  expect "opencode skill" '"name": "active-collab"' "${skills}" || status=1
  mcp="$("${opencode_bin}" mcp list 2>&1 | strip_ansi)"
  expect "opencode mcp server" 'active-collab' "${mcp}" || status=1
  rm -f opencode.json
  skills="$("${opencode_bin}" debug skill 2>&1)"
  expect_absent "opencode skill after uninstall" '"name": "active-collab"' "${skills}" || status=1
  mcp="$("${opencode_bin}" mcp list 2>&1 | strip_ansi)"
  expect_absent "opencode mcp server after uninstall" 'active-collab' "${mcp}" || status=1
  return "${status}"
}

prove_pi() {
  home="${work}/pi"
  mkdir -p "${home}/agent"
  export HOME="${home}" PI_CODING_AGENT_DIR="${home}/agent" XDG_CONFIG_HOME="${home}/xdg-config" \
    XDG_DATA_HOME="${home}/xdg-data" XDG_CACHE_HOME="${home}/xdg-cache" XDG_STATE_HOME="${home}/xdg-state"
  cd "${root}" || return 1
  "${pi_bin}" install "${root}/packages/active-collab-agent" >/dev/null 2>&1 || return 1
  status=0
  listed="$("${pi_bin}" list 2>&1)"
  expect "pi package" 'packages/active-collab-agent' "${listed}" || status=1
  commands="$(printf '{"type":"get_commands"}\n' | timeout 60 "${pi_bin}" --mode rpc --offline 2>&1)"
  expect "pi skill" 'skill:active-collab' "${commands}" || status=1
  extensions="$(printf '%s\n' "${commands}" | grep '^{"type":"response"' | jq -r '.data.commands[] | select(.source == "extension" and (.sourceInfo.path | contains("active-collab-agent"))) | .name')"
  expect_absent "pi extension from the package" '.' "${extensions}" || status=1
  "${pi_bin}" remove "${root}/packages/active-collab-agent" >/dev/null 2>&1 || status=1
  listed="$("${pi_bin}" list 2>&1)"
  expect_absent "pi package after remove" 'packages/active-collab-agent' "${listed}" || status=1
  return "${status}"
}

failed=0

run_harness() {
  # run_harness <name> <binary> <prove function>
  if ! command -v "$2" >/dev/null 2>&1; then
    echo "skip ${1}: ${2} is not on PATH"
    return 0
  fi
  if ("$3"); then
    echo "ok ${1}"
  else
    echo "FAIL ${1}" >&2
    failed=1
  fi
}

run_harness claude "${claude_bin}" prove_claude
run_harness codex "${codex_bin}" prove_codex
run_harness opencode "${opencode_bin}" prove_opencode
run_harness pi "${pi_bin}" prove_pi

exit "${failed}"
