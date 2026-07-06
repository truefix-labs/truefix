//! T067/T068 (US10) — socket options and numbered `SocketConnectHost<N>`/`SocketConnectPort<N>`
//! backup endpoints map from a settings file into a runnable `ResolvedSession` (FR-019).

use std::time::Duration;

use truefix_config::{ResolvedSession, SessionSettings, SocketEndpoint};

fn resolved(cfg: &str) -> ResolvedSession {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn acceptor_base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

fn initiator_base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn default_socket_options_match_transport_defaults() {
    let rs = resolved(&acceptor_base(""));
    assert!(rs.socket_options.tcp_no_delay);
    assert!(!rs.socket_options.keep_alive);
    assert!(!rs.socket_options.reuse_address);
    assert_eq!(rs.socket_options.linger, None);
    assert!(!rs.socket_options.oob_inline);
    assert_eq!(rs.socket_options.recv_buffer_size, None);
    assert_eq!(rs.socket_options.send_buffer_size, None);
    assert_eq!(rs.socket_options.traffic_class, None);
}

#[test]
fn full_socket_option_set_is_mapped() {
    let rs = resolved(&acceptor_base(
        "SocketTcpNoDelay=N\nSocketKeepAlive=Y\nSocketReuseAddress=Y\nSocketLinger=5\n\
         SocketOobInline=Y\nSocketReceiveBufferSize=65536\nSocketSendBufferSize=32768\n\
         SocketTrafficClass=8\n",
    ));
    assert!(!rs.socket_options.tcp_no_delay);
    assert!(rs.socket_options.keep_alive);
    assert!(rs.socket_options.reuse_address);
    assert_eq!(rs.socket_options.linger, Some(Duration::from_secs(5)));
    assert!(rs.socket_options.oob_inline);
    assert_eq!(rs.socket_options.recv_buffer_size, Some(65536));
    assert_eq!(rs.socket_options.send_buffer_size, Some(32768));
    assert_eq!(rs.socket_options.traffic_class, Some(8));
}

#[test]
fn negative_linger_is_disabled() {
    let rs = resolved(&acceptor_base("SocketLinger=-1\n"));
    assert_eq!(rs.socket_options.linger, None);
}

#[test]
fn invalid_socket_option_is_a_typed_error() {
    let cfg = acceptor_base("SocketReceiveBufferSize=not-a-number\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::InvalidValue { key, .. } => {
            assert_eq!(key, "SocketReceiveBufferSize")
        }
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}

#[test]
fn acceptor_sessions_never_get_failover_addresses() {
    let rs = resolved(&acceptor_base(
        "SocketConnectHost1=10.0.0.2\nSocketConnectPort1=5002\n",
    ));
    assert!(rs.failover_addresses.is_empty());
}

#[test]
fn no_numbered_endpoints_means_no_failover_addresses() {
    let rs = resolved(&initiator_base(""));
    assert!(rs.failover_addresses.is_empty());
}

#[test]
fn numbered_backup_endpoints_are_mapped_in_order() {
    let rs = resolved(&initiator_base(
        "SocketConnectHost1=10.0.0.2\nSocketConnectPort1=5002\n\
         SocketConnectHost2=10.0.0.3\nSocketConnectPort2=5003\n",
    ));
    let expected = vec![
        SocketEndpoint::new("10.0.0.2", 5002),
        SocketEndpoint::new("10.0.0.3", 5003),
    ];
    assert_eq!(rs.failover_addresses, expected);
}

#[test]
fn numbering_stops_at_first_gap() {
    // SocketConnectHost2/Port2 present but 1 is missing: only N=1.. contiguous from 1 is honored,
    // so a gap at N=1 means nothing after it is picked up.
    let rs = resolved(&initiator_base(
        "SocketConnectHost2=10.0.0.3\nSocketConnectPort2=5003\n",
    ));
    assert!(rs.failover_addresses.is_empty());
}

#[test]
fn mismatched_numbered_host_and_port_is_a_typed_error() {
    let cfg = initiator_base("SocketConnectHost1=10.0.0.2\n");
    let err = SessionSettings::parse(&cfg).unwrap().resolve().unwrap_err();
    match err {
        truefix_config::ConfigError::InvalidValue { key, .. } => {
            assert_eq!(key, "SocketConnectHost1/SocketConnectPort1")
        }
        other => panic!("expected InvalidValue, got {other:?}"),
    }
}
