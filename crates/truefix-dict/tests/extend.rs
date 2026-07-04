//! T036 (US6) — `DataDictionary::extend` merge semantics (FR-010).

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
message D NewOrderSingle req:11
";

#[test]
fn clean_merge_adds_new_fields_and_messages() {
    let mut base = parse(BASE).unwrap();
    let ext = parse(&format!(
        "{BASE}field 9001 HouseTag STRING\nmessage UE UnknownExtension req:9001\n"
    ))
    .unwrap();
    base.extend(&ext).unwrap();
    assert!(base.field(9001).is_some());
    assert!(base.message("UE").is_some());
    // The original message is untouched.
    assert!(base.message("D").is_some());
}

#[test]
fn extending_with_identical_redefinitions_is_idempotent() {
    let mut base = parse(BASE).unwrap();
    let same = parse(BASE).unwrap();
    // BASE redefines every field/message BASE already has, identically — must be a no-op, not an
    // error.
    base.extend(&same).unwrap();
    assert!(base.message("D").is_some());
    assert_eq!(base.field_count(), same.field_count());
}

#[test]
fn conflicting_field_redefinition_is_a_typed_error_and_aborts_the_merge() {
    let mut base = parse(BASE).unwrap();
    // Tag 11 (ClOrdID/STRING in BASE) redefined with a different type.
    let conflicting =
        parse("version FIX.4.4\nheader 8 9 35\ntrailer 10\nfield 11 ClOrdID INT\n").unwrap();
    let before_count = base.field_count();
    let err = base.extend(&conflicting).unwrap_err();
    assert_eq!(
        err,
        DictMergeConflict {
            kind: "field",
            key: "11".to_owned(),
        }
    );
    // Aborted: self is completely unmodified, even though some earlier keys might otherwise have
    // merged cleanly.
    assert_eq!(base.field_count(), before_count);
}

#[test]
fn conflicting_message_redefinition_is_a_typed_error() {
    let mut base = parse(BASE).unwrap();
    let conflicting = parse(
        "version FIX.4.4\nheader 8 9 35\ntrailer 10\n\
         field 55 Symbol STRING\n\
         message D NewOrderSingle req:55\n", // "D" redefined with different required fields
    )
    .unwrap();
    let err = base.extend(&conflicting).unwrap_err();
    assert_eq!(
        err,
        DictMergeConflict {
            kind: "message",
            key: "D".to_owned(),
        }
    );
}

#[test]
fn header_and_trailer_tags_are_unioned_not_conflict_checked() {
    let mut base = parse(BASE).unwrap();
    let ext = parse(
        "version FIX.4.4\nheader 8 9 35 22\ntrailer 10 93\n\
         field 8 BeginString STRING\nfield 9 BodyLength LENGTH\nfield 35 MsgType STRING\n\
         field 22 SecurityIDSource STRING\nfield 10 CheckSum STRING\nfield 93 SignatureLength LENGTH\n",
    )
    .unwrap();
    base.extend(&ext).unwrap();
    assert!(base.is_header(22));
    assert!(base.is_trailer(93));
    // Original header/trailer membership is preserved too.
    assert!(base.is_header(49));
}

#[test]
fn extend_leaves_the_original_dual_track_hash_unchanged() {
    let mut base = parse(BASE).unwrap();
    let original_hash = base.hash();
    let ext = parse(&format!("{BASE}field 9001 HouseTag STRING\n")).unwrap();
    base.extend(&ext).unwrap();
    assert_eq!(base.hash(), original_hash);
}
