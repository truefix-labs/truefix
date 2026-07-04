//! Builders for session-level admin messages.

use truefix_core::{Field, Message};

use crate::config::SessionConfig;
use crate::tags::{
    BEGIN_SEQ_NO, BUSINESS_REJECT_REASON, ENCRYPT_METHOD, END_SEQ_NO, GAP_FILL_FLAG, HEART_BT_INT,
    NEW_SEQ_NO, NEXT_EXPECTED_MSG_SEQ_NUM, ORIG_SENDING_TIME, POSS_DUP_FLAG, REF_MSG_TYPE,
    REF_SEQ_NUM, REF_TAG_ID, RESET_SEQ_NUM_FLAG, SENDING_TIME, SESSION_REJECT_REASON, TEST_REQ_ID,
    TEXT,
};
use crate::time_util::now_utc_timestamp_prec;

/// Build a message with the standard header (8/35/49/56/34/52, plus 50/142/57/143 when
/// configured).
fn base(config: &SessionConfig, msg_type: &str, seq: u64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, &config.begin_string));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, &config.sender_comp_id));
    // BUG-07/FR-010 (feature 006): stamp SenderSubID/SenderLocationID/TargetSubID/
    // TargetLocationID (tags 50/142/57/143) when configured, so a SubID/LocationID-distinguished
    // session's own outbound messages actually carry the identity the counterparty's routing
    // lookup needs to see — without this, the fields exist only in config, never on the wire.
    if let Some(v) = &config.sender_sub_id {
        m.header.set(Field::string(50, v));
    }
    if let Some(v) = &config.sender_location_id {
        m.header.set(Field::string(142, v));
    }
    m.header.set(Field::string(56, &config.target_comp_id));
    if let Some(v) = &config.target_sub_id {
        m.header.set(Field::string(57, v));
    }
    if let Some(v) = &config.target_location_id {
        m.header.set(Field::string(143, v));
    }
    m.header.set(Field::int(34, seq as i64));
    m.header.set(Field::string(
        52,
        &now_utc_timestamp_prec(config.timestamp_precision),
    ));
    m
}

/// Logon (35=A) with EncryptMethod=0 and HeartBtInt; optional ResetSeqNumFlag and
/// NextExpectedMsgSeqNum (789). The caller decides `ResetSeqNumFlag` explicitly: an acceptor's
/// Logon *response* must echo whatever the inbound Logon that triggered it actually carried
/// (BUG-28/FR-005, feature 007), while an initiator's own first outbound Logon sends it based on
/// `isResetNeeded()`-equivalent logic — any of `ResetOnLogon`/`ResetOnLogout`/`ResetOnDisconnect`
/// combined with both sequence numbers already being 1 (BUG-92/BUG-109/FR-025, feature 007) —
/// rather than always deferring to the static `config.reset_on_logon` value alone.
pub(crate) fn logon_with_reset_flag(
    config: &SessionConfig,
    seq: u64,
    next_expected: Option<u64>,
    reset_seq_num_flag: bool,
) -> Message {
    let mut m = base(config, "A", seq);
    m.body.set(Field::int(ENCRYPT_METHOD, 0));
    m.body.set(Field::int(
        HEART_BT_INT,
        i64::from(config.heartbeat_interval),
    ));
    if reset_seq_num_flag {
        m.body.set(Field::string(RESET_SEQ_NUM_FLAG, "Y"));
    }
    if let Some(n) = next_expected {
        m.body.set(Field::int(NEXT_EXPECTED_MSG_SEQ_NUM, n as i64));
    }
    for (tag, value) in &config.logon_tags {
        m.body.set(Field::string(*tag, value));
    }
    m
}

/// Logout (35=5) with optional Text (58).
pub(crate) fn logout(config: &SessionConfig, seq: u64, text: Option<&str>) -> Message {
    let mut m = base(config, "5", seq);
    if let Some(t) = text {
        m.body.set(Field::string(TEXT, t));
    }
    m
}

/// Heartbeat (35=0), optionally echoing a TestReqID (112).
pub(crate) fn heartbeat(config: &SessionConfig, seq: u64, test_req_id: Option<&str>) -> Message {
    let mut m = base(config, "0", seq);
    if let Some(id) = test_req_id {
        m.body.set(Field::string(TEST_REQ_ID, id));
    }
    m
}

/// TestRequest (35=1) with a TestReqID (112).
pub(crate) fn test_request(config: &SessionConfig, seq: u64, id: &str) -> Message {
    let mut m = base(config, "1", seq);
    m.body.set(Field::string(TEST_REQ_ID, id));
    m
}

/// ResendRequest (35=2) for `[begin, end]` (end `0` means "to infinity").
pub(crate) fn resend_request(config: &SessionConfig, seq: u64, begin: u64, end: u64) -> Message {
    let mut m = base(config, "2", seq);
    m.body.set(Field::int(BEGIN_SEQ_NO, begin as i64));
    m.body.set(Field::int(END_SEQ_NO, end as i64));
    m
}

/// SequenceReset (35=4) with NewSeqNo (36); `gap_fill` sets GapFillFlag (123=Y) and PossDupFlag.
pub(crate) fn sequence_reset(
    config: &SessionConfig,
    seq: u64,
    new_seq_no: u64,
    gap_fill: bool,
) -> Message {
    let mut m = base(config, "4", seq);
    m.body.set(Field::int(NEW_SEQ_NO, new_seq_no as i64));
    if gap_fill {
        m.body.set(Field::string(GAP_FILL_FLAG, "Y"));
        m.header.set(Field::string(POSS_DUP_FLAG, "Y"));
    }
    m
}

/// Parses a session's own `begin_string` (e.g. `"FIX.4.2"` -> `(4, 2)`) into its major/minor
/// version, used to gate two session-level `Reject(35=3)` fields whose presence/range QuickFIX/J
/// varies by version (BUG-58/BUG-59, feature 007). `None` for anything that doesn't parse as
/// `FIX.<major>.<minor>` (e.g. `FIXT.1.1`) — callers treat that conservatively as "include
/// unconditionally", matching this codebase's behavior before this version-gating existed.
pub(crate) fn fix_version(begin_string: &str) -> Option<(u8, u8)> {
    let rest = begin_string.strip_prefix("FIX.")?;
    let mut parts = rest.splitn(2, '.');
    let major: u8 = parts.next()?.parse().ok()?;
    let minor: u8 = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Whether `RefMsgType(372)` belongs on a session-level Reject for this session's own version
/// (BUG-58/FR-023, feature 007): FIX.4.2+ only (QFJ: `beginString.compareTo(FixVersions.BEGINSTRING_FIX42) >= 0`).
fn ref_msg_type_applies(config: &SessionConfig) -> bool {
    match fix_version(&config.begin_string) {
        Some((major, minor)) => (major, minor) >= (4, 2),
        None => true,
    }
}

/// Whether `SessionRejectReason(373)=reason_code` belongs on a session-level Reject for this
/// session's own version (BUG-59/FR-023, feature 007): FIX.4.0/4.1 omit the field entirely;
/// FIX.4.2 only defines codes <=11; FIX.4.3 only <=15; FIX.4.4 and later (including FIX.5.0+/
/// FIXT.1.1) only <=16 or 99 (`Other`).
fn session_reject_reason_applies(config: &SessionConfig, reason_code: u32) -> bool {
    match fix_version(&config.begin_string) {
        Some((4, 0)) | Some((4, 1)) => false,
        Some((4, 2)) => reason_code <= 11,
        Some((4, 3)) => reason_code <= 15,
        Some((4, _)) => reason_code <= 16 || reason_code == 99,
        Some((major, _)) if major > 4 => reason_code <= 16 || reason_code == 99,
        _ => true,
    }
}

/// Session-level Reject (35=3) referencing a sequence number with optional Text.
pub(crate) fn reject(
    config: &SessionConfig,
    seq: u64,
    ref_seq: u64,
    ref_msg_type: Option<&str>,
    text: &str,
) -> Message {
    let mut m = base(config, "3", seq);
    m.body.set(Field::int(REF_SEQ_NUM, ref_seq as i64));
    // BUG-58/FR-023 (feature 007): RefMsgType(372), FIX.4.2+ only.
    if let Some(mt) = ref_msg_type {
        if ref_msg_type_applies(config) {
            m.body.set(Field::string(REF_MSG_TYPE, mt));
        }
    }
    m.body.set(Field::string(TEXT, text));
    m
}

/// Session-level Reject (35=3) with a SessionRejectReason (373) and optional RefTagID (371).
#[allow(clippy::too_many_arguments)]
pub(crate) fn reject_with_reason(
    config: &SessionConfig,
    seq: u64,
    ref_seq: u64,
    ref_msg_type: Option<&str>,
    ref_tag: Option<u32>,
    reason_code: u32,
    text: &str,
) -> Message {
    let mut m = base(config, "3", seq);
    m.body.set(Field::int(REF_SEQ_NUM, ref_seq as i64));
    // BUG-58/FR-023 (feature 007): RefMsgType(372), FIX.4.2+ only.
    if let Some(mt) = ref_msg_type {
        if ref_msg_type_applies(config) {
            m.body.set(Field::string(REF_MSG_TYPE, mt));
        }
    }
    if let Some(t) = ref_tag {
        m.body.set(Field::int(REF_TAG_ID, i64::from(t)));
    }
    // BUG-59/FR-023 (feature 007): SessionRejectReason(373), version-filtered.
    if session_reject_reason_applies(config, reason_code) {
        m.body
            .set(Field::int(SESSION_REJECT_REASON, i64::from(reason_code)));
    }
    m.body.set(Field::string(TEXT, text));
    m
}

/// Business Message Reject (35=j) with RefMsgType (372), BusinessRejectReason (380), and optional
/// RefTagID (371).
pub(crate) fn business_message_reject(
    config: &SessionConfig,
    seq: u64,
    ref_seq: u64,
    ref_msg_type: Option<&str>,
    ref_tag: Option<u32>,
    reason_code: u32,
    text: &str,
) -> Message {
    let mut m = base(config, "j", seq);
    m.body.set(Field::int(REF_SEQ_NUM, ref_seq as i64));
    if let Some(mt) = ref_msg_type {
        m.body.set(Field::string(REF_MSG_TYPE, mt));
    }
    if let Some(t) = ref_tag {
        m.body.set(Field::int(REF_TAG_ID, i64::from(t)));
    }
    m.body
        .set(Field::int(BUSINESS_REJECT_REASON, i64::from(reason_code)));
    m.body.set(Field::string(TEXT, text));
    m
}

/// Prepare a stored application message for resend: stamp PossDupFlag (43=Y) and OrigSendingTime
/// (122 = the original SendingTime), and refresh SendingTime (52) at the session's configured
/// timestamp precision.
pub(crate) fn prepare_resend(config: &SessionConfig, original: &Message) -> Message {
    let mut m = original.clone();
    let orig_sending = m
        .header
        .get(SENDING_TIME)
        .and_then(|f| f.as_str().ok())
        .map(str::to_owned);
    m.header.set(Field::string(POSS_DUP_FLAG, "Y"));
    if let Some(orig) = orig_sending {
        m.header.set(Field::string(ORIG_SENDING_TIME, &orig));
    }
    // BUG-66/FR-037 (feature 007): `now_utc_timestamp_prec(config.timestamp_precision)`, not
    // hardcoded milliseconds — resent messages previously always used millisecond precision
    // regardless of `TimeStampPrecision`, unlike every other outbound timestamp in this file (e.g.
    // `logon`'s own `now_utc_timestamp_prec` call above).
    m.header.set(Field::string(
        SENDING_TIME,
        &now_utc_timestamp_prec(config.timestamp_precision),
    ));
    m
}
