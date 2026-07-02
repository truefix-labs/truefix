//! T067 (US12) — PROXY protocol trust, forward-proxy, inline PEM bytes, cipher suites, and
//! synchronous-write-timeout settings map from a settings file into a runnable `ResolvedSession`
//! (FR-015/016/017).

use std::net::IpAddr;
use std::time::Duration;

use truefix_config::{ConfigError, ProxyKind, ResolvedSession, SessionSettings};

fn resolved(cfg: &str) -> ResolvedSession {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn resolve_err(cfg: &str) -> ConfigError {
    SessionSettings::parse(cfg).unwrap().resolve().unwrap_err()
}

fn initiator_base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=initiator\n\
         SocketConnectHost=127.0.0.1\nSocketConnectPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

fn acceptor_base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn defaults_have_no_proxy_no_trust_no_sync_timeout() {
    let rs = resolved(&initiator_base(""));
    assert!(rs.proxy.is_none());
    assert!(rs.trusted_proxy_addresses.is_empty());
    assert_eq!(rs.sync_write_timeout, None);
}

#[test]
fn proxy_type_host_port_map_without_credentials() {
    let rs = resolved(&initiator_base(
        "ProxyType=Socks5\nProxyHost=proxy.example.com\nProxyPort=1080\n",
    ));
    let proxy = rs.proxy.expect("proxy configured");
    assert_eq!(proxy.proxy_type, ProxyKind::Socks5);
    assert_eq!(proxy.host, "proxy.example.com");
    assert_eq!(proxy.port, 1080);
    assert_eq!(proxy.username, None);
    assert_eq!(proxy.password, None);
}

#[test]
fn proxy_user_and_password_map_when_set() {
    let rs = resolved(&initiator_base(
        "ProxyType=Socks5\nProxyHost=proxy.example.com\nProxyPort=1080\n\
         ProxyUser=alice\nProxyPassword=s3cret\n",
    ));
    let proxy = rs.proxy.expect("proxy configured");
    assert_eq!(proxy.username.as_deref(), Some("alice"));
    assert_eq!(proxy.password.as_deref(), Some("s3cret"));
}

#[test]
fn all_three_proxy_types_are_recognized() {
    for (value, expected) in [
        ("Socks4", ProxyKind::Socks4),
        ("Socks5", ProxyKind::Socks5),
        ("HttpConnect", ProxyKind::HttpConnect),
    ] {
        let rs = resolved(&initiator_base(&format!(
            "ProxyType={value}\nProxyHost=proxy.example.com\nProxyPort=1080\n"
        )));
        assert_eq!(rs.proxy.expect("proxy configured").proxy_type, expected);
    }
}

#[test]
fn unknown_proxy_type_is_a_typed_error() {
    let err = resolve_err(&initiator_base(
        "ProxyType=Bogus\nProxyHost=proxy.example.com\nProxyPort=1080\n",
    ));
    assert!(matches!(err, ConfigError::InvalidValue { key, .. } if key == "ProxyType"));
}

#[test]
fn proxy_host_is_required_when_proxy_type_is_set() {
    let err = resolve_err(&initiator_base("ProxyType=Socks5\nProxyPort=1080\n"));
    assert!(matches!(err, ConfigError::MissingRequired { key, .. } if key == "ProxyHost"));
}

#[test]
fn use_tcp_proxy_off_ignores_trusted_proxy_addresses() {
    let rs = resolved(&acceptor_base("TrustedProxyAddresses=10.0.0.1,10.0.0.2\n"));
    assert!(rs.trusted_proxy_addresses.is_empty());
}

#[test]
fn trusted_proxy_addresses_map_as_a_comma_separated_ip_list() {
    let rs = resolved(&acceptor_base(
        "UseTCPProxy=Y\nTrustedProxyAddresses=10.0.0.1, 10.0.0.2\n",
    ));
    assert_eq!(
        rs.trusted_proxy_addresses,
        vec![
            "10.0.0.1".parse::<IpAddr>().unwrap(),
            "10.0.0.2".parse::<IpAddr>().unwrap()
        ]
    );
}

#[test]
fn invalid_trusted_proxy_address_is_a_typed_error() {
    let err = resolve_err(&acceptor_base(
        "UseTCPProxy=Y\nTrustedProxyAddresses=not-an-ip\n",
    ));
    assert!(matches!(err, ConfigError::InvalidValue { key, .. } if key == "TrustedProxyAddresses"));
}

#[test]
fn sync_write_timeout_maps_from_seconds_when_enabled() {
    let rs = resolved(&acceptor_base(
        "SocketSynchronousWrites=Y\nSocketSynchronousWriteTimeout=5\n",
    ));
    assert_eq!(rs.sync_write_timeout, Some(Duration::from_secs(5)));
}

#[test]
fn sync_write_timeout_absent_without_the_timeout_value() {
    let rs = resolved(&acceptor_base("SocketSynchronousWrites=Y\n"));
    assert_eq!(rs.sync_write_timeout, None);
}

#[test]
fn cipher_suites_map_as_a_comma_separated_list() {
    let rs = resolved(&acceptor_base(
        "SocketUseSSL=Y\nSocketKeyStoreBytes=dummy\n\
         CipherSuites=TLS13_AES_128_GCM_SHA256, TLS13_AES_256_GCM_SHA384\n",
    ));
    let tls = rs.tls.expect("tls configured");
    assert_eq!(
        tls.cipher_suites,
        vec![
            "TLS13_AES_128_GCM_SHA256".to_owned(),
            "TLS13_AES_256_GCM_SHA384".to_owned()
        ]
    );
}

#[test]
fn key_store_bytes_decode_backslash_n_as_a_newline() {
    let rs = resolved(&acceptor_base(
        "SocketUseSSL=Y\nSocketKeyStoreBytes=line1\\nline2\n",
    ));
    let tls = rs.tls.expect("tls configured");
    assert_eq!(
        tls.key_store_bytes.as_deref(),
        Some(b"line1\nline2".as_ref())
    );
    assert_eq!(tls.key_store_path, None);
}

#[test]
fn trust_store_bytes_map_independently_of_key_store_bytes() {
    let rs = resolved(&acceptor_base(
        "SocketUseSSL=Y\nSocketKeyStoreBytes=key\nSocketTrustStoreBytes=ca\n",
    ));
    let tls = rs.tls.expect("tls configured");
    assert_eq!(tls.trust_store_bytes.as_deref(), Some(b"ca".as_ref()));
}

#[test]
fn tls_requires_either_a_key_store_path_or_bytes() {
    let err = resolve_err(&acceptor_base("SocketUseSSL=Y\n"));
    assert!(matches!(
        err,
        ConfigError::MissingRequired { key, .. } if key == "SocketKeyStore or SocketKeyStoreBytes"
    ));
}

#[test]
fn key_store_path_still_works_unchanged_when_bytes_are_absent() {
    let rs = resolved(&acceptor_base(
        "SocketUseSSL=Y\nSocketKeyStore=/tmp/combined.pem\n",
    ));
    let tls = rs.tls.expect("tls configured");
    assert_eq!(
        tls.key_store_path,
        Some(std::path::PathBuf::from("/tmp/combined.pem"))
    );
    assert_eq!(tls.key_store_bytes, None);
}
