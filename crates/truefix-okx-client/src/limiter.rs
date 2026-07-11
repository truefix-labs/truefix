use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::{error::OkxError, request::RetrySafety};

/// Minimal shared reservation gate for REST and WebSocket operations.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    next_allowed: Arc<Mutex<Instant>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            next_allowed: Arc::new(Mutex::new(Instant::now())),
        }
    }
}

impl RateLimiter {
    /// Waits until a local reservation can start.
    pub async fn reserve(&self) {
        let deadline = *self.next_allowed.lock().await;
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
    }
    /// Delays later reservations after a server throttle response.
    pub async fn throttle_for(&self, delay: Duration) {
        *self.next_allowed.lock().await = Instant::now() + delay;
    }
    /// Classifies whether an error is eligible for automatic retry.
    pub fn may_retry(safety: RetrySafety, error: &OkxError) -> bool {
        safety == RetrySafety::ReadOnly
            && matches!(
                error,
                OkxError::Transport(_) | OkxError::Timeout | OkxError::RateLimited { .. }
            )
    }
}
