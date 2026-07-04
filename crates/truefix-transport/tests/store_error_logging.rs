//! T029 (US3, feature 006): store-operation failures are routed through the log as
//! operator-visible events instead of being silently swallowed (BUG-08/FR-013).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use truefix_session::{Application, Role, SessionConfig, SessionId};
use truefix_store::{MessageStore, StoreError};
use truefix_transport::{Acceptor, Services, connect_initiator_with};

/// A `MessageStore` that succeeds on reads but fails on every write, so every currently-swallowed
/// call site (`set_next_sender_seq`/`set_next_target_seq`/`reset`/`save_and_advance_sender`)
/// actually gets exercised by a normal connect+logon cycle.
struct FailingStore;

#[async_trait::async_trait]
impl MessageStore for FailingStore {
    async fn next_sender_seq(&self) -> Result<u64, StoreError> {
        Ok(1)
    }
    async fn next_target_seq(&self) -> Result<u64, StoreError> {
        Ok(1)
    }
    async fn set_next_sender_seq(&self, _seq: u64) -> Result<(), StoreError> {
        Err(StoreError::Backend("simulated failure".into()))
    }
    async fn set_next_target_seq(&self, _seq: u64) -> Result<(), StoreError> {
        Err(StoreError::Backend("simulated failure".into()))
    }
    async fn save(&self, _seq: u64, _message: &[u8]) -> Result<(), StoreError> {
        Err(StoreError::Backend("simulated failure".into()))
    }
    async fn get(&self, _begin: u64, _end: u64) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        Err(StoreError::Backend("simulated failure".into()))
    }
    async fn reset(&self) -> Result<(), StoreError> {
        Err(StoreError::Backend("simulated failure".into()))
    }
}

#[derive(Default)]
struct RecordingLog {
    events: Mutex<Vec<String>>,
}

impl truefix_log::Log for RecordingLog {
    fn on_incoming(&self, _message: &str) {}
    fn on_outgoing(&self, _message: &str) {}
    fn on_event(&self, text: &str) {
        self.events.lock().unwrap().push(text.to_owned());
    }
}

struct NoopApp;

#[async_trait::async_trait]
impl Application for NoopApp {
    async fn on_logon(&self, _s: &SessionId) {}
}

#[tokio::test]
async fn a_store_operation_failure_produces_an_operator_visible_log_event() {
    let acc_cfg = SessionConfig::new("FIX.4.4", "SERVER", "CLIENT", Role::Acceptor);
    let acceptor = Acceptor::bind("127.0.0.1:0".parse().unwrap(), acc_cfg, Arc::new(NoopApp))
        .await
        .unwrap();
    let addr = acceptor.local_addr().unwrap();
    acceptor.serve();

    let mut init_cfg = SessionConfig::new("FIX.4.4", "CLIENT", "SERVER", Role::Initiator);
    init_cfg.reset_on_logon = true; // triggers Action::ResetStore -> store.reset() on connect

    let logged_on = Arc::new(AtomicBool::new(false));
    struct FlagApp {
        on: Arc<AtomicBool>,
    }
    #[async_trait::async_trait]
    impl Application for FlagApp {
        async fn on_logon(&self, _s: &SessionId) {
            self.on.store(true, Ordering::SeqCst);
        }
    }

    let log = Arc::new(RecordingLog::default());
    let services = Services {
        store: Some(Arc::new(FailingStore)),
        log: Some(log.clone() as Arc<dyn truefix_log::Log>),
        ..Services::default()
    };

    let handle = connect_initiator_with(
        addr,
        init_cfg,
        Arc::new(FlagApp {
            on: logged_on.clone(),
        }),
        services,
    )
    .await
    .unwrap();

    // Give the connection time to complete its Logon exchange and the post-action store
    // persistence attempt(s), all of which will fail against FailingStore.
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.logout().await;

    let events = log.events.lock().unwrap();
    assert!(
        !events.is_empty(),
        "a store-operation failure must be routed through the log as an operator-visible \
         event, not silently swallowed"
    );
    assert!(
        events.iter().any(|e| e.contains("simulated failure")),
        "expected the logged event to include the underlying store error, got: {events:?}"
    );
}
