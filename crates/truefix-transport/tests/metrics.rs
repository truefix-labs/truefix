//! T061 (US9) — exported metrics reflect real session state over a logon->traffic->reconnect
//! cycle (FR-023; SC-010; Constitution Principle I observability).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use metrics_util::CompositeKey;

use truefix_core::{Field, Message};
use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_transport::{connect_initiator_reconnecting, AcceptorBuilder, Monitor, Services};

struct FlagApp {
    logged_on: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl Application for FlagApp {
    async fn on_logon(&self, _s: &SessionId) {
        self.logged_on.store(true, Ordering::SeqCst);
    }
}

async fn wait_until<F: Fn() -> bool>(cond: F, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    cond()
}

fn gauge_value(
    snapshot: &std::collections::HashMap<
        CompositeKey,
        (
            Option<metrics::Unit>,
            Option<metrics::SharedString>,
            DebugValue,
        ),
    >,
    name: &str,
) -> Option<f64> {
    snapshot.iter().find_map(|(k, (_, _, v))| {
        if k.key().name() == name {
            match v {
                DebugValue::Gauge(g) => Some(f64::from(*g)),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn counter_sum(
    snapshot: &std::collections::HashMap<
        CompositeKey,
        (
            Option<metrics::Unit>,
            Option<metrics::SharedString>,
            DebugValue,
        ),
    >,
    name: &str,
) -> u64 {
    snapshot
        .iter()
        .filter_map(|(k, (_, _, v))| {
            if k.key().name() == name {
                match v {
                    DebugValue::Counter(c) => Some(*c),
                    _ => None,
                }
            } else {
                None
            }
        })
        .sum()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_reflect_logon_traffic_and_reconnect() {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    // Installing a global recorder can only happen once per process; ignore if another test in
    // the same binary already installed one (metrics still flow to whichever was installed first
    // — acceptable for this visibility-only assertion).
    let _ = recorder.install();

    let monitor = Monitor::new();
    let logged_on_acc = Arc::new(AtomicBool::new(false));
    let logged_on_init = Arc::new(AtomicBool::new(false));

    let mut acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    acc_cfg.heartbeat_interval = 30;
    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.heartbeat_interval = 30;
    init_cfg.reconnect_interval = 1;

    let acc_services = Services {
        monitor: Some(monitor.clone()),
        ..Services::default()
    };

    let acceptor = AcceptorBuilder::bind(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(FlagApp {
            logged_on: logged_on_acc.clone(),
        }),
    )
    .await
    .unwrap()
    .with_session(acc_cfg)
    .with_services(acc_services);
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let init_handle = connect_initiator_reconnecting(
        addr,
        init_cfg,
        Arc::new(FlagApp {
            logged_on: logged_on_init.clone(),
        }),
        Services::default(),
    );

    assert!(
        wait_until(
            || logged_on_acc.load(Ordering::SeqCst) && logged_on_init.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "both sides should log on"
    );

    // Send an application message from the acceptor to generate sent/received traffic.
    let acc_id = SessionId::new("FIX.4.4", "SERVER", "CLIENT");
    let mut order = Message::new();
    order.header.set(Field::string(35, "D"));
    order.body.set(Field::string(11, "ORDER-1"));
    monitor.send_app(&acc_id, order).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Force the acceptor to drop the initiator's session; the reconnecting initiator should
    // notice and reconnect (reconnect_interval=1s).
    logged_on_init.store(false, Ordering::SeqCst);
    logged_on_acc.store(false, Ordering::SeqCst);
    monitor.force_logout(&acc_id).await;
    assert!(
        wait_until(
            || logged_on_acc.load(Ordering::SeqCst) && logged_on_init.load(Ordering::SeqCst),
            Duration::from_secs(5),
        )
        .await,
        "the initiator should reconnect after being logged out"
    );

    let snapshot = snapshotter.snapshot().into_hashmap();

    let state = gauge_value(&snapshot, "truefix_session_state");
    assert_eq!(
        state,
        Some(2.0),
        "session-state gauge should reflect LoggedOn (2)"
    );

    let sender_seq = gauge_value(&snapshot, "truefix_next_sender_seqnum");
    assert!(
        sender_seq.unwrap_or(0.0) > 1.0,
        "sender seqnum should have advanced"
    );

    let sent = counter_sum(&snapshot, "truefix_messages_sent_total");
    let received = counter_sum(&snapshot, "truefix_messages_received_total");
    assert!(
        sent >= 1,
        "messages_sent_total should have incremented (got {sent})"
    );
    assert!(
        received >= 1,
        "messages_received_total should have incremented (got {received})"
    );

    let reconnects = counter_sum(&snapshot, "truefix_reconnects_total");
    assert!(
        reconnects >= 1,
        "reconnects_total should have incremented (got {reconnects})"
    );

    // `stop()` only prevents the *next* reconnect attempt; the current connection is healthy and
    // logged on, so it must be explicitly torn down (via a final force_logout) or `join()` would
    // wait forever for a disconnect that never happens.
    init_handle.stop();
    monitor.force_logout(&acc_id).await;
    tokio::time::timeout(Duration::from_secs(5), init_handle.join())
        .await
        .expect("reconnect loop should stop after the final logout");
}
