use truefix_okx_client::types::{
    account::{Balance, Position},
    gateway::GatewayProject,
    market::Ticker,
    order::{Fill, Order},
};

#[test]
fn common_projections_preserve_order_and_execution_extension_fields() {
    let order: Order = serde_json::from_value(serde_json::json!({
        "ordId": "100", "clOrdId": "client-100", "instId": "BTC-USDT-SWAP",
        "state": "live", "accFillSz": "0.10", "px": "65000.25", "reduceOnly": "true"
    }))
    .expect("test fixture is a valid OKX order");
    let fill: Fill = serde_json::from_value(serde_json::json!({
        "fillId": "200", "ordId": "100", "fillPx": "65000.25", "fillSz": "0.10",
        "fee": "-0.02", "liquidity": "M"
    }))
    .expect("test fixture is a valid OKX fill");

    let projected_order = order.project_gateway();
    assert_eq!(projected_order.venue_order_id, "100");
    assert_eq!(projected_order.instrument_id, "BTC-USDT-SWAP");
    assert_eq!(projected_order.native_fields["reduceOnly"], "true");
    assert_eq!(projected_order.native_fields["px"], "65000.25");
    assert_eq!(order.native_fields, projected_order.native_fields);

    let projected_fill = fill.project_gateway();
    assert_eq!(projected_fill.venue_execution_id, "200");
    assert_eq!(projected_fill.venue_order_id, "100");
    assert_eq!(projected_fill.native_fields["fee"], "-0.02");
    assert_eq!(fill.native_fields, projected_fill.native_fields);
}

#[test]
fn account_position_and_market_projections_preserve_native_data() {
    let balance: Balance = serde_json::from_value(serde_json::json!({
        "ccy": "USDT", "availBal": "12.34", "eq": "15.00", "frozenBal": "2.66",
        "crossLiab": "1.20"
    }))
    .expect("test fixture is a valid OKX balance");
    let position: Position = serde_json::from_value(serde_json::json!({
        "instId": "BTC-USDT-SWAP", "pos": "0.10", "avgPx": "64000", "upl": "100",
        "liqPx": "50000", "mgnRatio": "0.23"
    }))
    .expect("test fixture is a valid OKX position");
    let ticker: Ticker = serde_json::from_value(serde_json::json!({
        "instId": "BTC-USDT", "last": "65000.25", "vol24h": "123.4",
        "sodUtc0": "64000", "bidPx": "65000.20"
    }))
    .expect("test fixture is a valid OKX ticker");

    let projected_balance = balance.project_gateway();
    assert_eq!(projected_balance.currency, "USDT");
    // `frozenBal` is intentionally outside the small common balance shape; callers retain the
    // source native model alongside the projection to access it.
    assert_eq!(
        balance.frozen.expect("fixture supplies frozen balance"),
        "2.66".parse().expect("valid decimal")
    );
    assert_eq!(projected_balance.native_fields["crossLiab"], "1.20");

    let projected_position = position.project_gateway();
    assert_eq!(projected_position.instrument_id, "BTC-USDT-SWAP");
    assert_eq!(projected_position.native_fields["liqPx"], "50000");
    assert_eq!(projected_position.native_fields["mgnRatio"], "0.23");

    let projected_ticker = ticker.project_gateway();
    assert_eq!(projected_ticker.instrument_id, "BTC-USDT");
    assert_eq!(projected_ticker.native_fields["sodUtc0"], "64000");
    assert_eq!(projected_ticker.native_fields["bidPx"], "65000.20");
}

#[test]
fn client_crate_has_no_gateway_dependency() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        !manifest.contains("truefix-gateway"),
        "the native OKX client must remain independently usable"
    );
}
