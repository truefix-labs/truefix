//! T063/T064 (US2, feature 009, `NEW-22`): a decoded repeating group's `NoXxx` count was silently
//! "corrected" to `entries.len()` on re-encode, discarding the wire's own -- possibly malformed --
//! declaration. Round-trip fidelity: a decoded message's declared count must survive an
//! encode-decode-encode cycle unchanged, even when it doesn't match the actual entry count.

use truefix_core::{Field, FieldMap, Group, Message, decode};

struct PartyIdsSpec;
impl truefix_core::GroupSpec for PartyIdsSpec {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        (count_tag == 453).then_some((448, [448u32, 447].as_slice()))
    }
}

fn wire_with_mismatched_count() -> Vec<u8> {
    // NoPartyIDs(453)=3 but only 2 PartyID(448) entries actually follow.
    let body =
        b"35=D\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01453=3\x01448=P1\x01448=P2\x01";
    let mut msg = Vec::new();
    msg.extend_from_slice(b"8=FIX.4.4\x01");
    msg.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    msg.extend_from_slice(body);
    let sum: u32 = msg.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    msg.extend_from_slice(format!("10={sum:03}\x01").as_bytes());
    msg
}

#[test]
fn a_mismatched_wire_group_count_is_preserved_through_a_decode_encode_round_trip() {
    let wire = wire_with_mismatched_count();
    let decoded = truefix_core::decode_with_groups(&wire, &PartyIdsSpec).expect("decodes");
    assert_eq!(decoded.body.group(453).map(<[_]>::len), Some(2));

    let re_encoded = decoded.encode();
    let re_encoded_str = String::from_utf8_lossy(&re_encoded);
    assert!(
        re_encoded_str.contains("453=3"),
        "re-encoded wire must preserve the original declared count of 3, not silently correct it \
         to 2 (NEW-22), got: {re_encoded_str:?}"
    );

    // Re-decoding without a GroupSpec routes 453/448 as flat body fields, so the count field's
    // own value is directly inspectable as a plain field.
    let re_decoded = decode(&re_encoded).expect("re-encoded bytes still decode");
    assert_eq!(
        re_decoded.body.get(453).and_then(|f| f.as_str().ok()),
        Some("3"),
        "the group's declared count must survive a decode-encode round trip unchanged"
    );
}

#[test]
fn a_freshly_built_group_still_encodes_its_true_entry_count() {
    let mut full = Message::new();
    full.header.set(Field::string(8, "FIX.4.4"));
    full.header.set(Field::string(35, "D"));
    full.header.set(Field::int(34, 1));
    full.header.set(Field::string(49, "A"));
    full.header.set(Field::string(56, "B"));
    full.header.set(Field::string(52, "20240101-00:00:00"));
    let mut g = Group::new(453);
    let mut entry = FieldMap::new();
    entry.add_field(Field::string(448, "P1"));
    g.add_entry(entry);
    full.body.add_group(g);
    full.trailer.set(Field::string(10, "000"));

    let out = full.encode();
    let out_str = String::from_utf8_lossy(&out);
    assert!(
        out_str.contains("453=1"),
        "a group built fresh (not decoded) must still encode its true entries.len(), got: \
         {out_str:?}"
    );
}
