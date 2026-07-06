//! T049/T050 (US1, feature 009, `NEW-90`): `MssqlStore` unconditionally called
//! `tiberius::Config::trust_cert()` (skipping TLS server-certificate-chain validation) with no
//! way to opt back into real validation. `MssqlStoreConfig` now has a `trust_server_certificate`
//! field, defaulting to `true` (today's behavior, backward compatible).
//!
//! **Testing approach**: `tiberius::Config` exposes no getters (same constraint as
//! `mssql_url_at_sign.rs`), so this can't directly assert whether `trust_cert()` was called
//! internally -- no MSSQL reference implementation exists to diff against either
//! (`docs/todo/005.md`'s own note). Instead, this confirms the config plumbing itself: the
//! documented default, and that setting the option to `false` doesn't break the connection
//! attempt's URL-parsing/connect-setup path (using the same raw-`TcpListener` reachability trick).

#![cfg(feature = "mssql")]

use std::time::Duration;

use tokio::net::TcpListener;
use truefix_store::MssqlStoreConfig;

#[test]
fn trust_server_certificate_defaults_to_true() {
    let config = MssqlStoreConfig::new("mssql://sa:secret@localhost/db");
    assert!(
        config.trust_server_certificate,
        "trust_server_certificate must default to true (today's unconditional behavior, \
         preserved for backward compatibility)"
    );
}

#[tokio::test]
async fn disabling_trust_server_certificate_does_not_break_the_connect_path() {
    use truefix_store::MssqlStore;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("mssql://sa:secret@127.0.0.1:{}/quickfixj", addr.port());
    let config = MssqlStoreConfig {
        trust_server_certificate: false,
        ..MssqlStoreConfig::new(&url)
    };

    let connect_fut = tokio::time::timeout(
        Duration::from_secs(3),
        MssqlStore::connect_with_config(config),
    );
    let accept_fut = tokio::time::timeout(Duration::from_secs(3), listener.accept());
    let (_connect_result, accept_result) = tokio::join!(connect_fut, accept_fut);

    assert!(
        accept_result.is_ok(),
        "the connection attempt must still reach the host with trust_server_certificate=false, \
         not fail before ever attempting to connect"
    );
}
