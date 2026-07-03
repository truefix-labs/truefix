//! T068 (US9, feature 005) — every QuickFIX-XML-sourced bundled dictionary stays in sync with
//! its source (GAP-33/FR-031): regenerating each `dict-src/normalized/FIX*.fixdict` from
//! `thrdpty/quickfix/spec/FIX*.xml` via the CLI's `generate-dict --format qfj` must reproduce the
//! shipped file byte-for-byte. A stronger, continuously-enforced guarantee than a one-time
//! coverage report — this is the regression test that would fail immediately if a future edit
//! silently reintroduced a thin/hand-picked field subset, mirroring `cli.rs`'s existing
//! `generate_dict_converts_the_bundled_orchestra_fixture` pattern for the Orchestra track.
#![cfg(feature = "dict-tooling")]

use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_truefix-dict"))
}

fn scratch_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!(
        "truefix-dict-coverage-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("spawn truefix-dict")
}

/// `(source XML basename, shipped .fixdict basename, target version string)`.
const VERSIONS: &[(&str, &str, &str)] = &[
    ("FIX40", "FIX40", "FIX.4.0"),
    ("FIX41", "FIX41", "FIX.4.1"),
    ("FIX42", "FIX42", "FIX.4.2"),
    ("FIX43", "FIX43", "FIX.4.3"),
    ("FIX44", "FIX44", "FIX.4.4"),
    ("FIX50", "FIX50", "FIX.5.0"),
    ("FIX50SP1", "FIX50SP1", "FIX.5.0SP1"),
    ("FIX50SP2", "FIX50SP2", "FIX.5.0SP2"),
    ("FIXT11", "FIXT11", "FIXT.1.1"),
];

#[test]
fn every_qfj_sourced_bundled_dictionary_matches_its_regenerated_source() {
    let dir = scratch_dir();
    for (xml_name, fixdict_name, version) in VERSIONS {
        let source = format!("../../thrdpty/quickfix/spec/{xml_name}.xml");
        let out_path = dir.join(format!("{fixdict_name}.fixdict"));
        let out = run(&[
            "generate-dict",
            "--format",
            "qfj",
            "--source",
            &source,
            "--version",
            version,
            "--out",
            out_path.to_str().unwrap(),
        ]);
        assert!(
            out.status.success(),
            "{xml_name}: stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let generated = std::fs::read_to_string(&out_path).unwrap();
        let shipped =
            std::fs::read_to_string(format!("dict-src/normalized/{fixdict_name}.fixdict")).unwrap();
        assert_eq!(
            generated, shipped,
            "{xml_name}: regenerating from the source XML should reproduce the shipped \
             .fixdict byte-for-byte — a mismatch means the bundled file has drifted from its \
             QuickFIX XML source (or the source itself changed) without being regenerated"
        );
    }
}

/// A weaker but standalone sanity check, independent of the shipped-file comparison above: every
/// regenerated dictionary parses and has a field/message count in the same order of magnitude as
/// QuickFIX/J's own real bundled dictionaries (GAP-33) — not the handful of hand-picked fields
/// the pre-US9 bundled dictionaries shipped with.
#[test]
fn regenerated_dictionaries_have_real_qfj_scale_coverage() {
    let dir = scratch_dir();
    // (xml basename, version, minimum expected field count, minimum expected message count) —
    // thresholds set comfortably below each version's actual QFJ count, just enough to catch a
    // regression back to a thin subset (the smallest bundled version, FIX40, has 139
    // fields/27 messages; every later version only grows from there).
    let expectations: &[(&str, &str, usize, usize)] = &[
        ("FIX40", "FIX.4.0", 100, 20),
        ("FIX50SP2", "FIX.5.0SP2", 1500, 100),
    ];
    for (xml_name, version, min_fields, min_messages) in expectations {
        let source = format!("../../thrdpty/quickfix/spec/{xml_name}.xml");
        let out_path = dir.join(format!("{xml_name}.fixdict"));
        let out = run(&[
            "generate-dict",
            "--format",
            "qfj",
            "--source",
            &source,
            "--version",
            version,
            "--out",
            out_path.to_str().unwrap(),
        ]);
        assert!(out.status.success());
        let text = std::fs::read_to_string(&out_path).unwrap();
        let dict = truefix_dict::parse(&text).unwrap();
        assert!(
            dict.field_count() >= *min_fields,
            "{xml_name}: expected >= {min_fields} fields, got {}",
            dict.field_count()
        );
        assert!(
            dict.message_count() >= *min_messages,
            "{xml_name}: expected >= {min_messages} messages, got {}",
            dict.message_count()
        );
    }
}
