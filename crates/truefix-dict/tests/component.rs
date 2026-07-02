//! T030 (US5) — the `component` directive: flat, nested, and cycle-detection cases (FR-009).

use truefix_dict::{parse, ParseError};

const HEADER_TRAILER: &str = "\
header 8 9 35 49 56 34 52
trailer 10
";

#[test]
fn flat_component_is_parsed_and_expands_into_a_referencing_message() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 448 PartyID STRING\n\
         field 447 PartyIDSource CHAR\n\
         field 452 PartyRole INT\n\
         field 11 ClOrdID STRING\n\
         component Parties 448,447,452\n\
         message D NewOrderSingle req:11 opt:component:Parties\n"
    );
    let d = parse(&dict).unwrap();
    let comp = d.component("Parties").unwrap();
    assert_eq!(comp.members, vec![448, 447, 452]);
    let mdef = d.message("D").unwrap();
    assert_eq!(mdef.optional, vec![448, 447, 452]);
}

#[test]
fn nested_component_expands_transitively() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 448 PartyID STRING\n\
         field 447 PartyIDSource CHAR\n\
         field 1 Account STRING\n\
         field 11 ClOrdID STRING\n\
         component Party 448,447\n\
         component Parties component:Party,1\n\
         message D NewOrderSingle req:11 opt:component:Parties\n"
    );
    let d = parse(&dict).unwrap();
    let parties = d.component("Parties").unwrap();
    assert_eq!(parties.members, vec![448, 447, 1]);
    let mdef = d.message("D").unwrap();
    assert_eq!(mdef.optional, vec![448, 447, 1]);
}

#[test]
fn component_referencing_a_group_nests_the_group() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 453 NoPartyIDs NUMINGROUP\n\
         field 448 PartyID STRING\n\
         field 447 PartyIDSource CHAR\n\
         field 11 ClOrdID STRING\n\
         group 453 NoPartyIDs 448 448,447\n\
         component Parties 453\n\
         message D NewOrderSingle req:11 opt:component:Parties\n"
    );
    let d = parse(&dict).unwrap();
    let comp = d.component("Parties").unwrap();
    assert_eq!(comp.members, vec![453]);
    // The message's member_tags (used for "tag defined for message type") transitively include
    // the group's own members too, via the existing group-expansion pass.
    let mdef = d.message("D").unwrap();
    assert!(mdef.contains_member(448));
    assert!(mdef.contains_member(447));
}

#[test]
fn direct_self_reference_is_a_cycle_error() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 448 PartyID STRING\n\
         component Loopy component:Loopy,448\n\
         message D NewOrderSingle req: opt:component:Loopy\n"
    );
    assert_eq!(
        parse(&dict).unwrap_err(),
        ParseError::ComponentCycle {
            name: "Loopy".to_owned()
        }
    );
}

#[test]
fn transitive_cycle_is_detected() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 448 PartyID STRING\n\
         component A component:B,448\n\
         component B component:A\n\
         message D NewOrderSingle req: opt:component:A\n"
    );
    let err = parse(&dict).unwrap_err();
    assert!(matches!(err, ParseError::ComponentCycle { .. }));
}

#[test]
fn reference_to_an_undefined_component_is_an_error() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 11 ClOrdID STRING\n\
         message D NewOrderSingle req:11 opt:component:NoSuchComponent\n"
    );
    assert_eq!(
        parse(&dict).unwrap_err(),
        ParseError::UnknownComponent {
            name: "NoSuchComponent".to_owned()
        }
    );
}

#[test]
fn a_component_can_be_referenced_by_two_different_messages() {
    let dict = format!(
        "version FIX.4.4\n{HEADER_TRAILER}\
         field 448 PartyID STRING\n\
         field 447 PartyIDSource CHAR\n\
         field 11 ClOrdID STRING\n\
         field 37 OrderID STRING\n\
         component Parties 448,447\n\
         message D NewOrderSingle req:11 opt:component:Parties\n\
         message 8 ExecutionReport req:37 opt:component:Parties\n"
    );
    let d = parse(&dict).unwrap();
    assert_eq!(d.message("D").unwrap().optional, vec![448, 447]);
    assert_eq!(d.message("8").unwrap().optional, vec![448, 447]);
}
