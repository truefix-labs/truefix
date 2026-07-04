//! Session round-trip latency benchmark (custom harness, mirroring `truefix-core`'s `codec`
//! benchmark; observation/regression tool only — no numeric gate is enforced, per Clarifications,
//! US11/FR-014/SC-010).
//!
//! Measures `Session::handle`/`Session::send_app` latency for a representative message-in ->
//! processed -> response-out mix (an inbound Heartbeat consumed silently, an inbound TestRequest
//! drawing a Heartbeat reply, and an outbound application message) against an already-logged-on
//! session. `Session` is sans-IO (no network, no disk), so this measures engine processing
//! latency, not wire/network round-trip time.
//!
//! Run with `cargo bench -p truefix-session`.

use std::time::Instant;

use truefix_core::{Field, Message};
use truefix_session::{Event, Role, Session, SessionConfig};

fn acc_cfg() -> SessionConfig {
    let mut c = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    c.heartbeat_interval = 30;
    c.check_latency = false;
    c
}

fn header(msg_type: &str, seq: i64, m: &mut Message) {
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::int(34, seq));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
}

fn logon(seq: i64) -> Message {
    let mut m = Message::new();
    header("A", seq, &mut m);
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m
}

fn heartbeat(seq: i64) -> Message {
    let mut m = Message::new();
    header("0", seq, &mut m);
    m
}

fn test_request(seq: i64) -> Message {
    let mut m = Message::new();
    header("1", seq, &mut m);
    m.body.set(Field::string(112, "BENCH"));
    m
}

fn order() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, "ORDER-1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::int(54, 1));
    m.body.set(Field::int(38, 100));
    m.body.set(Field::string(40, "2"));
    m.body.set(Field::string(44, "150.25"));
    m
}

fn main() {
    let n: u32 = 200_000;

    let mut s = Session::new(acc_cfg());
    s.handle(Event::Connected);
    s.handle(Event::Received(logon(1))); // consumes inbound seq 1

    let start = Instant::now();
    for i in 0..n {
        let hb_seq = i64::from(i) * 2 + 2;
        let tr_seq = hb_seq + 1;
        let _ = s.handle(Event::Received(heartbeat(hb_seq)));
        let _ = s.handle(Event::Received(test_request(tr_seq)));
        let _ = s.send_app(order());
    }
    let elapsed = start.elapsed();

    let per_iter = elapsed / n;
    let iters_per_sec = f64::from(n) / elapsed.as_secs_f64();
    println!(
        "session round-trip mix: Heartbeat(in) + TestRequest(in)->Heartbeat(out) + NewOrderSingle(out)"
    );
    println!("  {n} iterations in {elapsed:?}");
    println!("  {iters_per_sec:>12.0} iterations/s   ({per_iter:?}/iteration)");
}
