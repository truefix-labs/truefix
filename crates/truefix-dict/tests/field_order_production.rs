//! T066 (US7, feature 006): a message's declared `fieldOrder` (`ordered` modifier) reaches the
//! real codegen-generated `encode()` path, not just the standalone `encode_with_order` primitive
//! (already covered by `crates/truefix-core/tests/field_order_encode.rs`) — GAP-27/FR-031.

const DICT: &str = "\
version FIX.4.4
field 8 BeginString STRING
field 9 BodyLength LENGTH
field 35 MsgType STRING
field 34 MsgSeqNum SEQNUM
field 49 SenderCompID STRING
field 56 TargetCompID STRING
field 52 SendingTime UTCTIMESTAMP
field 10 CheckSum STRING
field 11 ClOrdID STRING
field 21 HandlInst CHAR
field 55 Symbol STRING
message D NewOrderSingle ordered req:11,21,55
";

#[test]
fn a_message_declaring_ordered_emits_a_field_order_const_and_uses_encode_with_order() {
    let generated = truefix_dict::codegen::generate("TESTDICT", DICT.as_bytes())
        .expect("codegen should succeed for a minimal ordered-message dictionary");

    assert!(
        generated.contains("encode_with_order"),
        "an `ordered` message's generated encode() must call encode_with_order, not the plain \
         self.0.encode() -- generated code:\n{generated}"
    );
    assert!(
        generated.contains("NEWORDERSINGLE_FIELD_ORDER") || generated.contains("_FIELD_ORDER"),
        "expected a generated FIELD_ORDER const array for the ordered message -- generated \
         code:\n{generated}"
    );
    // The declared order (11, 21, 55) must appear in the emitted const array literal.
    assert!(
        generated.contains("[11, 21, 55]") || generated.contains("[11u32, 21u32, 55u32]"),
        "expected the FIELD_ORDER const to contain the declared order [11, 21, 55] -- generated \
         code:\n{generated}"
    );
}

#[test]
fn a_message_without_ordered_still_uses_the_plain_encode_path() {
    let dict_no_order = DICT.replace("ordered ", "");
    let generated = truefix_dict::codegen::generate("TESTDICT2", dict_no_order.as_bytes())
        .expect("codegen should succeed");
    assert!(
        generated.contains("self.0.encode()"),
        "a message with no `ordered` modifier must keep using the plain encode path unchanged"
    );
}
