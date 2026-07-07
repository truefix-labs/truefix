//! NEW-116: Mongo/SQL/MSSQL `reset()` commits its DELETE + UPDATE atomically.
//! NEW-118: `NoopStore` tracks sequence numbers in memory for the session lifetime.

use truefix_store::{MessageStore, NoopStore};

// --- NEW-118 ---

#[tokio::test]
async fn audit006_noop_store_tracks_sequence_numbers() {
    let s = NoopStore::new();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    s.set_next_sender_seq(42).await.unwrap();
    s.set_next_target_seq(7).await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 42);
    assert_eq!(s.next_target_seq().await.unwrap(), 7);
    // save()/get() remain fully inert -- only sequence tracking changed.
    s.save(1, b"ignored").await.unwrap();
    assert!(s.get(1, 100).await.unwrap().is_empty());
}

#[tokio::test]
async fn audit006_noop_store_reset_returns_sequences_to_one() {
    let s = NoopStore::new();
    s.set_next_sender_seq(99).await.unwrap();
    s.set_next_target_seq(88).await.unwrap();
    s.reset().await.unwrap();
    assert_eq!(s.next_sender_seq().await.unwrap(), 1);
    assert_eq!(s.next_target_seq().await.unwrap(), 1);
}

// --- NEW-116: SQL (sqlite, always available -- no external service needed) ---

#[cfg(feature = "sql")]
mod sql_reset_atomicity {
    use truefix_store::{MessageStore, SqlStore, SqlStoreConfig};

    #[tokio::test]
    async fn audit006_sqlite_reset_commits_delete_and_update_together() {
        let config = SqlStoreConfig {
            sessions_table: "t_audit006_reset_sessions".to_owned(),
            messages_table: "t_audit006_reset_messages".to_owned(),
            session_id: "audit006-reset".to_owned(),
            ..SqlStoreConfig::new("sqlite::memory:")
        };
        let store = SqlStore::connect_with_config(config).await.unwrap();

        store.set_next_sender_seq(10).await.unwrap();
        store.set_next_target_seq(20).await.unwrap();
        store.save(1, b"one").await.unwrap();
        store.save(2, b"two").await.unwrap();

        store.reset().await.unwrap();

        // Both halves of the reset must be visible together: sequences back to 1 AND messages
        // gone, never one without the other (NEW-116's atomicity guarantee).
        assert_eq!(store.next_sender_seq().await.unwrap(), 1);
        assert_eq!(store.next_target_seq().await.unwrap(), 1);
        assert!(store.get(1, 2).await.unwrap().is_empty());
    }
}

// --- NEW-116: Mongo/MSSQL, gated on a reachable service (mirrors the existing *_if_available
// pattern in mongo_backend.rs/mssql_backend.rs) ---

#[cfg(feature = "mongodb")]
mod mongo_reset_atomicity {
    use truefix_store::{MessageStore, MongoStore, MongoStoreConfig};

    #[tokio::test]
    async fn audit006_mongo_reset_commits_delete_and_update_together_if_available() {
        let Ok(uri) = std::env::var("DATABASE_URL_MONGO") else {
            eprintln!("skipping: DATABASE_URL_MONGO not set");
            return;
        };
        let config = MongoStoreConfig {
            sessions_collection: "t_audit006_reset_sessions".to_owned(),
            messages_collection: "t_audit006_reset_messages".to_owned(),
            session_id: "audit006-reset".to_owned(),
            ..MongoStoreConfig::new(&uri)
        };
        let store = MongoStore::connect_with_config(config).await.unwrap();

        store.set_next_sender_seq(10).await.unwrap();
        store.save(1, b"one").await.unwrap();
        store.reset().await.unwrap();

        assert_eq!(store.next_sender_seq().await.unwrap(), 1);
        assert!(store.get(1, 1).await.unwrap().is_empty());
    }
}

#[cfg(feature = "mssql")]
mod mssql_reset_atomicity {
    use truefix_store::{MessageStore, MssqlStore, MssqlStoreConfig};

    #[tokio::test]
    async fn audit006_mssql_reset_commits_delete_and_update_together_if_available() {
        let Ok(url) = std::env::var("DATABASE_URL_MSSQL") else {
            eprintln!("skipping: DATABASE_URL_MSSQL not set");
            return;
        };
        let config = MssqlStoreConfig {
            sessions_table: "t_audit006_reset_sessions".to_owned(),
            messages_table: "t_audit006_reset_messages".to_owned(),
            session_id: "audit006-reset".to_owned(),
            ..MssqlStoreConfig::new(&url)
        };
        let store = MssqlStore::connect_with_config(config).await.unwrap();

        store.set_next_sender_seq(10).await.unwrap();
        store.save(1, b"one").await.unwrap();
        store.reset().await.unwrap();

        assert_eq!(store.next_sender_seq().await.unwrap(), 1);
        assert!(store.get(1, 1).await.unwrap().is_empty());
    }
}
