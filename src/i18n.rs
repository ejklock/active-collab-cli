use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

pub const SUPPORTED: [&str; 2] = ["en", "pt_BR"];

static LANGUAGE: RwLock<String> = RwLock::new(String::new());

static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();

fn pt_br_catalog() -> &'static HashMap<String, String> {
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/pt_BR.json"))
            .expect("pt_BR.json is embedded and must be valid JSON")
    })
}

/// Strip the encoding suffix (e.g. ".UTF-8") and map to a supported locale code.
/// Returns `Some("pt_BR")` for `"pt_BR"` variants, `None` for everything else.
fn normalize_locale(raw: &str) -> Option<&'static str> {
    let locale = raw.split('.').next().unwrap_or("");
    if locale == "pt_BR" {
        return Some("pt_BR");
    }
    None
}

/// Return the active language applying precedence: `env_value` > `db_value` > `"en"`.
///
/// Unknown or empty values fall through to the next source in the chain.
/// Mirrors `i18n.resolve_language(env_value, db_value)` from the Python parity oracle.
pub fn resolve_language(env_value: Option<&str>, db_value: Option<&str>) -> String {
    for candidate in [env_value, db_value].into_iter().flatten() {
        if candidate.is_empty() {
            continue;
        }
        // normalize_locale recognizes only "pt_BR" variants; "en" yields None
        // and falls through to the next source, matching Python _normalize_locale.
        if let Some(code) = normalize_locale(candidate) {
            if SUPPORTED.contains(&code) {
                return code.to_owned();
            }
        }
    }
    "en".to_owned()
}

/// Set the process-global display language.
/// Panics only if the lock is poisoned (unrecoverable).
pub fn set_language(lang: &str) {
    let mut guard = LANGUAGE.write().expect("language lock poisoned");
    *guard = lang.to_owned();
}

/// Return the current display language code.
pub fn current_language() -> String {
    let guard = LANGUAGE.read().expect("language lock poisoned");
    if guard.is_empty() {
        "en".to_owned()
    } else {
        guard.clone()
    }
}

/// Translate `s` using the active-language catalog. Mirrors Python `__(s)`.
///
/// Under `"pt_BR"`, returns the catalog translation for known keys and `s` unchanged
/// for unknown keys. Under any other language, returns `s` unchanged (identity).
pub fn t(s: &str) -> String {
    if current_language() == "pt_BR" {
        pt_br_catalog()
            .get(s)
            .cloned()
            .unwrap_or_else(|| s.to_owned())
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
#[path = "../tests/unit/i18n.rs"]
mod tests;
