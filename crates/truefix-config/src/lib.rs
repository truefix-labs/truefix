//! `truefix-config` — QuickFIX-style `.cfg` settings parsing.
//!
//! Parses the `[DEFAULT]` / `[SESSION]` dialect: a `[DEFAULT]` section whose keys are inherited
//! by every `[SESSION]` (per-session keys override), with `${name}` variable interpolation.
//! The full Appendix A key surface and the mapping to `SessionConfig` are filled in across
//! later stages; Stage S2 provides parsing + inheritance + interpolation.
//!
//! Design: `specs/001-fix-engine-parity/`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use std::collections::BTreeMap;

use thiserror::Error;

/// An error parsing a settings document.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// A non-empty, non-comment line was not `key=value` and not a `[Section]` header.
    #[error("line {line}: expected `key=value` or `[Section]`, got {text:?}")]
    Malformed {
        /// 1-based line number.
        line: usize,
        /// The offending text.
        text: String,
    },
    /// A `${name}` reference could not be resolved.
    #[error("line {line}: unresolved variable ${{{name}}}")]
    UnresolvedVariable {
        /// 1-based line number.
        line: usize,
        /// The variable name.
        name: String,
    },
}

/// A parsed settings document: the `[DEFAULT]` section plus the `[SESSION]` sections (each with
/// defaults already merged in).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionSettings {
    default: BTreeMap<String, String>,
    sessions: Vec<BTreeMap<String, String>>,
}

impl SessionSettings {
    /// Parse a settings document.
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        // `current` accumulates the raw key/values of the section being read.
        let mut default: BTreeMap<String, String> = BTreeMap::new();
        let mut sessions: Vec<BTreeMap<String, String>> = Vec::new();
        // None = no section yet; Some(true) = DEFAULT; Some(false) = a SESSION.
        let mut in_default: Option<bool> = None;
        let mut current: BTreeMap<String, String> = BTreeMap::new();

        for (idx, raw) in input.lines().enumerate() {
            let line_no = idx + 1;
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }

            if let Some(name) = section_header(line) {
                flush_section(in_default, &mut current, &mut default, &mut sessions);
                in_default = Some(name.eq_ignore_ascii_case("DEFAULT"));
                current = BTreeMap::new();
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| ConfigError::Malformed {
                line: line_no,
                text: line.to_owned(),
            })?;
            current.insert(key.trim().to_owned(), value.trim().to_owned());
        }
        flush_section(in_default, &mut current, &mut default, &mut sessions);

        // Resolve `${name}` in default first, then in each session against (default + session).
        let default = interpolate_map(&default, &default)?;
        let mut resolved_sessions = Vec::with_capacity(sessions.len());
        for session in &sessions {
            let mut merged = default.clone();
            for (k, v) in session {
                merged.insert(k.clone(), v.clone());
            }
            resolved_sessions.push(interpolate_map(&merged, &merged)?);
        }

        Ok(Self {
            default,
            sessions: resolved_sessions,
        })
    }

    /// The resolved `[DEFAULT]` section.
    pub fn default_section(&self) -> &BTreeMap<String, String> {
        &self.default
    }

    /// The resolved `[SESSION]` sections (each already merged with defaults).
    pub fn sessions(&self) -> &[BTreeMap<String, String>] {
        &self.sessions
    }
}

fn flush_section(
    in_default: Option<bool>,
    current: &mut BTreeMap<String, String>,
    default: &mut BTreeMap<String, String>,
    sessions: &mut Vec<BTreeMap<String, String>>,
) {
    match in_default {
        Some(true) => {
            for (k, v) in current.iter() {
                default.insert(k.clone(), v.clone());
            }
        }
        Some(false) if !current.is_empty() => {
            sessions.push(std::mem::take(current));
        }
        _ => {}
    }
    current.clear();
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => line.get(..idx).unwrap_or(""),
        None => line,
    }
}

fn section_header(line: &str) -> Option<&str> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.trim())
}

/// Interpolate `${name}` occurrences in every value of `map`, looking names up in `lookup`.
fn interpolate_map(
    map: &BTreeMap<String, String>,
    lookup: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ConfigError> {
    let mut out = BTreeMap::new();
    for (k, v) in map {
        out.insert(k.clone(), interpolate_value(v, lookup)?);
    }
    Ok(out)
}

fn interpolate_value(
    value: &str,
    lookup: &BTreeMap<String, String>,
) -> Result<String, ConfigError> {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        let (before, after) = rest.split_at(start);
        result.push_str(before);
        let after = after.get(2..).unwrap_or("");
        let end = after
            .find('}')
            .ok_or_else(|| ConfigError::UnresolvedVariable {
                line: 0,
                name: after.to_owned(),
            })?;
        let name = after.get(..end).unwrap_or("");
        let replacement = lookup
            .get(name)
            .ok_or_else(|| ConfigError::UnresolvedVariable {
                line: 0,
                name: name.to_owned(),
            })?;
        result.push_str(replacement);
        rest = after.get(end + 1..).unwrap_or("");
    }
    result.push_str(rest);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_inherited_and_session_overrides() {
        let cfg = "\
[DEFAULT]
ConnectionType=acceptor
HeartBtInt=30

[SESSION]
BeginString=FIX.4.2
SenderCompID=SERVER

[SESSION]
BeginString=FIX.4.4
HeartBtInt=10
SenderCompID=SERVER2
";
        let s = SessionSettings::parse(cfg).unwrap();
        assert_eq!(s.sessions().len(), 2);
        // inherited default
        assert_eq!(s.sessions()[0].get("HeartBtInt"), Some(&"30".to_string()));
        // per-session override
        assert_eq!(s.sessions()[1].get("HeartBtInt"), Some(&"10".to_string()));
        assert_eq!(
            s.sessions()[0].get("BeginString"),
            Some(&"FIX.4.2".to_string())
        );
    }

    #[test]
    fn variable_interpolation() {
        let cfg = "\
[DEFAULT]
Host=example.com
Port=5001

[SESSION]
SenderCompID=A
Endpoint=${Host}:${Port}
";
        let s = SessionSettings::parse(cfg).unwrap();
        assert_eq!(
            s.sessions()[0].get("Endpoint"),
            Some(&"example.com:5001".to_string())
        );
    }

    #[test]
    fn comments_and_blank_lines_ignored() {
        let cfg = "\
# a comment
[DEFAULT]
ConnectionType=initiator   # trailing comment

[SESSION]
SenderCompID=A
";
        let s = SessionSettings::parse(cfg).unwrap();
        assert_eq!(
            s.default_section().get("ConnectionType"),
            Some(&"initiator".to_string())
        );
        assert_eq!(s.sessions().len(), 1);
    }

    #[test]
    fn unresolved_variable_errors() {
        let cfg = "[SESSION]\nX=${Missing}\n";
        assert!(matches!(
            SessionSettings::parse(cfg),
            Err(ConfigError::UnresolvedVariable { .. })
        ));
    }

    #[test]
    fn malformed_line_errors() {
        let cfg = "[SESSION]\nthis is not valid\n";
        assert!(matches!(
            SessionSettings::parse(cfg),
            Err(ConfigError::Malformed { .. })
        ));
    }
}
