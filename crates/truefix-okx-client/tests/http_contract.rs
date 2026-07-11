mod support;

use truefix_okx_client::types::{common::DecimalValue, order::PlaceOrder};
use truefix_okx_client::{ClientConfig, Credentials, Environment, OkxClient};

fn custom_config(base: String, credentials: Option<Credentials>) -> ClientConfig {
    ClientConfig {
        environment: Environment::Custom {
            rest_base: base.clone(),
            public_ws: format!("ws://{base}"),
            private_ws: format!("ws://{base}"),
            business_ws: format!("ws://{base}"),
            simulated: true,
        },
        credentials,
        ..ClientConfig::default()
    }
}

#[test]
fn exchange_rejection_and_pagination_metadata_are_preserved() {
    let rejected: Result<Vec<serde_json::Value>, _> = truefix_okx_client::response::decode_envelope(
        br#"{"code":"51008","msg":"insufficient balance","data":[]}"#,
    );
    assert!(
        matches!(rejected, Err(truefix_okx_client::OkxError::Exchange { code, .. }) if code == "51008")
    );
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("OK-BEFORE", "old".parse().unwrap());
    headers.insert("OK-AFTER", "new".parse().unwrap());
    let page = truefix_okx_client::response::page_metadata(&headers);
    assert_eq!(page.before.as_deref(), Some("old"));
    assert_eq!(page.after.as_deref(), Some("new"));
}

#[tokio::test]
async fn public_requests_are_unsigned_and_preserve_encoded_query() {
    let (base, captured) =
        support::http::start(r#"{"code":"0","msg":"","data":[{"instId":"BTC-USDT","last":"1"}]}"#)
            .await;
    let client = OkxClient::new(custom_config(base, None)).unwrap();
    let tickers = client.market().ticker("BTC-USDT").await.unwrap();
    assert_eq!(tickers.len(), 1);
    let request = captured.await.unwrap();
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v5/market/ticker?instId=BTC-USDT");
    assert_eq!(
        request
            .headers
            .get("x-simulated-trading")
            .map(String::as_str),
        Some("1")
    );
    assert!(!request.headers.contains_key("ok-access-key"));
}

#[tokio::test]
async fn demo_writes_are_signed_once_and_send_exact_body() {
    let (base, captured) = support::http::start(
        r#"{"code":"0","msg":"","data":[{"ordId":"42","sCode":"0","sMsg":""}]}"#,
    )
    .await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let order = PlaceOrder {
        instrument_id: "BTC-USDT".to_owned(),
        trade_mode: "cash".to_owned(),
        side: "buy".to_owned(),
        order_type: "market".to_owned(),
        size: "1".parse::<DecimalValue>().unwrap(),
        price: None,
        client_order_id: None,
    };
    assert_eq!(
        client.trade().place_order(&order).await.unwrap()[0].order_id,
        "42"
    );
    let request = captured.await.unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v5/trade/order");
    assert!(request.headers.contains_key("ok-access-key"));
    assert!(request.headers.contains_key("ok-access-sign"));
    assert_eq!(
        request.headers.get("content-type").map(String::as_str),
        Some("application/json")
    );
    let timestamp = request.headers.get("ok-access-timestamp").unwrap();
    assert!(timestamp.ends_with('Z'));
    assert_eq!(timestamp.split_once('.').unwrap().1.len(), 4);
    assert_eq!(
        request
            .headers
            .get("x-simulated-trading")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        request.body,
        br#"{"instId":"BTC-USDT","tdMode":"cash","side":"buy","ordType":"market","sz":"1"}"#
    );
}

#[tokio::test]
async fn safe_read_retries_one_429_after_the_server_throttle() {
    let (base, captured) = support::http::start_sequence(vec![
        (
            429,
            r#"{"code":"50011","msg":"rate limited","data":[]}"#,
            Some("1"),
        ),
        (
            200,
            r#"{"code":"0","msg":"","data":[{"instId":"BTC-USDT","last":"1"}]}"#,
            None,
        ),
    ])
    .await;
    let client = OkxClient::new(custom_config(base, None)).unwrap();
    let started = std::time::Instant::now();
    assert_eq!(client.market().ticker("BTC-USDT").await.unwrap().len(), 1);
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(900),
        "the Retry-After throttle was not observed"
    );
    let requests = captured.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.method == "GET"));
    assert_eq!(requests[0].target, requests[1].target);
}

#[tokio::test]
async fn state_changing_write_is_not_replayed_after_429() {
    let (base, captured) = support::http::start_sequence(vec![(
        429,
        r#"{"code":"50011","msg":"rate limited","data":[]}"#,
        Some("0"),
    )])
    .await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let order = PlaceOrder {
        instrument_id: "BTC-USDT".to_owned(),
        trade_mode: "cash".to_owned(),
        side: "buy".to_owned(),
        order_type: "market".to_owned(),
        size: "1".parse::<DecimalValue>().unwrap(),
        price: None,
        client_order_id: None,
    };
    assert!(matches!(
        client.trade().place_order(&order).await,
        Err(truefix_okx_client::OkxError::RateLimited { .. })
    ));
    let requests = captured.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
}

#[test]
fn corrected_domain_paths_match_the_baseline_and_reject_known_invalid_paths() {
    let sources = [
        include_str!("../src/services/account.rs"),
        include_str!("../src/services/funding.rs"),
        include_str!("../src/services/finance.rs"),
        include_str!("../src/services/professional.rs"),
        include_str!("../src/services/strategy.rs"),
        include_str!("../src/services/trade.rs"),
    ]
    .join("\n");
    for required in [
        "/api/v5/account/vip-loan-order-list",
        "/api/v5/account/fixed-loan/borrowing-orders-list",
        "/api/v5/account/spot-manual-borrow-repay",
        "/api/v5/asset/currencies",
        "/api/v5/asset/deposit-lightning",
        "/api/v5/finance/flexible-loan/borrow-currencies",
        "/api/v5/finance/staking-defi/offers",
        "/api/v5/finance/sfp/dcd/products",
        "/api/v5/broker/fd/rebate-per-orders",
    ] {
        assert!(
            sources.contains(required),
            "missing corrected path {required}"
        );
    }
    for invalid in [
        "/api/v5/asset/lightning",
        "/api/v5/asset/deposit/currencies",
        "/api/v5/account/vip-loan/loan-order-list",
        "/api/v5/account/fixed-loan/borrowing-order-list",
        "/api/v5/finance/staking-defi/defi/offer-list",
        "/api/v5/finance/staking-defi/dual-investment/products",
        "/api/v5/broker/nd/rebate-per-orders",
        "/api/v5/finance/savings/purchase-redempt-history",
        "/api/v5/finance/savings/interest-accrued",
    ] {
        assert!(
            !sources.contains(invalid),
            "invalid path remains: {invalid}"
        );
    }
    for duplicate_or_misleading_method in [
        "deposit_currencies",
        "fixed_loan_repayments",
        "algo_advance_orders_pending",
        "algo_advance_orders_history",
        "copy_lead_positions",
        "dust_assets",
        "savings_products",
    ] {
        assert!(
            !sources.contains(duplicate_or_misleading_method),
            "duplicate, unsupported, or misleading method remains: {duplicate_or_misleading_method}"
        );
    }
}
