use std::collections::{HashMap, HashSet};

use crate::types::websocket::{RequestId, SubscriptionArg};

/// Canonical subscription identity used for routing and deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptionKey {
    pub channel: String,
    pub instrument_id: Option<String>,
    pub extra: Vec<(String, String)>,
}
impl From<&SubscriptionArg> for SubscriptionKey {
    fn from(arg: &SubscriptionArg) -> Self {
        Self {
            channel: arg.channel.clone(),
            instrument_id: arg.instrument_id.clone(),
            extra: arg
                .extra
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
        }
    }
}

/// Desired, active, and in-flight subscriptions. Request identifiers are bounded by
/// `MAX_IN_FLIGHT`, avoiding unbounded correlation state under a broken peer.
#[derive(Debug, Default)]
pub struct Subscriptions {
    desired: HashSet<SubscriptionKey>,
    active: HashSet<SubscriptionKey>,
    pending: HashMap<RequestId, SubscriptionKey>,
}
impl Subscriptions {
    pub const MAX_IN_FLIGHT: usize = 1_024;
    /// Adds a desired key. `true` means the caller should issue a subscribe command.
    pub fn request(&mut self, key: SubscriptionKey) -> bool {
        self.desired.insert(key)
    }
    /// Records correlation for a pending subscribe. Returns false when saturated or duplicate.
    pub fn correlate(&mut self, request_id: RequestId, key: &SubscriptionKey) -> bool {
        self.desired.contains(key)
            && self.pending.len() < Self::MAX_IN_FLIGHT
            && self.pending.insert(request_id, key.clone()).is_none()
    }
    /// Activates the correlated subscription only on a successful acknowledgement.
    pub fn acknowledge_id(
        &mut self,
        request_id: &RequestId,
        success: bool,
    ) -> Option<SubscriptionKey> {
        let key = self.pending.remove(request_id)?;
        if success && self.desired.contains(&key) {
            self.active.insert(key.clone());
        }
        Some(key)
    }
    pub fn acknowledge(&mut self, key: SubscriptionKey) {
        if self.desired.contains(&key) {
            self.active.insert(key);
        }
    }
    /// Cancels a desired subscription and prevents future event routing/replay.
    pub fn cancel(&mut self, key: &SubscriptionKey) -> bool {
        self.active.remove(key);
        self.pending.retain(|_, pending| pending != key);
        self.desired.remove(key)
    }
    pub fn active(&self, key: &SubscriptionKey) -> bool {
        self.active.contains(key)
    }
    pub fn desired(&self, key: &SubscriptionKey) -> bool {
        self.desired.contains(key)
    }
    pub fn disconnect(&mut self) {
        self.active.clear();
        self.pending.clear();
    }
    pub fn replay(&self) -> impl Iterator<Item = &SubscriptionKey> {
        self.desired.iter()
    }
}
