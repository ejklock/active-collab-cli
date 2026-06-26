use super::*;
use std::path::PathBuf;
use std::time::Duration;

fn unique_tmp_path(label: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    p.push(format!("ac-timing-test-{label}-{pid}-{ts}.log"));
    p
}

#[test]
fn record_to_writes_one_line_when_enabled() {
    let path = unique_tmp_path("enabled");
    record_to(&path, true, "fetch_task", Duration::from_millis(42));

    let contents = std::fs::read_to_string(&path).expect("log file must exist");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one line written");

    let line = lines[0];
    assert!(
        line.contains("phase=fetch_task"),
        "line must contain phase name; got: {line}"
    );
    assert!(
        line.contains("ms=42"),
        "line must contain ms value; got: {line}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn record_to_writes_nothing_when_disabled() {
    let path = unique_tmp_path("disabled");
    record_to(&path, false, "fetch_task", Duration::from_millis(10));

    assert!(
        !path.exists(),
        "no file must be created when is_enabled=false"
    );
}

#[test]
fn record_to_appends_multiple_calls() {
    let path = unique_tmp_path("append");
    record_to(&path, true, "phase_a", Duration::from_millis(1));
    record_to(&path, true, "phase_b", Duration::from_millis(2));

    let contents = std::fs::read_to_string(&path).expect("log file must exist");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "two appended lines");
    assert!(lines[0].contains("phase=phase_a"));
    assert!(lines[1].contains("phase=phase_b"));

    std::fs::remove_file(&path).ok();
}

#[test]
fn resolve_log_path_uses_temp_dir_when_unset() {
    let result = resolve_log_path(None);
    let expected = std::env::temp_dir().join("ac-timing.log");
    assert_eq!(result, expected);
}

#[test]
fn resolve_log_path_uses_temp_dir_when_value_is_one() {
    let result = resolve_log_path(Some("1"));
    let expected = std::env::temp_dir().join("ac-timing.log");
    assert_eq!(result, expected);
}

#[test]
fn resolve_log_path_uses_temp_dir_when_value_is_empty() {
    let result = resolve_log_path(Some(""));
    let expected = std::env::temp_dir().join("ac-timing.log");
    assert_eq!(result, expected);
}

#[test]
fn resolve_log_path_returns_explicit_path_when_set() {
    let result = resolve_log_path(Some("/tmp/my-timing.log"));
    assert_eq!(result, PathBuf::from("/tmp/my-timing.log"));
}

#[test]
fn record_to_line_contains_iso_timestamp() {
    let path = unique_tmp_path("timestamp");
    record_to(&path, true, "user_map_cache_read", Duration::from_millis(5));

    let contents = std::fs::read_to_string(&path).expect("log file must exist");
    let line = contents.lines().next().expect("at least one line");

    assert!(
        line.contains('T') && line.contains('Z'),
        "line must contain ISO-8601 timestamp (T and Z markers); got: {line}"
    );

    std::fs::remove_file(&path).ok();
}
