//! Untrusted-text render boundary (ADR 0068).
//!
//! Server-supplied task and comment text reaches this module before it is written
//! to a terminal, so a collaborator cannot smuggle ANSI/OSC escape sequences into a
//! victim's terminal via a task body, comment, or name (issue 0062 C1).

/// Remove control characters from server-supplied text before it reaches a terminal.
///
/// Keeps `\n` and `\t`; drops every other character in Rust's `char::is_control`
/// range (Unicode Cc): C0 (U+0000-U+001F), DEL (U+007F), and C1 (U+0080-U+009F).
pub fn strip_control_chars(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

#[cfg(test)]
#[path = "../tests/unit/sanitize.rs"]
mod tests;
