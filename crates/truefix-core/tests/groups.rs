//! T012 — repeating-group model + encode (dictionary-driven group *parsing* is Stage S4).

use truefix_core::{Field, FieldMap, Group, Message, decode, encode};

#[test]
fn build_group_encodes_to_known_vector() {
    // Known-good wire vector (FIX-spec data): NoPartyIDs=1, BodyLength 35, CheckSum 040.
    let expected: &[u8] =
        b"8=FIX.4.4\x019=35\x0135=D\x01453=1\x01448=PARTY_1\x01447=I\x01452=6\x0110=040\x01";

    let mut msg = Message::new();
    msg.header.set(Field::string(8, "FIX.4.4"));
    msg.header.set(Field::string(35, "D"));

    let mut entry = FieldMap::new();
    entry.add_field(Field::string(448, "PARTY_1"));
    entry.add_field(Field::string(447, "I"));
    entry.add_field(Field::int(452, 6));
    let mut group = Group::new(453);
    group.add_entry(entry);
    msg.body.add_group(group);

    assert_eq!(encode(&msg), expected);
}

#[test]
fn empty_group_renders_zero_count() {
    let mut msg = Message::new();
    msg.header.set(Field::string(8, "FIX.4.2"));
    msg.header.set(Field::string(35, "D"));
    msg.body.add_group(Group::new(453));

    let s = String::from_utf8(encode(&msg)).unwrap();
    assert!(s.contains("453=0\u{1}"), "{s:?}");
}

#[test]
fn nested_group_roundtrips_at_byte_level() {
    // Build a group whose entry contains a nested group, encode it, then confirm a flat
    // decode re-encodes to identical bytes (group-aware decode arrives in S4).
    let mut nested_entry = FieldMap::new();
    nested_entry.add_field(Field::string(524, "SUBPARTY"));
    let mut nested = Group::new(523);
    nested.add_entry(nested_entry);

    let mut outer_entry = FieldMap::new();
    outer_entry.add_field(Field::string(448, "P1"));
    outer_entry.add_field(Field::string(447, "I"));
    outer_entry.add_group(nested);
    let mut outer = Group::new(453);
    outer.add_entry(outer_entry);

    let mut msg = Message::new();
    msg.header.set(Field::string(8, "FIX.4.4"));
    msg.header.set(Field::string(35, "D"));
    msg.body.add_group(outer);

    let bytes = encode(&msg);
    let decoded = decode(&bytes).unwrap();
    assert_eq!(encode(&decoded), bytes);
}

// --- T062 (US9, feature 005): replace_group/remove_group/get_group (FR-024/025) ---

fn three_party_group() -> FieldMap {
    let mut fm = FieldMap::new();
    let mut group = Group::new(453);
    for name in ["P1", "P2", "P3"] {
        let mut entry = FieldMap::new();
        entry.add_field(Field::string(448, name));
        group.add_entry(entry);
    }
    fm.add_group(group);
    fm
}

#[test]
fn get_group_returns_the_entry_at_index() {
    let fm = three_party_group();
    let entry = fm.get_group(453, 1).unwrap();
    assert_eq!(entry.get(448).unwrap().as_str().unwrap(), "P2");
}

#[test]
fn get_group_out_of_range_index_is_none() {
    let fm = three_party_group();
    assert!(fm.get_group(453, 99).is_none());
}

#[test]
fn get_group_unknown_count_tag_is_none() {
    let fm = three_party_group();
    assert!(fm.get_group(999, 0).is_none());
}

#[test]
fn replace_group_swaps_the_entry_in_place() {
    let mut fm = three_party_group();
    let mut replacement = FieldMap::new();
    replacement.add_field(Field::string(448, "REPLACED"));
    fm.replace_group(453, 1, replacement);
    assert_eq!(
        fm.group(453).unwrap()[1]
            .get(448)
            .unwrap()
            .as_str()
            .unwrap(),
        "REPLACED"
    );
    // Other entries untouched.
    assert_eq!(
        fm.group(453).unwrap()[0]
            .get(448)
            .unwrap()
            .as_str()
            .unwrap(),
        "P1"
    );
    assert_eq!(
        fm.group(453).unwrap()[2]
            .get(448)
            .unwrap()
            .as_str()
            .unwrap(),
        "P3"
    );
    assert_eq!(fm.group(453).unwrap().len(), 3);
}

#[test]
fn replace_group_out_of_range_index_is_a_no_op() {
    let mut fm = three_party_group();
    let mut replacement = FieldMap::new();
    replacement.add_field(Field::string(448, "REPLACED"));
    fm.replace_group(453, 99, replacement);
    assert_eq!(fm.group(453).unwrap().len(), 3); // unchanged
}

#[test]
fn remove_group_deletes_the_entry_and_shifts_later_ones_down() {
    let mut fm = three_party_group();
    fm.remove_group(453, 0);
    let entries = fm.group(453).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get(448).unwrap().as_str().unwrap(), "P2");
    assert_eq!(entries[1].get(448).unwrap().as_str().unwrap(), "P3");
}

#[test]
fn remove_group_out_of_range_index_is_a_no_op() {
    let mut fm = three_party_group();
    fm.remove_group(453, 99);
    assert_eq!(fm.group(453).unwrap().len(), 3); // unchanged
}
