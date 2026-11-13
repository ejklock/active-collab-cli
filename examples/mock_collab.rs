//! A throwaway ActiveCollab stand-in for terminal replays of `ac`.
//!
//! Usage: `mock_collab <store-path> <ac-binary>`. The store path is resolved
//! once to an absolute path that is checked, handed to `ac` and opened alike.
//!
//! It creates a fresh store at `<store-path>` through `<ac-binary>`, registers
//! one instance pointing at its own loopback address, prints one ready line
//! holding that address, serves the seeded routes and exits when stdin closes.
//! Each request is logged as one line in `requests.log` beside the store,
//! saying whether the instance token came with it; a request no route serves is
//! marked `unmatched` and answered 404.
//! It refuses to run against an existing store, against the owner's default
//! store, or without an executable `ac`, so a replay can never touch the real
//! instance.

use anyhow::{bail, Context, Result};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use wiremock::MockServer;

#[path = "../tests/support/mock_collab.rs"]
mod mock_collab;

use mock_collab::{mount_routes, Fixture};

const INSTANCE_NAME: &str = "mock";
const REQUEST_LOG: &str = "requests.log";

/// The store path made absolute with its parent directory resolved, so a
/// symlinked folder or a relative spelling compares equal to the real place.
fn resolved(store: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(store).context("cannot make the store path absolute")?;
    let name = absolute
        .file_name()
        .context("the store path has no file name")?
        .to_owned();
    let parent = absolute
        .parent()
        .context("the store path has no parent folder")?;
    let parent = parent
        .canonicalize()
        .with_context(|| format!("the store folder {} does not exist", parent.display()))?;
    Ok(parent.join(name))
}

/// Judges the resolved store path, the one `ac` and rusqlite open.
fn is_owner_store(store: &Path) -> Result<bool> {
    let Some(home) = dirs::home_dir() else {
        return Ok(false);
    };
    let owner_dir = home.join(".config").join("active-collab");
    let owner_dir = owner_dir.canonicalize().unwrap_or(owner_dir);
    let owner_store = owner_dir.join("active-collab.db");
    Ok(store.parent() == owner_store.parent()
        && match (store.file_name(), owner_store.file_name()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        })
}

fn check_arguments(store: &Path, ac: &Path) -> Result<()> {
    if store.symlink_metadata().is_ok() {
        bail!("refusing to reuse the existing store {}", store.display());
    }
    if is_owner_store(store)? {
        bail!(
            "refusing to use the owner's default store {}",
            store.display()
        );
    }
    let meta = ac
        .metadata()
        .with_context(|| format!("the ac binary {} is missing", ac.display()))?;
    if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
        bail!("the ac binary {} is not executable", ac.display());
    }
    Ok(())
}

/// Creates the store by running `ac` against it with a closed environment, so
/// the binary can only ever see the scratch folder.
fn create_store(store: &Path, ac: &Path) -> Result<()> {
    let scratch = store
        .parent()
        .map(Path::to_path_buf)
        .context("the store path has no parent folder")?;
    let output = Command::new(ac)
        .args(["setup", "list"])
        .env_clear()
        .env("ACTIVE_COLLAB_DB", store)
        .env("ACTIVE_COLLAB_LANG", "en")
        .env("HOME", &scratch)
        .env("TMPDIR", &scratch)
        .current_dir(&scratch)
        .output()
        .context("cannot run the ac binary")?;
    if !output.status.success() {
        bail!(
            "creating the store failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn register_instance(store: &Path, base_url: &str, user_id: i64) -> Result<()> {
    let conn = rusqlite::Connection::open(store).context("cannot open the new store")?;
    conn.execute(
        "INSERT INTO instances (name, base_url, email, token, user_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, '2026-01-01T00:00:00Z')",
        rusqlite::params![
            INSTANCE_NAME,
            base_url,
            "mock@example.com",
            "mock-token",
            user_id
        ],
    )
    .context("cannot register the mock instance")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(store), Some(ac), None) = (args.next(), args.next(), args.next()) else {
        bail!("usage: mock_collab <store-path> <ac-binary>");
    };
    let store = resolved(Path::new(&store))?;
    let ac = std::path::absolute(&ac).context("cannot make the ac path absolute")?;

    check_arguments(&store, &ac)?;
    create_store(&store, &ac)?;

    let server = MockServer::start().await;
    let fixture = Fixture::seeded();
    register_instance(&store, &server.uri(), fixture.user_id)?;
    let log_path = store
        .parent()
        .context("the store path has no parent folder")?
        .join(REQUEST_LOG);
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("cannot create the request log {}", log_path.display()))?;
    mount_routes(&server, &fixture, Some(&Arc::new(Mutex::new(log)))).await;

    println!("mock-collab ready {}", server.uri());
    std::io::stdout()
        .flush()
        .context("cannot print the ready line")?;

    tokio::task::spawn_blocking(|| std::io::copy(&mut std::io::stdin(), &mut std::io::sink()))
        .await
        .context("the stdin watcher failed")?
        .context("cannot read stdin")?;
    Ok(())
}
