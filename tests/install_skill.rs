use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_tmp_dir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ac_install_skill_test_{label}_{}_{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_install(dir: &Path, harness: &str, extra_args: &[&str]) -> Output {
    let dir_str = dir.to_str().expect("tmp dir path must be valid utf-8");
    let mut args = vec!["install-skill.sh", "--harness", harness, "--dir", dir_str];
    args.extend_from_slice(extra_args);
    Command::new("sh")
        .args(&args)
        .output()
        .expect("failed to run install-skill.sh")
}

const SKILL_MD_HARNESSES: &[(&str, &str)] = &[
    ("claude", ".claude/skills/ac-json/SKILL.md"),
    ("codex", ".codex/skills/ac-json/SKILL.md"),
    ("opencode", ".opencode/skills/ac-json/SKILL.md"),
    ("pi", ".pi/skills/ac-json/SKILL.md"),
    ("copilot", ".github/skills/ac-json/SKILL.md"),
];

const CURSOR_PATH: &str = ".cursor/rules/ac-json.mdc";

#[test]
fn installs_all_six_harness_stubs() {
    let tmp = unique_tmp_dir("all_six");

    let output = run_install(&tmp, "all", &[]);
    assert!(
        output.status.success(),
        "install-skill.sh --harness all failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (_, rel_path) in SKILL_MD_HARNESSES {
        let full = tmp.join(rel_path);
        assert!(full.is_file(), "expected {} to exist", full.display());
        let contents = std::fs::read_to_string(&full).unwrap();
        assert!(
            contents.contains("ac skill ac-json"),
            "{} should contain the pointer command, got:\n{contents}",
            full.display()
        );
    }

    let cursor_full = tmp.join(CURSOR_PATH);
    assert!(
        cursor_full.is_file(),
        "expected {} to exist",
        cursor_full.display()
    );
    let cursor_contents = std::fs::read_to_string(&cursor_full).unwrap();
    assert!(
        cursor_contents.contains("ac skill ac-json"),
        "cursor stub should contain the pointer command, got:\n{cursor_contents}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn skill_stub_has_required_frontmatter() {
    let tmp = unique_tmp_dir("frontmatter");

    let output = run_install(&tmp, "all", &[]);
    assert!(output.status.success());

    for (_, rel_path) in SKILL_MD_HARNESSES {
        let full = tmp.join(rel_path);
        let contents = std::fs::read_to_string(&full).unwrap();
        assert!(
            contents.starts_with("---\n"),
            "{} should start with frontmatter delimiter",
            full.display()
        );
        assert!(
            contents.contains("name: ac-json"),
            "{} frontmatter should declare name: ac-json",
            full.display()
        );
        assert!(
            contents.contains("description:"),
            "{} frontmatter should have a description line",
            full.display()
        );
    }

    let cursor_full = tmp.join(CURSOR_PATH);
    let cursor_contents = std::fs::read_to_string(&cursor_full).unwrap();
    assert!(
        cursor_contents.contains("alwaysApply: false"),
        "cursor stub should set alwaysApply: false, got:\n{cursor_contents}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn cursor_only_writes_mdc() {
    let tmp = unique_tmp_dir("cursor_only");

    let output = run_install(&tmp, "cursor", &[]);
    assert!(
        output.status.success(),
        "install-skill.sh --harness cursor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cursor_full = tmp.join(CURSOR_PATH);
    assert!(
        cursor_full.is_file(),
        "expected {} to exist",
        cursor_full.display()
    );
    let cursor_contents = std::fs::read_to_string(&cursor_full).unwrap();
    assert!(cursor_contents.contains("alwaysApply: false"));

    for (_, rel_path) in SKILL_MD_HARNESSES {
        let full = tmp.join(rel_path);
        assert!(
            !full.exists(),
            "cursor-only install should not have written {}",
            full.display()
        );
    }

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn existing_file_not_clobbered() {
    let tmp = unique_tmp_dir("no_clobber");

    let first = run_install(&tmp, "all", &[]);
    assert!(first.status.success());

    let claude_path = tmp.join(".claude/skills/ac-json/SKILL.md");
    let sentinel = "CANONICAL CONTENT DO NOT OVERWRITE";
    std::fs::write(&claude_path, sentinel).unwrap();

    let second = run_install(&tmp, "all", &[]);
    assert!(
        second.status.success(),
        "re-running install-skill.sh --harness all should not error: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("exists, skipping"),
        "second run should report skipping existing files, got stdout:\n{stdout}"
    );

    let preserved = std::fs::read_to_string(&claude_path).unwrap();
    assert_eq!(
        preserved, sentinel,
        "existing file must not be clobbered without --force"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn unknown_harness_exits_nonzero() {
    let tmp = unique_tmp_dir("unknown_harness");

    let output = run_install(&tmp, "definitely-not-a-harness", &[]);
    assert!(
        !output.status.success(),
        "unknown --harness value should exit non-zero"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
