use super::pick_instance;
use crate::client::ActiveCollabClient;
use crate::config;
use crate::http::Http;
use crate::store::instances::{Instance, InstanceRepository};
use crate::store::Store;
use std::io::Write;

/// The git branch of the process working directory, or `None` outside a
/// repository or on a detached HEAD.
pub(crate) fn current_git_branch() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}

/// Open the local store, writing the failure to `err` and returning `None`.
pub(crate) fn open_store(err: &mut dyn Write) -> Option<Store> {
    let config = config::load();
    match Store::open(&config) {
        Ok(store) => Some(store),
        Err(e) => {
            writeln!(err, "Error opening database: {e}").ok();
            None
        }
    }
}

/// Open the store and build the HTTP client. `Err(exit_code)` after the
/// failure has been written to `err`.
pub(crate) fn open_store_and_http(err: &mut dyn Write) -> Result<(Store, Http), i32> {
    let store = open_store(err).ok_or(1)?;
    let http = Http::new().map_err(|e| {
        writeln!(err, "Error building HTTP client: {e}").ok();
        1
    })?;
    Ok((store, http))
}

/// The store, the resolved target instance and its API client, for the
/// commands that act on a single instance.
pub(crate) struct InstanceContext {
    pub store: Store,
    pub instance: Instance,
    pub client: ActiveCollabClient,
}

/// Open the store, build the HTTP client, load the instances, resolve the
/// target through `instance_name` (or the only configured instance) and build
/// its API client. `Err(exit_code)` after the failure has been written to
/// `err`.
pub(crate) fn setup_instance_client(
    instance_name: Option<&str>,
    err: &mut dyn Write,
) -> Result<InstanceContext, i32> {
    let (store, http) = open_store_and_http(err)?;
    let instances = InstanceRepository::new(store.conn())
        .load_all()
        .map_err(|e| {
            writeln!(err, "Error loading instances: {e}").ok();
            1
        })?;
    let idx = pick_instance(&instances, instance_name, err)?;
    let instance = instances[idx].clone();
    let client = ActiveCollabClient::new(instance.clone(), http);
    Ok(InstanceContext {
        store,
        instance,
        client,
    })
}
