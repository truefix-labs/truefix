//! Example acceptor that "executes" orders: every NewOrderSingle (35=D) it receives is filled
//! with an ExecutionReport (35=8). Run with `cargo run -p truefix --example executor`.

use std::sync::Arc;

use truefix_core::{Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, Monitor, Services};

struct Executor {
    monitor: Monitor,
}

#[async_trait::async_trait]
impl Application for Executor {
    async fn on_logon(&self, id: &SessionId) {
        println!("executor: logon {id}");
    }
    async fn from_app(&self, message: &Message, id: &SessionId) -> Result<(), String> {
        if message.msg_type() == Some("D") {
            self.monitor.send_app(id, execution_report(message)).await;
        }
        Ok(())
    }
}

fn execution_report(order: &Message) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "8")); // ExecutionReport
    m.body.set(Field::string(37, "ORDER-1")); // OrderID
    m.body.set(Field::string(17, "EXEC-1")); // ExecID
    m.body.set(Field::string(150, "0")); // ExecType = New
    m.body.set(Field::string(39, "0")); // OrdStatus = New
    for tag in [11u32, 55, 54, 38] {
        if let Some(f) = order.body.get(tag) {
            m.body.set(Field::new(tag, f.value_bytes().to_vec()));
        }
    }
    m
}

#[tokio::main]
async fn main() {
    let monitor = Monitor::new();
    let services = Services {
        monitor: Some(monitor.clone()),
        ..Services::default()
    };
    let mut template = SessionConfig::new("FIX.4.4", "EXEC", "CLIENT", Role::Acceptor);
    template.check_latency = false;

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:9881".parse().expect("addr"),
        Arc::new(Executor { monitor }),
    )
    .await
    .expect("bind")
    .with_dynamic_template(template)
    .with_services(services);

    println!(
        "executor listening on {}",
        acceptor.local_addr().expect("local_addr")
    );
    let _ = acceptor.serve().await;
}
