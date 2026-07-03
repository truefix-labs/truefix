//! T063 (US9, feature 005) — `decode_with_groups` structures a repeating group declared in the
//! *header* or *trailer* section, not just the body (FR-026). Dormant until a dictionary source
//! declares one (today none of TrueFix's bundled dictionaries do) — this test exercises the core
//! codec-layer mechanism directly via a synthetic `GroupSpec`, using two tag numbers
//! `truefix_core::tags::is_header` already classifies as header fields (128/115 — DeliverToCompID/
//! OnBehalfOfCompID), independent of their real FIX semantics — only their section classification
//! matters for this mechanism-level test. (90/91 were tried first but rejected: they're a
//! length-prefixed binary-data pair (`SecureDataLen`/`SecureData`), which the tokenizer treats
//! specially — a real, if incidental, discovery about which header tags are safe to repurpose for
//! this kind of synthetic test.)

use truefix_core::{decode_with_groups, encode, Field, FieldMap, Group, GroupSpec, Message};

/// Declares tag 128 as a repeating-group count tag (delimiter 115, member [115]) — both already
/// classified as header fields by `tags::is_header`.
struct HeaderGroupSpec;

impl GroupSpec for HeaderGroupSpec {
    fn group_of(&self, count_tag: u32) -> Option<(u32, &[u32])> {
        (count_tag == 128).then_some((115, &[115][..]))
    }
}

fn msg_with_header_group() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, "FIX.4.4"));
    m.header.set(Field::string(35, "D"));
    let mut group = Group::new(128);
    let mut entry_a = FieldMap::new();
    entry_a.add_field(Field::string(115, "A"));
    let mut entry_b = FieldMap::new();
    entry_b.add_field(Field::string(115, "B"));
    group.add_entry(entry_a);
    group.add_entry(entry_b);
    m.header.add_group(group);
    m.body.set(Field::string(11, "ORD1"));
    m
}

#[test]
fn a_header_repeating_group_is_structured_on_decode() {
    let bytes = encode(&msg_with_header_group());
    let decoded = decode_with_groups(&bytes, &HeaderGroupSpec).expect("decode");
    let entries = decoded.header.group(128).expect("header group present");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].get(115).unwrap().as_str().unwrap(), "A");
    assert_eq!(entries[1].get(115).unwrap().as_str().unwrap(), "B");
}

#[test]
fn a_header_repeating_group_round_trips_byte_identically() {
    let bytes = encode(&msg_with_header_group());
    let decoded = decode_with_groups(&bytes, &HeaderGroupSpec).expect("decode");
    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn without_a_matching_group_spec_the_same_bytes_decode_as_flat_fields() {
    // Confirms the "dormant until declared" framing: the identical wire bytes, decoded against a
    // spec that declares no groups at all, produce flat repeated fields instead of a group.
    struct NoGroups;
    impl GroupSpec for NoGroups {
        fn group_of(&self, _count_tag: u32) -> Option<(u32, &[u32])> {
            None
        }
    }
    let bytes = encode(&msg_with_header_group());
    let decoded = decode_with_groups(&bytes, &NoGroups).expect("decode");
    assert!(decoded.header.group(128).is_none());
    // The count field (128=2) and both delimiter-tag fields land as plain header fields instead.
    assert_eq!(decoded.header.get(128).unwrap().as_str().unwrap(), "2");
}
