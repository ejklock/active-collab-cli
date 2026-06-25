#![allow(dead_code)]

use crate::store::now_iso;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

pub struct CachedTask {
    pub fields: Value,
    pub fetched_at: String,
}

pub struct TaskCache<'a> {
    conn: &'a Connection,
}

impl<'a> TaskCache<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        TaskCache { conn }
    }

    pub fn read(
        &self,
        instance: &str,
        project_id: i64,
        task_id: i64,
    ) -> Result<Option<CachedTask>> {
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT fields_json, fetched_at FROM ticket_cache \
                 WHERE instance=?1 AND project_id=?2 AND task_id=?3",
                params![instance, project_id, task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match row {
            None => Ok(None),
            Some((fields_json, fetched_at)) => {
                let fields: Value = serde_json::from_str(&fields_json)?;
                Ok(Some(CachedTask { fields, fetched_at }))
            }
        }
    }

    pub fn write(
        &self,
        instance: &str,
        project_id: i64,
        task_id: i64,
        task: &Value,
        comments: &Value,
    ) -> Result<()> {
        let mut payload = task.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("comments".to_string(), comments.clone());
        }
        let fields_json = serde_json::to_string(&payload)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO ticket_cache \
             (instance, project_id, task_id, fields_json, fetched_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![instance, project_id, task_id, fields_json, now_iso()],
        )?;
        Ok(())
    }

    pub fn delete_for_instance(&self, instance: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM ticket_cache WHERE instance = ?1",
            params![instance],
        )?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/store/cache.rs"]
mod tests;
