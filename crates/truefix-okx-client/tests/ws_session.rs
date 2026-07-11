mod support;

use time::macros::datetime;
use truefix_okx_client::{
    auth::{Clock, sign_websocket_login},
    config::Credentials,
    error::OkxError,
    transport::websocket::WebSocketTransport,
    types::{
        common::ExpirationTime,
        websocket::{RequestId, SubscriptionArg, WsAcknowledgement, WsCommand, WsEvent, WsOrder},
    },
    ws::{
        business::BusinessSession,
        coordinator::{AcknowledgementOutcome, WsStateCoordinator},
        event,
        private::PrivateSession,
        public::PublicSession,
        session::{Session, SessionState},
        subscription::{SubscriptionKey, Subscriptions},
    },
};

struct FixedClock(time::OffsetDateTime);

impl Clock for FixedClock {
    fn now(&self) -> time::OffsetDateTime {
        self.0
    }
}

fn ticker() -> SubscriptionArg {
    SubscriptionArg::new("tickers", Some("BTC-USDT".to_owned()))
}

#[tokio::test]
async fn public_session_correlates_subscription_ack_and_event() {
    let (endpoint, command) = support::websocket::start().await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    socket
        .send(&WsCommand::new(
            "subscribe",
            Some(RequestId("subscription-1".to_owned())),
            vec![ticker()],
        ))
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("subscription-1"));
    let acknowledgement: WsAcknowledgement =
        serde_json::from_str(&socket.receive().await.unwrap()).unwrap();
    assert_eq!(acknowledgement.event.as_deref(), Some("subscribe"));
    assert_eq!(acknowledgement.code.as_deref(), Some("0"));
    let event: WsEvent = serde_json::from_str(&socket.receive().await.unwrap()).unwrap();
    assert_eq!(event.arg, ticker());
}

#[tokio::test]
async fn fixture_drives_login_ping_and_disconnect() {
    let (endpoint, command) = support::websocket::start_login_ping_disconnect().await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    socket
        .send(&WsCommand::new(
            "login",
            Some(RequestId("login-1".to_owned())),
            Vec::<serde_json::Value>::new(),
        ))
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("login"));
    assert!(
        socket
            .receive()
            .await
            .unwrap()
            .contains("\"event\":\"login\"")
    );
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
        .send(&WsCommand::new(
            "subscribe",
            Some(RequestId("bad-subscription".to_owned())),
            vec![ticker()],
        ))
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
    assert_eq!(acknowledgement.event.as_deref(), Some("error"));
}

#[tokio::test]
async fn receive_for_session_disconnects_after_a_peer_close() {
    let (endpoint, command) = support::websocket::start_scripted(&[], true).await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    let mut session = Session::default();
    session.connected();
    socket
        .send(&WsCommand::new(
            "subscribe",
            Some(RequestId("close-1".to_owned())),
            vec![ticker()],
        ))
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("close-1"));

    assert!(matches!(
        socket.receive_for_session(&mut session).await,
        Err(OkxError::UnknownCompletion)
    ));
    assert_eq!(session.state(), SessionState::Backoff);
}

#[tokio::test]
async fn receive_for_session_disconnects_after_a_non_text_frame() {
    let (endpoint, command) = support::websocket::start_binary_frame().await;
    let mut socket = WebSocketTransport::connect(&endpoint).await.unwrap();
    let mut session = Session::default();
    session.connected();
    socket
        .send(&WsCommand::new(
            "subscribe",
            Some(RequestId("binary-1".to_owned())),
            vec![ticker()],
        ))
        .await
        .unwrap();
    assert!(command.await.unwrap().contains("binary-1"));

    assert!(matches!(
        socket.receive_for_session(&mut session).await,
        Err(OkxError::Decode(_))
    ));
    assert_eq!(session.state(), SessionState::Backoff);
}

#[test]
fn session_login_recovery_and_private_write_gate() {
    let mut session = Session::default();
    assert_eq!(session.state(), SessionState::Disconnected);
    session.connecting();
    session.login_required();
    assert_eq!(session.state(), SessionState::Authenticating);
    session.login_acknowledged(true);
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
fn subscriptions_require_an_active_session() {
    let mut public = PublicSession::default();
    assert!(matches!(
        public.subscribe(vec![ticker()]),
        Err(OkxError::UnknownCompletion)
    ));
    public.connected();
    assert!(public.subscribe(vec![ticker()]).is_ok());
    assert!(public.send("ping", Vec::new()).is_ok());
    assert!(
        public
            .subscribe_raw(vec![serde_json::json!({
                "channel": "instruments",
                "instType": "FUTURES"
            })])
            .is_ok()
    );

    let mut private = PrivateSession::default();
    assert!(matches!(
        private.subscribe(vec![ticker()]),
        Err(OkxError::UnknownCompletion)
    ));
    private.connected();
    assert_eq!(private.0.state(), SessionState::Authenticating);
    assert!(matches!(
        private.subscribe(vec![ticker()]),
        Err(OkxError::UnknownCompletion)
    ));
    private.login_acknowledged(false);
    private.subscriptions_replayed();
    assert!(matches!(
        private.subscribe(vec![ticker()]),
        Err(OkxError::UnknownCompletion)
    ));
    private.login_acknowledged(true);
    private.subscriptions_replayed();
    assert!(private.subscribe(vec![ticker()]).is_ok());
}

#[test]
fn subscriptions_deduplicate_route_only_active_events_and_replay_after_disconnect() {
    let key = SubscriptionKey::from(&ticker());
    let other = SubscriptionKey {
        channel: "trades".to_owned(),
        instrument_id: Some("BTC-USDT".to_owned()),
        extra: Vec::new(),
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

#[test]
fn signed_login_uses_the_injected_clock_for_private_and_business_sessions() {
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let clock = FixedClock(datetime!(2024-01-01 0:00 UTC));

    let mut private = PrivateSession::default();
    let private_login = private.signed_login(&credentials, &clock).unwrap();
    assert_eq!(private_login.op, "login");
    assert!(private_login.id.is_none());
    let private_args = &private_login.args[0];
    assert_eq!(private_args["apiKey"], "key");
    assert_eq!(private_args["passphrase"], "passphrase");
    assert_eq!(private_args["timestamp"], "1704067200");
    assert_eq!(
        private_args["sign"],
        sign_websocket_login(&credentials, 1_704_067_200).unwrap()
    );

    let mut business = BusinessSession::default();
    let business_login = business.signed_login(&credentials, &clock).unwrap();
    assert_eq!(business_login.op, "login");
    assert!(business_login.id.is_none());
    let business_args = &business_login.args[0];
    assert_eq!(business_args["apiKey"], "key");
    assert_eq!(business_args["passphrase"], "passphrase");
    assert_eq!(business_args["timestamp"], "1704067200");
    assert_eq!(
        business_args["sign"],
        sign_websocket_login(&credentials, 1_704_067_200).unwrap()
    );
}

#[test]
fn subscriptions_with_distinct_channel_parameters_have_distinct_keys() {
    let futures = SubscriptionArg::new("orders", None).with_parameter("instType", "FUTURES");
    let swaps = SubscriptionArg::new("orders", None).with_parameter("instType", "SWAP");
    assert_ne!(
        SubscriptionKey::from(&futures),
        SubscriptionKey::from(&swaps)
    );
}

#[test]
fn inbound_messages_arm_heartbeat_and_a_missing_pong_disconnects_the_session() {
    let mut session = Session::default();
    session.connected();
    let now = std::time::Instant::now();
    session.message_received(now);
    assert!(
        !session.ping_due(now + Session::HEARTBEAT_INTERVAL - std::time::Duration::from_millis(1))
    );
    assert!(session.ping_due(now + Session::HEARTBEAT_INTERVAL));
    assert!(!session.ping_due(now + Session::HEARTBEAT_INTERVAL * 2));
    assert_eq!(session.state(), SessionState::Backoff);
}

#[test]
fn private_order_expiration_is_available_only_on_order_and_amend_helpers() {
    let mut session = PrivateSession::default();
    session.connected();
    session.login_acknowledged(true);
    session.subscriptions_replayed();
    let command = session
        .place_order_with_expiration(
            WsOrder {
                instrument_id: "BTC-USDT".to_owned(),
                trade_mode: "cash".to_owned(),
                side: "buy".to_owned(),
                order_type: "market".to_owned(),
                size: "1".to_owned(),
                price: None,
                client_order_id: None,
            },
            ExpirationTime::new(1_704_067_200_123).unwrap(),
        )
        .unwrap();
    assert_eq!(command.op, "order");
    assert_eq!(
        command.expiration_time(),
        Some(ExpirationTime::new(1_704_067_200_123).unwrap())
    );
}

#[test]
fn routing_ignores_server_metadata_but_keeps_requested_parameters() {
    let requested = SubscriptionArg::new("orders", None)
        .with_parameter("instType", "FUTURES")
        .with_parameter("instFamily", "BTC-USD");
    let key = SubscriptionKey::from(&requested);
    let mut subscriptions = Subscriptions::default();
    assert!(subscriptions.request(key.clone()));
    subscriptions.acknowledge(key);

    let server_event = WsEvent {
        arg: requested.clone().with_parameter("uid", "12345"),
        data: Vec::new(),
    };
    assert!(event::matches(
        &server_event,
        &SubscriptionKey::from(&requested)
    ));
    assert!(event::route(&subscriptions, server_event).is_some());

    let wrong_instrument_type = WsEvent {
        arg: requested.with_parameter("instType", "SWAP"),
        data: Vec::new(),
    };
    assert!(event::route(&subscriptions, wrong_instrument_type).is_none());
}

#[test]
fn wildcard_subscription_routes_events_with_server_selected_instrument() {
    let requested = SubscriptionArg::new("instruments", None).with_parameter("instType", "SWAP");
    let key = SubscriptionKey::from(&requested);
    let mut subscriptions = Subscriptions::default();
    assert!(subscriptions.request(key.clone()));
    subscriptions.acknowledge(key);

    let event = WsEvent {
        arg: SubscriptionArg::new("instruments", Some("BTC-USDT-SWAP".to_owned()))
            .with_parameter("instType", "SWAP"),
        data: Vec::new(),
    };
    assert!(event::route(&subscriptions, event).is_some());
}

#[test]
fn acknowledgement_coordinator_applies_login_subscription_error_and_notice() {
    let requested = SubscriptionArg::new("orders", None).with_parameter("instType", "SWAP");
    let key = SubscriptionKey::from(&requested);
    let request_id = RequestId("subscribe-1".to_owned());
    let mut coordinator = WsStateCoordinator::default();
    assert!(coordinator.subscriptions_mut().request(key.clone()));
    assert!(
        coordinator
            .subscriptions_mut()
            .correlate(request_id.clone(), &key)
    );
    let mut session = Session::default();
    session.login_required();

    let login = WsAcknowledgement {
        event: Some("login".to_owned()),
        request_id: None,
        code: Some("0".to_owned()),
        msg: None,
    };
    assert_eq!(
        coordinator.apply(&mut session, &login),
        AcknowledgementOutcome::Login { success: true }
    );
    assert_eq!(session.state(), SessionState::Resubscribing);

    let subscribed = WsAcknowledgement {
        event: Some("subscribe".to_owned()),
        request_id: Some(request_id.clone()),
        code: Some("0".to_owned()),
        msg: None,
    };
    assert_eq!(
        coordinator.apply(&mut session, &subscribed),
        AcknowledgementOutcome::Subscription {
            request_id,
            key: key.clone(),
            success: true,
        }
    );
    assert!(coordinator.subscriptions().active(&key));

    let error_id = RequestId("subscribe-2".to_owned());
    assert!(
        coordinator
            .subscriptions_mut()
            .correlate(error_id.clone(), &key)
    );
    let error = WsAcknowledgement {
        event: Some("error".to_owned()),
        request_id: Some(error_id.clone()),
        code: Some("60012".to_owned()),
        msg: Some("bad subscription".to_owned()),
    };
    assert!(matches!(
        coordinator.apply(&mut session, &error),
        AcknowledgementOutcome::Subscription { success: false, .. }
    ));

    let notice = WsAcknowledgement {
        event: Some("notice".to_owned()),
        request_id: None,
        code: Some("0".to_owned()),
        msg: None,
    };
    assert_eq!(
        coordinator.apply(&mut session, &notice),
        AcknowledgementOutcome::Notice
    );
    assert_eq!(session.state(), SessionState::Backoff);
    assert!(!coordinator.subscriptions().active(&key));
    assert!(coordinator.subscriptions().desired(&key));
}
