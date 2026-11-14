use crate::client::ActiveCollabClient;
use crate::http::Http;
use crate::i18n::{t, SUPPORTED};
use crate::store::cache::TaskCache;
use crate::store::instances::{Instance, InstanceRepository};
use crate::store::settings::SettingsRepository;
use std::io::Write;

/// Parity: Python cmd_setup_list.
pub(crate) fn setup_list(repo: &InstanceRepository<'_>, out: &mut dyn Write) -> i32 {
    let rows = match repo.list_for_display() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error reading instances: {e}");
            return 1;
        }
    };

    if rows.is_empty() {
        writeln!(out, "{}", t("No instances configured. Run: ac setup add")).ok();
        return 0;
    }

    writeln!(out, "{:<20} {:<40} {:<30} USER_ID", "NAME", "URL", "EMAIL").ok();
    writeln!(out, "{}", "-".repeat(100)).ok();
    for (name, base_url, email, user_id) in &rows {
        let uid_str = user_id.map(|v| v.to_string()).unwrap_or_default();
        writeln!(
            out,
            "{:<20} {:<40} {:<30} {}",
            name, base_url, email, uid_str
        )
        .ok();
    }
    0
}

/// Parity: Python cmd_setup_remove.
pub(crate) fn setup_remove(
    repo: &InstanceRepository<'_>,
    cache: &TaskCache<'_>,
    name: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let deleted = match repo.delete(name) {
        Ok(n) => n,
        Err(e) => {
            writeln!(err, "Error deleting instance: {e}").ok();
            return 1;
        }
    };
    cache.delete_for_instance(name).ok();

    if deleted == 0 {
        writeln!(
            err,
            "{}",
            t("Error: instance '{name}' not found.").replace("{name}", name)
        )
        .ok();
        return 2;
    }
    writeln!(
        out,
        "{}",
        t("Instance '{name}' removed.").replace("{name}", name)
    )
    .ok();
    0
}

/// Parity: Python cmd_setup_language.
pub(crate) fn setup_language(
    settings: &SettingsRepository<'_>,
    code: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match code {
        None => {
            let current = settings
                .get("language", Some("en"))
                .unwrap_or(Some("en".to_owned()))
                .unwrap_or_else(|| "en".to_owned());
            writeln!(
                out,
                "{}",
                t("Current language: {code}").replace("{code}", &current)
            )
            .ok();
            0
        }
        Some(c) => {
            if !SUPPORTED.contains(&c) {
                let supported = SUPPORTED.join(", ");
                writeln!(
                    err,
                    "{}",
                    t("Error: unsupported language '{code}'. Supported: {supported}.")
                        .replace("{supported}", &supported)
                        .replace("{code}", c)
                )
                .ok();
                return 2;
            }
            if let Err(e) = settings.set("language", c) {
                writeln!(err, "Error saving language: {e}").ok();
                return 1;
            }
            writeln!(
                out,
                "{}",
                t("Language set to '{code}'.").replace("{code}", c)
            )
            .ok();
            0
        }
    }
}

const THEMES: [&str; 3] = ["angie", "slate", "nord"];

/// Parity: setup_language, adapted for the theme setting.
pub(crate) fn setup_theme(
    settings: &SettingsRepository<'_>,
    code: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match code {
        None => {
            let current = settings
                .get("theme", Some("angie"))
                .unwrap_or(Some("angie".to_owned()))
                .unwrap_or_else(|| "angie".to_owned());
            writeln!(
                out,
                "{}",
                t("Current theme: {code}").replace("{code}", &current)
            )
            .ok();
            0
        }
        Some(c) => {
            let c = c.to_ascii_lowercase();
            if !THEMES.contains(&c.as_str()) {
                let list = THEMES.join(", ");
                writeln!(
                    err,
                    "{}",
                    t("Error: unsupported theme '{code}'. Supported: {list}.")
                        .replace("{list}", &list)
                        .replace("{code}", &c)
                )
                .ok();
                return 2;
            }
            if let Err(e) = settings.set("theme", &c) {
                writeln!(err, "Error saving theme: {e}").ok();
                return 1;
            }
            writeln!(out, "{}", t("Theme set to '{code}'.").replace("{code}", &c)).ok();
            0
        }
    }
}

/// Inner core for connectivity test — takes pre-resolved rows.
///
/// Parity: Python cmd_setup_test inner loop.
pub(crate) async fn setup_test_core(
    rows: Vec<(String, String, String)>,
    http: Http,
    out: &mut dyn Write,
) -> i32 {
    let mut exit_code = 0i32;
    for (name, base_url, token) in rows {
        let inst = Instance {
            name: name.clone(),
            base_url: base_url.clone(),
            email: String::new(),
            token: token.clone(),
            user_id: None,
        };
        let client = ActiveCollabClient::new(inst, http.clone());
        match client.test_connectivity().await {
            Ok((200, _)) => {
                writeln!(
                    out,
                    "  {name}: {}",
                    t("OK ({status})").replace("{status}", "200")
                )
                .ok();
            }
            Ok((status, _)) => {
                writeln!(
                    out,
                    "  {name}: {}",
                    t("FAILED (HTTP {status})").replace("{status}", &status.to_string())
                )
                .ok();
                exit_code = 1;
            }
            Err(exc) => {
                writeln!(
                    out,
                    "  {name}: {}",
                    t("FAILED ({exc})").replace("{exc}", &exc.to_string())
                )
                .ok();
                exit_code = 1;
            }
        }
    }
    exit_code
}

/// Thin wrapper that resolves rows from repo and delegates to setup_test_core.
///
/// Parity: Python cmd_setup_test (resolution + dispatch).
pub(crate) async fn setup_test(
    repo: &InstanceRepository<'_>,
    name: Option<&str>,
    http: Http,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let rows = match name {
        Some(n) => {
            let found = match repo.find_by_name(n) {
                Ok(r) => r,
                Err(e) => {
                    writeln!(err, "Error querying instances: {e}").ok();
                    return 1;
                }
            };
            if found.is_empty() {
                writeln!(
                    err,
                    "{}",
                    t("Error: instance '{name}' not found.").replace("{name}", n)
                )
                .ok();
                return 2;
            }
            found
        }
        None => match repo.list_connectivity() {
            Ok(r) => r,
            Err(e) => {
                writeln!(err, "Error querying instances: {e}").ok();
                return 1;
            }
        },
    };

    setup_test_core(rows, http, out).await
}

/// The fields resolved from flags / prompts before calling setup_add.
pub(crate) struct SetupAddFields {
    pub name: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
}

struct AddInput {
    name: String,
    url: String,
    email: String,
    password: String,
}

fn parse_add_input(fields: SetupAddFields, password: Option<String>) -> Result<AddInput, String> {
    let (name, url, email) = match (fields.name, fields.url, fields.email) {
        (Some(n), Some(u), Some(e)) if !n.is_empty() && !u.is_empty() && !e.is_empty() => (n, u, e),
        _ => return Err(t("Error: --name, --url and --email are required.")),
    };
    let password = match password {
        Some(p) if !p.is_empty() => p,
        _ => return Err(t("Error: password is required.")),
    };
    Ok(AddInput {
        name,
        url,
        email,
        password,
    })
}

/// Parity: Python cmd_setup_add (the testable core without stdin/rpassword).
///
/// `check_connectivity`: when true (interactive TTY), run connectivity check after save.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn setup_add(
    fields: SetupAddFields,
    password: Option<String>,
    repo: &InstanceRepository<'_>,
    http: Http,
    check_connectivity: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let AddInput {
        name,
        url,
        email,
        password,
    } = match parse_add_input(fields, password) {
        Ok(input) => input,
        Err(message) => {
            writeln!(err, "{message}").ok();
            return 2;
        }
    };

    let base_url = url.trim_end_matches('/').to_owned();

    let dummy_inst = Instance {
        name: String::new(),
        base_url: base_url.clone(),
        email: email.clone(),
        token: String::new(),
        user_id: None,
    };
    let client = ActiveCollabClient::new(dummy_inst, http.clone());

    let (token_opt, response) = match client.exchange_token(&base_url, &email, &password).await {
        Ok(pair) => pair,
        Err(exc) => {
            writeln!(
                err,
                "{}",
                t("Error: {exc}").replace("{exc}", &exc.to_string())
            )
            .ok();
            return 1;
        }
    };

    // Drop the password immediately after token exchange — never retain it.
    let _password = password;

    let token = match token_opt {
        Some(t) => t,
        None => {
            let detail = response
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("token exchange failed")
                .to_owned();
            writeln!(err, "{}", t("Error: {detail}").replace("{detail}", &detail)).ok();
            return 1;
        }
    };

    let authed_inst = Instance {
        name: name.clone(),
        base_url: base_url.clone(),
        email: email.clone(),
        token: token.clone(),
        user_id: None,
    };
    let authed_client = ActiveCollabClient::new(authed_inst, http.clone());

    let user_id = authed_client
        .resolve_user_id(&base_url, &token, &email)
        .await
        .unwrap_or(None);

    let instance = Instance {
        name: name.clone(),
        base_url,
        email,
        token,
        user_id,
    };

    if let Err(e) = repo.save(&instance) {
        writeln!(err, "Error saving instance: {e}").ok();
        return 1;
    }

    writeln!(
        out,
        "{}",
        t("Instance '{name}' saved.").replace("{name}", &name)
    )
    .ok();

    if check_connectivity {
        run_connectivity_check(&authed_client, out).await;
    }

    0
}

/// Parity: Python _run_connectivity_check.
pub(crate) async fn run_connectivity_check(client: &ActiveCollabClient, out: &mut dyn Write) {
    match client.test_connectivity().await {
        Ok((200, _)) => {
            writeln!(out, "{}", t("Connectivity: OK")).ok();
        }
        Ok((status, _)) => {
            writeln!(
                out,
                "{}",
                t("Connectivity: FAILED (HTTP {status})").replace("{status}", &status.to_string())
            )
            .ok();
        }
        Err(exc) => {
            writeln!(
                out,
                "{}",
                t("Connectivity: FAILED ({exc})").replace("{exc}", &exc.to_string())
            )
            .ok();
        }
    }
}
