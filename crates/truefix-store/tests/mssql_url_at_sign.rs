//! T071/T072 (US2, feature 007): an MSSQL connection URL with an `@` character in its password
//! parses correctly (BUG-70/FR-030) — `parse_url`'s `split_once('@')` split at the *first* `@`,
//! wrongly truncating a password that itself contains one (`user:p@ssword@host/db` mis-split into
//! userinfo `"user:p"` and a mangled host `"ssword@host/db"`).
//!
//! **Testing approach**: `tiberius::Config` exposes no getters, so the parsed host/user/password
//! can't be asserted on directly. Instead, this binds a raw local `TcpListener` (not a real MSSQL
//! server) and points the URL's host at it: if `@127.0.0.1:<port>` in the URL is parsed as the
//! *host* correctly, the connection attempt's DNS-resolution-then-TCP-connect step reaches that
//! listener (observable via `listener.accept()` succeeding) before failing later at the TDS
//! handshake (this raw listener doesn't speak the protocol, so `MssqlStore::connect` itself is
//! always expected to return `Err` here — that failure is not what's under test). If the password's
//! embedded `@` instead corrupts the host into something unresolvable, DNS resolution fails before
//! ever reaching the listener, and `accept()` never fires at all. No `DATABASE_URL_MSSQL`/live
//! server needed.

#![cfg(feature = "mssql")]

use std::time::Duration;

use tokio::net::TcpListener;
use truefix_store::MssqlStore;

#[tokio::test]
async fn a_password_containing_at_sign_does_not_corrupt_the_parsed_host() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Password "p@ssword" carries an '@' -- the exact shape BUG-70 mis-splits on.
    let url = format!("mssql://sa:p@ssword@127.0.0.1:{}/quickfixj", addr.port());

    // The raw listener doesn't speak the TDS protocol, so `MssqlStore::connect` never actually
    // succeeds -- but tiberius's own handshake has no timeout of its own, so it must be wrapped
    // here rather than awaited directly (otherwise a successful TCP connect, exactly the case this
    // test wants to detect, would hang forever waiting for a TDS response that will never come).
    let connect_fut = tokio::time::timeout(Duration::from_secs(3), MssqlStore::connect(&url));
    let accept_fut = tokio::time::timeout(Duration::from_secs(3), listener.accept());

    let (_connect_result, accept_result) = tokio::join!(connect_fut, accept_fut);

    assert!(
        accept_result.is_ok(),
        "the connection attempt never reached 127.0.0.1 at all -- the password's embedded '@' \
         corrupted the parsed host instead of being correctly retained as part of the password"
    );
}
