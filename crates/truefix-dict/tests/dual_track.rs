//! T047 — the dual-track invariant: codegen and runtime derive from the same source.

use truefix_dict::{
    fix44_msgs, fix50_msgs, fixt11_msgs, load_fix44, load_fix50, load_fixt11, FIX44_DICT_HASH,
    FIX50_DICT_HASH, FIXT11_DICT_HASH,
};

#[test]
fn codegen_hash_matches_runtime_parsed_hash() {
    // build.rs hashed the dictionary source; the runtime parses the same source. Equality proves
    // the two tracks cannot diverge (Constitution Principle IV).
    assert_eq!(load_fix44().unwrap().hash(), FIX44_DICT_HASH);
    assert_eq!(load_fixt11().unwrap().hash(), FIXT11_DICT_HASH);
    assert_eq!(load_fix50().unwrap().hash(), FIX50_DICT_HASH);
}

#[test]
fn generated_msgtype_constants_present() {
    assert_eq!(fix44_msgs::LOGON, "A");
    assert_eq!(fix44_msgs::HEARTBEAT, "0");
    assert_eq!(fix44_msgs::NEWORDERSINGLE, "D");
    assert_eq!(fixt11_msgs::LOGON, "A");
    assert_eq!(fix50_msgs::NEWORDERSINGLE, "D");
}
