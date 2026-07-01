//! T072-adjacent (US12) — `FileStorePath`/`FileStoreSync`/`FileStoreMaxCachedMsgs` and
//! `FileLogPath`/its output switches map from a settings file into a runnable `ResolvedSession`
//! (FR-025/FR-026).

use truefix_config::{ResolvedSession, SessionSettings};
use truefix_store::StoreConfig;

fn resolved(cfg: &str) -> ResolvedSession {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn no_file_store_path_means_memory_store() {
    let rs = resolved(&base(""));
    assert!(matches!(rs.store, StoreConfig::Memory));
}

#[test]
fn bare_file_store_path_is_a_plain_file_store_with_sync_default_on() {
    let rs = resolved(&base("FileStorePath=/tmp/whatever\n"));
    match rs.store {
        StoreConfig::File { options, .. } => {
            assert!(options.sync);
            assert_eq!(options.max_cached_msgs, 0);
        }
        other => panic!("expected StoreConfig::File, got {other:?}"),
    }
}

#[test]
fn file_store_sync_n_disables_fsync() {
    let rs = resolved(&base("FileStorePath=/tmp/whatever\nFileStoreSync=N\n"));
    match rs.store {
        StoreConfig::File { options, .. } => assert!(!options.sync),
        other => panic!("expected StoreConfig::File, got {other:?}"),
    }
}

#[test]
fn max_cached_msgs_selects_cached_file_store() {
    let rs = resolved(&base(
        "FileStorePath=/tmp/whatever\nFileStoreMaxCachedMsgs=500\n",
    ));
    match rs.store {
        StoreConfig::CachedFile { options, .. } => {
            assert_eq!(options.max_cached_msgs, 500);
            assert!(options.sync);
        }
        other => panic!("expected StoreConfig::CachedFile, got {other:?}"),
    }
}

#[test]
fn invalid_max_cached_msgs_is_a_typed_error() {
    let cfg = base("FileStorePath=/tmp/whatever\nFileStoreMaxCachedMsgs=lots\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::InvalidValue { key, .. } => {
            assert_eq!(key, "FileStoreMaxCachedMsgs")
        }
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn no_file_log_path_means_no_log() {
    let rs = resolved(&base(""));
    assert!(rs.log.is_none());
}

#[test]
fn file_log_path_builds_a_log_spec_with_registered_switches() {
    let rs = resolved(&base(
        "FileLogPath=/tmp/logs\nFileLogHeartbeats=N\nFileIncludeMilliseconds=Y\n\
         FileIncludeTimeStampForMessages=Y\n",
    ));
    let log = rs.log.expect("FileLogPath should produce a LogSpec");
    assert_eq!(log.dir, std::path::PathBuf::from("/tmp/logs"));
    assert!(!log.include_heartbeats);
    assert!(log.include_timestamp);
    assert!(log.include_milliseconds);
}

#[test]
fn file_log_switches_default_to_backward_compatible_values() {
    let rs = resolved(&base("FileLogPath=/tmp/logs\n"));
    let log = rs.log.unwrap();
    assert!(log.include_heartbeats);
    assert!(!log.include_timestamp);
    assert!(!log.include_milliseconds);
}
