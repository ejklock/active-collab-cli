use super::*;

fn canonical_body() -> &'static str {
    include_str!("../../.claude/skills/active-collab/SKILL.md")
}

#[test]
fn skill_prints_named_body() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = skill_output(Some("active-collab"), &mut out, &mut err);

    assert_eq!(code, 0, "known skill must exit 0");
    let expected = format!("{}\n", canonical_body());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        expected,
        "stdout must be the embedded body plus a trailing newline"
    );
    assert!(err.is_empty(), "no error output expected for a known skill");
}

#[test]
fn skill_lists_registry() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = skill_output(Some("list"), &mut out, &mut err);

    assert_eq!(code, 0, "list must exit 0");
    let stdout = String::from_utf8(out).unwrap();
    assert!(
        stdout.contains(
            "active-collab\tRead ActiveCollab task data — a task, your assignments, comments, or projects — as machine-readable JSON from the ac CLI, non-interactively without the TUI."
        ),
        "list must contain a tab-separated name/description line: {stdout}"
    );
    assert!(err.is_empty(), "no error output expected for list");
}

#[test]
fn skill_bare_prints_single() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = skill_output(None, &mut out, &mut err);

    assert_eq!(code, 0, "bare invocation with one skill must exit 0");
    let expected = format!("{}\n", canonical_body());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        expected,
        "bare invocation must print the single skill's body"
    );
}

#[test]
fn skill_unknown_exits_2() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = skill_output(Some("nope"), &mut out, &mut err);

    assert_eq!(code, 2, "unknown skill must exit 2");
    assert!(out.is_empty(), "nothing must be written to stdout");
    let stderr = String::from_utf8(err).unwrap();
    assert!(
        stderr.contains("unknown skill 'nope'"),
        "stderr must name the unknown skill: {stderr}"
    );
    assert!(
        stderr.contains("known: active-collab"),
        "stderr must list known skill names: {stderr}"
    );
}

#[test]
fn skill_old_name_is_unknown_after_rename() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = skill_output(Some("ac-json"), &mut out, &mut err);

    assert_eq!(code, 2, "the old name ac-json must be unknown, not aliased");
    assert!(out.is_empty(), "nothing must be written to stdout");
    let stderr = String::from_utf8(err).unwrap();
    assert!(
        stderr.contains("unknown skill 'ac-json'"),
        "stderr must name the unknown old skill name: {stderr}"
    );
}

#[test]
fn skill_registry_body_is_the_canonical_source() {
    let active_collab = SKILLS
        .iter()
        .find(|s| s.name == "active-collab")
        .expect("active-collab must be registered");
    assert!(
        active_collab
            .body
            .contains("# ac --json — agent read contract"),
        "embedded body must be the real SKILL.md, not a stand-in"
    );
    assert_eq!(
        active_collab.body,
        canonical_body(),
        "the registry body must be include_str! of the one canonical SKILL.md"
    );
}

#[test]
fn skill_is_known_command_and_not_rewritten_to_get() {
    assert!(crate::cli::KNOWN_COMMANDS.contains(&"skill"));
    assert_eq!(
        crate::cli::normalize_argv(&["skill".into(), "active-collab".into()], None),
        vec!["skill", "active-collab"]
    );
}
