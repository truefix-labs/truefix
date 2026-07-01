//! T037 (US11) — reverse-routing of OnBehalfOf/DeliverTo header pairs (Appendix B `ReverseRoute`).

use truefix_core::{Field, Message};

fn original_with_routing() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(115, "BROKER")); // OnBehalfOfCompID
    m.header.set(Field::string(116, "SUB1")); // OnBehalfOfSubID
    m.header.set(Field::string(144, "LOC1")); // OnBehalfOfLocationID
    m
}

#[test]
fn routing_fields_are_reversed_onto_the_reply() {
    let original = original_with_routing();
    let mut reply = Message::new();
    reply.reverse_route(&original);

    assert_eq!(reply.header.get(128).unwrap().as_str().unwrap(), "BROKER"); // DeliverToCompID
    assert_eq!(reply.header.get(129).unwrap().as_str().unwrap(), "SUB1"); // DeliverToSubID
    assert_eq!(reply.header.get(145).unwrap().as_str().unwrap(), "LOC1"); // DeliverToLocationID
                                                                          // No OnBehalfOf* fields should appear on the reply (only their DeliverTo counterparts).
    assert!(reply.header.get(115).is_none());
}

#[test]
fn empty_routing_tag_still_reverses() {
    let mut original = Message::new();
    original.header.set(Field::string(115, "")); // present but empty
    let mut reply = Message::new();
    reply.reverse_route(&original);
    assert_eq!(reply.header.get(128).unwrap().as_str().unwrap(), "");
}

#[test]
fn absent_routing_tags_are_left_unset() {
    let original = Message::new(); // no routing tags at all
    let mut reply = Message::new();
    reply.reverse_route(&original);
    for tag in [115, 116, 128, 129, 144, 145] {
        assert!(reply.header.get(tag).is_none(), "tag {tag} should be unset");
    }
}
