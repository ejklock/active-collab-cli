#![allow(dead_code)]

use crate::store::instances::Instance;
use crate::store::{now_epoch_secs, now_iso};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::HashMap;

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

pub struct CachedUserMap {
    pub users: HashMap<i64, String>,
    pub fetched_at: String,
}

pub struct UserMapCache<'a> {
    conn: &'a Connection,
}

impl<'a> UserMapCache<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        UserMapCache { conn }
    }

    pub fn read(&self, instance: &str) -> Result<Option<CachedUserMap>> {
        let row: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT users_json, fetched_at FROM user_map_cache WHERE instance=?1",
                params![instance],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match row {
            None => Ok(None),
            Some((users_json, fetched_at)) => {
                let raw: HashMap<String, String> = serde_json::from_str(&users_json)?;
                let users = raw
                    .into_iter()
                    .filter_map(|(k, v)| k.parse::<i64>().ok().map(|id| (id, v)))
                    .collect();
                Ok(Some(CachedUserMap { users, fetched_at }))
            }
        }
    }

    pub fn write(&self, instance: &str, users: &HashMap<i64, String>) -> Result<()> {
        let string_keyed: HashMap<String, &String> =
            users.iter().map(|(k, v)| (k.to_string(), v)).collect();
        let users_json = serde_json::to_string(&string_keyed)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO user_map_cache (instance, users_json, fetched_at) \
             VALUES (?1, ?2, ?3)",
            params![instance, users_json, now_iso()],
        )?;
        Ok(())
    }
}

pub struct CachedProjectNames {
    pub names: HashMap<i64, String>,
    pub fetched_at: i64,
}

pub struct ProjectNamesCache<'a> {
    conn: &'a Connection,
}

impl<'a> ProjectNamesCache<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        ProjectNamesCache { conn }
    }

    pub fn read(&self, instance: &str) -> Result<Option<CachedProjectNames>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT names_json, fetched_at FROM project_names_cache WHERE instance=?1",
                params![instance],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match row {
            None => Ok(None),
            Some((names_json, fetched_at)) => {
                let raw: HashMap<String, String> = serde_json::from_str(&names_json)?;
                let names = raw
                    .into_iter()
                    .filter_map(|(k, v)| k.parse::<i64>().ok().map(|id| (id, v)))
                    .collect();
                Ok(Some(CachedProjectNames { names, fetched_at }))
            }
        }
    }

    /// Reads the cached entry and returns it only if still within
    /// `max_age_secs` of `now_epoch_secs()`; absent or stale entries yield
    /// `None` (ADR 0052 — the cache owns freshness, not its callers).
    pub fn read_fresh(
        &self,
        instance: &str,
        max_age_secs: i64,
    ) -> Result<Option<CachedProjectNames>> {
        let cached = match self.read(instance)? {
            Some(cached) => cached,
            None => return Ok(None),
        };
        let age = now_epoch_secs() - cached.fetched_at;
        if age <= max_age_secs {
            Ok(Some(cached))
        } else {
            Ok(None)
        }
    }

    pub fn write(&self, instance: &str, names: &HashMap<i64, String>) -> Result<()> {
        self.write_with_fetched_at(instance, names, now_epoch_secs())
    }

    pub(crate) fn write_with_fetched_at(
        &self,
        instance: &str,
        names: &HashMap<i64, String>,
        fetched_at: i64,
    ) -> Result<()> {
        let string_keyed: HashMap<String, &String> =
            names.iter().map(|(k, v)| (k.to_string(), v)).collect();
        let names_json = serde_json::to_string(&string_keyed)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO project_names_cache (instance, names_json, fetched_at) \
             VALUES (?1, ?2, ?3)",
            params![instance, names_json, fetched_at],
        )?;
        Ok(())
    }
}

pub struct TaskListCache<'a> {
    conn: &'a Connection,
}

impl<'a> TaskListCache<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        TaskListCache { conn }
    }

    pub fn read(
        &self,
        scope: &str,
        instances_key: &str,
        max_age_secs: i64,
    ) -> Result<Option<String>> {
        let row: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT list_json, fetched_at FROM task_list_cache \
                 WHERE scope=?1 AND instances_key=?2",
                params![scope, instances_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match row {
            None => Ok(None),
            Some((list_json, fetched_at)) => {
                let age = now_epoch_secs() - fetched_at;
                if age <= max_age_secs {
                    Ok(Some(list_json))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn write(&self, scope: &str, instances_key: &str, list_json: &str) -> Result<()> {
        self.write_with_fetched_at(scope, instances_key, list_json, now_epoch_secs())
    }

    /// Visible for testing: write with an explicit fetched_at so tests can
    /// control the age without sleeping.
    pub(crate) fn write_with_fetched_at(
        &self,
        scope: &str,
        instances_key: &str,
        list_json: &str,
        fetched_at: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO task_list_cache \
             (scope, instances_key, list_json, fetched_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![scope, instances_key, list_json, fetched_at],
        )?;
        Ok(())
    }
}

/// Returns a stable, order-independent key for a set of instances.
///
/// Sorts instance names before joining so that callers passing the same
/// instances in different orders always get the same key.
pub fn instances_key(targets: &[Instance]) -> String {
    let mut names: Vec<&str> = targets.iter().map(|i| i.name.as_str()).collect();
    names.sort_unstable();
    names.join("|")
}

#[cfg(test)]
#[path = "../../tests/unit/store/cache.rs"]
mod tests;
