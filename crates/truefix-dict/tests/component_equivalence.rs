//! T031 (US5) — a component-using dictionary validates identically to an equivalent hand-inlined
//! dictionary (SC-005). This is the core promise of FR-009: components are a pure authoring
//! convenience, expanded away before decode/validate ever see them.

use truefix_core::{Field, Message};
use truefix_dict::{parse, ValidationOptions};

const COMMON: &str = "\
version FIX.4.4
header 8 9 35 49 56 34 52
trailer 10
field 8 BeginString STRING
field 9 BodyLength LENGTH
field 35 MsgType STRING
field 34 MsgSeqNum SEQNUM
field 49 SenderCompID STRING
field 56 TargetCompID STRING
field 52 SendingTime UTCTIMESTAMP
field 10 CheckSum STRING
field 448 PartyID STRING
field 447 PartyIDSource CHAR
field 452 PartyRole INT
field 11 ClOrdID STRING
field 21 HandlInst CHAR
field 55 Symbol STRING
field 54 Side CHAR
field 60 TransactTime STRING
field 40 OrdType CHAR
";

fn componentized_dict() -> truefix_dict::DataDictionary {
    let src = format!(
        "{COMMON}\
         component Parties 448,447,452\n\
         message D NewOrderSingle req:11,21,55,54,60,40 opt:component:Parties\n"
    );
    parse(&src).unwrap()
}

fn hand_inlined_dict() -> truefix_dict::DataDictionary {
    let src = format!(
        "{COMMON}\
         message D NewOrderSingle req:11,21,55,54,60,40 opt:448,447,452\n"
    );
    parse(&src).unwrap()
}

fn valid_order_with_party() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    m.header.set(Field::int(34, 1));
    m.header.set(Field::string(49, "CLIENT"));
    m.header.set(Field::string(56, "SERVER"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::string(11, "O1"));
    m.body.set(Field::string(21, "1"));
    m.body.set(Field::string(55, "AAPL"));
    m.body.set(Field::string(54, "1"));
    m.body.set(Field::string(60, "20240101-00:00:00"));
    m.body.set(Field::string(40, "2"));
    // The component-supplied fields.
    m.body.set(Field::string(448, "BROKER-1"));
    m.body.set(Field::string(447, "D"));
    m.body.set(Field::int(452, 1));
    m
}

#[test]
fn valid_message_using_component_fields_passes_both_dictionaries_identically() {
    let msg = valid_order_with_party();
    let opts = ValidationOptions::default();
    assert_eq!(
        componentized_dict().validate(&msg, &opts).is_ok(),
        hand_inlined_dict().validate(&msg, &opts).is_ok()
    );
    assert!(componentized_dict().validate(&msg, &opts).is_ok());
}

#[test]
fn undefined_tag_for_message_type_rejected_identically() {
    let mut msg = valid_order_with_party();
    msg.body.set(Field::string(999, "not-a-defined-tag"));
    let opts = ValidationOptions::default();
    let a = componentized_dict().validate(&msg, &opts);
    let b = hand_inlined_dict().validate(&msg, &opts);
    assert!(a.is_err() && b.is_err());
    assert_eq!(a.unwrap_err().reason, b.unwrap_err().reason);
}

#[test]
fn missing_required_field_rejected_identically() {
    let mut msg = valid_order_with_party();
    msg.body = {
        let mut b = truefix_core::FieldMap::new();
        for f in msg.body.fields() {
            if f.tag() != 55 {
                b.set(Field::new(f.tag(), f.value_bytes().to_vec()));
            }
        }
        b
    };
    let opts = ValidationOptions::default();
    let a = componentized_dict().validate(&msg, &opts);
    let b = hand_inlined_dict().validate(&msg, &opts);
    assert!(a.is_err() && b.is_err());
    assert_eq!(a.unwrap_err().reason, b.unwrap_err().reason);
}

#[test]
fn component_field_bad_data_format_rejected_identically() {
    let mut msg = valid_order_with_party();
    msg.body.set(Field::string(452, "not-an-int")); // PartyRole is INT
    let opts = ValidationOptions::default();
    let a = componentized_dict().validate(&msg, &opts);
    let b = hand_inlined_dict().validate(&msg, &opts);
    assert!(a.is_err() && b.is_err());
    assert_eq!(a.unwrap_err().reason, b.unwrap_err().reason);
}

#[test]
fn dictionaries_share_the_same_member_tags_for_the_message_type() {
    let c = componentized_dict();
    let h = hand_inlined_dict();
    let cm = c.message("D").unwrap();
    let hm = h.message("D").unwrap();
    assert_eq!(cm.optional, hm.optional);
    assert_eq!(cm.member_tags, hm.member_tags);
}
