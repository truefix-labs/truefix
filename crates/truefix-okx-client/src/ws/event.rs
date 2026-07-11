use crate::{
    types::websocket::WsEvent,
    ws::subscription::{SubscriptionKey, Subscriptions},
};

/// Checks whether an event belongs to a client subscription key.
///
/// Server-added `arg` metadata is ignored, while requested subscription
/// parameters remain part of the match.
pub fn matches(event: &WsEvent, key: &SubscriptionKey) -> bool {
    key.matches_event(&event.arg)
}

/// Returns an event only after a matching desired subscription became active.
pub fn route(subscriptions: &Subscriptions, event: WsEvent) -> Option<WsEvent> {
    subscriptions.routes_event(&event.arg).then_some(event)
}
