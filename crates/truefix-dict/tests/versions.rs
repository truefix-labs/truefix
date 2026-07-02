//! T073 / T076 — per-version dictionaries: parse, validate, and version differences.

use truefix_core::{Field, Message};
use truefix_dict::{
    load_fix40, load_fix41, load_fix42, load_fix43, load_fix44, load_fix50, load_fix50sp1,
    load_fix50sp2, load_fixlatest, load_fixt11, parse, DataDictionary, ParseError,
    ValidationOptions, ALL_DICTS,
};

type Loader = fn() -> Result<DataDictionary, ParseError>;

fn logon(begin: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(8, begin));
    m.header.set(Field::string(35, "A"));
    m.header.set(Field::string(49, "S"));
    m.header.set(Field::string(56, "T"));
    m.header.set(Field::string(52, "20240101-00:00:00"));
    m.body.set(Field::int(98, 0));
    m.body.set(Field::int(108, 30));
    m.trailer.set(Field::string(10, "000"));
    m
}

#[test]
fn all_bundled_dictionaries_parse() {
    assert_eq!(ALL_DICTS.len(), 10);
    for (ver, src) in ALL_DICTS {
        let d = parse(src).unwrap();
        assert_eq!(&d.version(), ver, "version line");
        assert!(d.field_count() > 0, "{ver} has fields");
        assert!(d.message_count() > 0, "{ver} has messages");
    }
}

#[test]
fn fix4x_versions_validate_a_logon() {
    let loaders: [Loader; 6] = [
        load_fix40,
        load_fix41,
        load_fix42,
        load_fix43,
        load_fix44,
        load_fixlatest,
    ];
    for loader in loaders {
        let d = loader().unwrap();
        let m = logon(d.version());
        assert!(
            d.validate(&m, &ValidationOptions::default()).is_ok(),
            "logon should validate for {}",
            d.version()
        );
    }
}

#[test]
fn fixt_logon_requires_default_appl_ver_id() {
    let d = load_fixt11().unwrap();
    // Without DefaultApplVerID(1137) -> RequiredTagMissing.
    assert!(d
        .validate(&logon("FIXT.1.1"), &ValidationOptions::default())
        .is_err());
    // With it -> valid.
    let mut m = logon("FIXT.1.1");
    m.body.set(Field::string(1137, "9"));
    assert!(d.validate(&m, &ValidationOptions::default()).is_ok());
}

#[test]
fn application_versions_define_newordersingle() {
    let loaders: [Loader; 3] = [load_fix50, load_fix50sp1, load_fix50sp2];
    for loader in loaders {
        let d = loader().unwrap();
        let nos = d.message("D").expect("NewOrderSingle present");
        assert!(nos.required.contains(&11)); // ClOrdID
        assert!(d.field(55).is_some()); // Symbol
    }
}

#[test]
fn version_specific_differences() {
    let f40 = load_fix40().unwrap();
    let f44 = load_fix44().unwrap();
    // FIX 4.4 NewOrderSingle requires more fields (e.g. HandlInst, TransactTime) than 4.0.
    assert!(
        f44.message("D").unwrap().required.len() > f40.message("D").unwrap().required.len(),
        "4.4 NewOrderSingle should require more fields than 4.0"
    );

    // FIXT 1.1 defines DefaultApplVerID (1137); FIX 4.4 does not.
    let fixt = load_fixt11().unwrap();
    assert!(fixt.field(1137).is_some());
    assert!(f44.field(1137).is_none());
}

/// US9 (FR-012) — the tenth dictionary, sourced from FIX Orchestra, exercises the field-type
/// (US7) and component-model (US5) work added earlier in this feature end-to-end.
#[test]
fn fixlatest_wires_component_group_and_new_field_types_together() {
    let d = load_fixlatest().unwrap();
    assert_eq!(d.version(), "FIX.Latest");
    assert!(d.component("Parties").is_some());
    assert!(d.is_group_count(453)); // NoPartyIDs

    let nos = d.message("D").expect("NewOrderSingle present");
    assert!(
        nos.contains_member(448),
        "Parties component's group member (PartyID) reachable"
    );
    assert!(nos.allows_tag(75)); // TradeDate, UtcDateOnly
    assert!(nos.allows_tag(96)); // RawData, Data

    assert_eq!(
        d.field(75).unwrap().field_type,
        truefix_dict::FieldType::UtcDateOnly
    );
    assert_eq!(
        d.field(96).unwrap().field_type,
        truefix_dict::FieldType::Data
    );
}
