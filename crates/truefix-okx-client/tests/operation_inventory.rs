use truefix_okx_client::inventory::{
    AuthClass, BASELINE_DOMAINS, BASELINE_OPERATION_MANIFEST, BASELINE_REST_OPERATION_COUNT,
    BaselineOperation, ManifestValidationError, ReplayClass, WEBSOCKET_TRADE_COMMANDS,
    validate_operation_manifest,
};

#[test]
fn source_baseline_count_is_stable() {
    assert_eq!(
        BASELINE_DOMAINS
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>(),
        BASELINE_REST_OPERATION_COUNT
    );
    assert_eq!(WEBSOCKET_TRADE_COMMANDS.len(), 7);
}

#[test]
fn baseline_domain_counts_cover_all_264_operations() {
    assert_eq!(BASELINE_REST_OPERATION_COUNT, 264);
    assert_eq!(
        BASELINE_DOMAINS
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>(),
        264
    );
}

#[test]
fn baseline_manifest_is_complete_and_classified() {
    assert_eq!(BASELINE_OPERATION_MANIFEST.len(), 264);
    validate_operation_manifest(BASELINE_OPERATION_MANIFEST, BASELINE_REST_OPERATION_COUNT)
        .unwrap();

    for (domain, expected) in BASELINE_DOMAINS {
        assert_eq!(
            BASELINE_OPERATION_MANIFEST
                .iter()
                .filter(|record| record.domain == *domain)
                .count(),
            *expected,
            "baseline count changed for {domain}"
        );
    }
}

const VALID_RECORD: BaselineOperation = BaselineOperation {
    source_identity: "Account.py:1:balance",
    domain: "account",
    operation: "balance",
    method: "GET",
    path: "/api/v5/account/balance",
    auth: AuthClass::Private,
    replay: ReplayClass::ReadOnly,
    native_entrypoint: "account::balances",
    fixture_id: "operation_inventory::baseline_manifest_is_complete_and_classified",
};

#[test]
fn duplicate_baseline_identity_is_rejected() {
    assert_eq!(
        validate_operation_manifest(&[VALID_RECORD, VALID_RECORD], 2),
        Err(ManifestValidationError::DuplicateSourceIdentity(
            VALID_RECORD.source_identity
        ))
    );
}

#[test]
fn unsupported_and_unclassified_records_are_rejected() {
    let unsupported = BaselineOperation {
        native_entrypoint: "UNSUPPORTED",
        ..VALID_RECORD
    };
    assert_eq!(
        validate_operation_manifest(&[unsupported], 1),
        Err(ManifestValidationError::Unsupported(
            unsupported.source_identity
        ))
    );

    let unclassified = BaselineOperation {
        method: "",
        ..VALID_RECORD
    };
    assert_eq!(
        validate_operation_manifest(&[unclassified], 1),
        Err(ManifestValidationError::Unclassified(
            unclassified.source_identity
        ))
    );
}

#[test]
fn us1_manifest_entries_have_native_and_fixture_evidence() {
    assert!(
        truefix_okx_client::inventory::US1_OPERATION_MANIFEST
            .iter()
            .all(|entry| !entry.native_entrypoint.is_empty() && !entry.fixture_id.is_empty())
    );
}

#[test]
fn every_manifest_record_has_operation_transport_and_evidence() {
    for record in truefix_okx_client::inventory::US1_OPERATION_MANIFEST
        .iter()
        .chain(truefix_okx_client::inventory::WEBSOCKET_OPERATION_MANIFEST.iter())
        .chain(truefix_okx_client::inventory::US3_OPERATION_MANIFEST.iter())
    {
        assert!(!record.domain.is_empty());
        assert!(!record.operation.is_empty());
        assert!(!record.transport.is_empty());
        assert!(!record.native_entrypoint.is_empty());
        assert!(!record.fixture_id.is_empty());
    }
}
