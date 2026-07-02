//! Builders for session-level admin messages.

use truefix_core::{Field, Message};

use crate::config::SessionConfig;
use crate::tags::{
    BEGIN_SEQ_NO, BUSINESS_REJECT_REASON, ENCRYPT_METHOD, END_SEQ_NO, GAP_FILL_FLAG, HEART_BT_INT,
    NEW_SEQ_NO, NEXT_EXPECTED_MSG_SEQ_NUM, ORIG_SENDING_TIME, POSS_DUP_FLAG, REF_MSG_TYPE,
    REF_SEQ_NUM, REF_TAG_ID, RESET_SEQ_NUM_FLAG, SENDING_TIME, SESSION_REJECT_REASON, TEST_REQ_ID,
    TEXT,
};
use crate::time_util::{now_utc_timestamp, now_utc_timestamp_prec};

/// Build a message with the standard header (8/35/49/56/34/52).
fn base(config: &SessionConfig, msg_type: &str, seq: u64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, &config.begin_string));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, &config.sender_comp_id));
    m.header.set(Field::string(56, &config.target_comp_id));
    m.header.set(Field::int(34, seq as i64));
    m.header.set(Field::string(
        52,
        &now_utc_timestamp_prec(config.timestamp_precision),
    ));
    m
}

/// Logon (35=A) with EncryptMethod=0 and HeartBtInt; optional ResetSeqNumFlag and
/// NextExpectedMsgSeqNum (789).
pub(crate) fn logon(config: &SessionConfig, seq: u64, next_expected: Option<u64>) -> Message {
    let mut m = base(config, "A", seq);
    m.body.set(Field::int(ENCRYPT_METHOD, 0));
    m.body.set(Field::int(
        HEART_BT_INT,
        i64::from(config.heartbeat_interval),
    ));
    if config.reset_on_logon {
        m.body.set(Field::string(RESET_SEQ_NUM_FLAG, "Y"));
    }
    if let Some(n) = next_expected {
        m.body.set(Field::int(NEXT_EXPECTED_MSG_SEQ_NUM, n as i64));
    }
    if let Some((tag, value)) = &config.logon_tag {
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

/// Session-level Reject (35=3) referencing a sequence number with optional Text.
pub(crate) fn reject(config: &SessionConfig, seq: u64, ref_seq: u64, text: &str) -> Message {
    let mut m = base(config, "3", seq);
    m.body.set(Field::int(REF_SEQ_NUM, ref_seq as i64));
    m.body.set(Field::string(TEXT, text));
    m
}

/// Session-level Reject (35=3) with a SessionRejectReason (373) and optional RefTagID (371).
pub(crate) fn reject_with_reason(
    config: &SessionConfig,
    seq: u64,
    ref_seq: u64,
    ref_tag: Option<u32>,
    reason_code: u32,
    text: &str,
) -> Message {
    let mut m = base(config, "3", seq);
    m.body.set(Field::int(REF_SEQ_NUM, ref_seq as i64));
    if let Some(t) = ref_tag {
        m.body.set(Field::int(REF_TAG_ID, i64::from(t)));
    }
    m.body
        .set(Field::int(SESSION_REJECT_REASON, i64::from(reason_code)));
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
/// (122 = the original SendingTime), and refresh SendingTime (52).
pub(crate) fn prepare_resend(original: &Message) -> Message {
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
    m.header
        .set(Field::string(SENDING_TIME, &now_utc_timestamp()));
    m
}
