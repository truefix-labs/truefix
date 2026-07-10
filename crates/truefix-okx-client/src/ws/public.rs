use crate::{
    types::websocket::{SubscriptionArg, WsCommand},
    ws::session::Session,
};

/// Public session entrypoint. It becomes active without credentials after connection setup.
#[derive(Debug, Default)]
pub struct PublicSession(pub Session);
impl PublicSession {
    pub fn connected(&mut self) {
        self.0.connected();
    }
    pub fn subscribe(&mut self, args: Vec<SubscriptionArg>) -> WsCommand<Vec<SubscriptionArg>> {
        WsCommand {
            op: "subscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        }
    }
    pub fn unsubscribe(&mut self, args: Vec<SubscriptionArg>) -> WsCommand<Vec<SubscriptionArg>> {
        WsCommand {
            op: "unsubscribe".to_owned(),
            id: Some(self.0.next_request_id()),
            args,
        }
    }
    pub fn ping(&mut self) -> WsCommand<Vec<()>> {
        WsCommand {
            op: "ping".to_owned(),
            id: Some(self.0.next_request_id()),
            args: vec![],
        }
    }
}
