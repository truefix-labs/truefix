//! SQL-backed message store (JDBC-equivalent), behind the `sql` feature.

use std::str::FromStr;

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;

use crate::{MessageStore, StoreError};

fn backend<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Backend(e.to_string())
}

/// A SQL-backed store using `sqlx` (SQLite). Maps the QuickFIX/J JDBC store tables
/// (sequence numbers + sent messages) to a relational schema.
pub struct SqlStore {
    pool: SqlitePool,
}

impl SqlStore {
    /// Connect to `url` (e.g. `sqlite://truefix.db` or `sqlite::memory:`) and ensure the schema.
    pub async fn connect(url: &str) -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::from_str(url)
            .map_err(backend)?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(opts).await.map_err(backend)?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS seqnums (\
             id INTEGER PRIMARY KEY CHECK (id = 0), sender INTEGER NOT NULL, target INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .map_err(backend)?;
        sqlx::query("INSERT OR IGNORE INTO seqnums (id, sender, target) VALUES (0, 1, 1)")
            .execute(&pool)
            .await
            .map_err(backend)?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (seq INTEGER PRIMARY KEY, data BLOB NOT NULL)",
        )
        .execute(&pool)
        .await
        .map_err(backend)?;

        Ok(Self { pool })
    }

    async fn get_seq(&self, column: &str) -> Result<u64, StoreError> {
        let sql = format!("SELECT {column} FROM seqnums WHERE id = 0");
        let row = sqlx::query(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(backend)?;
        let value: i64 = row.try_get(column).map_err(backend)?;
        Ok(value as u64)
    }

    async fn set_seq(&self, column: &str, seq: u64) -> Result<(), StoreError> {
        let sql = format!("UPDATE seqnums SET {column} = ? WHERE id = 0");
        sqlx::query(&sql)
            .bind(seq as i64)
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }
}

#[async_trait]
impl MessageStore for SqlStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        self.get_seq("sender").await
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        self.get_seq("target").await
    }
    async fn set_next_sender_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq("sender", seq).await
    }
    async fn set_next_target_seq(&self, seq: u64) -> Result<(), StoreError> {
        self.set_seq("target", seq).await
    }
    async fn save(&self, seq: u64, message: &[u8]) -> Result<(), StoreError> {
        sqlx::query("INSERT OR REPLACE INTO messages (seq, data) VALUES (?, ?)")
            .bind(seq as i64)
            .bind(message)
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }
    async fn get(&self, begin: u64, end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        let rows =
            sqlx::query("SELECT seq, data FROM messages WHERE seq BETWEEN ? AND ? ORDER BY seq")
                .bind(begin as i64)
                .bind(end as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(backend)?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let seq: i64 = row.try_get("seq").map_err(backend)?;
            let data: Vec<u8> = row.try_get("data").map_err(backend)?;
            out.push((seq as u64, data));
        }
        Ok(out)
    }
    async fn reset(&self) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM messages")
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        sqlx::query("UPDATE seqnums SET sender = 1, target = 1 WHERE id = 0")
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }
}
