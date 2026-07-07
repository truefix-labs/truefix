//! MongoDB-backed message store, behind the `mongodb` feature (US6, feature 004).
//!
//! Matches QuickFIX/Go's `MongoStore` option (`go.mongodb.org`-based), which QuickFIX/J does not
//! offer. Uses the official `mongodb` crate's native async API directly — no `spawn_blocking`
//! needed, unlike `redb` (research.md §6). Per research.md §6's driver-cancellation caveat, no
//! `mongodb` future here is ever wrapped in `tokio::time::timeout`/raced in a `select!`.

use async_trait::async_trait;
use mongodb::bson::{Binary, doc, spec::BinarySubtype};
use mongodb::error::{Error as MongoError, ErrorKind, WriteFailure};
use mongodb::options::IndexOptions;
use mongodb::{Client, Collection, IndexModel};
use serde::{Deserialize, Serialize};

use crate::{MessageStore, StoreError};

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn session_unique_index() -> IndexModel {
    IndexModel::builder()
        .keys(doc! { "session_id": 1 })
        .options(
            IndexOptions::builder()
                .name("session_id_unique".to_owned())
                .unique(true)
                .build(),
        )
        .build()
}

fn is_duplicate_key(error: &MongoError) -> bool {
    match error.kind.as_ref() {
        ErrorKind::Command(command) => command.code == 11000,
        ErrorKind::Write(WriteFailure::WriteError(write)) => write.code == 11000,
        _ => false,
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionDoc {
    session_id: String,
    sender: i64,
    target: i64,
    /// GAP-38/FR-017 (feature 005): Unix seconds. `Option` so documents written before this field
    /// existed still deserialize (`None` rather than a hard decode error).
    #[serde(default)]
    creation_time: Option<i64>,
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
    client: Client,
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
        sessions
            .create_index(session_unique_index())
            .await
            .map_err(backend)?;
        let index = IndexModel::builder()
            .keys(doc! { "session_id": 1, "seq": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        messages.create_index(index).await.map_err(backend)?;

        let store = Self {
            client,
            sessions,
            messages,
            session_id: config.session_id,
        };
        store.ensure_session_row().await?;
        Ok(store)
    }

    async fn ensure_session_row(&self) -> Result<(), StoreError> {
        let filter = doc! { "session_id": &self.session_id };
        let update = doc! {
            "$setOnInsert": {
                "session_id": &self.session_id,
                "sender": 1i64,
                "target": 1i64,
                "creation_time": now_unix(),
            },
        };
        match self
            .sessions
            .update_one(filter.clone(), update)
            .upsert(true)
            .await
        {
            Ok(_) => Ok(()),
            // Two exact-filter upserts can race after both observe no match. The unique index
            // guarantees only one insert wins; if the loser receives E11000, confirm the winner's
            // row is visible and treat initialization as complete.
            Err(error) if is_duplicate_key(&error) => {
                if self
                    .sessions
                    .find_one(filter)
                    .await
                    .map_err(backend)?
                    .is_some()
                {
                    Ok(())
                } else {
                    Err(backend(error))
                }
            }
            Err(error) => Err(backend(error)),
        }
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
    // NEW-08 (feature 009): a transactional override, mirroring SqlStore/MssqlStore/RedbStore/
    // FileStore's existing atomicity guarantee -- the trait's default two-call implementation
    // (`save()` then `set_next_sender_seq()`) leaves a crash window between the two calls where a
    // message is durably saved but the sender sequence isn't advanced (or vice versa).
    async fn save_and_advance_sender(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        let mut txn_session = self.client.start_session().await.map_err(backend)?;
        txn_session.start_transaction().await.map_err(backend)?;

        let msg_filter = doc! { "session_id": &self.session_id, "seq": seq as i64 };
        let msg_update = doc! {
            "$set": {
                "data": Binary {
                    subtype: BinarySubtype::Generic,
                    bytes: message.to_vec(),
                },
            },
        };
        if let Err(e) = self
            .messages
            .update_one(msg_filter, msg_update)
            .upsert(true)
            .session(&mut txn_session)
            .await
        {
            let _ = txn_session.abort_transaction().await;
            return Err(backend(e));
        }

        let seq_filter = doc! { "session_id": &self.session_id };
        let seq_update = doc! { "$set": { "sender": (seq + 1) as i64 } };
        if let Err(e) = self
            .sessions
            .update_one(seq_filter, seq_update)
            .session(&mut txn_session)
            .await
        {
            let _ = txn_session.abort_transaction().await;
            return Err(backend(e));
        }

        txn_session.commit_transaction().await.map_err(backend)?;
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
        // NEW-116 (audit 006): a real transaction, mirroring `save_and_advance_sender`'s existing
        // NEW-08 atomicity pattern -- previously a crash between the delete and the update left
        // messages deleted but sequence numbers at their pre-reset values.
        let mut txn_session = self.client.start_session().await.map_err(backend)?;
        txn_session.start_transaction().await.map_err(backend)?;

        let filter = doc! { "session_id": &self.session_id };
        if let Err(e) = self
            .messages
            .delete_many(filter.clone())
            .session(&mut txn_session)
            .await
        {
            let _ = txn_session.abort_transaction().await;
            return Err(backend(e));
        }

        let update =
            doc! { "$set": { "sender": 1i64, "target": 1i64, "creation_time": now_unix() } };
        if let Err(e) = self
            .sessions
            .update_one(filter, update)
            .session(&mut txn_session)
            .await
        {
            let _ = txn_session.abort_transaction().await;
            return Err(backend(e));
        }

        txn_session.commit_transaction().await.map_err(backend)?;
        Ok(())
    }
    async fn creation_time(&self) -> Result<Option<time::OffsetDateTime>, StoreError> {
        let filter = doc! { "session_id": &self.session_id };
        let row = self.sessions.find_one(filter).await.map_err(backend)?;
        Ok(row
            .and_then(|r| r.creation_time)
            .and_then(|s| time::OffsetDateTime::from_unix_timestamp(s).ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_index_is_unique_on_session_id() {
        let index = session_unique_index();

        assert_eq!(index.keys, doc! { "session_id": 1 });
        let options = index.options.expect("index options");
        assert_eq!(options.unique, Some(true));
        assert_eq!(options.name.as_deref(), Some("session_id_unique"));
    }
}
