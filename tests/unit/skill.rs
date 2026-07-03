use super::*;

fn canonical_body() -> &'static str {
    include_str!("../../.claude/skills/ac-json/SKILL.md")
}

#[test]
fn skill_prints_named_body() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = skill_output(Some("ac-json"), &mut out, &mut err);

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
        stdout.contains("ac-json\tRead ActiveCollab task data as machine-readable JSON"),
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
        stderr.contains("known: ac-json"),
        "stderr must list known skill names: {stderr}"
    );
}

#[test]
fn skill_registry_body_is_the_canonical_source() {
    let ac_json = SKILLS
        .iter()
        .find(|s| s.name == "ac-json")
        .expect("ac-json must be registered");
    assert!(
        ac_json.body.contains("# ac --json — agent read contract"),
        "embedded body must be the real SKILL.md, not a stand-in"
    );
    assert_eq!(
        ac_json.body,
        canonical_body(),
        "the registry body must be include_str! of the one canonical SKILL.md"
    );
}

#[test]
fn skill_is_known_command_and_not_rewritten_to_get() {
    assert!(crate::cli::KNOWN_COMMANDS.contains(&"skill"));
    assert_eq!(
        crate::cli::normalize_argv(&["skill".into(), "ac-json".into()], None),
        vec!["skill", "ac-json"]
    );
}
