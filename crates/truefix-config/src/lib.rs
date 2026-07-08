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

pub mod builder;
pub mod keys;

use std::collections::BTreeMap;

use thiserror::Error;

pub use builder::{
    ConnectionType, LenientResolve, LogKind, LogSpec, ProxyKind, ProxySpec, ResolvedSession,
    SocketEndpoint, SocketOptionsSpec, SqlLogSpec, TlsSpec, TlsVersion,
};
pub use keys::{APPENDIX_A_KEYS, KeyInfo, Stance, key_info};

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
    /// NEW-46 (feature 009): a `${var}` reference resolves back to itself, directly or
    /// transitively, instead of ever bottoming out at a literal value.
    #[error("line {line}: circular variable reference ${{{name}}}")]
    CircularVariableReference {
        /// 1-based line number of the key whose value transitively references itself.
        line: usize,
        /// The variable name at which the cycle was detected.
        name: String,
    },
    /// A required configuration key is missing for a session (FR-015).
    #[error("session {session}: missing required key `{key}`")]
    MissingRequired {
        /// The missing key.
        key: String,
        /// The session label (SenderCompID->TargetCompID, or an index).
        session: String,
    },
    /// A recognized key has an invalid value (FR-015).
    #[error("session {session}: invalid value for `{key}`: {reason}")]
    InvalidValue {
        /// The offending key.
        key: String,
        /// The session label.
        session: String,
        /// Why the value is invalid.
        reason: String,
    },
    /// `JdbcURL`'s scheme names a backend that either isn't recognized at all, or is recognized
    /// but its Cargo feature isn't compiled in (US3, feature 004, FR-003/004) — never a panic or a
    /// silent fallback to the memory store.
    #[error("session {session}: unsupported JdbcURL scheme `{scheme}`")]
    UnsupportedBackend {
        /// The session label.
        session: String,
        /// The offending URL scheme (e.g. `"mssql"`).
        scheme: String,
    },
    /// `ConnectionType` was neither `acceptor` nor `initiator` (FR-014).
    #[error("session {session}: unknown ConnectionType `{value}`")]
    UnknownConnectionType {
        /// The session label.
        session: String,
        /// The offending value.
        value: String,
    },
    /// More than one `[SESSION]` block sharing one acceptor bind address set `DynamicSession=Y`/
    /// `AcceptorTemplate` (US2, feature 005, BUG-03/FR-006) — the acceptor group has exactly one
    /// dynamic-session template slot, so this is a genuine misconfiguration, never silently
    /// resolved by picking one member's template over the other's.
    #[error("acceptor group at {addr}: more than one session declares a dynamic-session template")]
    AmbiguousAcceptorTemplate {
        /// The shared bind address every session in the ambiguous group targets.
        addr: std::net::SocketAddr,
    },
    /// NEW-129 (audit 006): two `[SESSION]` blocks resolve to an identical FIX session identity
    /// (`BeginString`+`SenderCompID`+`SenderSubID`+`SenderLocationID`+`TargetCompID`+
    /// `TargetSubID`+`TargetLocationID`+`SessionQualifier`) — matching QFJ's `SessionID`, which
    /// naturally deduplicates via map-keyed storage. Two identical `[SESSION]` blocks previously
    /// silently created two session instances instead of erroring.
    #[error("duplicate [SESSION] identity: {identity}")]
    DuplicateSession {
        /// The duplicated session identity, formatted for diagnostics.
        identity: String,
    },
    /// Two sessions in one acceptor group (sharing a `SocketAcceptPort`) would produce an
    /// identical wire-extractable routing key — distinguished only by `SessionQualifier`, which
    /// has no wire tag and so cannot disambiguate a live inbound connection (BUG-07/FR-011,
    /// feature 006). `SessionQualifier`-distinguished sessions must each be bound to their own
    /// distinct listener/port, resolved via `/speckit-clarify`.
    #[error(
        "acceptor group at {addr}: sessions {session_a} and {session_b} are distinguished only \
         by SessionQualifier, which has no wire tag -- each SessionQualifier-distinguished \
         session must be bound to its own distinct SocketAcceptPort"
    )]
    AmbiguousSessionQualifier {
        /// The shared bind address both conflicting sessions target.
        addr: std::net::SocketAddr,
        /// The first conflicting session's label.
        session_a: String,
        /// The second conflicting session's label.
        session_b: String,
    },
    /// NEW-158 (feature 012): a session was given a caller-supplied custom store/log override
    /// (via `Engine::start_with_overrides`) alongside a built-in-only `.cfg` setting that the
    /// override would silently ignore (e.g. `MaxFileLogSize`/the `FileLogMaxGenerations`/
    /// `FileLogRollIntervalSecs` retention keys only apply to the built-in `FileLog`, not to a
    /// custom `Log` override) -- rejected at startup (FR-010) rather than silently doing nothing.
    #[error(
        "session {session}: a custom {kind} override was supplied alongside built-in-only .cfg \
         settings ({detail}), which the override would silently ignore"
    )]
    CustomOverrideWithBuiltinOnlySetting {
        /// The session label.
        session: String,
        /// Which override collided: `"store"` or `"log"`.
        kind: String,
        /// The specific built-in-only setting(s) that would be ignored.
        detail: String,
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
        // `current` accumulates the raw key/(value, source line) pairs of the section being read.
        // NEW-47 (feature 009): the line number travels alongside each value all the way through
        // default/session merging so an `UnresolvedVariable`/`CircularVariableReference` error can
        // report the original source line instead of always reporting `line: 0`.
        let mut default: RawSection = BTreeMap::new();
        let mut sessions: Vec<RawSection> = Vec::new();
        // None = no section yet; Some(true) = DEFAULT; Some(false) = a SESSION.
        let mut in_default: Option<bool> = None;
        let mut current: RawSection = BTreeMap::new();

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
            current.insert(key.trim().to_owned(), (value.trim().to_owned(), line_no));
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

        let sessions: Vec<BTreeMap<String, String>> =
            resolved_sessions.into_iter().map(strip_lines).collect();
        check_duplicate_sessions(&sessions)?;

        Ok(Self {
            default: strip_lines(default),
            sessions,
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

/// A section's raw key/value pairs, each tagged with the 1-based source line it was declared on
/// (NEW-47/FR-081) — carried through parsing, default/session merging, and interpolation so a
/// resolution error can report where the offending `${var}` reference actually appeared.
type RawSection = BTreeMap<String, (String, usize)>;

fn strip_lines(map: RawSection) -> BTreeMap<String, String> {
    map.into_iter().map(|(k, (v, _))| (k, v)).collect()
}

/// NEW-129 (audit 006): reject two `[SESSION]` blocks that resolve to an identical FIX session
/// identity, mirroring QFJ's `SessionID`-keyed storage (which naturally deduplicates).
fn check_duplicate_sessions(sessions: &[BTreeMap<String, String>]) -> Result<(), ConfigError> {
    let identity_of = |m: &BTreeMap<String, String>| -> Vec<String> {
        [
            "BeginString",
            "SenderCompID",
            "SenderSubID",
            "SenderLocationID",
            "TargetCompID",
            "TargetSubID",
            "TargetLocationID",
            "SessionQualifier",
        ]
        .into_iter()
        .map(|k| m.get(k).cloned().unwrap_or_default())
        .collect()
    };
    let mut seen: Vec<Vec<String>> = Vec::with_capacity(sessions.len());
    for session in sessions {
        let identity = identity_of(session);
        if seen.contains(&identity) {
            return Err(ConfigError::DuplicateSession {
                identity: identity.join("/"),
            });
        }
        seen.push(identity);
    }
    Ok(())
}

fn flush_section(
    in_default: Option<bool>,
    current: &mut RawSection,
    default: &mut RawSection,
    sessions: &mut Vec<RawSection>,
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

/// Strip a trailing/whole-line `#` comment (BUG-01, feature 005). A `#` only starts a comment when
/// it's the first character of the line or immediately preceded by whitespace — matching this
/// codebase's own pre-existing, intentionally-tested trailing-comment support (`ConnectionType=
/// initiator   # comment`), while no longer truncating a value like `Password=ab#cd`, where `#` sits
/// immediately after a non-whitespace character. Neither reference engine supports trailing comments
/// at all (QFJ/QFGo both only treat a `#`-led *line* as a comment); this whitespace-boundary rule is
/// a deliberate, disclosed TrueFix-only extension of that stricter behavior, not a departure from it
/// for the case both references actually define (`Password=ab#cd` still keeps the `#`, matching both).
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut search_from = 0;
    while let Some(rel) = line.get(search_from..).and_then(|s| s.find('#')) {
        let idx = search_from + rel;
        let preceded_by_boundary =
            idx == 0 || bytes.get(idx - 1).is_some_and(|b| b.is_ascii_whitespace());
        if preceded_by_boundary {
            return line.get(..idx).unwrap_or("");
        }
        search_from = idx + 1;
    }
    line
}

fn section_header(line: &str) -> Option<&str> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    Some(inner.trim())
}

/// Interpolate `${name}` occurrences in every value of `map`, looking names up in `lookup`.
fn interpolate_map(map: &RawSection, lookup: &RawSection) -> Result<RawSection, ConfigError> {
    let mut out = BTreeMap::new();
    for (k, (v, line)) in map {
        let resolved = interpolate_value(v, *line, lookup, &mut Vec::new())?;
        out.insert(k.clone(), (resolved, *line));
    }
    Ok(out)
}

/// Resolves `${name}` occurrences in `value`, transitively following a chain of variable
/// references (e.g. `A=${B}`, `B=literal`) rather than only substituting one level. `line` is the
/// source line of the top-level key being resolved — attributed to any error raised while
/// resolving it, however many hops deep the reference chain that triggered it goes.
///
/// `resolving` is the set of variable names already being expanded on the current call stack
/// (NEW-46/FR-069): encountering one of them again means the reference chain loops back on
/// itself without ever bottoming out at a literal value, which is reported as a
/// `CircularVariableReference` rather than silently emitting the literal, still-templated text.
fn interpolate_value(
    value: &str,
    line: usize,
    lookup: &RawSection,
    resolving: &mut Vec<String>,
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
                line,
                name: after.to_owned(),
            })?;
        let name = after.get(..end).unwrap_or("");
        if resolving.iter().any(|n| n == name) {
            return Err(ConfigError::CircularVariableReference {
                line,
                name: name.to_owned(),
            });
        }
        // GAP-44/FR-041 (feature 006): fall back to an environment variable when `name` isn't in
        // the settings map itself — previously `${var}` only ever resolved against `lookup`.
        let replacement = match lookup.get(name) {
            Some((raw, _)) => {
                resolving.push(name.to_owned());
                let resolved = interpolate_value(raw, line, lookup, resolving);
                resolving.pop();
                resolved?
            }
            None => std::env::var(name).map_err(|_| ConfigError::UnresolvedVariable {
                line,
                name: name.to_owned(),
            })?,
        };
        result.push_str(&replacement);
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

    // --- T077 (US8, feature 006): ${var} environment-variable fallback (GAP-44/FR-041) ---
    //
    // `std::env::set_var`/`remove_var` require `unsafe` in this toolchain, which this workspace
    // forbids everywhere (including tests, per `unsafe_code = "forbid"` — Constitution Principle
    // I) — so these tests use `PATH`, a variable every test process already has set, rather than
    // mutating the process environment themselves.

    #[test]
    fn variable_interpolation_falls_back_to_environment_variable() {
        let expected_path = std::env::var("PATH").expect("PATH must be set in the test process");
        let cfg = "\
[SESSION]
SenderCompID=A
Endpoint=${PATH}
";
        let s = SessionSettings::parse(cfg).unwrap();
        assert_eq!(s.sessions()[0].get("Endpoint"), Some(&expected_path));
    }

    #[test]
    fn variable_interpolation_prefers_settings_map_over_environment() {
        // PATH is (almost certainly) set in the environment too, but the settings map's own
        // PATH key must win.
        let cfg = "\
[DEFAULT]
PATH=from-settings-not-the-real-path

[SESSION]
SenderCompID=A
Endpoint=${PATH}
";
        let s = SessionSettings::parse(cfg).unwrap();
        assert_eq!(
            s.sessions()[0].get("Endpoint"),
            Some(&"from-settings-not-the-real-path".to_string())
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
    fn hash_immediately_after_a_value_is_not_treated_as_a_comment() {
        // BUG-01 (feature 005): `strip_comment` used to find the *first* `#` anywhere on the line,
        // truncating a value like `Password=ab#cd` to `ab`. The fix only treats `#` as a comment
        // start when it's at the start of the line or preceded by whitespace — preserving the
        // pre-existing, intentionally-tested trailing-comment support (`comments_and_blank_lines_
        // ignored` above) while no longer corrupting a value with a `#` immediately inside it.
        let cfg = "\
[SESSION]
SenderCompID=A
Password=ab#cd
";
        let s = SessionSettings::parse(cfg).unwrap();
        assert_eq!(s.sessions()[0].get("Password"), Some(&"ab#cd".to_string()));
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
