//! Shared helpers for audit 006 transport and engine lifecycle tests.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimeoutExpectation {
    finding: &'static str,
    timeout: Duration,
}

impl TimeoutExpectation {
    fn new(finding: &'static str, timeout: Duration) -> Self {
        Self { finding, timeout }
    }
}

fn assert_timeout_is_bounded(expectation: TimeoutExpectation) {
    assert!(
        expectation.timeout > Duration::ZERO,
        "{} timeout must be positive",
        expectation.finding
    );
}

#[test]
fn audit006_transport_helper_checks_positive_timeouts() {
    assert_timeout_is_bounded(TimeoutExpectation::new("NEW-151", Duration::from_millis(1)));
}
