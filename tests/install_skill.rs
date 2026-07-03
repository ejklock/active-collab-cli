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

fn script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("install-skill.sh")
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

/// Runs install-skill.sh with no `--dir` so the script falls through to its
/// scope-resolution logic, and points the child's `HOME` at a unique temp
/// dir so `--scope global` writes are hermetic and never touch the real
/// user home.
fn run_install_home(home: &Path, harness: &str, extra_args: &[&str]) -> Output {
    let home_str = home.to_str().expect("tmp home path must be valid utf-8");
    let mut args = vec!["--harness", harness];
    args.extend_from_slice(extra_args);
    Command::new("sh")
        .arg(script_path())
        .args(&args)
        .env("HOME", home_str)
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

#[test]
fn project_scope_default_unchanged_with_dir_and_no_scope() {
    let tmp = unique_tmp_dir("project_default_unchanged");

    let output = run_install(&tmp, "all", &[]);
    assert!(
        output.status.success(),
        "default project scope with --dir should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Install scope?"),
        "a non-TTY run with --dir must never emit the scope prompt, got stdout:\n{stdout}"
    );

    for (_, rel_path) in SKILL_MD_HARNESSES {
        assert!(
            tmp.join(rel_path).is_file(),
            "expected {} to exist under project scope",
            tmp.join(rel_path).display()
        );
    }
    assert!(tmp.join(CURSOR_PATH).is_file());

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn project_scope_defaults_without_dir_or_scope_on_non_tty() {
    let home = unique_tmp_dir("project_default_home");
    let cwd = unique_tmp_dir("project_default_cwd");

    let output = Command::new("sh")
        .arg(script_path())
        .args(["--harness", "claude"])
        .current_dir(&cwd)
        .env(
            "HOME",
            home.to_str().expect("tmp home path must be valid utf-8"),
        )
        .output()
        .expect("failed to run install-skill.sh");

    assert!(
        output.status.success(),
        "non-interactive run without --scope/--dir should default to project scope: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Install scope?"),
        "non-TTY run must not emit the interactive scope prompt, got stdout:\n{stdout}"
    );

    let claude_stub = cwd.join(".claude/skills/ac-json/SKILL.md");
    assert!(
        claude_stub.is_file(),
        "expected project-scope install under cwd, got: {}",
        claude_stub.display()
    );

    std::fs::remove_dir_all(&home).ok();
    std::fs::remove_dir_all(&cwd).ok();
}

#[test]
fn global_scope_all_writes_supported_harnesses_under_home() {
    let home = unique_tmp_dir("global_all_home");

    let output = run_install_home(&home, "all", &["--scope", "global"]);
    assert!(
        output.status.success(),
        "install-skill.sh --harness all --scope global failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected = [
        ".claude/skills/ac-json/SKILL.md",
        ".pi/agent/skills/ac-json/SKILL.md",
        ".codex/skills/ac-json/SKILL.md",
    ];
    for rel_path in expected {
        let full = home.join(rel_path);
        assert!(full.is_file(), "expected {} to exist", full.display());
        let contents = std::fs::read_to_string(&full).unwrap();
        assert!(
            contents.contains("ac skill ac-json"),
            "{} should contain the pointer command, got:\n{contents}",
            full.display()
        );
    }

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn global_scope_all_skips_unsupported_harnesses() {
    let home = unique_tmp_dir("global_all_skip");

    let output = run_install_home(&home, "all", &["--scope", "global"]);
    assert!(
        output.status.success(),
        "install-skill.sh --harness all --scope global should still exit 0 when skipping unsupported harnesses: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for rel_path in [
        ".opencode/skills/ac-json/SKILL.md",
        ".github/skills/ac-json/SKILL.md",
        ".cursor/rules/ac-json.mdc",
    ] {
        let full = home.join(rel_path);
        assert!(
            !full.exists(),
            "expected {} NOT to exist under global scope",
            full.display()
        );
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    for harness in ["opencode", "copilot", "cursor"] {
        assert!(
            stderr.contains(harness),
            "stderr should note the skipped harness {harness}, got:\n{stderr}"
        );
    }

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn global_scope_single_unsupported_harness_exits_2() {
    for harness in ["opencode", "copilot", "cursor"] {
        let home = unique_tmp_dir(&format!("global_unsupported_{harness}"));

        let output = run_install_home(&home, harness, &["--scope", "global"]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected exit 2 for --harness {harness} --scope global, got: {:?}, stderr: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("not supported") && stderr.contains(harness),
            "stderr should explain {harness} is unsupported under global scope, got:\n{stderr}"
        );

        let written = std::fs::read_dir(&home).unwrap().count();
        assert_eq!(
            written, 0,
            "no file should be written for unsupported harness {harness} under global scope"
        );

        std::fs::remove_dir_all(&home).ok();
    }
}

#[test]
fn dir_with_global_scope_errors() {
    let tmp = unique_tmp_dir("dir_plus_global");

    let output = run_install(&tmp, "claude", &["--scope", "global"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "--dir combined with --scope global should exit 2, got: {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--dir") && stderr.contains("--scope global"),
        "stderr should explain the incompatibility, got:\n{stderr}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn invalid_scope_value_exits_2() {
    let tmp = unique_tmp_dir("invalid_scope");

    let output = run_install(&tmp, "claude", &["--scope", "nope"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "an invalid --scope value should exit 2, got: {:?}",
        output.status.code()
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scope") && stderr.contains("nope"),
        "stderr should report the invalid scope, got:\n{stderr}"
    );

    std::fs::remove_dir_all(&tmp).ok();
}
