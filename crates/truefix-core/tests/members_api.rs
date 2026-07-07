//! T005 (feature 011, FR-001/FR-002) — `FieldMap::members()`/`MemberRef`.

use truefix_core::{Field, FieldMap, Group, GroupSpec, MemberRef};

struct PartyIdsSpec;
impl GroupSpec for PartyIdsSpec {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        (count_tag == 453).then_some((448, [448u32, 447].as_slice()))
    }
}

/// NoPartyIDs(453)=3 declared, but only 2 PartyID(448) entries actually follow — same
/// mismatched-count fixture shape as `group_count_round_trip.rs`.
fn wire_with_mismatched_count() -> Vec<u8> {
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
fn flat_only_map_yields_the_same_tags_as_fields_wrapped_in_member_ref_field() {
    let mut map = FieldMap::new();
    map.add_field(Field::string(11, "CLORD1"));
    map.add_field(Field::int(38, 100));

    let via_members: Vec<u32> = map
        .members()
        .map(|m| match m {
            MemberRef::Field(f) => f.tag(),
            MemberRef::Group { .. } => panic!("unexpected group"),
        })
        .collect();
    let via_fields: Vec<u32> = map.fields().map(Field::tag).collect();
    assert_eq!(via_members, via_fields);
    assert_eq!(via_members, vec![11, 38]);
}

#[test]
fn single_level_group_is_visible_via_members_but_invisible_via_fields() {
    let mut map = FieldMap::new();
    map.add_field(Field::string(11, "CLORD1"));
    let mut entry = FieldMap::new();
    entry.add_field(Field::string(448, "PARTY_1"));
    let mut group = Group::new(453);
    group.add_entry(entry);
    map.add_group(group);

    // `.fields()` still skips the group entirely (FR-002 — unchanged).
    let via_fields: Vec<u32> = map.fields().map(Field::tag).collect();
    assert_eq!(via_fields, vec![11]);

    // `.members()` sees both the plain field and the group's structure.
    let via_members: Vec<MemberRef<'_>> = map.members().collect();
    assert_eq!(via_members.len(), 2);
    assert!(matches!(via_members[0], MemberRef::Field(f) if f.tag() == 11));
    match via_members[1] {
        MemberRef::Group {
            count_tag, entries, ..
        } => {
            assert_eq!(count_tag, 453);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].get(448).map(|f| f.tag()), Some(448));
        }
        MemberRef::Field(_) => panic!("expected a group"),
    }
}

#[test]
fn nested_group_is_visible_recursively_via_members() {
    let mut nested_entry = FieldMap::new();
    nested_entry.add_field(Field::string(524, "SUBPARTY"));
    let mut nested = Group::new(523);
    nested.add_entry(nested_entry);

    let mut outer_entry = FieldMap::new();
    outer_entry.add_field(Field::string(448, "P1"));
    outer_entry.add_group(nested);
    let mut outer = Group::new(453);
    outer.add_entry(outer_entry);

    let mut map = FieldMap::new();
    map.add_group(outer);

    let members: Vec<MemberRef<'_>> = map.members().collect();
    assert_eq!(members.len(), 1);
    let MemberRef::Group { entries, .. } = members[0] else {
        panic!("expected the outer group");
    };
    let entry_members: Vec<MemberRef<'_>> = entries[0].members().collect();
    assert_eq!(entry_members.len(), 2);
    match entry_members[1] {
        MemberRef::Group {
            count_tag, entries, ..
        } => {
            assert_eq!(count_tag, 523);
            assert_eq!(entries[0].get(524).map(|f| f.tag()), Some(524));
        }
        MemberRef::Field(_) => panic!("expected the nested group"),
    }
}

#[test]
fn declared_count_mismatch_is_surfaced_through_member_ref() {
    let wire = wire_with_mismatched_count();
    let decoded = truefix_core::decode_with_groups(&wire, &PartyIdsSpec).expect("decodes");

    let members: Vec<MemberRef<'_>> = decoded.body.members().collect();
    let group = members
        .iter()
        .find(|m| matches!(m, MemberRef::Group { count_tag: 453, .. }))
        .expect("NoPartyIDs(453) is present as a structured group");
    match *group {
        MemberRef::Group {
            entries,
            declared_count,
            ..
        } => {
            assert_eq!(
                entries.len(),
                2,
                "only 2 entries actually followed on the wire"
            );
            assert_eq!(
                declared_count,
                Some(3),
                "the wire declared 453=3 even though only 2 entries followed"
            );
        }
        MemberRef::Field(_) => unreachable!(),
    }
}

#[test]
fn a_freshly_built_group_has_no_declared_count() {
    let mut map = FieldMap::new();
    map.add_group(Group::new(453));
    let members: Vec<MemberRef<'_>> = map.members().collect();
    match members[0] {
        MemberRef::Group { declared_count, .. } => assert_eq!(declared_count, None),
        MemberRef::Field(_) => panic!("expected a group"),
    }
}
