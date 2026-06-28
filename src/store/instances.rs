#![allow(dead_code)]

use crate::store::now_iso;
use anyhow::Result;
use rusqlite::{params, Connection};

type DisplayRow = (String, String, String, Option<i64>);

#[derive(Clone)]
pub struct Instance {
    pub name: String,
    pub base_url: String,
    pub email: String,
    pub token: String,
    pub user_id: Option<i64>,
}

pub struct InstanceRepository<'a> {
    conn: &'a Connection,
}

impl<'a> InstanceRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        InstanceRepository { conn }
    }

    pub fn save(&self, instance: &Instance) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO instances \
             (name, base_url, email, token, user_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                instance.name,
                instance.base_url,
                instance.email,
                instance.token,
                instance.user_id,
                now_iso(),
            ],
        )?;
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<Instance>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, base_url, email, token, user_id \
             FROM instances ORDER BY created_at, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Instance {
                name: row.get(0)?,
                base_url: row.get(1)?,
                email: row.get(2)?,
                token: row.get(3)?,
                user_id: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn delete(&self, name: &str) -> Result<usize> {
        let affected = self
            .conn
            .execute("DELETE FROM instances WHERE name = ?1", params![name])?;
        Ok(affected)
    }

    pub fn list_for_display(&self) -> Result<Vec<DisplayRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, base_url, email, user_id \
             FROM instances ORDER BY created_at, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn list_connectivity(&self) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, base_url, token \
             FROM instances ORDER BY created_at, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn find_by_name(&self, name: &str) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, base_url, token FROM instances WHERE name = ?1")?;
        let rows = stmt.query_map(params![name], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/store/instances.rs"]
mod tests;
