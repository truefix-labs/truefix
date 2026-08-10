use std::time::Duration;

use futures_util::StreamExt;
use truefix_ig_client::{ClientConfig, Credentials, IgClient};

fn credentials_from_environment() -> Credentials {
    let username = std::env::var("IG_USERNAME").expect("IG_USERNAME is required");
    let password = std::env::var("IG_PASSWORD").expect("IG_PASSWORD is required");
    let api_key = std::env::var("IG_API_KEY").expect("IG_API_KEY is required");
    Credentials::new(username, password, api_key).expect("valid credentials")
}

#[tokio::test]
#[ignore = "requires IG demo credentials and network access"]
async fn demo_v3_rest_working_orders() {
    let account_id = std::env::var("IG_ACCOUNT_ID").expect("IG_ACCOUNT_ID is required");
    let config = ClientConfig::demo(Some(credentials_from_environment()))
        .with_v3_authentication(account_id)
        .expect("valid v3 config");
    let client = IgClient::new(config).expect("valid config");

    eprintln!("stage=v3_login");
    tokio::time::timeout(Duration::from_secs(20), client.login())
        .await
        .expect("v3 login timeout")
        .expect("v3 demo login");
    eprintln!("stage=accounts");
    let accounts = tokio::time::timeout(Duration::from_secs(20), client.accounts())
        .await
        .expect("accounts timeout")
        .expect("accounts request");
    assert!(!accounts.accounts.is_empty());
    eprintln!("stage=working_orders");
    tokio::time::timeout(Duration::from_secs(20), client.working_orders())
        .await
        .expect("working-orders timeout")
        .expect("working-orders request");
    eprintln!("stage=lightstreamer");
    let streaming = tokio::time::timeout(Duration::from_secs(20), client.connect_streaming())
        .await
        .expect("Lightstreamer connection timeout")
        .expect("Lightstreamer connection");
    let mut account_updates = streaming
        .subscribe_account()
        .await
        .expect("account subscription");
    tokio::time::timeout(Duration::from_secs(15), account_updates.next())
        .await
        .expect("account streaming update timeout")
        .expect("account subscription ended");
    drop(account_updates);
    streaming.disconnect().await.expect("stream disconnect");
    eprintln!("stage=logout");
    tokio::time::timeout(Duration::from_secs(20), client.logout())
        .await
        .expect("logout timeout")
        .expect("logout");
}

#[tokio::test]
#[ignore = "requires IG demo credentials and network access"]
async fn demo_login_rest_and_lightstreamer() {
    let client = IgClient::new(ClientConfig::demo(Some(credentials_from_environment())))
        .expect("valid config");

    client.login_v2().await.expect("v2 demo login");
    let accounts = client.accounts().await.expect("accounts request");
    assert!(!accounts.accounts.is_empty());
    client
        .working_orders()
        .await
        .expect("working-orders request");

    let streaming = tokio::time::timeout(Duration::from_secs(20), client.connect_streaming())
        .await
        .expect("Lightstreamer connection timeout")
        .expect("Lightstreamer connection");
    let mut account_updates = streaming
        .subscribe_account()
        .await
        .expect("account subscription");
    tokio::time::timeout(Duration::from_secs(15), account_updates.next())
        .await
        .expect("account streaming update timeout")
        .expect("account subscription ended");

    drop(account_updates);
    streaming.disconnect().await.expect("stream disconnect");
    client.logout().await.expect("logout");
}
