//! Auditable source-capability inventory.

use std::collections::HashSet;

/// Authentication required by a baseline operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthClass {
    /// The operation is available without API credentials.
    Public,
    /// The operation requires the OKX REST signing headers.
    Private,
}

/// Automatic replay classification for a baseline operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayClass {
    /// A safe read may be retried once after a transient failure.
    ReadOnly,
    /// A state-changing request must never be replayed automatically.
    NeverReplay,
}

/// One source operation and all evidence required to audit its Rust implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineOperation {
    /// Stable source file, line, and Python method identity.
    pub source_identity: &'static str,
    /// Source business domain.
    pub domain: &'static str,
    /// Python capability name.
    pub operation: &'static str,
    /// Approved HTTP method.
    pub method: &'static str,
    /// Approved current OKX path.
    pub path: &'static str,
    /// Authentication classification.
    pub auth: AuthClass,
    /// Automatic replay classification.
    pub replay: ReplayClass,
    /// Native Rust service and method implementing the operation.
    pub native_entrypoint: &'static str,
    /// Local fixture which audits the record.
    pub fixture_id: &'static str,
}

/// Why an operation manifest cannot be accepted as complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestValidationError {
    /// The manifest does not contain exactly the pinned baseline count.
    WrongCount { expected: usize, actual: usize },
    /// More than one record claims the same source identity.
    DuplicateSourceIdentity(&'static str),
    /// A required evidence field is empty or has an unknown classification.
    Unclassified(&'static str),
    /// The baseline operation has no native Rust implementation.
    Unsupported(&'static str),
}

#[path = "generated_operation_inventory.rs"]
mod generated;
pub use generated::BASELINE_OPERATION_MANIFEST;

/// Validates exact count, uniqueness, classifications, native support, and fixture evidence.
pub fn validate_operation_manifest(
    records: &[BaselineOperation],
    expected_count: usize,
) -> Result<(), ManifestValidationError> {
    if records.len() != expected_count {
        return Err(ManifestValidationError::WrongCount {
            expected: expected_count,
            actual: records.len(),
        });
    }
    let mut identities = HashSet::with_capacity(records.len());
    for record in records {
        if !identities.insert(record.source_identity) {
            return Err(ManifestValidationError::DuplicateSourceIdentity(
                record.source_identity,
            ));
        }
        if record.source_identity.is_empty()
            || record.domain.is_empty()
            || record.operation.is_empty()
            || !matches!(record.method, "GET" | "POST")
            || !record.path.starts_with("/api/v5/")
            || record.fixture_id.is_empty()
        {
            return Err(ManifestValidationError::Unclassified(
                record.source_identity,
            ));
        }
        if record.native_entrypoint.is_empty() || record.native_entrypoint == "UNSUPPORTED" {
            return Err(ManifestValidationError::Unsupported(record.source_identity));
        }
        if (record.method == "GET" && record.replay != ReplayClass::ReadOnly)
            || (record.method == "POST" && record.replay != ReplayClass::NeverReplay)
        {
            return Err(ManifestValidationError::Unclassified(
                record.source_identity,
            ));
        }
    }
    Ok(())
}

/// A source capability that must have a native entrypoint and automated evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationInventoryEntry {
    /// Source business domain.
    pub domain: &'static str,
    /// Stable source capability identity.
    pub operation: &'static str,
    /// REST, public WebSocket, private WebSocket, or business WebSocket.
    pub transport: &'static str,
    /// Native Rust method which implements this operation.
    pub native_entrypoint: &'static str,
    /// Fixture scenario proving request and response behavior.
    pub fixture_id: &'static str,
}

/// US1 operations already linked to their public method and fixture evidence.
pub const US1_OPERATION_MANIFEST: &[OperationInventoryEntry] = &[
    OperationInventoryEntry {
        domain: "account",
        operation: "balance",
        transport: "REST",
        native_entrypoint: "AccountService::balances",
        fixture_id: "http_contract::demo_writes_are_signed_once_and_send_exact_body",
    },
    OperationInventoryEntry {
        domain: "account",
        operation: "positions",
        transport: "REST",
        native_entrypoint: "AccountService::positions",
        fixture_id: "http_contract::public_requests_are_unsigned_and_preserve_encoded_query",
    },
    OperationInventoryEntry {
        domain: "trade",
        operation: "place_order",
        transport: "REST",
        native_entrypoint: "TradeService::place_order",
        fixture_id: "http_contract::demo_writes_are_signed_once_and_send_exact_body",
    },
    OperationInventoryEntry {
        domain: "market_data",
        operation: "ticker",
        transport: "REST",
        native_entrypoint: "MarketService::ticker",
        fixture_id: "http_contract::public_requests_are_unsigned_and_preserve_encoded_query",
    },
    OperationInventoryEntry {
        domain: "market_data",
        operation: "pagination",
        transport: "REST",
        native_entrypoint: "response::page_metadata",
        fixture_id: "http_contract::exchange_rejection_and_pagination_metadata_are_preserved",
    },
    OperationInventoryEntry {
        domain: "account",
        operation: "bills_leverage_margin_risk_fee",
        transport: "REST",
        native_entrypoint: "AccountService::bills/leverage/adjust_margin/position_risk/fee_rates",
        fixture_id: "http_contract::demo_writes_are_signed_once_and_send_exact_body",
    },
    OperationInventoryEntry {
        domain: "trade",
        operation: "batch_cancel_amend_close_query_fills_history",
        transport: "REST",
        native_entrypoint: "TradeService::place_batch/cancel_batch/amend_batch/close_positions/order/orders/order_history/fills",
        fixture_id: "http_contract::demo_writes_are_signed_once_and_send_exact_body",
    },
    OperationInventoryEntry {
        domain: "market_data",
        operation: "books_candles_trades_index_mark_funding",
        transport: "REST",
        native_entrypoint: "MarketService::books/candles/history_candles/trades/index_tickers/mark_price/funding_rate/funding_rate_history",
        fixture_id: "http_contract::public_requests_are_unsigned_and_preserve_encoded_query",
    },
];

/// High-level baseline domains derived from `python-okx@fa8d738`.
///
/// The expanded generated manifest is added with the domain services so tests can require every
/// operation, not merely these categories.
pub const BASELINE_DOMAINS: &[(&str, usize)] = &[
    ("account", 52),
    ("trade", 28),
    ("public_data", 21),
    ("market_data", 19),
    ("grid", 19),
    ("funding", 18),
    ("block_trading", 17),
    ("trading_data", 11),
    ("spread_trading", 11),
    ("sub_account", 10),
    ("copy_trading", 9),
    ("dual_invest", 8),
    ("flexible_loan", 8),
    ("eth_staking", 7),
    ("savings", 6),
    ("sol_staking", 6),
    ("staking_defi", 6),
    ("convert", 5),
    ("fd_broker", 2),
    ("status", 1),
];

/// Count of baseline REST capabilities.
pub const BASELINE_REST_OPERATION_COUNT: usize = 264;

/// WebSocket trade commands that require corresponding native support.
pub const WEBSOCKET_TRADE_COMMANDS: &[&str] = &[
    "order",
    "batch-orders",
    "cancel-order",
    "batch-cancel-orders",
    "amend-order",
    "batch-amend-orders",
    "mass-cancel",
];

/// Real-time capability evidence; write commands deliberately share the no-replay fixture.
pub const WEBSOCKET_OPERATION_MANIFEST: &[OperationInventoryEntry] = &[
    OperationInventoryEntry {
        domain: "websocket",
        operation: "public_subscribe",
        transport: "public WebSocket",
        native_entrypoint: "PublicSession",
        fixture_id: "ws_session::public_session_correlates_subscription_ack_and_event",
    },
    OperationInventoryEntry {
        domain: "websocket",
        operation: "private_login_gate",
        transport: "private WebSocket",
        native_entrypoint: "PrivateSession::write_allowed",
        fixture_id: "ws_session::private_write_is_rejected_before_login",
    },
    OperationInventoryEntry {
        domain: "websocket",
        operation: "business",
        transport: "business WebSocket",
        native_entrypoint: "BusinessSession",
        fixture_id: "ws_session::business_session_supports_optional_login",
    },
    OperationInventoryEntry {
        domain: "websocket",
        operation: "trade_commands_no_replay",
        transport: "private/business WebSocket",
        native_entrypoint: "PrivateSession::write_allowed",
        fixture_id: "ws_session::write_is_never_replayed_after_disconnect",
    },
];

/// Long-tail domain endpoint metadata and fixture evidence.
pub const US3_OPERATION_MANIFEST: &[OperationInventoryEntry] = &[
    OperationInventoryEntry {
        domain: "public_data",
        operation: "instruments_platform",
        transport: "REST",
        native_entrypoint: "PublicDataService",
        fixture_id: "operation_inventory::every_manifest_record_has_operation_transport_and_evidence",
    },
    OperationInventoryEntry {
        domain: "funding",
        operation: "assets_transfer_withdrawal",
        transport: "REST",
        native_entrypoint: "FundingService",
        fixture_id: "operation_inventory::every_manifest_record_has_operation_transport_and_evidence",
    },
    OperationInventoryEntry {
        domain: "sub_account",
        operation: "balances_transfers_permissions",
        transport: "REST",
        native_entrypoint: "SubAccountService",
        fixture_id: "operation_inventory::every_manifest_record_has_operation_transport_and_evidence",
    },
    OperationInventoryEntry {
        domain: "finance_strategy",
        operation: "staking_grid_copy",
        transport: "REST",
        native_entrypoint: "FinanceService/StrategyService",
        fixture_id: "operation_inventory::every_manifest_record_has_operation_transport_and_evidence",
    },
    OperationInventoryEntry {
        domain: "professional",
        operation: "rfq_spread_convert_status",
        transport: "REST",
        native_entrypoint: "ProfessionalService",
        fixture_id: "operation_inventory::every_manifest_record_has_operation_transport_and_evidence",
    },
];
