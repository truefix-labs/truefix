//! T024/T025 (US1, feature 009, `NEW-08`): `MongoStore` lacked a `save_and_advance_sender`
//! override -- `SqlStore`/`MssqlStore`/`RedbStore`/`FileStore` all override it with a single
//! transaction, but Mongo fell back to the trait's default two-call implementation (`save()` then
//! `set_next_sender_seq()`), leaving the crash window the trait doc warns about. This test
//! requires a reachable MongoDB instance supporting multi-document transactions (a replica-set or
//! sharded deployment -- a standalone `mongod` does not support transactions); CI provides one by
//! starting `mongod --replSet rs0` and running `rs.initiate()` before the test step (see
//! `.github/workflows/ci.yml`'s `mongo` job). Skips (not fails) when `DATABASE_URL_MONGO` isn't
//! set, since no such instance was available in this development environment to verify
//! interactively.

#![cfg(feature = "mongodb")]

use truefix_store::{MessageStore, MongoStore, MongoStoreConfig};

#[tokio::test]
async fn save_and_advance_sender_is_atomic_if_available() {
    let Ok(uri) = std::env::var("DATABASE_URL_MONGO") else {
        eprintln!("skipping: DATABASE_URL_MONGO not set");
        return;
    };
    let config = MongoStoreConfig {
        sessions_collection: "atomic_sessions".to_owned(),
        messages_collection: "atomic_messages".to_owned(),
        session_id: "atomic1".to_owned(),
        ..MongoStoreConfig::new(&uri)
    };
    let store = MongoStore::connect_with_config(config).await.unwrap();

    assert_eq!(store.next_sender_seq().await.unwrap(), 1);
    store.save_and_advance_sender(1, b"first").await.unwrap();
    assert_eq!(
        store.next_sender_seq().await.unwrap(),
        2,
        "save_and_advance_sender must advance the sender sequence"
    );
    let saved = store.get(1, 1).await.unwrap();
    assert_eq!(
        saved,
        vec![(1, b"first".to_vec())],
        "save_and_advance_sender must persist the message body"
    );

    // A second call proves both effects land together across repeated invocations, not just once.
    store.save_and_advance_sender(2, b"second").await.unwrap();
    assert_eq!(store.next_sender_seq().await.unwrap(), 3);
    assert_eq!(
        store.get(1, 2).await.unwrap(),
        vec![(1, b"first".to_vec()), (2, b"second".to_vec())]
    );
}
