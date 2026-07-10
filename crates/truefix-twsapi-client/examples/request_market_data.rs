use std::time::Duration;

use truefix_twsapi_client::client::{ClientConfig, TwsApiClient};
use truefix_twsapi_client::events::Event;
use truefix_twsapi_client::requests::MarketDataRequest;
use truefix_twsapi_client::types::Contract;

#[tokio::main]
async fn main() -> truefix_twsapi_client::error::TwsApiResult<()> {
    let host = std::env::var("TWS_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = std::env::var("TWS_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(7497);
    let client_id = std::env::var("TWS_CLIENT_ID")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1002);
    let symbol = std::env::var("TWS_SYMBOL").unwrap_or_else(|_| "AAPL".to_owned());
    let exchange = std::env::var("TWS_EXCHANGE").unwrap_or_else(|_| "SMART".to_owned());
    let currency = std::env::var("TWS_CURRENCY").unwrap_or_else(|_| "USD".to_owned());
    let market_data_type = std::env::var("TWS_MARKET_DATA_TYPE")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(1);

    let req_id = 9001;
    let mut client = TwsApiClient::connect(ClientConfig::new(host, port, client_id)).await?;
    wait_until_api_ready(&mut client).await?;
    client.req_market_data_type(market_data_type).await?;
    client
        .req_mkt_data(MarketDataRequest {
            req_id,
            contract: Contract {
                symbol,
                sec_type: "STK".to_owned(),
                exchange,
                currency,
                ..Contract::default()
            },
            generic_tick_list: String::new(),
            snapshot: false,
            regulatory_snapshot: false,
            market_data_options: Vec::new(),
        })
        .await?;

    let result = tokio::time::timeout(Duration::from_secs(30), async {
        let should_cancel = loop {
            match client.read_event().await? {
                Event::TickPrice {
                    req_id: event_req_id,
                    tick_type,
                    price,
                    attrib,
                } if event_req_id == req_id => {
                    println!("price tick_type={tick_type} price={price} attrib={attrib}");
                    break true;
                }
                Event::TickSize {
                    req_id: event_req_id,
                    tick_type,
                    size,
                } if event_req_id == req_id => {
                    println!("size tick_type={tick_type} size={size}");
                }
                Event::Error {
                    req_id: event_req_id,
                    code,
                    message,
                    ..
                } if event_req_id < 0 && is_market_data_status_code(code) => {
                    eprintln!("TWS status {code}: {message}");
                }
                Event::Error {
                    req_id: event_req_id,
                    code,
                    message,
                    ..
                } if event_req_id == req_id && is_delayed_market_data_notice(code) => {
                    eprintln!("TWS notice {code}: {message}");
                }
                Event::Error {
                    req_id: event_req_id,
                    code,
                    message,
                    ..
                } if event_req_id == req_id => {
                    eprintln!("TWS error {code}: {message}");
                    break false;
                }
                Event::Error { code, message, .. } => {
                    eprintln!("TWS notice {code}: {message}");
                }
                _ => {}
            }
        };
        truefix_twsapi_client::error::TwsApiResult::Ok(should_cancel)
    })
    .await;

    let should_cancel = match &result {
        Ok(Ok(should_cancel)) => *should_cancel,
        Ok(Err(_)) | Err(_) => true,
    };
    if should_cancel && let Err(error) = client.cancel_mkt_data(req_id).await {
        eprintln!("cancelMktData failed: {error}");
    }
    if should_cancel {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    match result {
        Ok(result) => result.map(|_| ()),
        Err(_) => {
            eprintln!("timed out waiting for market data");
            Ok(())
        }
    }
}

fn is_market_data_status_code(code: i32) -> bool {
    matches!(code, 2103 | 2104 | 2106 | 2107 | 2108 | 2158)
}

fn is_delayed_market_data_notice(code: i32) -> bool {
    code == 10167
}

async fn wait_until_api_ready(
    client: &mut TwsApiClient,
) -> truefix_twsapi_client::error::TwsApiResult<()> {
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        while !client.api_ready() {
            match client.read_event().await? {
                Event::Error { code, message, .. } if is_market_data_status_code(code) => {
                    eprintln!("TWS status {code}: {message}");
                }
                Event::Error { code, message, .. } => {
                    eprintln!("TWS notice {code}: {message}");
                }
                _ => {}
            }
        }
        truefix_twsapi_client::error::TwsApiResult::Ok(())
    })
    .await;

    match result {
        Ok(result) => result,
        Err(_) => {
            eprintln!("timed out waiting for initial API callbacks");
            Ok(())
        }
    }
}
