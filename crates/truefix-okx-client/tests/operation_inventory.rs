use truefix_okx_client::inventory::{
    BASELINE_DOMAINS, BASELINE_REST_OPERATION_COUNT, WEBSOCKET_TRADE_COMMANDS,
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
