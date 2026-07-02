//! T031 (US6, feature 004) — MongoDB log (behind the `mongodb` feature): entries are persisted
//! via the background writer, mirroring `sql_log.rs`'s/`mssql_log.rs`'s pattern. Gated on
//! `DATABASE_URL_MONGO` being set to a reachable instance (CI provides one via a service
//! container — see `.github/workflows/ci.yml`'s `mongo` job); dev boxes without that service
//! shouldn't fail the suite.

#![cfg(feature = "mongodb")]

use std::time::{Duration, Instant};

use mongodb::bson::{doc, Document};
use mongodb::{Client, Collection};
use truefix_log::{Log, MongoLog, MongoLogConfig};

/// Polls `count_documents` rather than sleeping a fixed duration once: a freshly-started `mongo`
/// service container (CI) can take longer than any fixed guess for its first real write (index
/// creation + initial WiredTiger warm-up), independent of `MongoLog`'s own background-writer
/// latency — a CI-only flake this reproduced (0/1 counts, ~0.3s total test time) that a fixed
/// 300ms sleep didn't reliably survive. Mirrors `redb_log.rs`'s `open_with_retry` lesson: retry a
/// bounded window instead of guessing a sleep duration.
async fn count_with_retry(collection: &Collection<Document>, timeout: Duration) -> u64 {
    let start = Instant::now();
    loop {
        let count = collection.count_documents(doc! {}).await.unwrap();
        if count > 0 || start.elapsed() > timeout {
            return count;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

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

    let client = Client::with_uri_str(&uri).await.unwrap();
    let db = client.database("truefix");
    let timeout = Duration::from_secs(5);
    let incoming = count_with_retry(&db.collection("t031_log_incoming"), timeout).await;
    let outgoing = count_with_retry(&db.collection("t031_log_outgoing"), timeout).await;
    let events = count_with_retry(&db.collection("t031_log_event"), timeout).await;

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

    let client = Client::with_uri_str(&uri).await.unwrap();
    let db = client.database("truefix");
    // Only the non-heartbeat should ever land, so a plain "wait for >0" retry can't distinguish
    // "not yet written" from "correctly filtered to zero" — wait a bounded window for the kept
    // message to show up, which also gives the (correctly-filtered) heartbeat every opportunity
    // to show up as a bug if the filter were broken.
    let incoming = count_with_retry(
        &db.collection("t031_hb_log_incoming"),
        Duration::from_secs(5),
    )
    .await;

    assert_eq!(incoming, 1, "only the non-heartbeat should be logged");
}
