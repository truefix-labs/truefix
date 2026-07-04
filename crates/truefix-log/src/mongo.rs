//! MongoDB-backed log, behind the `mongodb` feature (US6, feature 004).
//!
//! Matches QuickFIX/Go's `MongoStore`/`MongoLog` option. The [`Log`] trait is synchronous and
//! infallible, while `mongodb` is async: each entry is pushed onto an unbounded channel (a
//! non-blocking, sync send) and written by a background task, mirroring `SqlLog`/`MssqlLog`'s
//! shape. Unlike `RedbLog`, no `spawn_blocking` is needed — `mongodb`'s API is natively async.

use mongodb::bson::doc;
use mongodb::{Client, Collection};
use tokio::sync::mpsc;

use crate::{Log, LogError, is_heartbeat};

fn io_err<E: std::fmt::Display>(e: E) -> LogError {
    LogError::Io(e.to_string())
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

enum Entry {
    Message {
        direction: &'static str,
        text: String,
    },
    Event {
        text: String,
    },
}

/// Configuration for [`MongoLog::connect_with_config`] (US6).
#[derive(Debug, Clone)]
pub struct MongoLogConfig {
    /// The MongoDB connection URI (`mongodb://...`).
    pub uri: String,
    /// Database name.
    pub database: String,
    /// Collection holding incoming messages.
    pub incoming_collection: String,
    /// Collection holding outgoing messages.
    pub outgoing_collection: String,
    /// Collection holding session events.
    pub event_collection: String,
    /// Whether Heartbeat (`35=0`) messages are persisted.
    pub include_heartbeats: bool,
    /// The session identity stamped onto every document's `session_id` field (GAP-41/FR-019,
    /// feature 005). See `truefix_log::SqlLogConfig::session_id`'s doc for the rationale.
    pub session_id: String,
}

impl MongoLogConfig {
    /// A config using `uri` with default database/collection names and `session_id` `"default"`.
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            database: "truefix".to_owned(),
            incoming_collection: "log_incoming".to_owned(),
            outgoing_collection: "log_outgoing".to_owned(),
            event_collection: "log_event".to_owned(),
            include_heartbeats: true,
            session_id: "default".to_owned(),
        }
    }
}

/// A MongoDB-backed log writing to configurable incoming/outgoing/event collections via a
/// background task (US6).
pub struct MongoLog {
    tx: mpsc::UnboundedSender<Entry>,
    include_heartbeats: bool,
}

impl MongoLog {
    /// Connect to `uri` with default database/collection names (backward-compatible shorthand
    /// for [`Self::connect_with_config`]).
    pub async fn connect(uri: &str) -> Result<Self, LogError> {
        Self::connect_with_config(MongoLogConfig::new(uri)).await
    }

    /// Connect using a full [`MongoLogConfig`].
    pub async fn connect_with_config(config: MongoLogConfig) -> Result<Self, LogError> {
        let client = Client::with_uri_str(&config.uri).await.map_err(io_err)?;
        let db = client.database(&config.database);
        let incoming: Collection<mongodb::bson::Document> =
            db.collection(&config.incoming_collection);
        let outgoing: Collection<mongodb::bson::Document> =
            db.collection(&config.outgoing_collection);
        let event: Collection<mongodb::bson::Document> = db.collection(&config.event_collection);

        let (tx, mut rx) = mpsc::unbounded_channel::<Entry>();
        let session_id = config.session_id;
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                match entry {
                    Entry::Message { direction, text } => {
                        let collection = if direction == "I" {
                            &incoming
                        } else {
                            &outgoing
                        };
                        let _ = collection
                            .insert_one(doc! {
                                "text": text,
                                "logged_at": now_unix(),
                                "session_id": &session_id,
                            })
                            .await;
                    }
                    Entry::Event { text } => {
                        let _ = event
                            .insert_one(doc! {
                                "text": text,
                                "logged_at": now_unix(),
                                "session_id": &session_id,
                            })
                            .await;
                    }
                }
            }
        });

        Ok(Self {
            tx,
            include_heartbeats: config.include_heartbeats,
        })
    }
}

impl Log for MongoLog {
    fn on_incoming(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        let _ = self.tx.send(Entry::Message {
            direction: "I",
            text: message.to_owned(),
        });
    }
    fn on_outgoing(&self, message: &str) {
        if is_heartbeat(message) && !self.include_heartbeats {
            return;
        }
        let _ = self.tx.send(Entry::Message {
            direction: "O",
            text: message.to_owned(),
        });
    }
    fn on_event(&self, text: &str) {
        let _ = self.tx.send(Entry::Event {
            text: text.to_owned(),
        });
    }
}
