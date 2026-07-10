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
