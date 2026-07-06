//! T083 — cyclic repeating-group definitions are rejected before grouped decode can recurse.

use truefix_core::{DecodeError, GroupSpec, decode_with_groups};
use truefix_dict::parse;

const CYCLIC_DICTIONARY: &str = "\
version FIX.TEST
field 1000 NoOuter NUMINGROUP
field 1001 OuterEntry STRING
field 2000 NoInner NUMINGROUP
field 2001 InnerEntry STRING
message Z Cyclic opt:1000
group 1000 Outer 1001 1001,2000
group 2000 Inner 2001 2001,1000
";

#[test]
fn cyclic_group_data_is_rejected_at_a_bounded_depth() {
    // This is the shape of body data that used to drive `build_group` through the cycle. Repeating
    // the two group count tags keeps increasing recursion depth while remaining far below the
    // decoder's body-size limit; grouped decode must therefore enforce its own depth bound.
    let dictionary = parse(CYCLIC_DICTIONARY)
        .expect("recursive group definitions used by real dictionaries must remain loadable");
    let mut crafted_body = String::from("35=Z\u{1}");
    for _ in 0..128 {
        crafted_body.push_str("1000=1\u{1}1001=A\u{1}2000=1\u{1}2001=B\u{1}");
    }
    assert!(
        crafted_body.len() < truefix_core::MAX_BODY_LEN,
        "the recursion-driving body remains within the accepted message-size bound"
    );

    let mut wire =
        format!("8=FIX.TEST\u{1}9={}\u{1}{crafted_body}", crafted_body.len()).into_bytes();
    let checksum = wire.iter().map(|byte| u32::from(*byte)).sum::<u32>() % 256;
    wire.extend_from_slice(format!("10={checksum:03}\u{1}").as_bytes());

    assert_eq!(
        decode_with_groups(&wire, &dictionary),
        Err(DecodeError::GroupNestingTooDeep { max: 32 })
    );
}

#[test]
fn acyclic_nested_groups_remain_valid() {
    let dictionary = CYCLIC_DICTIONARY.replace("2001,1000", "2001");
    let parsed = parse(&dictionary).expect("acyclic nested groups should parse");

    assert_eq!(parsed.group_of(1000), Some((1001, [1001, 2000].as_slice())));
    assert_eq!(parsed.group_of(2000), Some((2001, [2001].as_slice())));
}
