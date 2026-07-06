//! T166/T167 (feature 009, NEW-95): a configured trust store that produces zero usable
//! certificates now surfaces a typed error instead of a silent, non-functional empty
//! `RootCertStore` (every subsequent handshake would otherwise fail with no diagnostic at all).

use truefix_transport::{TlsSpec, build_client_config};

fn spec_with_trust_store_bytes(bytes: Vec<u8>) -> TlsSpec {
    TlsSpec {
        key_store_path: None,
        key_store_bytes: None,
        trust_store_path: None,
        trust_store_bytes: Some(bytes),
        need_client_auth: false,
        min_version: None,
        server_name: None,
        cipher_suites: Vec::new(),
    }
}

#[test]
fn an_empty_trust_store_surfaces_a_typed_error_instead_of_an_empty_silent_store() {
    let spec = spec_with_trust_store_bytes(Vec::new());
    let err = build_client_config(&spec).expect_err(
        "a configured-but-empty trust store must error, not silently succeed with no trust anchors",
    );
    let message = err.to_string();
    assert!(
        message.contains("no usable certificates"),
        "expected an EmptyTrustStore-shaped error, got: {message}"
    );
}

#[test]
fn a_trust_store_with_at_least_one_valid_certificate_still_succeeds() {
    let issued = rcgen::generate_simple_self_signed(vec!["example.invalid".to_string()]).unwrap();
    let cert_pem = issued.cert.pem();
    let spec = spec_with_trust_store_bytes(cert_pem.into_bytes());
    build_client_config(&spec).expect("a trust store with a valid certificate should still build");
}
