#![allow(dead_code)]

pub mod cache;
pub mod instances;
pub mod settings;

use crate::config::Config;
use anyhow::Result;
use rusqlite::Connection;
use std::fs;
use std::path::Path;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(config: &Config) -> Result<Store> {
        let db_path = &config.db_path;
        prepare_parent_dir(db_path)?;
        let conn = Connection::open(db_path)?;
        set_file_mode_600(db_path)?;
        apply_pragmas(&conn)?;
        init_schema(&conn)?;
        Ok(Store { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

fn prepare_parent_dir(db_path: &Path) -> Result<()> {
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            set_dir_mode_700(parent)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_dir_mode_700(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_dir_mode_700(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_file_mode_600(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if path.exists() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_file_mode_600(_path: &Path) -> Result<()> {
    Ok(())
}

fn apply_pragmas(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE;\
         PRAGMA busy_timeout=5000;\
         PRAGMA foreign_keys=ON;",
    )?;
    Ok(())
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS instances (
            name       TEXT PRIMARY KEY,
            base_url   TEXT NOT NULL,
            email      TEXT NOT NULL,
            token      TEXT NOT NULL,
            user_id    INTEGER,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS ticket_cache (
            instance    TEXT NOT NULL,
            project_id  INTEGER NOT NULL,
            task_id     INTEGER NOT NULL,
            fields_json TEXT NOT NULL,
            fetched_at  TEXT NOT NULL,
            PRIMARY KEY (instance, project_id, task_id)
        );
        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    Ok(())
}

pub(crate) fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs();
    let (year, month, day, hour, min, sec) = secs_to_utc_parts(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, min, sec
    )
}

pub(crate) fn secs_to_utc_parts(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = secs % 60;
    let mins = secs / 60;
    let min = mins % 60;
    let hours = mins / 60;
    let hour = hours % 24;
    let days = hours / 24;

    let (year, doy) = days_to_year_and_doy(days);
    let (month, day) = doy_to_month_day(doy, is_leap_year(year));

    (year, month, day, hour, min, sec)
}

fn days_to_year_and_doy(mut days: u64) -> (u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            return (year, days);
        }
        days -= days_in_year;
        year += 1;
    }
}

fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn doy_to_month_day(doy: u64, leap: bool) -> (u64, u64) {
    let days_in_month: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut remaining = doy;
    for (i, &d) in days_in_month.iter().enumerate() {
        if remaining < d {
            return ((i as u64) + 1, remaining + 1);
        }
        remaining -= d;
    }
    (12, 31)
}

#[cfg(test)]
#[path = "../../tests/unit/store/mod.rs"]
mod tests;
