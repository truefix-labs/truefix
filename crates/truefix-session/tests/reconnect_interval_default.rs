//! T097 — the programmatic reconnect default matches the `.cfg` and QuickFIX/J default.

use truefix_session::{Role, SessionConfig};

#[test]
fn programmatic_reconnect_interval_defaults_to_thirty_seconds() {
    let config = SessionConfig::new("FIX.4.4", "SENDER", "TARGET", Role::Initiator);

    assert_eq!(config.reconnect_interval, 30);
    assert!(
        config.reconnect_interval_steps.is_empty(),
        "the scalar default must not implicitly enable stepped reconnect backoff"
    );
}
