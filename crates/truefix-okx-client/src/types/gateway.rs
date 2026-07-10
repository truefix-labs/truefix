//! Narrow, dependency-free common-data projections for gateway adapters.
//!
//! These types deliberately describe only the data that is meaningful across venues.  They do
//! not attempt to turn an OKX product into a different product: callers that need exchange
//! semantics can always inspect the source record's `native_fields`.

use std::collections::BTreeMap;

use crate::types::{common::DecimalValue, order::ClientOrderId};

/// Exchange-specific fields which are not part of a common projection.
pub type NativeFields = BTreeMap<String, serde_json::Value>;

/// Converts an OKX value into the small common representation useful to a future gateway.
///
/// This crate owns no gateway dependency.  An adapter can consume these values while retaining
/// the originating OKX model (and its [`NativeFields`]) alongside the projection.
pub trait GatewayProject {
    /// The common, venue-neutral representation of this value.
    type Projection;

    /// Produces a loss-aware common projection.
    fn project_gateway(&self) -> Self::Projection;
}

/// Common order state with the OKX identity retained for reconciliation.
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayOrder {
    /// OKX exchange order identifier.
    pub venue_order_id: String,
    /// Optional caller supplied order identifier.
    pub client_order_id: Option<ClientOrderId>,
    /// Venue instrument identifier.
    pub instrument_id: String,
    /// Venue order state; gateway adapters may map it to their own state vocabulary.
    pub state: String,
    /// Cumulative filled quantity, when supplied by OKX.
    pub filled_size: Option<DecimalValue>,
    /// Fields outside this projection's common vocabulary.
    pub native_fields: NativeFields,
}

/// Common execution/fill representation.
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayExecution {
    /// OKX fill identifier.
    pub venue_execution_id: String,
    /// OKX order identifier.
    pub venue_order_id: String,
    /// Executed price.
    pub price: DecimalValue,
    /// Executed size.
    pub size: DecimalValue,
    /// Fields outside this projection's common vocabulary.
    pub native_fields: NativeFields,
}

/// Common cash-balance representation.
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayBalance {
    /// Currency code.
    pub currency: String,
    /// Spendable balance when supplied by OKX.
    pub available: Option<DecimalValue>,
    /// Account equity when supplied by OKX.
    pub equity: Option<DecimalValue>,
    /// Fields outside this projection's common vocabulary.
    pub native_fields: NativeFields,
}

/// Common open-position representation.
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayPosition {
    /// Venue instrument identifier.
    pub instrument_id: String,
    /// Signed position quantity.
    pub quantity: DecimalValue,
    /// Average entry price when supplied by OKX.
    pub average_price: Option<DecimalValue>,
    /// Unrealized profit/loss when supplied by OKX.
    pub unrealized_pnl: Option<DecimalValue>,
    /// Fields outside this projection's common vocabulary.
    pub native_fields: NativeFields,
}

/// Common last-price market-data representation.
#[derive(Debug, Clone, PartialEq)]
pub struct GatewayTicker {
    /// Venue instrument identifier.
    pub instrument_id: String,
    /// Last traded price.
    pub last: DecimalValue,
    /// Rolling 24-hour volume when supplied by OKX.
    pub volume_24h: Option<DecimalValue>,
    /// Fields outside this projection's common vocabulary.
    pub native_fields: NativeFields,
}
