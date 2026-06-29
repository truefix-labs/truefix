//! Builders for session-level admin messages.

use truefix_core::{Field, Message};

use crate::config::SessionConfig;
use crate::time_util::now_utc_timestamp;

/// Build a message with the standard header (8/35/49/56/34/52).
fn base(config: &SessionConfig, msg_type: &str, seq: u64) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, &config.begin_string));
    m.header.set(Field::string(35, msg_type));
    m.header.set(Field::string(49, &config.sender_comp_id));
    m.header.set(Field::string(56, &config.target_comp_id));
    m.header.set(Field::int(34, seq as i64));
    m.header.set(Field::string(52, &now_utc_timestamp()));
    m
}

/// Logon (35=A) with EncryptMethod=0 and HeartBtInt; optional ResetSeqNumFlag.
pub(crate) fn logon(config: &SessionConfig, seq: u64) -> Message {
    let mut m = base(config, "A", seq);
    m.body.set(Field::int(98, 0));
    m.body
        .set(Field::int(108, i64::from(config.heartbeat_interval)));
    if config.reset_on_logon {
        m.body.set(Field::string(141, "Y"));
    }
    m
}

/// Logout (35=5) with optional Text (58).
pub(crate) fn logout(config: &SessionConfig, seq: u64, text: Option<&str>) -> Message {
    let mut m = base(config, "5", seq);
    if let Some(t) = text {
        m.body.set(Field::string(58, t));
    }
    m
}

/// Heartbeat (35=0), optionally echoing a TestReqID (112).
pub(crate) fn heartbeat(config: &SessionConfig, seq: u64, test_req_id: Option<&str>) -> Message {
    let mut m = base(config, "0", seq);
    if let Some(id) = test_req_id {
        m.body.set(Field::string(112, id));
    }
    m
}

/// TestRequest (35=1) with a TestReqID (112).
pub(crate) fn test_request(config: &SessionConfig, seq: u64, id: &str) -> Message {
    let mut m = base(config, "1", seq);
    m.body.set(Field::string(112, id));
    m
}
