#![allow(dead_code)]

use std::path::PathBuf;

pub struct Config {
    pub db_path: PathBuf,
    pub task_cache_ttl_hours: u32,
}

pub fn load() -> Config {
    let db_path = resolve_db_path();
    Config {
        db_path,
        task_cache_ttl_hours: 24,
    }
}

fn resolve_db_path() -> PathBuf {
    if let Ok(val) = std::env::var("ACTIVE_COLLAB_DB") {
        return PathBuf::from(val);
    }
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".config")
        .join("active-collab")
        .join("active-collab.db")
}

#[cfg(test)]
#[path = "../tests/unit/config.rs"]
mod tests;
