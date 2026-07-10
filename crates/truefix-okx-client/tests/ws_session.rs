mod support;

use truefix_okx_client::{
    error::OkxError,
    transport::websocket::WebSocketTransport,
    types::websocket::{RequestId, SubscriptionArg, WsAcknowledgement, WsCommand, WsEvent},
    ws::{
        business::BusinessSession,
        event,
        private::PrivateSession,
        session::{Session, SessionState},
        subscription::{SubscriptionKey, Subscriptions},
    },
};

fn ticker() -> SubscriptionArg {
    SubscriptionArg {
        channel: "tickers".to_owned(),
        instrument_id: Some("BTC-USDT".to_owned()),
    }
}

#[tokio::test]
async fn public_session_correlates_subscription_ack_and_event() {
    let (endpoint, command) = support::websocket::start().await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    socket
        .send(&WsCommand {
            op: "subscribe".to_owned(),
            id: Some(RequestId("subscription-1".to_owned())),
            args: vec![ticker()],
        })
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("subscription-1"));
    let acknowledgement: WsAcknowledgement =
        serde_json::from_str(&socket.receive().await.unwrap()).unwrap();
    assert_eq!(acknowledgement.op.as_deref(), Some("subscribe"));
    assert_eq!(acknowledgement.code.as_deref(), Some("0"));
    let event: WsEvent = serde_json::from_str(&socket.receive().await.unwrap()).unwrap();
    assert_eq!(event.arg, ticker());
}

#[tokio::test]
async fn fixture_drives_login_ping_and_disconnect() {
    let (endpoint, command) = support::websocket::start_login_ping_disconnect().await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    socket
        .send(&WsCommand {
            op: "login".to_owned(),
            id: Some(RequestId("login-1".to_owned())),
            args: Vec::<serde_json::Value>::new(),
        })
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("login"));
    assert!(socket.receive().await.unwrap().contains("\"op\":\"login\""));
    assert_eq!(socket.receive().await.unwrap(), "ping");
    assert!(matches!(
        socket.receive().await,
        Err(OkxError::UnknownCompletion)
    ));
}

#[tokio::test]
async fn fixture_exposes_correlated_exchange_error() {
    let (endpoint, command) = support::websocket::start_error("60012", "bad-subscription").await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    socket
        .send(&WsCommand {
            op: "subscribe".to_owned(),
            id: Some(RequestId("bad-subscription".to_owned())),
            args: vec![ticker()],
        })
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("bad-subscription"));
    let acknowledgement: WsAcknowledgement =
        serde_json::from_str(&socket.receive().await.unwrap()).unwrap();
    assert_eq!(
        acknowledgement.request_id,
        Some(RequestId("bad-subscription".to_owned()))
    );
    assert_eq!(acknowledgement.code.as_deref(), Some("60012"));
}

#[test]
fn session_login_recovery_and_private_write_gate() {
    let mut session = Session::default();
    assert_eq!(session.state(), SessionState::Disconnected);
    session.connecting();
    session.login_required();
    assert_eq!(session.state(), SessionState::Authenticating);
    session.login_acknowledged();
    assert_eq!(session.state(), SessionState::Resubscribing);
    assert!(!session.can_write());
    session.subscriptions_replayed();
    assert!(session.can_write());
    session.disconnected();
    assert_eq!(session.state(), SessionState::Backoff);
    assert!(!session.can_write());

    let private = PrivateSession::default();
    assert!(matches!(
        private.write_allowed(),
        Err(OkxError::UnknownCompletion)
    ));
}

#[test]
fn subscriptions_deduplicate_route_only_active_events_and_replay_after_disconnect() {
    let key = SubscriptionKey::from(&ticker());
    let other = SubscriptionKey {
        channel: "trades".to_owned(),
        instrument_id: Some("BTC-USDT".to_owned()),
    };
    let mut subscriptions = Subscriptions::default();
    assert!(subscriptions.request(key.clone()));
    assert!(!subscriptions.request(key.clone()));
    subscriptions.acknowledge(key.clone());
    assert!(subscriptions.active(&key));
    let event = WsEvent {
        arg: ticker(),
        data: Vec::new(),
    };
    assert!(event::matches(&event, &key));
    assert!(!event::matches(&event, &other));
    subscriptions.disconnect();
    assert!(!subscriptions.active(&key));
    assert_eq!(subscriptions.replay().collect::<Vec<_>>(), vec![&key]);
}

#[test]
fn business_session_keeps_optional_login_state_separate_from_private_writes() {
    let business = BusinessSession::default();
    assert_eq!(business.0.state(), SessionState::Disconnected);
    let private = PrivateSession::default();
    assert!(matches!(
        private.write_allowed(),
        Err(OkxError::UnknownCompletion)
    ));
}
