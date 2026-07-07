//! T006 (feature 011, FR-008 groundwork) — `restructure_groups` produces a structurally-equal
//! result to `decode_with_groups` for the same input.

use truefix_core::{GroupSpec, decode, decode_with_groups, restructure_groups};

struct PartyIdsSpec;
impl GroupSpec for PartyIdsSpec {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        (count_tag == 453).then_some((448, [448u32, 447].as_slice()))
    }
}

struct NestedGroupSpec;
impl GroupSpec for NestedGroupSpec {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        match count_tag {
            453 => Some((448, [448u32, 523].as_slice())),
            523 => Some((524, [524u32].as_slice())),
            _ => None,
        }
    }
}

fn wire_with_body_group() -> Vec<u8> {
    let body =
        b"35=D\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01453=2\x01448=P1\x01448=P2\x01";
    wrap(body)
}

fn wire_with_nested_body_group() -> Vec<u8> {
    // NoPartyIDs(453)=1, one entry whose PartyID(448) is followed by a nested NoNested(523)=1
    // group entry carrying SubID(524).
    let body = b"35=D\x0134=1\x0149=A\x0156=B\x0152=20240101-00:00:00\x01453=1\x01448=P1\x01523=1\x01524=SUB1\x01";
    wrap(body)
}

fn wrap(body: &[u8]) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.extend_from_slice(b"8=FIX.4.4\x01");
    msg.extend_from_slice(format!("9={}\x01", body.len()).as_bytes());
    msg.extend_from_slice(body);
    let sum: u32 = msg.iter().map(|&b| u32::from(b)).sum::<u32>() & 0xFF;
    msg.extend_from_slice(format!("10={sum:03}\x01").as_bytes());
    msg
}

#[test]
fn restructure_groups_matches_decode_with_groups_for_a_single_level_group() {
    let wire = wire_with_body_group();

    let via_decode_with_groups =
        decode_with_groups(&wire, &PartyIdsSpec).expect("decode_with_groups");

    let mut via_flat_then_restructure = decode(&wire).expect("flat decode");
    restructure_groups(&mut via_flat_then_restructure.body, &PartyIdsSpec).expect("restructure");

    assert_eq!(via_decode_with_groups.body, via_flat_then_restructure.body);
    let entries = via_flat_then_restructure
        .body
        .group(453)
        .expect("NoPartyIDs group");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].get(448).and_then(|f| f.as_str().ok()),
        Some("P1")
    );
    assert_eq!(
        entries[1].get(448).and_then(|f| f.as_str().ok()),
        Some("P2")
    );
}

#[test]
fn restructure_groups_matches_decode_with_groups_for_a_nested_group() {
    let wire = wire_with_nested_body_group();

    let via_decode_with_groups =
        decode_with_groups(&wire, &NestedGroupSpec).expect("decode_with_groups");

    let mut via_flat_then_restructure = decode(&wire).expect("flat decode");
    restructure_groups(&mut via_flat_then_restructure.body, &NestedGroupSpec).expect("restructure");

    assert_eq!(via_decode_with_groups.body, via_flat_then_restructure.body);

    let entries = via_flat_then_restructure
        .body
        .group(453)
        .expect("NoPartyIDs group");
    assert_eq!(entries.len(), 1);
    let nested = entries[0].group(523).expect("nested NoNested group");
    assert_eq!(nested.len(), 1);
    assert_eq!(
        nested[0].get(524).and_then(|f| f.as_str().ok()),
        Some("SUB1")
    );
}

#[test]
fn restructure_groups_is_idempotent_on_an_already_structured_map() {
    let wire = wire_with_body_group();
    let mut msg = decode_with_groups(&wire, &PartyIdsSpec).expect("decode_with_groups");
    let before = msg.body.clone();

    // Re-running restructure_groups against the same spec on already-structured content must be
    // a safe no-op (flatten-then-regroup lands on the same structure).
    restructure_groups(&mut msg.body, &PartyIdsSpec).expect("restructure again");

    assert_eq!(before, msg.body);
}

#[test]
fn restructure_groups_leaves_content_flat_when_spec_declares_no_groups() {
    struct NoGroups;
    impl GroupSpec for NoGroups {
        fn group_of(&self, _count_tag: u32) -> Option<(u32, &[u32])> {
            None
        }
    }

    let wire = wire_with_body_group();
    let mut msg = decode(&wire).expect("flat decode");
    restructure_groups(&mut msg.body, &NoGroups).expect("restructure");

    // 453/448/448 all remain plain fields (NoGroups recognizes nothing as a group).
    assert!(msg.body.group(453).is_none());
    assert_eq!(msg.body.fields().filter(|f| f.tag() == 448).count(), 2);
}
