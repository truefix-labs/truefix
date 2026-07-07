//! Shared helpers for audit 006 store durability tests.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceSnapshot {
    sender: u64,
    target: u64,
}

impl SequenceSnapshot {
    fn new(sender: u64, target: u64) -> Self {
        Self { sender, target }
    }
}

fn assert_sequence_state_unchanged(before: SequenceSnapshot, after: SequenceSnapshot) {
    assert_eq!(
        after, before,
        "sequence state changed after failed persistence"
    );
}

#[test]
fn audit006_store_helper_checks_sequence_snapshot() {
    let snapshot = SequenceSnapshot::new(7, 9);
    assert_sequence_state_unchanged(snapshot, snapshot);
}
