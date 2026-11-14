#!/bin/sh
# Proves scripts/check-skill-stubs.sh is green on an aligned tree and red on
# each drift it guards. Every case runs on a fresh copy of the needed files
# under a mktemp dir; the repository is never edited.
set -u

repo="$(cd "$(dirname "$0")/.." && pwd)"
check="${repo}/scripts/check-skill-stubs.sh"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT INT TERM

failures=0
out=""
code=0

fail() {
  echo "FAIL $1: $2" >&2
  failures=$((failures + 1))
}

# fresh_tree: copy the files the check reads into a new tree and print its path.
fresh_tree() {
  tree="$(mktemp -d "${work}/tree.XXXXXX")"
  mkdir -p "${tree}/scripts"
  cp "${repo}/install-skill.sh" "${repo}/Cargo.toml" "${tree}/"
  cp -R "${repo}/plugins" "${repo}/.claude-plugin" "${repo}/.agents" "${tree}/"
  mkdir -p "${tree}/packages"
  cp -R "${repo}/packages/active-collab-agent" "${tree}/packages/"
  echo "${tree}"
}

# run_check TREE: run the repository's check against TREE, capturing out and code.
run_check() {
  out="$(CHECK_STUBS_ROOT="$1" sh "${check}" 2>&1)"
  code=$?
}

# expect_red NAME: last run must exit non-zero; remaining args are substrings the output must hold.
expect_red() {
  name="$1"
  shift
  if [ "${code}" -eq 0 ]; then
    fail "${name}" "exit 0, expected non-zero"
    return
  fi
  for want in "$@"; do
    case "${out}" in
      *"${want}"*) ;;
      *) fail "${name}" "output lacks '${want}': ${out}" ;;
    esac
  done
}

expect_green() {
  if [ "$code" -ne 0 ]; then
    fail "$1" "exit ${code}, expected 0: ${out}"
  fi
}

# edit TREE FILE JQ_FILTER: rewrite a json file in the copy.
edit() {
  jq "$3" "$1/$2" >"$1/$2.new" && mv "$1/$2.new" "$1/$2"
}

test_aligned_tree_passes() {
  t="$(fresh_tree)"
  run_check "${t}"
  expect_green aligned_tree_passes
}

test_cargo_version_bump_alone_fails() {
  t="$(fresh_tree)"
  sed 's/^version *= *"[^"]*"$/version = "99.0.0"/' "${t}/Cargo.toml" >"${t}/Cargo.toml.new"
  mv "${t}/Cargo.toml.new" "${t}/Cargo.toml"
  run_check "${t}"
  expect_red cargo_version_bump_alone_fails \
    "plugins/active-collab/.claude-plugin/plugin.json: .version" \
    "plugins/active-collab/plugin.json: .version"
}

test_marketplace_entry_version_fails() {
  for m in .claude-plugin/marketplace.json .agents/plugins/marketplace.json; do
    t="$(fresh_tree)"
    edit "${t}" "${m}" '.plugins[0].version = "1.0.0"'
    run_check "${t}"
    expect_red "marketplace_entry_version_fails ${m}" "${m}: " ".version"
  done
}

test_renamed_marketplace_entry_fails() {
  for m in .claude-plugin/marketplace.json .agents/plugins/marketplace.json; do
    t="$(fresh_tree)"
    edit "${t}" "${m}" '.plugins[0].name = "active-collabX"'
    run_check "${t}"
    expect_red "renamed_marketplace_entry_fails ${m}" "${m}: " ".name"
  done
}

test_second_entry_with_wrong_name_fails() {
  t="$(fresh_tree)"
  m=.claude-plugin/marketplace.json
  edit "${t}" "${m}" '.plugins += [{"name": "other", "source": "./plugins/other"}]'
  run_check "${t}"
  expect_red second_entry_with_wrong_name_fails "${m}: " "'other'"
}

test_moved_entry_is_blamed_by_its_own_position() {
  for m in .claude-plugin/marketplace.json .agents/plugins/marketplace.json; do
    t="$(fresh_tree)"
    edit "${t}" "${m}" '.plugins = [{"name": "active-collab", "source": "./plugins/other"}] + .plugins'
    run_check "${t}"
    expect_green "moved_entry_with_correct_name_passes ${m}"
    edit "${t}" "${m}" '.plugins[1].name = "active-collabX"'
    run_check "${t}"
    expect_red "moved_entry_is_blamed_by_its_own_position ${m}" "${m}: " "active-collabX"
    case "${out}" in
      *"'active-collab', expected"*) fail "moved_entry_is_blamed_by_its_own_position ${m}" "blames the correct first entry: ${out}" ;;
    esac
  done
}

test_missing_plugin_entry_fails() {
  for m in .claude-plugin/marketplace.json .agents/plugins/marketplace.json; do
    t="$(fresh_tree)"
    edit "${t}" "${m}" '.plugins[0].source = "./plugins/other"'
    run_check "${t}"
    expect_red "missing_plugin_entry_fails ${m}" "${m}: " "plugins/active-collab"
  done
}

test_skill_stub_change_fails() {
  t="$(fresh_tree)"
  s="${t}/plugins/active-collab/skills/active-collab/SKILL.md"
  printf 'x\n' >>"${s}"
  run_check "${t}"
  expect_red skill_stub_change_fails "plugin SKILL.md differs from the install-skill.sh stub"
}

test_package_skill_stub_change_fails() {
  t="$(fresh_tree)"
  printf 'x\n' >>"${t}/packages/active-collab-agent/skills/active-collab/SKILL.md"
  run_check "${t}"
  expect_red package_skill_stub_change_fails "packages/active-collab-agent SKILL.md differs from the install-skill.sh stub"
}

test_package_version_drift_fails() {
  t="$(fresh_tree)"
  edit "${t}" packages/active-collab-agent/package.json '.version = "99.0.0"'
  run_check "${t}"
  expect_red package_version_drift_fails "packages/active-collab-agent/package.json: .version is '99.0.0'"
}

test_package_name_drift_fails() {
  t="$(fresh_tree)"
  edit "${t}" packages/active-collab-agent/package.json '.name = "other"'
  run_check "${t}"
  expect_red package_name_drift_fails "packages/active-collab-agent/package.json: .name is 'other'"
}

test_failing_installer_fails() {
  t="$(fresh_tree)"
  printf 'exit 7\n' >"${t}/install-skill.sh"
  run_check "${t}"
  expect_red failing_installer_fails "install-skill.sh failed; cannot compare the skill stub"
}

test_unrelated_root_variable_is_ignored() {
  t="$(fresh_tree)"
  edit "${t}" .claude-plugin/marketplace.json '.plugins[0].name = "active-collabX"'
  out="$(unset CHECK_STUBS_ROOT; ROOT="${t}" sh "${check}" 2>&1)"
  code=$?
  expect_green unrelated_root_variable_is_ignored
  case "${out}" in
    *"active-collabX"*) fail unrelated_root_variable_is_ignored "checked the ROOT tree: ${out}" ;;
  esac
}

for t in \
  test_aligned_tree_passes \
  test_cargo_version_bump_alone_fails \
  test_marketplace_entry_version_fails \
  test_renamed_marketplace_entry_fails \
  test_second_entry_with_wrong_name_fails \
  test_moved_entry_is_blamed_by_its_own_position \
  test_missing_plugin_entry_fails \
  test_skill_stub_change_fails \
  test_package_skill_stub_change_fails \
  test_package_version_drift_fails \
  test_package_name_drift_fails \
  test_failing_installer_fails \
  test_unrelated_root_variable_is_ignored; do
  "$t"
done

if [ "${failures}" -ne 0 ]; then
  echo "${failures} failure(s)" >&2
  exit 1
fi
echo "check-skill-stubs: all cases pass"
