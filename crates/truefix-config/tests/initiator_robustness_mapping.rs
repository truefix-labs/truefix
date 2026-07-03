//! T040/T041 (US6, feature 005) — reconnect backoff steps, local-bind address, and connect
//! timeout map from a settings file into `SessionConfig` (GAP-14/GAP-15/GAP-16; FR-014-016).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use truefix_config::SessionSettings;
use truefix_session::SessionConfig;

fn resolved(cfg: &str) -> SessionConfig {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
        .session
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn reconnect_interval_defaults_leave_steps_empty() {
    let c = resolved(&base(""));
    assert_eq!(c.reconnect_interval, 30);
    assert!(c.reconnect_interval_steps.is_empty());
}

#[test]
fn single_integer_reconnect_interval_is_unchanged() {
    let c = resolved(&base("ReconnectInterval=7\n"));
    assert_eq!(c.reconnect_interval, 7);
    assert!(c.reconnect_interval_steps.is_empty());
}

#[test]
fn space_separated_reconnect_interval_populates_steps() {
    let c = resolved(&base("ReconnectInterval=2 5 10 30\n"));
    assert_eq!(c.reconnect_interval, 2);
    assert_eq!(c.reconnect_interval_steps, vec![2, 5, 10, 30]);
}

#[test]
fn comma_separated_reconnect_interval_populates_steps() {
    let c = resolved(&base("ReconnectInterval=2,5,10,30\n"));
    assert_eq!(c.reconnect_interval_steps, vec![2, 5, 10, 30]);
}

#[test]
fn malformed_reconnect_interval_is_a_typed_error() {
    let err = SessionSettings::parse(&base("ReconnectInterval=2 not-a-number\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { key, .. } if key == "ReconnectInterval"
    ));
}

#[test]
fn local_bind_addr_defaults_to_none() {
    let c = resolved(&base(""));
    assert_eq!(c.local_bind_addr, None);
}

#[test]
fn socket_local_host_and_port_together_resolve_the_bind_address() {
    let c = resolved(&base("SocketLocalHost=127.0.0.1\nSocketLocalPort=9999\n"));
    assert_eq!(
        c.local_bind_addr,
        Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            9999
        ))
    );
}

#[test]
fn socket_local_port_without_host_is_a_typed_error() {
    let err = SessionSettings::parse(&base("SocketLocalPort=9999\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { key, .. }
            if key == "SocketLocalHost/SocketLocalPort"
    ));
}

#[test]
fn connect_timeout_defaults_to_none() {
    let c = resolved(&base(""));
    assert_eq!(c.connect_timeout, None);
}

#[test]
fn socket_connect_timeout_maps_from_seconds() {
    let c = resolved(&base("SocketConnectTimeout=15\n"));
    assert_eq!(c.connect_timeout, Some(Duration::from_secs(15)));
}
