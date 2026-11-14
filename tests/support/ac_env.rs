//! A scratch store and the `ac` command pointed at it, so a test never
//! reads or writes the owner's store, home or temp directory.
//!
//! Included with `#[path]` by integration tests; each includes it as `mod
//! ac_env` and uses the part it needs.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;
use wiremock::MockServer;

pub struct Env {
    pub dir: TempDir,
    pub db: PathBuf,
}

impl Env {
    /// A fresh store holding one instance per `(name, server, user id)`.
    pub fn new(instances: &[(&str, &MockServer, i64)]) -> Env {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("ac.db");
        std::fs::create_dir(dir.path().join("tmp")).unwrap();
        let env = Env { dir, db };
        let init = env.command().args(["setup", "list"]).output().unwrap();
        assert!(init.status.success(), "store init failed: {init:?}");
        let conn = rusqlite::Connection::open(&env.db).unwrap();
        for (name, server, user_id) in instances {
            conn.execute(
                "INSERT INTO instances (name, base_url, email, token, user_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, '2026-01-01T00:00:00Z')",
                rusqlite::params![
                    name,
                    server.uri(),
                    format!("{name}@example.com"),
                    format!("tok-{name}"),
                    user_id
                ],
            )
            .unwrap();
        }
        env
    }

    /// The `ac` binary with the store, language, home and temp directory
    /// confined to this scratch directory.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ac"));
        cmd.env("ACTIVE_COLLAB_DB", &self.db)
            .env("ACTIVE_COLLAB_LANG", "en")
            .env("HOME", self.dir.path())
            .env("TMPDIR", self.dir.path().join("tmp"))
            .current_dir(self.dir.path());
        cmd
    }

    /// Runs `ac` with `args`, requires success and returns its stdout.
    pub fn cli(&self, args: &[&str]) -> String {
        let output = self.command().args(args).output().unwrap();
        assert!(output.status.success(), "ac {args:?} failed: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }
}
