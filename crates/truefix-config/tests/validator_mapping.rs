//! T010 (US2, feature 004) — `UseDataDictionary`/`DataDictionary`/`AppDataDictionary`/
//! `TransportDataDictionary` and the five already-implemented `ValidationOptions` toggles map from
//! a settings file into `ResolvedSession.validator` (FR-002).

use truefix_config::{ResolvedSession, SessionSettings};

fn resolved(cfg: &str) -> ResolvedSession {
    SessionSettings::parse(cfg)
        .unwrap()
        .resolve()
        .unwrap()
        .into_iter()
        .next()
        .unwrap()
}

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn no_use_data_dictionary_means_no_validator() {
    let rs = resolved(&base(""));
    assert!(rs.validator.is_none());
}

#[test]
fn use_data_dictionary_n_means_no_validator() {
    let rs = resolved(&base("UseDataDictionary=N\nDataDictionary=FIX.4.4\n"));
    assert!(rs.validator.is_none());
}

#[test]
fn bundled_version_string_selects_the_bundled_dictionary() {
    let rs = resolved(&base("UseDataDictionary=Y\nDataDictionary=FIX.4.4\n"));
    let (dict, _) = rs.validator.expect("validator wired");
    assert_eq!(dict.version(), "FIX.4.4");
}

#[test]
fn app_data_dictionary_is_a_fallback_for_data_dictionary() {
    let rs = resolved(&base("UseDataDictionary=Y\nAppDataDictionary=FIX.4.2\n"));
    let (dict, _) = rs.validator.expect("validator wired");
    assert_eq!(dict.version(), "FIX.4.2");
}

#[test]
fn transport_data_dictionary_is_a_fallback_for_data_dictionary() {
    let rs = resolved(&base(
        "UseDataDictionary=Y\nTransportDataDictionary=FIXT.1.1\n",
    ));
    let (dict, _) = rs.validator.expect("validator wired");
    assert_eq!(dict.version(), "FIXT.1.1");
}

#[test]
fn a_file_path_loads_a_dictionary_from_disk() {
    let dir =
        std::env::temp_dir().join(format!("truefix-validator-mapping-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("custom.fixdict");
    std::fs::write(&path, truefix_dict::FIX44_DICT).unwrap();

    let rs = resolved(&base(&format!(
        "UseDataDictionary=Y\nDataDictionary={}\n",
        path.display()
    )));
    let (dict, _) = rs.validator.expect("validator wired");
    assert_eq!(dict.version(), "FIX.4.4");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn use_data_dictionary_without_a_dictionary_key_is_a_typed_error() {
    let err = SessionSettings::parse(&base("UseDataDictionary=Y\n"))
        .unwrap()
        .resolve()
        .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::MissingRequired { .. }
    ));
}

#[test]
fn an_unresolvable_dictionary_value_is_a_typed_error() {
    let err = SessionSettings::parse(&base(
        "UseDataDictionary=Y\nDataDictionary=/no/such/path.fixdict\n",
    ))
    .unwrap()
    .resolve()
    .unwrap_err();
    assert!(matches!(
        err,
        truefix_config::ConfigError::InvalidValue { .. }
    ));
}

#[test]
fn default_validation_options_match_the_five_documented_defaults() {
    let rs = resolved(&base("UseDataDictionary=Y\nDataDictionary=FIX.4.4\n"));
    let (_, opts) = rs.validator.expect("validator wired");
    // NEW-104/NEW-111 (audit 006): both defaults changed to match QFJ parity.
    assert!(opts.validate_fields_out_of_order);
    assert!(opts.validate_checksum);
    assert!(opts.validate_incoming_message);
    assert!(!opts.allow_pos_dup);
    assert!(!opts.requires_orig_sending_time);
}

#[test]
fn all_five_validation_option_toggles_map_from_settings() {
    let rs = resolved(&base(
        "UseDataDictionary=Y\nDataDictionary=FIX.4.4\n\
         ValidateFieldsOutOfOrder=Y\nValidateChecksum=N\nValidateIncomingMessage=N\n\
         AllowPosDup=N\nRequiresOrigSendingTime=Y\n",
    ));
    let (_, opts) = rs.validator.expect("validator wired");
    assert!(opts.validate_fields_out_of_order);
    assert!(!opts.validate_checksum);
    assert!(!opts.validate_incoming_message);
    assert!(!opts.allow_pos_dup);
    assert!(opts.requires_orig_sending_time);
}

// --- T036/T037 (US1, feature 009, NEW-10): 5 more validation keys registered as `Impl` but
// previously never wired in resolve_validator ---

#[test]
fn the_five_new_validation_option_toggles_map_from_settings() {
    let rs = resolved(&base(
        "UseDataDictionary=Y\nDataDictionary=FIX.4.4\n\
         ValidateFieldsHaveValues=N\nValidateUnorderedGroupFields=N\n\
         ValidateUserDefinedFields=Y\nAllowUnknownMsgFields=Y\n\
         FirstFieldInGroupIsDelimiter=N\n",
    ));
    let (_, opts) = rs.validator.expect("validator wired");
    assert!(!opts.validate_fields_have_values);
    assert!(!opts.validate_unordered_group_fields);
    assert!(opts.validate_user_defined_fields);
    assert!(opts.allow_unknown_msg_fields);
    assert!(!opts.first_field_in_group_is_delimiter);
}

#[test]
fn the_five_new_validation_option_toggles_default_correctly_when_unset() {
    let rs = resolved(&base("UseDataDictionary=Y\nDataDictionary=FIX.4.4\n"));
    let (_, opts) = rs.validator.expect("validator wired");
    assert!(opts.validate_fields_have_values);
    assert!(opts.validate_unordered_group_fields);
    assert!(!opts.validate_user_defined_fields);
    assert!(!opts.allow_unknown_msg_fields);
    assert!(opts.first_field_in_group_is_delimiter);
}

// --- T079 (US8, feature 006): real FIXT transport/application dictionary split, and the
// transport-priority fix to `.validator`'s single-dict fallback (GAP-18c part 2) ---

#[test]
fn when_both_app_and_transport_dictionaries_are_set_validator_prefers_transport() {
    // Previously `AppDataDictionary` won this priority race, meaning admin/session-layer
    // structural validation (what `.validator` is actually used for) ran against the
    // *application* dictionary under a real FIXT split -- the actual aliasing bug.
    let rs = resolved(&base(
        "UseDataDictionary=Y\nAppDataDictionary=FIX.5.0\nTransportDataDictionary=FIXT.1.1\n",
    ));
    let (dict, _) = rs.validator.expect("validator wired");
    assert_eq!(dict.version(), "FIXT.1.1");
}

#[test]
fn only_one_of_app_or_transport_dictionary_produces_no_fixt_dictionaries() {
    let rs = resolved(&base("UseDataDictionary=Y\nAppDataDictionary=FIX.4.2\n"));
    assert!(rs.fixt_dictionaries.is_none());
}

#[test]
fn both_app_and_transport_dictionaries_produce_a_real_fixt_dictionaries() {
    let rs = resolved(&base(
        "UseDataDictionary=Y\nAppDataDictionary=FIX.5.0\nTransportDataDictionary=FIXT.1.1\n\
         DefaultApplVerID=FIX.5.0\n",
    ));
    let dicts = rs.fixt_dictionaries.expect("fixt_dictionaries wired");
    assert_eq!(dicts.transport().version(), "FIXT.1.1");
    assert_eq!(
        dicts.application_for(Some("FIX.5.0")).unwrap().version(),
        "FIX.5.0"
    );
    // DefaultApplVerID also drives the no-explicit-ApplVerID fallback.
    assert_eq!(dicts.application_for(None).unwrap().version(), "FIX.5.0");
}

#[test]
fn default_appl_ver_id_absent_falls_back_to_the_app_dictionarys_own_version_string() {
    let rs = resolved(&base(
        "UseDataDictionary=Y\nAppDataDictionary=FIX.5.0\nTransportDataDictionary=FIXT.1.1\n",
    ));
    let dicts = rs.fixt_dictionaries.expect("fixt_dictionaries wired");
    assert_eq!(dicts.application_for(None).unwrap().version(), "FIX.5.0");
}
