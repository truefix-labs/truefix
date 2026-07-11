//! Demo-only lifecycle. Credentials are loaded from process environment, never source control.
//!
//! Set `OKX_DEMO_API_KEY`, `OKX_DEMO_SECRET`, `OKX_DEMO_PASSPHRASE`, and explicitly set
//! `OKX_DEMO_PLACE_ORDER=1` before the write path is enabled.

use truefix_okx_client::{
    ClientConfig, Credentials, OkxClient,
    types::{common::DecimalValue, order::PlaceOrder},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = match std::env::var("OKX_DEMO_API_KEY") {
        Ok(value) => value,
        Err(_) => {
            println!("Demo credentials are not configured; no request was sent.");
            return Ok(());
        }
    };
    let credentials = Credentials::new(
        key,
        std::env::var("OKX_DEMO_SECRET")?,
        std::env::var("OKX_DEMO_PASSPHRASE")?,
    )?;
    let client = OkxClient::new(ClientConfig::demo(Some(credentials)))?;
    println!(
        "Demo balances: {} records",
        client.account().balances().await?.len()
    );
    if std::env::var("OKX_DEMO_PLACE_ORDER").ok().as_deref() != Some("1") {
        println!("Order placement is disabled; set OKX_DEMO_PLACE_ORDER=1 to continue.");
        return Ok(());
    }
    let order = PlaceOrder::new(
        "BTC-USDT",
        "cash",
        "buy",
        "market",
        "0.00001".parse::<DecimalValue>()?,
    );
    println!(
        "Demo order acknowledgement: {:?}",
        client.trade().place_order(&order).await?
    );
    Ok(())
}
