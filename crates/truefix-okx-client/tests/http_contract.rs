mod support;

use truefix_okx_client::types::{
    common::{DecimalValue, ExpirationTime},
    order::PlaceOrder,
};
use truefix_okx_client::{ClientConfig, Credentials, Environment, OkxClient};

struct FixedClock(time::OffsetDateTime);

impl truefix_okx_client::auth::Clock for FixedClock {
    fn now(&self) -> time::OffsetDateTime {
        self.0
    }
}

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
async fn account_operations_use_okx_canonical_risk_offset_and_simulated_margin_paths() {
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();

    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let client = OkxClient::new(custom_config(base, Some(credentials.clone()))).unwrap();
    client
        .account()
        .set_risk_offset_type(&serde_json::json!({"type": "1"}))
        .await
        .unwrap();
    assert_eq!(
        captured.await.unwrap().target,
        "/api/v5/account/set-riskOffset-type"
    );

    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    client
        .account()
        .simulated_margin(&serde_json::json!({"instType": "SWAP"}))
        .await
        .unwrap();
    assert_eq!(
        captured.await.unwrap().target,
        "/api/v5/account/simulated_margin"
    );
}

#[tokio::test]
async fn baseline_operations_preserve_pagination_and_request_metadata() {
    let (base, captured) = support::http::start_with_headers(
        r#"{"code":"0","msg":"","data":[{"instId":"BTC-USDT"}]}"#,
        &[
            ("OK-BEFORE", "older"),
            ("OK-AFTER", "newer"),
            ("x-request-id", "okx-request-42"),
        ],
    )
    .await;
    let client = OkxClient::new(custom_config(base, None)).unwrap();
    let response = client
        .execute_baseline_operation_with_metadata(
            "market_data",
            "get_ticker",
            std::collections::BTreeMap::from([("instId".to_owned(), "BTC-USDT".to_owned())]),
            None,
        )
        .await
        .unwrap();
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.metadata.page.before.as_deref(), Some("older"));
    assert_eq!(response.metadata.page.after.as_deref(), Some("newer"));
    assert_eq!(
        response.metadata.request_id.as_deref(),
        Some("okx-request-42")
    );
    assert_eq!(
        captured.await.unwrap().target,
        "/api/v5/market/ticker?instId=BTC-USDT"
    );
}

#[tokio::test]
async fn account_helpers_preserve_optional_filters_and_empty_body_semantics() {
    let (base, captured) = support::http::start(
        r#"{"code":"0","msg":"","data":[{"totalEq":"1","details":[{"ccy":"BTC","availBal":"1","eq":"1","frozenBal":"0"}]}]}"#,
    )
    .await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();

    let balances = client
        .account()
        .balances_with_currency(Some("BTC"))
        .await
        .unwrap();
    assert_eq!(balances.len(), 1);
    assert_eq!(balances[0].details.len(), 1);
    assert_eq!(balances[0].details[0].currency, "BTC");
    let request = captured.await.unwrap();
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v5/account/balance?ccy=BTC");

    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let _ = client
        .account()
        .positions_with_filters(Some("SWAP"), Some("BTC-USDT-SWAP"), Some("123"))
        .await
        .unwrap();
    let request = captured.await.unwrap();
    assert_eq!(
        request.target,
        "/api/v5/account/positions?instId=BTC-USDT-SWAP&instType=SWAP&posId=123"
    );

    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let _ = client.account().activate_option().await.unwrap();
    let request = captured.await.unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v5/account/activate-option");
    assert_eq!(request.body, b"{}");
}

#[tokio::test]
async fn parameterless_baseline_posts_send_empty_json_bodies() {
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();

    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let client = OkxClient::new(custom_config(base, Some(credentials.clone()))).unwrap();
    client
        .execute_baseline_operation(
            "account",
            "activate_option",
            std::collections::BTreeMap::new(),
            None,
        )
        .await
        .unwrap();
    let request = captured.await.unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v5/account/activate-option");
    assert_eq!(request.body, b"{}");

    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    client.professional().reset_mmp().await.unwrap();
    let request = captured.await.unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v5/rfq/mmp-reset");
    assert_eq!(request.body, b"{}");
}

#[tokio::test]
async fn parameterized_baseline_posts_still_require_a_body() {
    let client = OkxClient::new(custom_config("http://127.0.0.1:1".to_owned(), None)).unwrap();
    let error = client
        .execute_baseline_operation(
            "trade",
            "place_order",
            std::collections::BTreeMap::new(),
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        truefix_okx_client::OkxError::InvalidConfiguration(message)
            if message == "POST operations require a JSON body"
    ));
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
        margin_currency: None,
        tag: None,
        position_side: None,
        reduce_only: None,
        target_currency: None,
        self_trade_prevention_mode: None,
        attached_algo_orders: None,
        price_usd: None,
        price_volatility: None,
        ban_amend: None,
        trade_quote_currency: None,
        price_amend_type: None,
        elp_taker_access: None,
        instrument_id_code: None,
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
async fn order_item_failure_is_not_reported_as_a_successful_write() {
    let (base, captured) = support::http::start(
        r#"{"code":"0","msg":"","data":[{"ordId":"42","sCode":"0","sMsg":""},{"ordId":"","sCode":"51008","sMsg":"insufficient balance","clOrdId":"second"}]}"#,
    )
    .await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let order = PlaceOrder::new(
        "BTC-USDT",
        "cash",
        "buy",
        "market",
        "1".parse::<DecimalValue>().unwrap(),
    );

    assert!(matches!(
        client.trade().place_order(&order).await,
        Err(truefix_okx_client::OkxError::PartialFailure { acknowledgements })
            if acknowledgements.len() == 2
                && acknowledgements[0].order_id == "42"
                && acknowledgements[1].code == "51008"
                && acknowledgements[1].message == "insufficient balance"
    ));
    assert_eq!(captured.await.unwrap().target, "/api/v5/trade/order");
}

#[tokio::test]
async fn advanced_order_fields_use_the_okx_wire_names() {
    let (base, captured) = support::http::start(
        r#"{"code":"0","msg":"","data":[{"ordId":"42","sCode":"0","sMsg":""}]}"#,
    )
    .await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let mut order = PlaceOrder::new(
        "BTC-USDT",
        "cash",
        "buy",
        "market",
        "1".parse::<DecimalValue>().unwrap(),
    );
    order.elp_taker_access = Some(true);
    let _ = client.trade().place_order(&order).await.unwrap();
    let request = captured.await.unwrap();
    assert!(
        std::str::from_utf8(&request.body)
            .unwrap()
            .contains("\"isElpTakerAccess\":true")
    );
}

#[tokio::test]
async fn order_expiration_is_sent_as_the_okx_request_header() {
    let (base, captured) = support::http::start(
        r#"{"code":"0","msg":"","data":[{"ordId":"42","sCode":"0","sMsg":""}]}"#,
    )
    .await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let client = OkxClient::new(custom_config(base, Some(credentials))).unwrap();
    let order = PlaceOrder::new(
        "BTC-USDT",
        "cash",
        "buy",
        "market",
        "1".parse::<DecimalValue>().unwrap(),
    );
    client
        .trade()
        .place_order_with_expiration(&order, ExpirationTime::new(1_704_067_200_123).unwrap())
        .await
        .unwrap();
    assert_eq!(
        captured
            .await
            .unwrap()
            .headers
            .get("exptime")
            .map(String::as_str),
        Some("1704067200123")
    );
}

#[tokio::test]
async fn configured_clock_offset_is_applied_to_private_rest_signatures() {
    let (base, captured) = support::http::start(r#"{"code":"0","msg":"","data":[]}"#).await;
    let credentials = Credentials::new("key", "secret", "passphrase").unwrap();
    let config =
        custom_config(base, Some(credentials)).with_clock_offset(time::Duration::seconds(2));
    let client = OkxClient::with_clock(
        config,
        std::sync::Arc::new(FixedClock(time::macros::datetime!(2024-01-01 0:00 UTC))),
    )
    .unwrap();
    client.account().activate_option().await.unwrap();
    assert_eq!(
        captured
            .await
            .unwrap()
            .headers
            .get("ok-access-timestamp")
            .map(String::as_str),
        Some("2024-01-01T00:00:02.000Z")
    );
}

#[tokio::test]
async fn server_time_measurement_returns_an_offset_suitable_for_client_config() {
    let (base, captured) =
        support::http::start(r#"{"code":"0","msg":"","data":[{"ts":"1704067200000"}]}"#).await;
    let client = OkxClient::new(custom_config(base, None)).unwrap();

    let before = time::OffsetDateTime::now_utc();
    let offset = client.measure_server_time_offset().await.unwrap();
    let after = time::OffsetDateTime::now_utc();

    let midpoint = before + (after - before) / 2;
    let expected: time::Duration = time::macros::datetime!(2024-01-01 0:00 UTC) - midpoint;
    assert!((offset - expected).abs() < time::Duration::seconds(1));
    assert_eq!(captured.await.unwrap().target, "/api/v5/public/time");
    assert_eq!(
        ClientConfig::default()
            .with_clock_offset(offset)
            .clock_offset,
        offset
    );
}

#[test]
fn timestamp_expiry_is_reported_as_clock_skew() {
    let error = truefix_okx_client::response::decode_envelope::<serde_json::Value>(
        br#"{"code":"50102","msg":"Timestamp request expired","data":[]}"#,
    )
    .unwrap_err();
    assert!(matches!(error, truefix_okx_client::OkxError::ClockSkew));
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
async fn safe_read_retries_an_okx_50011_envelope_returned_with_http_200() {
    let (base, captured) = support::http::start_sequence(vec![
        (
            200,
            r#"{"code":"50011","msg":"rate limited","data":[]}"#,
            None,
        ),
        (
            200,
            r#"{"code":"0","msg":"","data":[{"instId":"BTC-USDT","last":"1"}]}"#,
            None,
        ),
    ])
    .await;
    let client = OkxClient::new(custom_config(base, None)).unwrap();

    assert_eq!(client.market().ticker("BTC-USDT").await.unwrap().len(), 1);
    assert_eq!(captured.await.unwrap().len(), 2);
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
        margin_currency: None,
        tag: None,
        position_side: None,
        reduce_only: None,
        target_currency: None,
        self_trade_prevention_mode: None,
        attached_algo_orders: None,
        price_usd: None,
        price_volatility: None,
        ban_amend: None,
        trade_quote_currency: None,
        price_amend_type: None,
        elp_taker_access: None,
        instrument_id_code: None,
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
    for public_spread_method in [
        "pub async fn spreads",
        "pub async fn spread_books",
        "pub async fn spread_ticker",
        "pub async fn spread_public_trades",
        "pub async fn public_rfq_trades",
    ] {
        assert!(
            sources.contains(public_spread_method),
            "missing expected public method {public_spread_method}"
        );
    }
    for public_spread_get in [
        "spreads(&self, q: BTreeMap<String, String>) -> OkxResult<Vec<serde_json::Value>> {",
        "self.public_get(\"/api/v5/sprd/spreads\", q).await",
        "self.public_get(\"/api/v5/sprd/books\", q).await",
        "self.public_get(\"/api/v5/sprd/ticker\", q).await",
        "self.public_get(\"/api/v5/sprd/public-trades\", q).await",
        "self.public_get(\"/api/v5/rfq/public-trades\", q).await",
    ] {
        assert!(
            sources.contains(public_spread_get),
            "missing expected public endpoint usage {public_spread_get}"
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
