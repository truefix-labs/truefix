//! MongoDB-backed message store, behind the `mongodb` feature (US6, feature 004).
//!
//! Matches QuickFIX/Go's `MongoStore` option (`go.mongodb.org`-based), which QuickFIX/J does not
//! offer. Uses the official `mongodb` crate's native async API directly — no `spawn_blocking`
//! needed, unlike `redb` (research.md §6). Per research.md §6's driver-cancellation caveat, no
//! `mongodb` future here is ever wrapped in `tokio::time::timeout`/raced in a `select!`.

use async_trait::async_trait;
use mongodb::bson::{doc, spec::BinarySubtype, Binary};
use mongodb::options::IndexOptions;
use mongodb::{Client, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::{MessageStore, StoreError};

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionDoc {
    session_id: String,
    sender: i64,
    target: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageDoc {
    session_id: String,
    seq: i64,
    data: Binary,
}

/// Configuration for [`MongoStore::connect_with_config`] (US6).
#[derive(Debug, Clone)]
pub struct MongoStoreConfig {
    /// The MongoDB connection URI (`mongodb://...`).
    pub uri: String,
    /// Database name.
    pub database: String,
    /// Collection holding sender/target sequence numbers.
    pub sessions_collection: String,
    /// Collection holding sent message bodies.
    pub messages_collection: String,
    /// The composite session key discriminating documents when several sessions share one
    /// collection pair.
    pub session_id: String,
}

impl MongoStoreConfig {
    /// A config using `uri` with default database/collection names and a default `session_id` of
    /// `"default"` (single-session use).
    pub fn new(uri: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            database: "truefix".to_owned(),
            sessions_collection: "seqnums".to_owned(),
            messages_collection: "messages".to_owned(),
            session_id: "default".to_owned(),
        }
    }
}

/// A MongoDB-backed store (US6). Maps the same sequence-number/sent-message schema shape as
/// [`crate::SqlStore`]: a `sessions` collection for sequence numbers, a `messages` collection for
/// message bodies with a compound `(session_id, seq)` unique index.
pub struct MongoStore {
    sessions: Collection<SessionDoc>,
    messages: Collection<MessageDoc>,
    session_id: String,
}

impl MongoStore {
    /// Connect to `uri` with default database/collection names (backward-compatible shorthand for
    /// [`Self::connect_with_config`]).
    pub async fn connect(uri: &str) -> Result<Self, StoreError> {
        Self::connect_with_config(MongoStoreConfig::new(uri)).await
    }

    /// Connect using a full [`MongoStoreConfig`].
    pub async fn connect_with_config(config: MongoStoreConfig) -> Result<Self, StoreError> {
        let client = Client::with_uri_str(&config.uri).await.map_err(backend)?;
        let db = client.database(&config.database);
        let sessions: Collection<SessionDoc> = db.collection(&config.sessions_collection);
        let messages: Collection<MessageDoc> = db.collection(&config.messages_collection);

        // Idempotent: creating an index that already exists with the same spec is a no-op.
        let index = IndexModel::builder()
            .keys(doc! { "session_id": 1, "seq": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        messages.create_index(index).await.map_err(backend)?;

        let store = Self {
            sessions,
            messages,
            session_id: config.session_id,
        };
        store.ensure_session_row().await?;
        Ok(store)
    }

    async fn ensure_session_row(&self) -> Result<(), StoreError> {
        let filter = doc! { "session_id": &self.session_id };
        let existing = self.sessions.find_one(filter).await.map_err(backend)?;
        if existing.is_none() {
            self.sessions
                .insert_one(SessionDoc {
                    session_id: self.session_id.clone(),
                    sender: 1,
                    target: 1,
                })
                .await
                .map_err(backend)?;
        }
        Ok(())
    }

    async fn get_seq(&self, sender: bool) -> Result<u64, StoreError> {
        let filter = doc! { "session_id": &self.session_id };
        let row = self
            .sessions
            .find_one(filter)
            .await
            .map_err(backend)?
            .ok_or_else(|| backend(format!("no row for session_id {:?}", self.session_id)))?;
        Ok(if sender { row.sender } else { row.target } as u64)
    }

    async fn set_seq(&self, sender: bool, seq: u64) -> Result<(), StoreError> {
        let filter = doc! { "session_id": &self.session_id };
        let field = if sender { "sender" } else { "target" };
        let update = doc! { "$set": { field: seq as i64 } };
        self.sessions
            .update_one(filter, update)
            .await
            .map_err(backend)?;
        Ok(())
    }
}

#[async_trait]
impl MessageStore for MongoStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        self.get_seq(true).await
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        self.get_seq(false).await
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq(true, seq).await
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq(false, seq).await
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        let filter = doc! { "session_id": &self.session_id, "seq": seq as i64 };
        let update = doc! {
            "$set": {
                "data": Binary {
                    subtype: BinarySubtype::Generic,
                    bytes: message.to_vec(),
                },
            },
        };
        self.messages
            .update_one(filter, update)
            .upsert(true)
            .await
            .map_err(backend)?;
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        use futures_util::TryStreamExt;
        let filter = doc! {
            "session_id": &self.session_id,
            "seq": { "$gte": begin as i64, "$lte": end as i64 },
        };
        let sort = doc! { "seq": 1 };
        let mut cursor = self
            .messages
            .find(filter)
            .sort(sort)
            .await
            .map_err(backend)?;
        let mut out = Vec::new();
        while let Some(doc) = cursor.try_next().await.map_err(backend)? {
            out.push((doc.seq as u64, doc.data.bytes));
        }
        Ok(out)
    }
    async fn reset(&self) -> Result<(), StoreError> {
        let filter = doc! { "session_id": &self.session_id };
        self.messages
            .delete_many(filter.clone())
            .await
            .map_err(backend)?;
        let update = doc! { "$set": { "sender": 1i64, "target": 1i64 } };
        self.sessions
            .update_one(filter, update)
            .await
            .map_err(backend)?;
        Ok(())
    }
}
