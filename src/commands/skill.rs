use std::io::Write;

/// A single registered agent skill: name, one-line description, and its full markdown body.
pub(crate) struct SkillEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub body: &'static str,
}

pub(crate) const SKILLS: &[SkillEntry] = &[SkillEntry {
    name: "ac-json",
    description: "Read ActiveCollab task data as machine-readable JSON from the ac CLI",
    body: include_str!("../../.claude/skills/ac-json/SKILL.md"),
}];

fn find_skill(name: &str) -> Option<&'static SkillEntry> {
    SKILLS.iter().find(|s| s.name == name)
}

fn write_list(out: &mut impl Write) -> i32 {
    for skill in SKILLS {
        let _ = writeln!(out, "{}\t{}", skill.name, skill.description);
    }
    0
}

fn write_unknown(name: &str, err: &mut impl Write) -> i32 {
    let known: Vec<&str> = SKILLS.iter().map(|s| s.name).collect();
    let _ = writeln!(err, "unknown skill '{name}'");
    let _ = writeln!(err, "known: {}", known.join(", "));
    2
}

fn write_body(skill: &SkillEntry, out: &mut impl Write) -> i32 {
    let _ = writeln!(out, "{}", skill.body);
    0
}

/// Print an embedded agent skill (or the registry listing) per BDR 0031.
///
/// `name == Some("list")` lists the registry; a matching name prints that skill's
/// body; an unmatched name errors to `err` and exits 2; `None` prints the single
/// registered skill or falls back to the list when more than one is registered.
pub(crate) fn skill_output(name: Option<&str>, out: &mut impl Write, err: &mut impl Write) -> i32 {
    match name {
        Some("list") => write_list(out),
        Some(n) => match find_skill(n) {
            Some(skill) => write_body(skill, out),
            None => write_unknown(n, err),
        },
        None if SKILLS.len() == 1 => write_body(&SKILLS[0], out),
        None => write_list(out),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/skill.rs"]
mod tests;
