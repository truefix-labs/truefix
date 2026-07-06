use truefix_dict::{FixtDictionaries, load_fix50, load_fixt11};

#[test]
fn empty_appl_ver_id_uses_the_session_default() {
    let dictionaries = FixtDictionaries::new(load_fixt11().unwrap())
        .with_application("FIX.5.0", load_fix50().unwrap())
        .with_default_appl_ver_id("FIX.5.0");

    let application = dictionaries
        .application_for(Some(""))
        .expect("an empty per-message ApplVerID should use DefaultApplVerID");

    assert_eq!(application.version(), "FIX.5.0");
}
