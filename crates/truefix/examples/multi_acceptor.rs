//! Example multi-session acceptor: serves two static sessions plus a dynamic-session template.
//! Run with `cargo run -p truefix --example multi_acceptor`.

use std::sync::Arc;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::AcceptorBuilder;

struct Server;

#[async_trait::async_trait]
impl Application for Server {
    async fn on_logon(&self, id: &SessionId) {
        println!("multi_acceptor: logon {id}");
    }
    async fn on_logout(&self, id: &SessionId) {
        println!("multi_acceptor: logout {id}");
    }
}

fn acceptor_session(sender: &str, target: &str) -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", sender, target, Role::Acceptor);
    c.check_latency = false;
    c
}

#[tokio::main]
async fn main() {
    let acceptor = AcceptorBuilder::bind("127.0.0.1:9882".parse().expect("addr"), Arc::new(Server))
        .await
        .expect("bind")
        .with_session(acceptor_session("SERVER1", "CLIENT1"))
        .with_session(acceptor_session("SERVER2", "CLIENT2"))
        .with_dynamic_template(acceptor_session("SERVER", "CLIENT"));

    println!(
        "multi_acceptor listening on {} (SERVER1/SERVER2 + dynamic)",
        acceptor.local_addr().expect("local_addr")
    );
    let _ = acceptor.serve().await;
}
