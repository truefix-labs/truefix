//! T031 (US6, feature 004) — MongoDB log (behind the `mongodb` feature): entries are persisted
//! via the background writer, mirroring `sql_log.rs`'s/`mssql_log.rs`'s pattern. Gated on
//! `DATABASE_URL_MONGO` being set to a reachable instance (CI provides one via a service
//! container — see `.github/workflows/ci.yml`'s `mongo` job); dev boxes without that service
//! shouldn't fail the suite.

#![cfg(feature = "mongodb")]

use std::time::Duration;

use mongodb::Client;
use truefix_log::{Log, MongoLog, MongoLogConfig};

#[tokio::test]
async fn mongo_log_persists_messages_and_events_if_available() {
    let Ok(uri) = std::env::var("DATABASE_URL_MONGO") else {
        eprintln!("skipping: DATABASE_URL_MONGO not set");
        return;
    };

    let config = MongoLogConfig {
        incoming_collection: "t031_log_incoming".to_owned(),
        outgoing_collection: "t031_log_outgoing".to_owned(),
        event_collection: "t031_log_event".to_owned(),
        ..MongoLogConfig::new(&uri)
    };
    let log = MongoLog::connect_with_config(config).await.unwrap();
    log.on_incoming("8=FIX.4.4\x0135=A\x01");
    log.on_outgoing("8=FIX.4.4\x0135=0\x01");
    log.on_event("logged on");

    // Allow the background writer to flush.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let client = Client::with_uri_str(&uri).await.unwrap();
    let db = client.database("truefix");
    let incoming = db
        .collection::<mongodb::bson::Document>("t031_log_incoming")
        .count_documents(mongodb::bson::doc! {})
        .await
        .unwrap();
    let outgoing = db
        .collection::<mongodb::bson::Document>("t031_log_outgoing")
        .count_documents(mongodb::bson::doc! {})
        .await
        .unwrap();
    let events = db
        .collection::<mongodb::bson::Document>("t031_log_event")
        .count_documents(mongodb::bson::doc! {})
        .await
        .unwrap();

    assert_eq!(incoming, 1, "one incoming message logged");
    assert_eq!(outgoing, 1, "one outgoing message logged");
    assert_eq!(events, 1, "one event logged");
}

#[tokio::test]
async fn mongo_log_heartbeats_n_filters_heartbeats_if_available() {
    let Ok(uri) = std::env::var("DATABASE_URL_MONGO") else {
        eprintln!("skipping: DATABASE_URL_MONGO not set");
        return;
    };

    let config = MongoLogConfig {
        incoming_collection: "t031_hb_log_incoming".to_owned(),
        outgoing_collection: "t031_hb_log_outgoing".to_owned(),
        event_collection: "t031_hb_log_event".to_owned(),
        include_heartbeats: false,
        ..MongoLogConfig::new(&uri)
    };
    let log = MongoLog::connect_with_config(config).await.unwrap();
    log.on_incoming("8=FIX.4.4\x0135=0\x01"); // heartbeat, should be filtered
    log.on_incoming("8=FIX.4.4\x0135=A\x01"); // logon, should be kept

    tokio::time::sleep(Duration::from_millis(300)).await;

    let client = Client::with_uri_str(&uri).await.unwrap();
    let db = client.database("truefix");
    let incoming = db
        .collection::<mongodb::bson::Document>("t031_hb_log_incoming")
        .count_documents(mongodb::bson::doc! {})
        .await
        .unwrap();

    assert_eq!(incoming, 1, "only the non-heartbeat should be logged");
}
