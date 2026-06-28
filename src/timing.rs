use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn enabled() -> bool {
    std::env::var_os("AC_TIMING").is_some()
}

pub fn log_path() -> PathBuf {
    resolve_log_path(std::env::var("AC_TIMING").ok().as_deref())
}

pub(crate) fn resolve_log_path(ac_timing_value: Option<&str>) -> PathBuf {
    match ac_timing_value {
        Some(v) if !v.is_empty() && v != "1" => PathBuf::from(v),
        _ => std::env::temp_dir().join("ac-timing.log"),
    }
}

pub fn record(phase: &str, dur: Duration) {
    record_to(&log_path(), enabled(), phase, dur);
}

pub(crate) fn record_to(path: &Path, is_enabled: bool, phase: &str, dur: Duration) {
    if !is_enabled {
        return;
    }
    let now = crate::store::now_iso();
    let ms = dur.as_millis();
    let line = format!("{now}\tphase={phase}\tms={ms}\n");
    use std::fs::OpenOptions;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        })
        .ok();
}

#[cfg(test)]
#[path = "../tests/unit/timing.rs"]
mod tests;
