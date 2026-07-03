//! T072-adjacent (US12) — `FileStorePath`/`FileStoreSync`/`FileStoreMaxCachedMsgs` and
//! `FileLogPath`/its output switches map from a settings file into a runnable `ResolvedSession`
//! (FR-025/FR-026).

use truefix_config::{LogKind, ResolvedSession, SessionSettings};
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

// --- T014 (US3, feature 004): `JdbcURL` -> `StoreConfig`/`ResolvedSession.sql_log` (FR-003) ---

#[test]
fn no_jdbc_url_means_existing_file_store_resolution_unchanged() {
    let rs = resolved(&base("FileStorePath=/tmp/whatever\n"));
    assert!(matches!(rs.store, StoreConfig::File { .. }));
    assert!(rs.sql_log.is_none());
}

#[test]
fn an_unrecognized_jdbc_url_scheme_is_a_typed_unsupported_backend_error() {
    // "oracle" is deliberately never supported (Oracle is a documented, final deferral —
    // TODO-14/feature 003) — this also proves the unrecognized-scheme path, not just Oracle
    // specifically.
    let cfg = base("JdbcURL=oracle://user:pass@localhost/db\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::UnsupportedBackend { scheme, .. } => {
            assert_eq!(scheme, "oracle")
        }
        other => panic!("expected UnsupportedBackend, got {other:?}"),
    }
}

// --- T041 (US4, feature 006): jdbc:h2: rejected cleanly, never silently misrouted (BUG-10/FR-018) ---

#[test]
fn a_jdbc_h2_url_is_a_typed_unsupported_backend_error_not_a_silent_sqlite_misroute() {
    let cfg = base("JdbcURL=jdbc:h2:mem:quickfixj\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::UnsupportedBackend { scheme, .. } => {
            assert_eq!(scheme, "h2");
        }
        other => panic!(
            "expected UnsupportedBackend for jdbc:h2:, got {other:?} -- a silent misroute to a \
             SQLite file named after the raw scheme string would be far worse than this failing \
             at all"
        ),
    }
}

#[cfg(not(feature = "sql"))]
#[test]
fn a_postgres_jdbc_url_without_the_sql_feature_is_unsupported_backend() {
    let cfg = base("JdbcURL=postgres://user:pass@localhost/db\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::UnsupportedBackend { .. }
    ));
}

#[cfg(not(feature = "mssql"))]
#[test]
fn an_mssql_jdbc_url_without_the_mssql_feature_is_unsupported_backend() {
    let cfg = base("JdbcURL=mssql://user:pass@localhost/db\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::UnsupportedBackend { .. }
    ));
}

#[cfg(feature = "sql")]
#[test]
fn postgres_mysql_sqlite_jdbc_urls_select_the_sql_store() {
    for url in [
        "postgres://user:pass@localhost/db",
        "postgresql://user:pass@localhost/db",
        "mysql://user:pass@localhost/db",
        "sqlite:/tmp/whatever.db",
    ] {
        let rs = resolved(&base(&format!("JdbcURL={url}\n")));
        match rs.store {
            StoreConfig::Sql { url: got, .. } => assert_eq!(got, url),
            other => panic!("expected StoreConfig::Sql for {url}, got {other:?}"),
        }
    }
}

#[cfg(feature = "mssql")]
#[test]
fn mssql_and_sqlserver_jdbc_urls_select_the_mssql_store() {
    for url in [
        "mssql://user:pass@localhost/db",
        "sqlserver://user:pass@localhost/db",
    ] {
        let rs = resolved(&base(&format!("JdbcURL={url}\n")));
        match rs.store {
            StoreConfig::Mssql { url: got, .. } => assert_eq!(got, url),
            other => panic!("expected StoreConfig::Mssql for {url}, got {other:?}"),
        }
    }
}

#[cfg(feature = "sql")]
#[test]
fn jdbc_url_also_populates_sql_log_with_default_table_names_and_heartbeats() {
    let rs = resolved(&base("JdbcURL=postgres://user:pass@localhost/db\n"));
    let sql_log = rs.sql_log.expect("sql_log populated alongside the store");
    assert_eq!(sql_log.url, "postgres://user:pass@localhost/db");
    assert!(sql_log.include_heartbeats);
}

#[cfg(feature = "sql")]
#[test]
fn jdbc_log_heartbeats_n_disables_heartbeat_logging() {
    let rs = resolved(&base(
        "JdbcURL=postgres://user:pass@localhost/db\nJdbcLogHeartBeats=N\n",
    ));
    let sql_log = rs.sql_log.expect("sql_log populated");
    assert!(!sql_log.include_heartbeats);
}

#[cfg(feature = "sql")]
#[test]
fn jdbc_url_present_takes_precedence_over_file_log_path_with_a_warning() {
    // Both configured: JdbcURL wins for the log side; FileLogPath is ignored (documented
    // precedence, not silently dropped — see the `tracing::warn!` in `resolve_log`).
    let rs = resolved(&base(
        "JdbcURL=postgres://user:pass@localhost/db\nFileLogPath=/tmp/logs\n",
    ));
    assert!(rs.sql_log.is_some());
    assert!(rs.log.is_none());
}

// --- T007/T008 (US2, feature 005): real QuickFIX/J `jdbc:` URL scheme recognition + credential
// splicing (BUG-04) ---

#[cfg(feature = "sql")]
#[test]
fn jdbc_prefixed_postgres_mysql_sqlite_urls_select_the_sql_store() {
    for (jdbc_url, expected) in [
        (
            "jdbc:postgresql://user:pass@localhost/db",
            "postgresql://user:pass@localhost/db",
        ),
        (
            "jdbc:postgres://user:pass@localhost/db",
            "postgres://user:pass@localhost/db",
        ),
        (
            "jdbc:mysql://user:pass@localhost/db",
            "mysql://user:pass@localhost/db",
        ),
        ("jdbc:sqlite:/tmp/whatever.db", "sqlite:/tmp/whatever.db"),
    ] {
        let rs = resolved(&base(&format!("JdbcURL={jdbc_url}\n")));
        match rs.store {
            StoreConfig::Sql { url, .. } => assert_eq!(url, expected, "for input {jdbc_url}"),
            other => panic!("expected StoreConfig::Sql for {jdbc_url}, got {other:?}"),
        }
    }
}

#[cfg(feature = "mssql")]
#[test]
fn jdbc_sqlserver_url_selects_the_mssql_store() {
    let rs = resolved(&base("JdbcURL=jdbc:sqlserver://user:pass@localhost/db\n"));
    match rs.store {
        StoreConfig::Mssql { url, .. } => assert_eq!(url, "sqlserver://user:pass@localhost/db"),
        other => panic!("expected StoreConfig::Mssql, got {other:?}"),
    }
}

#[cfg(feature = "sql")]
#[test]
fn jdbc_user_and_password_are_spliced_into_a_credential_less_jdbc_url() {
    // Real QuickFIX/J `.cfg` files carry JdbcURL without embedded credentials plus separate
    // JdbcUser/JdbcPassword keys (JdbcUtil.java:69-72) — this is the drop-in-compatible case.
    let rs = resolved(&base(
        "JdbcURL=jdbc:postgresql://localhost/db\nJdbcUser=alice\nJdbcPassword=secret\n",
    ));
    match rs.store {
        StoreConfig::Sql { url, .. } => {
            assert_eq!(url, "postgresql://alice:secret@localhost/db")
        }
        other => panic!("expected StoreConfig::Sql, got {other:?}"),
    }
}

#[cfg(feature = "sql")]
#[test]
fn an_already_credentialed_jdbc_url_is_not_double_spliced() {
    let rs = resolved(&base(
        "JdbcURL=jdbc:postgresql://bob:hunter2@localhost/db\nJdbcUser=alice\nJdbcPassword=secret\n",
    ));
    match rs.store {
        StoreConfig::Sql { url, .. } => {
            assert_eq!(url, "postgresql://bob:hunter2@localhost/db")
        }
        other => panic!("expected StoreConfig::Sql, got {other:?}"),
    }
}

// --- T043 (US4, feature 006): credential percent-encoding (GAP-55/FR-020) ---

#[cfg(feature = "sql")]
#[test]
fn jdbc_user_and_password_containing_reserved_characters_are_percent_encoded_when_spliced() {
    let rs = resolved(&base(
        "JdbcURL=jdbc:postgresql://localhost/db\nJdbcUser=ali@ce\nJdbcPassword=p:a/ss\n",
    ));
    match rs.store {
        StoreConfig::Sql { url, .. } => {
            assert_eq!(
                url, "postgresql://ali%40ce:p%3Aa%2Fss@localhost/db",
                "reserved characters in JdbcUser/JdbcPassword must be percent-encoded so they \
                 don't corrupt the URL's own authority-delimiter structure"
            );
        }
        other => panic!("expected StoreConfig::Sql, got {other:?}"),
    }
}

// --- T090 (US8, feature 005): JDBC pool/table-name keys apply to StoreConfig::Sql/Mssql
// (FR-020/021) ---

#[cfg(feature = "sql")]
#[test]
fn no_jdbc_pool_or_table_name_keys_means_sql_store_defaults() {
    let rs = resolved(&base("JdbcURL=jdbc:sqlite:/tmp/whatever.db\n"));
    match rs.store {
        StoreConfig::Sql {
            sessions_table,
            messages_table,
            session_id,
            pool,
            ..
        } => {
            assert_eq!(sessions_table, None);
            assert_eq!(messages_table, None);
            assert_eq!(session_id, None);
            assert!(pool.is_none());
        }
        other => panic!("expected StoreConfig::Sql, got {other:?}"),
    }
}

#[cfg(feature = "sql")]
#[test]
fn jdbc_table_name_keys_apply_to_the_sql_store() {
    let rs = resolved(&base(
        "JdbcURL=jdbc:sqlite:/tmp/whatever.db\n\
         JdbcStoreSessionsTableName=my_sessions\n\
         JdbcStoreMessagesTableName=my_messages\n\
         JdbcSessionIdDefaultPropertyValue=SERVER->CLIENT\n",
    ));
    match rs.store {
        StoreConfig::Sql {
            sessions_table,
            messages_table,
            session_id,
            ..
        } => {
            assert_eq!(sessions_table.as_deref(), Some("my_sessions"));
            assert_eq!(messages_table.as_deref(), Some("my_messages"));
            assert_eq!(session_id.as_deref(), Some("SERVER->CLIENT"));
        }
        other => panic!("expected StoreConfig::Sql, got {other:?}"),
    }
}

#[cfg(feature = "sql")]
#[test]
fn jdbc_pool_tuning_keys_apply_to_the_sql_store() {
    let rs = resolved(&base(
        "JdbcURL=jdbc:sqlite:/tmp/whatever.db\n\
         JdbcMaxActiveConnection=25\n\
         JdbcMinIdleConnection=2\n\
         JdbcConnectionTimeout=10\n\
         JdbcConnectionIdleTimeout=60\n\
         JdbcMaxConnectionLifeTime=3600\n\
         JdbcConnectionKeepaliveTime=30\n",
    ));
    match rs.store {
        StoreConfig::Sql { pool, .. } => {
            let pool = pool.expect("pool options should be Some when any pool key is set");
            assert_eq!(pool.max_connections, 25);
            assert_eq!(pool.min_connections, 2);
            assert_eq!(pool.acquire_timeout, std::time::Duration::from_secs(10));
            assert_eq!(pool.idle_timeout, Some(std::time::Duration::from_secs(60)));
            assert_eq!(
                pool.max_lifetime,
                Some(std::time::Duration::from_secs(3600))
            );
            assert_eq!(pool.keepalive, Some(std::time::Duration::from_secs(30)));
        }
        other => panic!("expected StoreConfig::Sql, got {other:?}"),
    }
}

#[cfg(feature = "mssql")]
#[test]
fn jdbc_table_name_keys_apply_to_the_mssql_store() {
    let rs = resolved(&base(
        "JdbcURL=jdbc:sqlserver://user:pass@localhost/db\n\
         JdbcStoreSessionsTableName=my_sessions\n\
         JdbcStoreMessagesTableName=my_messages\n\
         JdbcSessionIdDefaultPropertyValue=SERVER->CLIENT\n",
    ));
    match rs.store {
        StoreConfig::Mssql {
            sessions_table,
            messages_table,
            session_id,
            ..
        } => {
            assert_eq!(sessions_table.as_deref(), Some("my_sessions"));
            assert_eq!(messages_table.as_deref(), Some("my_messages"));
            assert_eq!(session_id.as_deref(), Some("SERVER->CLIENT"));
        }
        other => panic!("expected StoreConfig::Mssql, got {other:?}"),
    }
}

// --- T081 (US8, feature 006): `.cfg`-selectable `ScreenLog`/`TracingLog`/`CompositeLog`
// (GAP-21) ---

#[test]
fn log_key_absent_leaves_log_kind_none() {
    let rs = resolved(&base(""));
    assert_eq!(rs.log_kind, None);
}

#[test]
fn log_screen_selects_screen_log_kind() {
    let rs = resolved(&base("Log=Screen\n"));
    assert_eq!(rs.log_kind, Some(LogKind::Screen));
}

#[test]
fn log_tracing_selects_tracing_log_kind_case_insensitively() {
    let rs = resolved(&base("Log=TRACING\n"));
    assert_eq!(rs.log_kind, Some(LogKind::Tracing));
}

#[test]
fn log_composite_selects_composite_log_kind() {
    let rs = resolved(&base("Log=Composite\n"));
    assert_eq!(rs.log_kind, Some(LogKind::Composite));
}

#[test]
fn log_file_and_sql_are_accepted_as_a_no_op_synonym() {
    assert_eq!(resolved(&base("Log=File\n")).log_kind, None);
    assert_eq!(resolved(&base("Log=Sql\n")).log_kind, None);
}

#[test]
fn log_unrecognized_value_is_a_typed_error() {
    let err = SessionSettings::parse(&base("Log=Bogus\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { .. }
    ));
}
