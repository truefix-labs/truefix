use truefix_transport::{TlsConfigError, TlsSpec, build_client_config};

#[test]
fn one_valid_cipher_suite_does_not_hide_an_unrecognized_name() {
    let spec = TlsSpec {
        key_store_path: None,
        key_store_bytes: None,
        trust_store_path: None,
        trust_store_bytes: None,
        need_client_auth: false,
        min_version: None,
        server_name: None,
        cipher_suites: vec![
            "TLS13_AES_128_GCM_SHA256".to_owned(),
            "TLS13_NOT_A_REAL_SUITE".to_owned(),
        ],
    };

    let error = build_client_config(&spec)
        .expect_err("every configured CipherSuites name must be recognized");

    match error {
        TlsConfigError::UnrecognizedCipherSuites(names) => {
            assert_eq!(names, ["TLS13_NOT_A_REAL_SUITE"]);
        }
        other => panic!("expected an unrecognized-cipher-suite error, got {other}"),
    }
}
