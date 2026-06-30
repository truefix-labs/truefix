//! SQL-backed log (JDBC-equivalent), behind the `sql` feature.
//!
//! The [`Log`] trait is synchronous and infallible, while `sqlx` is async. [`SqlLog`] bridges the
//! two: each entry is pushed onto an unbounded channel (a non-blocking, sync send) and written to
//! the database by a background task.

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use tokio::sync::mpsc;

use crate::{Log, LogError};

enum Entry {
    Message {
        direction: &'static str,
        text: String,
    },
    Event {
        text: String,
    },
}

/// A SQL-backed log writing to `log_messages` and `log_events` tables via a background task.
pub struct SqlLog {
    tx: mpsc::UnboundedSender<Entry>,
}

impl SqlLog {
    /// Connect to `url` (e.g. `sqlite://truefix-log.db`), ensure the schema, and spawn the writer.
    pub async fn connect(url: &str) -> Result<Self, LogError> {
        let opts = SqliteConnectOptions::from_str(url)
            .map_err(|e| LogError::Io(e.to_string()))?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts)
            .await
            .map_err(|e| LogError::Io(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS log_messages (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             direction TEXT NOT NULL, text TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .map_err(|e| LogError::Io(e.to_string()))?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS log_events (id INTEGER PRIMARY KEY AUTOINCREMENT, \
             text TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .map_err(|e| LogError::Io(e.to_string()))?;

        let (tx, mut rx) = mpsc::unbounded_channel::<Entry>();
        tokio::spawn(async move {
            while let Some(entry) = rx.recv().await {
                match entry {
                    Entry::Message { direction, text } => {
                        let _ =
                            sqlx::query("INSERT INTO log_messages (direction, text) VALUES (?, ?)")
                                .bind(direction)
                                .bind(text)
                                .execute(&pool)
                                .await;
                    }
                    Entry::Event { text } => {
                        let _ = sqlx::query("INSERT INTO log_events (text) VALUES (?)")
                            .bind(text)
                            .execute(&pool)
                            .await;
                    }
                }
            }
        });

        Ok(Self { tx })
    }
}

impl Log for SqlLog {
    fn on_incoming(&self, message: &str) {
        let _ = self.tx.send(Entry::Message {
            direction: "I",
            text: message.to_owned(),
        });
    }
    fn on_outgoing(&self, message: &str) {
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
