//! Binance-specific `.cfg` keys.
//!
//! `truefix_config::SessionSettings` only maps the standard QuickFIX Appendix-A keys (session
//! identity, socket, heartbeat, ...) into a `SessionConfig`/address. Binance's own auth and
//! endpoint behavior (Ed25519 API key, `MessageHandling`/`ResponseMode`/`RecvWindow`,
//! `DropCopyFlag`) has no Appendix-A key, so it's read directly off each `[SESSION]`'s raw parsed
//! map — `truefix-config` passes unrecognized keys through untouched, so this coexists cleanly
//! with the standard keys in the same `.cfg` file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::DecodePrivateKey;

/// Which of Binance's three FIX endpoints a `[SESSION]` block connects to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinanceEndpoint {
    OrderEntry,
    DropCopy,
    MarketData,
}

impl BinanceEndpoint {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "OrderEntry" => Ok(Self::OrderEntry),
            "DropCopy" => Ok(Self::DropCopy),
            "MarketData" => Ok(Self::MarketData),
            other => bail!("BinanceEndpoint must be OrderEntry|DropCopy|MarketData, got {other:?}"),
        }
    }

    /// Whether the Logon should carry `DropCopyFlag(9406)=Y` (required by the drop-copy server).
    pub fn is_drop_copy(self) -> bool {
        matches!(self, Self::DropCopy)
    }

    fn log_label(self) -> &'static str {
        match self {
            Self::OrderEntry => "order-entry",
            Self::DropCopy => "drop-copy",
            Self::MarketData => "market-data",
        }
    }
}

impl std::fmt::Display for BinanceEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::OrderEntry => "OrderEntry",
            Self::DropCopy => "DropCopy",
            Self::MarketData => "MarketData",
        })
    }
}

/// `MessageHandling(25035)`: whether client messages may be reordered before the matching engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageHandling {
    Unordered,
    Sequential,
}

impl MessageHandling {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "Unordered" => Ok(Self::Unordered),
            "Sequential" => Ok(Self::Sequential),
            other => bail!("BinanceMessageHandling must be Unordered|Sequential, got {other:?}"),
        }
    }

    /// The tag 25035 wire value.
    pub fn tag_value(self) -> &'static str {
        match self {
            Self::Unordered => "1",
            Self::Sequential => "2",
        }
    }
}

/// `ResponseMode(25036)`: which acknowledgements the server sends back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Everything,
    OnlyAcks,
}

impl ResponseMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "Everything" => Ok(Self::Everything),
            "OnlyAcks" => Ok(Self::OnlyAcks),
            other => bail!("BinanceResponseMode must be Everything|OnlyAcks, got {other:?}"),
        }
    }

    /// The tag 25036 wire value.
    pub fn tag_value(self) -> &'static str {
        match self {
            Self::Everything => "1",
            Self::OnlyAcks => "2",
        }
    }
}

/// `BinanceLogBackend`: which `truefix_log::Log` implementation records this session's
/// message/event audit trail. Both are wired through `truefix::transport::Services::log` in
/// `main.rs` -- this is a client-side choice, not something Binance's FIX API is aware of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogBackend {
    /// Embedded transactional log via `redb` (`truefix_log::RedbLog`) -- the default. Queryable
    /// with `--dump-log`.
    Redb,
    /// Plain-text `messages.log`/`event.log` via `truefix_log::FileLog` (truefix audit 007,
    /// NEW-156/157): writes are queued onto a bounded channel and persisted by a background task,
    /// so a slow disk never blocks the session's async read/dispatch loop, and rotation is
    /// configurable (size, generation count, and/or a time-based roll interval) instead of
    /// `redb`'s single ever-growing database file.
    File,
}

impl LogBackend {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "Redb" => Ok(Self::Redb),
            "File" => Ok(Self::File),
            other => bail!("BinanceLogBackend must be Redb|File, got {other:?}"),
        }
    }
}

/// `truefix_log::FileLogOptions`/`RetentionPolicy`'s fields, read directly off the `[SESSION]`
/// map under the same key names `truefix-config`'s own `LogSpec` resolution uses
/// (`FileLogHeartbeats`/`FileIncludeTimeStampForMessages`/`FileIncludeMilliseconds`/
/// `MaxFileLogSize`/`FileLogMaxGenerations`/`FileLogRollIntervalSecs`) -- only consulted when
/// `BinanceLogBackend=File`. Kept as plain data here (rather than depending on `truefix_config`'s
/// private parsing) since `main.rs` already builds `truefix_log::FileLog` directly via the
/// `Services` escape hatch, not through `truefix::Engine::start`'s `.cfg`-driven log resolution.
#[derive(Debug, Clone, Copy)]
pub struct FileLogSettings {
    pub include_heartbeats: bool,
    pub include_timestamp: bool,
    pub include_milliseconds: bool,
    pub max_size_bytes: Option<u64>,
    pub max_generations: Option<u32>,
    pub roll_interval_secs: Option<u64>,
}

/// Binance-specific settings for one `[SESSION]` block, parsed alongside (not through) the
/// standard `truefix_config::ResolvedSession`.
#[derive(Debug, Clone)]
pub struct BinanceSessionExt {
    pub endpoint: BinanceEndpoint,
    pub api_key: String,
    pub signing_key: SigningKey,
    pub message_handling: MessageHandling,
    pub response_mode: Option<ResponseMode>,
    pub recv_window: Option<u64>,
    pub log_backend: LogBackend,
    /// The `redb` database file (`LogBackend::Redb`) or the directory `FileLog` writes
    /// `messages.log`/`event.log` into (`LogBackend::File`).
    pub log_db: PathBuf,
    pub file_log: FileLogSettings,
    pub trust_store: Option<PathBuf>,
}

fn required<'a>(map: &'a BTreeMap<String, String>, key: &str, session: &str) -> Result<&'a str> {
    map.get(key)
        .map(String::as_str)
        .with_context(|| format!("session {session}: missing required key `{key}`"))
}

/// A `Y`/`N` boolean (case-insensitive), matching `truefix-config`'s own `bool_key` convention --
/// a present-but-unrecognized value is a hard error rather than a silent fallback.
fn bool_key(
    map: &BTreeMap<String, String>,
    key: &str,
    session: &str,
    default: bool,
) -> Result<bool> {
    match map.get(key).map(String::as_str) {
        None => Ok(default),
        Some(v) if v.eq_ignore_ascii_case("Y") => Ok(true),
        Some(v) if v.eq_ignore_ascii_case("N") => Ok(false),
        Some(other) => bail!("session {session}: {key} must be Y/N, got {other:?}"),
    }
}

fn opt_u32_key(map: &BTreeMap<String, String>, key: &str, session: &str) -> Result<Option<u32>> {
    match map.get(key) {
        None => Ok(None),
        Some(v) => Ok(Some(v.parse().with_context(|| {
            format!("session {session}: {key} must be a non-negative integer, got {v:?}")
        })?)),
    }
}

fn opt_u64_key(map: &BTreeMap<String, String>, key: &str, session: &str) -> Result<Option<u64>> {
    match map.get(key) {
        None => Ok(None),
        Some(v) => Ok(Some(v.parse().with_context(|| {
            format!("session {session}: {key} must be a non-negative integer, got {v:?}")
        })?)),
    }
}

/// Parse the Binance-specific keys out of one `[SESSION]`'s raw map (as returned by
/// `SessionSettings::sessions()`). `session_label` is used only for error messages (e.g.
/// `"SENDER->TARGET"`).
pub fn parse_binance_ext(
    map: &BTreeMap<String, String>,
    session_label: &str,
) -> Result<BinanceSessionExt> {
    let endpoint = BinanceEndpoint::parse(required(map, "BinanceEndpoint", session_label)?)
        .with_context(|| format!("session {session_label}"))?;
    let api_key = required(map, "BinanceApiKey", session_label)?.to_owned();
    let private_key_path = required(map, "BinancePrivateKeyPath", session_label)?;
    let signing_key = load_signing_key(Path::new(private_key_path)).with_context(|| {
        format!("session {session_label}: BinancePrivateKeyPath={private_key_path}")
    })?;

    let message_handling = match map.get("BinanceMessageHandling") {
        Some(v) => MessageHandling::parse(v).with_context(|| format!("session {session_label}"))?,
        None => MessageHandling::Unordered,
    };
    let response_mode = match map.get("BinanceResponseMode") {
        Some(v) => {
            Some(ResponseMode::parse(v).with_context(|| format!("session {session_label}"))?)
        }
        None => None,
    };
    let recv_window = match map.get("BinanceRecvWindow") {
        Some(v) => Some(v.parse::<u64>().with_context(|| {
            format!("session {session_label}: BinanceRecvWindow must be a non-negative integer, got {v:?}")
        })?),
        None => None,
    };
    let log_backend = match map.get("BinanceLogBackend") {
        Some(v) => LogBackend::parse(v).with_context(|| format!("session {session_label}"))?,
        None => LogBackend::Redb,
    };
    let log_db = match map.get("BinanceLogDb") {
        Some(v) => PathBuf::from(v),
        None => {
            let sender = map
                .get("SenderCompID")
                .map(String::as_str)
                .unwrap_or("session");
            match log_backend {
                // A single `redb` database file, as before.
                LogBackend::Redb => {
                    PathBuf::from(format!("log/{sender}-{}-log.redb", endpoint.log_label()))
                }
                // `FileLog` owns a directory (it writes `messages.log`/`event.log` inside it).
                LogBackend::File => {
                    PathBuf::from(format!("log/{sender}-{}-log", endpoint.log_label()))
                }
            }
        }
    };
    let file_log = FileLogSettings {
        include_heartbeats: bool_key(map, "FileLogHeartbeats", session_label, true)?,
        include_timestamp: bool_key(map, "FileIncludeTimeStampForMessages", session_label, true)?,
        include_milliseconds: bool_key(map, "FileIncludeMilliseconds", session_label, true)?,
        max_size_bytes: opt_u64_key(map, "MaxFileLogSize", session_label)?,
        max_generations: opt_u32_key(map, "FileLogMaxGenerations", session_label)?,
        roll_interval_secs: opt_u64_key(map, "FileLogRollIntervalSecs", session_label)?,
    };
    let trust_store = map.get("BinanceTrustStore").map(PathBuf::from);

    Ok(BinanceSessionExt {
        endpoint,
        api_key,
        signing_key,
        message_handling,
        response_mode,
        recv_window,
        log_backend,
        log_db,
        file_log,
        trust_store,
    })
}

fn load_signing_key(path: &Path) -> Result<SigningKey> {
    let pem = std::fs::read_to_string(path)
        .with_context(|| format!("reading private key at {}", path.display()))?;
    SigningKey::from_pkcs8_pem(&pem).context("private key is not a valid Ed25519 PKCS#8 PEM")
}

/// Error out if two sessions declare the same `BinanceEndpoint` — REPL command routing picks the
/// (only) session for a given endpoint, so more than one would be ambiguous.
pub fn ensure_unique_endpoints(exts: &[(String, BinanceSessionExt)]) -> Result<()> {
    for (i, (label_a, a)) in exts.iter().enumerate() {
        for (label_b, b) in &exts[i + 1..] {
            if a.endpoint == b.endpoint {
                bail!(
                    "sessions {label_a} and {label_b} both declare BinanceEndpoint={}; at most one session per endpoint is supported",
                    a.endpoint
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // A throwaway Ed25519 PKCS#8 key (generated via `openssl genpkey -algorithm ed25519`), used
    // only to exercise the signing-key-loading path — not tied to any real Binance account.
    const TEST_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
        MC4CAQAwBQYDK2VwBCIEIBbnWbVzkyH2KhR3D2Ik4m3eJGr6Zz/hCDbYvwYWNJlo\n\
        -----END PRIVATE KEY-----\n";

    /// Writes `TEST_PEM` to a fresh temp path and returns it; caller removes it when done.
    ///
    /// Uses a monotonic counter (not `line!()`, which resolves to *this* line for every caller
    /// regardless of call site) to keep each call's path unique -- tests run in parallel by
    /// default, so two callers sharing one path would race on write/read/remove.
    fn write_test_key() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "truefix-binance-example-test-key-{}-{n}",
            std::process::id()
        ));
        std::fs::write(&path, TEST_PEM).unwrap();
        path
    }

    #[test]
    fn missing_required_key_errors() {
        let m = map(&[("BinanceEndpoint", "OrderEntry")]);
        let err = parse_binance_ext(&m, "A->B").unwrap_err();
        assert!(err.to_string().contains("BinanceApiKey"));
    }

    #[test]
    fn unknown_endpoint_value_errors() {
        let m = map(&[("BinanceEndpoint", "Bogus")]);
        assert!(parse_binance_ext(&m, "A->B").is_err());
    }

    #[test]
    fn ensure_unique_endpoints_detects_duplicates() {
        let key = write_test_key();
        let m1 = map(&[
            ("BinanceEndpoint", "OrderEntry"),
            ("BinanceApiKey", "k"),
            ("BinancePrivateKeyPath", key.to_str().unwrap()),
        ]);
        let m2 = map(&[
            ("BinanceEndpoint", "OrderEntry"),
            ("BinanceApiKey", "k"),
            ("BinancePrivateKeyPath", key.to_str().unwrap()),
        ]);
        let ext1 = parse_binance_ext(&m1, "A->B").unwrap();
        let ext2 = parse_binance_ext(&m2, "C->D").unwrap();
        std::fs::remove_file(&key).ok();
        let exts = vec![("A->B".to_string(), ext1), ("C->D".to_string(), ext2)];
        assert!(ensure_unique_endpoints(&exts).is_err());
    }

    #[test]
    fn log_backend_defaults_to_redb_with_a_dot_redb_path() {
        let key = write_test_key();
        let m = map(&[
            ("BinanceEndpoint", "OrderEntry"),
            ("BinanceApiKey", "k"),
            ("BinancePrivateKeyPath", key.to_str().unwrap()),
            ("SenderCompID", "SNDR"),
        ]);
        let ext = parse_binance_ext(&m, "A->B").unwrap();
        std::fs::remove_file(&key).ok();
        assert_eq!(ext.log_backend, LogBackend::Redb);
        assert_eq!(ext.log_db, PathBuf::from("log/SNDR-order-entry-log.redb"));
    }

    #[test]
    fn log_backend_file_derives_a_directory_and_reads_retention_keys() {
        let key = write_test_key();
        let m = map(&[
            ("BinanceEndpoint", "MarketData"),
            ("BinanceApiKey", "k"),
            ("BinancePrivateKeyPath", key.to_str().unwrap()),
            ("SenderCompID", "SNDR"),
            ("BinanceLogBackend", "File"),
            ("FileLogMaxGenerations", "7"),
            ("FileLogRollIntervalSecs", "86400"),
            ("FileLogHeartbeats", "N"),
        ]);
        let ext = parse_binance_ext(&m, "A->B").unwrap();
        std::fs::remove_file(&key).ok();
        assert_eq!(ext.log_backend, LogBackend::File);
        assert_eq!(ext.log_db, PathBuf::from("log/SNDR-market-data-log"));
        assert_eq!(ext.file_log.max_generations, Some(7));
        assert_eq!(ext.file_log.roll_interval_secs, Some(86400));
        assert!(!ext.file_log.include_heartbeats);
        assert!(ext.file_log.include_timestamp);
    }

    #[test]
    fn unknown_log_backend_value_errors() {
        let key = write_test_key();
        let m = map(&[
            ("BinanceEndpoint", "OrderEntry"),
            ("BinanceApiKey", "k"),
            ("BinancePrivateKeyPath", key.to_str().unwrap()),
            ("BinanceLogBackend", "Bogus"),
        ]);
        let err = parse_binance_ext(&m, "A->B").unwrap_err();
        std::fs::remove_file(&key).ok();
        assert!(format!("{err:#}").contains("BinanceLogBackend"));
    }
}
