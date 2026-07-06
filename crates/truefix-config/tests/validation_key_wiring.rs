//! T036/T038 (US1, feature 009, `NEW-10`): 7 validation config keys were registered as
//! `Impl`(emented) in the Appendix A key registry, but only 5 of them (`ValidateFieldsOutOfOrder`,
//! `ValidateChecksum`, `ValidateIncomingMessage`, `AllowPosDup`, `RequiresOrigSendingTime`) were
//! actually read in `resolve_validator`. The other 5 covered by this task's own test
//! (`ValidateFieldsHaveValues`, `ValidateUnorderedGroupFields`, `ValidateUserDefinedFields`,
//! `AllowUnknownMsgFields`, `FirstFieldInGroupIsDelimiter`) are now wired (see
//! `validator_mapping.rs`); the remaining 2 (`ValidateSequenceNumbers`, `RejectInvalidMessage`)
//! have no corresponding `ValidationOptions` field at all, so per `/speckit-clarify`'s decision
//! they are downgraded to `Recognized` here instead of gaining new enforcement logic.

use truefix_config::keys::{Stance, key_info};

#[test]
fn validate_sequence_numbers_is_downgraded_to_recognized_not_implemented() {
    let info = key_info("ValidateSequenceNumbers").expect("key registered");
    assert_eq!(
        info.stance,
        Stance::Recognized,
        "ValidateSequenceNumbers has no corresponding ValidationOptions field -- it must be \
         Recognized (accepted, not enforced), not Impl (falsely claiming full implementation)"
    );
}

#[test]
fn reject_invalid_message_is_downgraded_to_recognized_not_implemented() {
    let info = key_info("RejectInvalidMessage").expect("key registered");
    assert_eq!(
        info.stance,
        Stance::Recognized,
        "RejectInvalidMessage has no corresponding ValidationOptions field -- it must be \
         Recognized (accepted, not enforced), not Impl (falsely claiming full implementation)"
    );
}
