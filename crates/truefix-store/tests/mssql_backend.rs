//! T073 (US14) — MSSQL store parity (FR-020), mirroring `sql_backends.rs`'s pattern. Gated on
//! `DATABASE_URL_MSSQL` being set to a reachable instance (CI provides one via a service
//! container — see `.github/workflows/ci.yml`'s `mssql` job); dev boxes without that service
//! shouldn't fail the suite.

#![cfg(feature = "mssql")]

use truefix_store::{MessageStore, MssqlStore, MssqlStoreConfig};

/// Exercises the full `MessageStore` contract against `url`, using a fresh, uniquely-named table
/// pair each call so repeated runs never collide.
async fn exercise_full_contract(url: &str, unique_suffix: &str) {
    let config = MssqlStoreConfig {
        sessions_table: format!("t_sessions_{unique_suffix}"),
        messages_table: format!("t_messages_{unique_suffix}"),
        session_id: "s1".to_owned(),
        ..MssqlStoreConfig::new(url)
    };
    let store = MssqlStore::connect_with_config(config).await.unwrap();

    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    assert_eq!(store.next_target_seq().await.unwrap(), 1);

    store.set_next_sender_seq(11).await.unwrap();
    store.set_next_target_seq(22).await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 11);
    assert_eq!(store.next_target_seq().await.unwrap(), 22);

    store.save(1, b"first").await.unwrap();
    store.save(2, b"second").await.unwrap();
    store.save(2, b"second-updated").await.unwrap(); // upsert overwrites, not duplicates
    let range = store.get(1, 2).await.unwrap();
    assert_eq!(
        range,
        vec![(1, b"first".to_vec()), (2, b"second-updated".to_vec())]
    );

    store.reset().await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    assert_eq!(store.next_target_seq().await.unwrap(), 1);
    assert!(store.get(1, 2).await.unwrap().is_empty());
}

/// Two distinct `session_id`s sharing the same table pair don't see each other's data.
async fn exercise_session_isolation(url: &str, unique_suffix: &str) {
    let cfg = |session_id: &str| MssqlStoreConfig {
        sessions_table: format!("iso_sessions_{unique_suffix}"),
        messages_table: format!("iso_messages_{unique_suffix}"),
        session_id: session_id.to_owned(),
        ..MssqlStoreConfig::new(url)
    };
    let a = MssqlStore::connect_with_config(cfg("A")).await.unwrap();
    let b = MssqlStore::connect_with_config(cfg("B")).await.unwrap();

    a.set_next_sender_seq(100).await.unwrap();
    a.save(1, b"a-only").await.unwrap();

    assert_eq!(b.next_sender_seq().await.unwrap(), 1); // unaffected by A
    assert!(b.get(1, 1).await.unwrap().is_empty());
    assert_eq!(a.get(1, 1).await.unwrap(), vec![(1, b"a-only".to_vec())]);
}

/// T048/T049 (US7, feature 005): creation-time persistence and atomic save+advance-sender
/// (GAP-38/GAP-39; FR-017/FR-018), mirroring `sql_backends.rs`'s equivalent test.
async fn exercise_creation_time_and_atomic_save(url: &str, unique_suffix: &str) {
    let config = MssqlStoreConfig {
        sessions_table: format!("ct_sessions_{unique_suffix}"),
        messages_table: format!("ct_messages_{unique_suffix}"),
        session_id: "s1".to_owned(),
        ..MssqlStoreConfig::new(url)
    };
    let store = MssqlStore::connect_with_config(config).await.unwrap();

    let created = store.creation_time().await.unwrap();
    assert!(created.is_some(), "expected a recorded creation time");

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    store.reset().await.unwrap();
    let after_reset = store.creation_time().await.unwrap();
    assert!(
        after_reset.unwrap().unix_timestamp() > created.unwrap().unix_timestamp(),
        "reset() should advance the recorded creation time"
    );

    store.set_next_sender_seq(5).await.unwrap();
    store.save_and_advance_sender(5, b"atomic").await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 6);
    assert_eq!(
        store.get(5, 5).await.unwrap(),
        vec![(5, b"atomic".to_vec())]
    );
}

// --- T033 (US3, feature 006): identifier validation (BUG-14/FR-017) ---
// Runs unconditionally (no DATABASE_URL_MSSQL needed): validation happens before any network
// I/O, so an invalid identifier is rejected regardless of whether a real server is reachable.

#[tokio::test]
async fn invalid_sessions_table_identifier_is_rejected_before_connecting() {
    let config = MssqlStoreConfig {
        sessions_table: "not a valid identifier; DROP TABLE x".to_owned(),
        ..MssqlStoreConfig::new("mssql://user:pass@localhost/db")
    };
    let err = match MssqlStore::connect_with_config(config).await {
        Ok(_) => panic!("expected an identifier-validation error"),
        Err(e) => e,
    };
    assert!(
        format!("{err}").contains("not a valid SQL table identifier"),
        "expected a clean identifier-validation error, got: {err}"
    );
}

#[tokio::test]
async fn invalid_messages_table_identifier_is_rejected_before_connecting() {
    let config = MssqlStoreConfig {
        messages_table: "1leading-digit".to_owned(),
        ..MssqlStoreConfig::new("mssql://user:pass@localhost/db")
    };
    let err = match MssqlStore::connect_with_config(config).await {
        Ok(_) => panic!("expected an identifier-validation error"),
        Err(e) => e,
    };
    assert!(
        format!("{err}").contains("not a valid SQL table identifier"),
        "expected a clean identifier-validation error, got: {err}"
    );
}

#[tokio::test]
async fn mssql_creation_time_and_atomic_save_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_MSSQL") else {
        eprintln!("skipping: DATABASE_URL_MSSQL not set");
        return;
    };
    exercise_creation_time_and_atomic_save(&url, "mssql3").await;
}

#[tokio::test]
async fn mssql_full_contract_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_MSSQL") else {
        eprintln!("skipping: DATABASE_URL_MSSQL not set");
        return;
    };
    exercise_full_contract(&url, "mssql1").await;
}

#[tokio::test]
async fn mssql_session_isolation_if_available() {
    let Ok(url) = std::env::var("DATABASE_URL_MSSQL") else {
        eprintln!("skipping: DATABASE_URL_MSSQL not set");
        return;
    };
    exercise_session_isolation(&url, "mssql2").await;
}
