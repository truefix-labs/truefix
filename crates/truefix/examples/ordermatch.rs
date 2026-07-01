//! Example order-matching acceptor (simplified): maintains a tiny per-symbol book and, when a
//! crossing order arrives, fills both sides with ExecutionReports. Run with
//! `cargo run -p truefix --example ordermatch`.
//!
//! This is a teaching example of the engine's application hooks, not a production matcher.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use truefix_core::{Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{AcceptorBuilder, Monitor, Services};

#[derive(Clone)]
struct RestingOrder {
    session: SessionId,
    cl_ord_id: String,
}

/// Per-symbol resting orders: `(buys, sells)`.
type Book = HashMap<String, (Vec<RestingOrder>, Vec<RestingOrder>)>;

struct OrderMatch {
    monitor: Monitor,
    book: Mutex<Book>,
}

#[async_trait::async_trait]
impl Application for OrderMatch {
    async fn from_app(&self, message: &Message, id: &SessionId) -> Result<(), String> {
        if message.msg_type() != Some("D") {
            return Ok(());
        }
        let symbol = field(message, 55);
        let side = field(message, 54);
        let cl_ord_id = field(message, 11);

        let opposite = {
            let mut book = self.book.lock().map_err(|_| "book poisoned".to_owned())?;
            let entry = book.entry(symbol.clone()).or_default();
            let (buys, sells) = entry;
            let incoming = RestingOrder {
                session: id.clone(),
                cl_ord_id,
            };
            if side == "1" {
                // buy: match against a resting sell, else rest
                let m = sells.pop();
                if m.is_none() {
                    buys.push(incoming.clone());
                }
                m.map(|s| (incoming, s))
            } else {
                let m = buys.pop();
                if m.is_none() {
                    sells.push(incoming.clone());
                }
                m.map(|b| (incoming, b))
            }
        };

        if let Some((a, b)) = opposite {
            self.monitor
                .send_app(&a.session, fill(&a.cl_ord_id, &symbol))
                .await;
            self.monitor
                .send_app(&b.session, fill(&b.cl_ord_id, &symbol))
                .await;
        }
        Ok(())
    }
}

fn field(msg: &Message, tag: u32) -> String {
    msg.body
        .get(tag)
        .and_then(|f| f.as_str().ok())
        .unwrap_or_default()
        .to_owned()
}

fn fill(cl_ord_id: &str, symbol: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "8")); // ExecutionReport
    m.body.set(Field::string(11, cl_ord_id));
    m.body.set(Field::string(55, symbol));
    m.body.set(Field::string(150, "2")); // ExecType = Filled
    m.body.set(Field::string(39, "2")); // OrdStatus = Filled
    m
}

#[tokio::main]
async fn main() {
    let monitor = Monitor::new();
    let services = Services {
        monitor: Some(monitor.clone()),
        ..Services::default()
    };
    let mut template = SessionConfig::new("FIX.4.4", "MATCH", "CLIENT", Role::Acceptor);
    template.check_latency = false;

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:9883".parse().expect("addr"),
        Arc::new(OrderMatch {
            monitor,
            book: Mutex::new(HashMap::new()),
        }),
    )
    .await
    .expect("bind")
    .with_dynamic_template(template)
    .with_services(services);

    println!(
        "ordermatch listening on {}",
        acceptor.local_addr().expect("local_addr")
    );
    let _ = acceptor.serve().await;
}
