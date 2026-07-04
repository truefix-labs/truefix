//! T046 — FIXT 1.1 transport/application split + DefaultApplVerID resolution.

use truefix_dict::{FixtDictionaries, load_fix50, load_fixt11};

#[test]
fn transport_and_application_dictionaries_are_separate() {
    let dicts = FixtDictionaries::new(load_fixt11().unwrap())
        .with_application("FIX.5.0", load_fix50().unwrap())
        .with_default_appl_ver_id("FIX.5.0");

    // Transport dictionary carries the session layer (Logon lives here).
    assert_eq!(dicts.transport().version(), "FIXT.1.1");
    assert!(dicts.transport().message("A").is_some());
    // Application messages (NewOrderSingle) are NOT in the transport dictionary.
    assert!(dicts.transport().message("D").is_none());
}

#[test]
fn default_appl_ver_id_resolves_application_dictionary() {
    let dicts = FixtDictionaries::new(load_fixt11().unwrap())
        .with_application("FIX.5.0", load_fix50().unwrap())
        .with_default_appl_ver_id("FIX.5.0");

    // No explicit ApplVerID -> falls back to DefaultApplVerID.
    let app = dicts.application_for(None).unwrap();
    assert_eq!(app.version(), "FIX.5.0");
    assert!(app.message("D").is_some());

    // Explicit ApplVerID.
    assert_eq!(
        dicts.application_for(Some("FIX.5.0")).unwrap().version(),
        "FIX.5.0"
    );
    // Unknown application version.
    assert!(dicts.application_for(Some("FIX.4.2")).is_none());
}

#[test]
fn no_default_and_no_explicit_yields_none() {
    let dicts = FixtDictionaries::new(load_fixt11().unwrap())
        .with_application("FIX.5.0", load_fix50().unwrap());
    assert!(dicts.application_for(None).is_none());
}
