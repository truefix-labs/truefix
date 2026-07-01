//! Example initiator client: logs on, sends a NewOrderSingle, prints any ExecutionReport, logs
//! out. Run an acceptor (e.g. the `executor` example) first, then
//! `cargo run -p truefix --example banzai`.

use std::sync::Arc;
use std::time::Duration;

use truefix_core::{BusinessReject, Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::connect_initiator;

struct Client;

#[async_trait::async_trait]
impl Application for Client {
    async fn on_logon(&self, id: &SessionId) {
        println!("banzai: logon {id}");
    }
    async fn from_app(&self, message: &Message, _id: &SessionId) -> Result<(), BusinessReject> {
        if message.msg_type() == Some("8") {
            println!("banzai: received ExecutionReport");
        }
        Ok(())
    }
}

fn new_order_single() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, "ORDER-1")); // ClOrdID
    m.body.set(Field::string(21, "1")); // HandlInst
    m.body.set(Field::string(55, "AAPL")); // Symbol
    m.body.set(Field::string(54, "1")); // Side = Buy
    m.body.set(Field::string(60, "20240101-00:00:00")); // TransactTime
    m.body.set(Field::int(38, 100)); // OrderQty
    m.body.set(Field::string(40, "1")); // OrdType = Market
    m
}

#[tokio::main]
async fn main() {
    let mut config = SessionConfig::new("FIX.4.4", "CLIENT", "EXEC", Role::Initiator);
    config.check_latency = false;

    let handle = connect_initiator(
        "127.0.0.1:9881".parse().expect("addr"),
        config,
        Arc::new(Client),
    )
    .await
    .expect("connect");

    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.send(new_order_single()).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.logout().await;
    handle.join().await;
}
