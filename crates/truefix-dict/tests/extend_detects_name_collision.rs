//! T153 (NEW-53) — `DataDictionary::extend()` detects a `field_by_name` collision (a different
//! tag reusing an already-defined field's name) and recomputes `member_tags` after a merge that
//! newly makes a group available.

use truefix_dict::{DictMergeConflict, parse};

const BASE: &str = "\
version FIX.4.4
header 8 9 35 49 56 34 52
trailer 10
field 8 BeginString STRING
field 9 BodyLength LENGTH
field 35 MsgType STRING
field 34 MsgSeqNum SEQNUM
field 49 SenderCompID STRING
field 56 TargetCompID STRING
field 52 SendingTime UTCTIMESTAMP
field 10 CheckSum STRING
field 11 ClOrdID STRING
field 9010 NoXYZ NUMINGROUP
field 9011 XYZTag STRING
message D NewOrderSingle req:11,9010
";

#[test]
fn field_name_collision_with_a_different_tag_is_a_typed_error_and_aborts_the_merge() {
    let mut base = parse(BASE).unwrap();
    let before_count = base.field_count();
    // Tag 9012 reuses the name "ClOrdID", already bound to tag 11 in `base`.
    let colliding =
        parse("version FIX.4.4\nheader 8 9 35\ntrailer 10\nfield 9012 ClOrdID STRING\n").unwrap();

    let err = base.extend(&colliding).unwrap_err();
    assert_eq!(
        err,
        DictMergeConflict {
            kind: "field name",
            key: "ClOrdID".to_owned(),
        }
    );
    // Aborted: self is completely unmodified.
    assert_eq!(base.field_count(), before_count);
    assert_eq!(base.field_by_name("ClOrdID"), Some(11));
}

#[test]
fn merging_in_a_group_definition_updates_an_existing_message_s_member_tags() {
    let mut base = parse(BASE).unwrap();
    // `base` requires tag 9010 on message D but never declared `group 9010 ...`, so 9010's own
    // member (9011) isn't yet recognized as belonging to message D.
    assert!(!base.message("D").unwrap().member_tags.contains(&9011));

    let ext = parse(
        "version FIX.4.4\nheader 8 9 35\ntrailer 10\n\
         field 8 BeginString STRING\nfield 9 BodyLength LENGTH\nfield 35 MsgType STRING\n\
         field 10 CheckSum STRING\n\
         field 9010 NoXYZ NUMINGROUP\nfield 9011 XYZTag STRING\n\
         group 9010 NoXYZ 9011 9011\n",
    )
    .unwrap();

    base.extend(&ext).unwrap();

    // `base`'s own "D" message definition (untouched by the merge itself) now recognizes 9011 as
    // a member, since group 9010 became available through the merge.
    assert!(base.message("D").unwrap().member_tags.contains(&9011));
}
