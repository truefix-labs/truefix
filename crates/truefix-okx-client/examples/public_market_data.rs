//! Credential-free public ticker lookup: `cargo run -p truefix-okx-client --example public_market_data`.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = truefix_okx_client::OkxClient::new(Default::default())?;
    for ticker in client.market().ticker("BTC-USDT").await? {
        println!("{} last={}", ticker.instrument_id, ticker.last);
    }
    Ok(())
}
