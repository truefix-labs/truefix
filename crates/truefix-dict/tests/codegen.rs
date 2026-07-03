//! T043 (US6) — codegen shape: typed messages, group entries, and field-value enums are
//! generated and usable (FR-020).

use time::macros::datetime;
use truefix_dict::fix44::{HandlInst, NewOrderSingle, NoPartyIDsEntry, OrdType, Side};

#[test]
fn typed_message_builds_with_named_accessors() {
    let mut order = NewOrderSingle::new();
    order
        .set_cl_ord_id("O1")
        .set_symbol("AAPL")
        .set_side(Side::BUY)
        .set_ord_type(OrdType::MARKET)
        .set_handl_inst(HandlInst::AUTOEXECPRIV);

    assert_eq!(order.cl_ord_id(), Some("O1"));
    assert_eq!(order.symbol(), Some("AAPL"));
    assert_eq!(order.side(), Some(Side::BUY));
    assert_eq!(order.ord_type(), Some(OrdType::MARKET));
    assert_eq!(order.0.msg_type(), Some("D"));
}

#[test]
fn field_value_enum_round_trips_the_wire_value() {
    assert_eq!(Side::BUY.as_str(), "1");
    assert_eq!(Side::parse("2"), Some(Side::SELL));
    assert_eq!(Side::parse("Z"), None); // not a real Side value
}

#[test]
fn utc_timestamp_setter_writes_the_fix_wire_format_not_display() {
    // OffsetDateTime's own `Display` (e.g. "2024-01-02 03:04:05.678 +00:00:00") is NOT the FIX
    // wire format ("20240102-03:04:05.678"); the generated setter must not use `.to_string()`.
    let dt = datetime!(2024-01-02 03:04:05.678 UTC);
    let mut order = NewOrderSingle::new();
    order.set_transact_time(dt);
    let raw = order.0.body.get(60).unwrap().as_str().unwrap();
    assert_eq!(raw, "20240102-03:04:05.678");
    // And the typed getter parses it back correctly.
    let read_back = order.transact_time().unwrap();
    assert_eq!(read_back.unix_timestamp(), dt.unix_timestamp());
}

#[test]
fn group_entries_are_typed_and_nestable() {
    let mut party = NoPartyIDsEntry::new();
    party.set_party_id("BROKER").set_party_role(1);

    let mut order = NewOrderSingle::new();
    order.set_no_party_ids(vec![party]);

    let entries = order.no_party_ids();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].party_id(), Some("BROKER"));
    assert_eq!(entries[0].party_role(), Some(1));
    assert!(entries[0].no_party_sub_ids().is_empty());
}
