use truefix_twsapi_client::client::{ClientConfig, TwsApiClient};
use truefix_twsapi_client::events::Event;

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
        .unwrap_or(1001);

    let mut client = TwsApiClient::connect(ClientConfig::new(host, port, client_id)).await?;
    client.req_current_time().await?;

    loop {
        match client.read_event().await? {
            Event::CurrentTime { time } => {
                println!("{time}");
                break;
            }
            Event::Error { code, message, .. } => {
                eprintln!("TWS error {code}: {message}");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
