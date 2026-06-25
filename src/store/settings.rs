#![allow(dead_code)]

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub struct SettingsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        SettingsRepository { conn }
    }

    pub fn get(&self, key: &str, default: Option<&str>) -> Result<Option<String>> {
        let result: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result.or_else(|| default.map(str::to_owned)))
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/store/settings.rs"]
mod tests;
