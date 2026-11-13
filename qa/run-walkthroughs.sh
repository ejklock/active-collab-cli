#!/usr/bin/env bash
# Builds the native `ac` and the mock ActiveCollab, then validates and replays
# every qa/*.walkthrough.json. It fails when a walkthrough fails, when the
# owner's default store changed (checked by modification time, never opened)
# or when a mock process outlives the last run. Before the first replay it
# installs the pinned MCP Inspector under qa/.run/tools from the committed
# qa/tools lockfile (npm ci), the only network fetch,
# so no replayed step downloads. One replay at a time per
# checkout: an atomic mkdir lock under qa/.run/ refuses a second one.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

owner_store="$HOME/.config/active-collab/active-collab.db"
mock_binary="$root/target/native/debug/examples/mock_collab"

store_stamp() {
  if [ -e "$owner_store" ]; then
    stat -c %Y "$owner_store" 2>/dev/null || stat -f %m "$owner_store"
  else
    echo absent
  fi
}

die() {
  echo "FAIL: $*" >&2
  exit 1
}

mock_alive() {
  local status=0
  pgrep -f "$mock_binary" >/dev/null 2>&1 || status=$?
  case "$status" in
    0) return 0 ;;
    1) return 1 ;;
    *) die "cannot list processes to check for a leftover mock" ;;
  esac
}

lock_dir="$root/qa/.run/replay.lock"
mkdir -p "$root/qa/.run"
mkdir "$lock_dir" 2>/dev/null \
  || die "another replay holds the lock $lock_dir; wait for it or remove the folder if no replay runs"
trap 'rmdir "$lock_dir"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

before="$(store_stamp)"

CARGO_TARGET_DIR="$root/target/native" cargo build --bin ac --example mock_collab \
  || die "the native build failed"

inspector_dir="$root/qa/.run/tools"
inspector_bin="$inspector_dir/node_modules/.bin/mcp-inspector"
inspector_stamp="$inspector_dir/installed.stamp"
if [ ! -x "$inspector_bin" ] || ! cat qa/tools/package.json qa/tools/package-lock.json | cmp -s - "$inspector_stamp"; then
  mkdir -p "$inspector_dir"
  rm -f "$inspector_stamp"
  cp qa/tools/package.json qa/tools/package-lock.json "$inspector_dir/"
  (cd "$inspector_dir" && npm ci --no-audit --no-fund) \
    || die "installing the MCP Inspector from qa/tools/package-lock.json failed"
  cat qa/tools/package.json qa/tools/package-lock.json > "$inspector_stamp"
fi

shopt -s nullglob
walkthroughs=(qa/*.walkthrough.json)
[ "${#walkthroughs[@]}" -gt 0 ] || die "no qa/*.walkthrough.json found"

failed=()
for walkthrough in "${walkthroughs[@]}"; do
  if ! qa-walkthrough check --strict "$walkthrough"; then
    failed+=("$walkthrough (check)")
    continue
  fi
  if ! qa-walkthrough run "$walkthrough"; then
    failed+=("$walkthrough (run)")
  fi
done

for _ in 1 2 3 4 5 6 7 8 9 10; do
  mock_alive || break
  sleep 0.5
done

[ "${#failed[@]}" -eq 0 ] || die "walkthrough failed: ${failed[*]}"
[ "$(store_stamp)" = "$before" ] || die "the owner's store $owner_store changed during the run"
if mock_alive; then
  die "a mock_collab process is still alive after the last run"
fi

echo "OK: ${#walkthroughs[@]} walkthrough(s) replayed"
