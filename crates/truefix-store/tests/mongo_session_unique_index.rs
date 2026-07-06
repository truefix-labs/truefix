//! T100 — concurrent MongoStore initialization creates one session row per session identity.

#![cfg(feature = "mongodb")]

use futures_util::future::join_all;
use mongodb::bson::doc;
use mongodb::{Client, Collection};
use truefix_store::{MongoStore, MongoStoreConfig};

#[tokio::test]
async fn concurrent_connects_create_one_session_document_if_available() {
    let Ok(uri) = std::env::var("DATABASE_URL_MONGO") else {
        eprintln!("skipping: DATABASE_URL_MONGO not set");
        return;
    };

    let suffix = format!(
        "{}_{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let database = format!("truefix_session_unique_{suffix}");
    let sessions_collection = "sessions".to_owned();
    let session_id = "concurrent-session".to_owned();
    let config = MongoStoreConfig {
        database: database.clone(),
        sessions_collection: sessions_collection.clone(),
        messages_collection: "messages".to_owned(),
        session_id: session_id.clone(),
        ..MongoStoreConfig::new(&uri)
    };

    let results = join_all((0..16).map(|_| MongoStore::connect_with_config(config.clone()))).await;
    for result in results {
        result.expect("every concurrent connection should initialize successfully");
    }

    let client = Client::with_uri_str(&uri).await.unwrap();
    let sessions: Collection<mongodb::bson::Document> =
        client.database(&database).collection(&sessions_collection);
    let count = sessions
        .count_documents(doc! { "session_id": &session_id })
        .await
        .unwrap();
    assert_eq!(count, 1, "session_id must identify exactly one document");

    client.database(&database).drop().await.unwrap();
}
