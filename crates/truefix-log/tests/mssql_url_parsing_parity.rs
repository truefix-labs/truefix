//! T034/T035 (US1, feature 009, `NEW-09`): `truefix-log::mssql::parse_url` was missing two
//! features `truefix-store::mssql::parse_url` already has: the semicolon-delimited JDBC-URL form
//! (`host[;databaseName=X;user=Y;password=Z;...]`, BUG-10/FR-019) and percent-decoding of
//! credentials (GAP-55/FR-020). A `JdbcURL` that works for the store failed to parse for the log;
//! credentials containing `%40`/`%3A`/`%2F` authenticated correctly in the store but failed in the
//! log.
//!
//! **Testing approach**: mirrors `truefix-store/tests/mssql_url_at_sign.rs` -- `tiberius::Config`
//! exposes no getters, so a raw local `TcpListener` stands in for the "host" and this asserts the
//! connection attempt reaches it (proving the URL was parsed to point at the right host), rather
//! than asserting on parsed field values directly. No `DATABASE_URL_MSSQL`/live server needed.

#![cfg(feature = "mssql")]

use std::time::Duration;

use tokio::net::TcpListener;
use truefix_log::MssqlLog;

/// The semicolon-delimited JDBC-URL form (host;databaseName=X;user=Y;password=Z) must parse and
/// reach the host, not fail to parse at all (as it did before this fix -- `parse_url` had no
/// semicolon-form branch).
#[tokio::test]
async fn semicolon_delimited_jdbc_url_form_is_parsed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!(
        "mssql://127.0.0.1:{};databaseName=quickfixj;user=sa;password=secret",
        addr.port()
    );

    let connect_fut = tokio::time::timeout(Duration::from_secs(3), MssqlLog::connect(&url));
    let accept_fut = tokio::time::timeout(Duration::from_secs(3), listener.accept());
    let (_connect_result, accept_result) = tokio::join!(connect_fut, accept_fut);

    assert!(
        accept_result.is_ok(),
        "the semicolon-delimited JDBC-URL form must be parsed and reach the host, not fail to \
         parse at all (NEW-09)"
    );
}

/// A URL using the semicolon-form with credentials embedded in the authority (rather than as
/// `user`/`password` properties) must also parse and reach the host.
#[tokio::test]
async fn semicolon_form_with_authority_credentials_is_parsed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!(
        "mssql://sa:secret@127.0.0.1:{};databaseName=quickfixj",
        addr.port()
    );

    let connect_fut = tokio::time::timeout(Duration::from_secs(3), MssqlLog::connect(&url));
    let accept_fut = tokio::time::timeout(Duration::from_secs(3), listener.accept());
    let (_connect_result, accept_result) = tokio::join!(connect_fut, accept_fut);

    assert!(
        accept_result.is_ok(),
        "a semicolon-form URL with authority-embedded credentials must be parsed and reach the \
         host (NEW-09)"
    );
}
