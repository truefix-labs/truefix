use crate::{
    types::websocket::WsEvent,
    ws::subscription::{SubscriptionKey, Subscriptions},
};

/// Checks whether an event belongs to an exact subscription key.
pub fn matches(event: &WsEvent, key: &SubscriptionKey) -> bool {
    SubscriptionKey::from(&event.arg) == *key
}

/// Returns an event only after its exact desired subscription became active.
pub fn route(subscriptions: &Subscriptions, event: WsEvent) -> Option<WsEvent> {
    subscriptions
        .active(&SubscriptionKey::from(&event.arg))
        .then_some(event)
}
