use truefix_at::runner::{FLAT_DICTIONARY_VERSIONS, dictionary_for_version};

#[test]
fn every_flat_dictionary_version_has_a_bundled_validator() {
    for version in FLAT_DICTIONARY_VERSIONS {
        assert!(
            dictionary_for_version(version).is_some(),
            "expected a validator dictionary for {version}, got none"
        );
    }
}

/// FIX 5.0/SP1/SP2/Latest split admin messages into a separate FIXT 1.1 transport dictionary;
/// wiring one of those application-only dictionaries into the flat `validator` field would make
/// every Logon fail as an unregistered MsgType (see `dictionary_for_version`'s doc comment for the
/// full rationale), so those four remain `None` here.
#[test]
fn fixt_split_versions_have_no_flat_validator() {
    for version in ["FIX.5.0", "FIX.5.0SP1", "FIX.5.0SP2", "FIX.Latest"] {
        assert!(
            dictionary_for_version(version).is_none(),
            "expected no flat validator for {version} (needs FixtDictionaries support)"
        );
    }
}
