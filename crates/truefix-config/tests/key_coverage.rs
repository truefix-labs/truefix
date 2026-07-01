//! T092 / SC-004 — every Appendix A config key has a known stance (none silently unrecognized).

use std::collections::HashSet;

use truefix_config::{key_info, Stance, APPENDIX_A_KEYS};

#[test]
fn appendix_a_is_covered() {
    // The spec's Appendix A enumerates ~150 keys; the registry must cover them all.
    assert!(
        APPENDIX_A_KEYS.len() >= 140,
        "expected the full Appendix A baseline (~150 keys), got {}",
        APPENDIX_A_KEYS.len()
    );
}

#[test]
fn no_duplicate_keys() {
    let mut seen = HashSet::new();
    for k in APPENDIX_A_KEYS {
        assert!(seen.insert(k.name), "duplicate key {}", k.name);
    }
}

#[test]
fn every_key_has_a_known_stance() {
    // SC-004: each key is implemented, recognized, or documented-unsupported-with-reason.
    for k in APPENDIX_A_KEYS {
        match k.stance {
            Stance::Implemented | Stance::Recognized => {}
            Stance::Unsupported(reason) => {
                assert!(
                    !reason.is_empty(),
                    "{} unsupported without a reason",
                    k.name
                );
            }
        }
    }
}

#[test]
fn key_lookup_works() {
    assert_eq!(
        key_info("HeartBtInt").map(|k| k.stance),
        Some(Stance::Implemented)
    );
    // Sleepycat keys are documented unsupported-with-reason (T093).
    assert!(matches!(
        key_info("SleepycatDatabaseDir").map(|k| k.stance),
        Some(Stance::Unsupported(_))
    ));
    // An unknown key is not silently accepted as "covered".
    assert!(key_info("NotARealKey").is_none());
}

#[test]
fn core_groups_present() {
    let groups: HashSet<&str> = APPENDIX_A_KEYS.iter().map(|k| k.group).collect();
    for expected in [
        "identity",
        "validation",
        "session",
        "scheduling",
        "acceptor",
        "initiator",
        "socket",
        "ssl",
        "proxy",
        "file-store",
        "file-log",
        "sql",
        "sleepycat",
    ] {
        assert!(groups.contains(expected), "missing group {expected}");
    }
}
