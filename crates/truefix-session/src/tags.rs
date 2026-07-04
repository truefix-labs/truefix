//! FIX tag numbers used by the session layer.

pub(crate) const MSG_SEQ_NUM: u32 = 34;
pub(crate) const POSS_DUP_FLAG: u32 = 43;
pub(crate) const SENDING_TIME: u32 = 52;
pub(crate) const ORIG_SENDING_TIME: u32 = 122;
pub(crate) const NEW_SEQ_NO: u32 = 36;
pub(crate) const GAP_FILL_FLAG: u32 = 123;
pub(crate) const BEGIN_SEQ_NO: u32 = 7;
pub(crate) const END_SEQ_NO: u32 = 16;
pub(crate) const RESET_SEQ_NUM_FLAG: u32 = 141;
pub(crate) const NEXT_EXPECTED_MSG_SEQ_NUM: u32 = 789;
pub(crate) const TEST_REQ_ID: u32 = 112;
pub(crate) const TEXT: u32 = 58;
pub(crate) const REF_SEQ_NUM: u32 = 45;
pub(crate) const REF_TAG_ID: u32 = 371;
pub(crate) const REF_MSG_TYPE: u32 = 372;
pub(crate) const SESSION_REJECT_REASON: u32 = 373;
pub(crate) const BUSINESS_REJECT_REASON: u32 = 380;
pub(crate) const ENCRYPT_METHOD: u32 = 98;
pub(crate) const HEART_BT_INT: u32 = 108;
pub(crate) const SESSION_STATUS: u32 = 573; // US10, FR-013
// T078/T079/GAP-18c (feature 006, FIXT 1.1): 1128 ApplVerID is a per-message header field any
// message may carry to select its own application version; 1137 DefaultApplVerID appears only on
// Logon, negotiating the counterparty's default for the rest of the connection when no per-message
// 1128 is present. Confirmed via `dict-src/normalized/FIXT11.fixdict`'s own field/header/message
// definitions -- distinct fields, not synonyms.
pub(crate) const APPL_VER_ID: u32 = 1128;
pub(crate) const DEFAULT_APPL_VER_ID: u32 = 1137;
