//! T068 (US9, feature 005) — the FIX Trading Community "Unified Repository" conversion pipeline
//! (`fix_repository.rs`, CLI `generate-dict --format fix-repository`) parses every vendored
//! version cleanly and produces real QuickFIX/J-scale field/message coverage (GAP-33/FR-031).
//!
//! This crate's *shipped* `dict-src/normalized/*.fixdict` files were originally generated from
//! `thrdpty/quickfix`'s bundled XML via the since-removed `qfj_xml` module — a "QuickFIX Software
//! License"-encumbered private data file, not redistributable and hence unusable as a CI input
//! (see `dict-src/fix-repository/PROVENANCE.md`). The Apache-2.0-licensed FIX Repository source
//! vendored under `dict-src/fix-repository/` uses different (functionally equivalent) enum-label
//! casing and, for a handful of messages, different canonical names than the shipped files (e.g.
//! FIX.4.0's `NewOrderSingle` is named `OrderSingle` in the official Repository) — those names
//! feed codegen identifiers, so regenerating and re-shipping from this source is tracked as
//! separate follow-up work, not done here. This suite therefore exercises the *pipeline*
//! (parses, right order-of-magnitude scale) rather than asserting byte-identity against the
//! shipped files.
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

/// `(fix-repository source dir name, target version string)` for every vendored version.
const VERSIONS: &[(&str, &str)] = &[
    ("FIX.4.0", "FIX.4.0"),
    ("FIX.4.1", "FIX.4.1"),
    ("FIX.4.2", "FIX.4.2"),
    ("FIX.4.3", "FIX.4.3"),
    ("FIX.4.4", "FIX.4.4"),
    ("FIX.5.0", "FIX.5.0"),
    ("FIX.5.0SP1", "FIX.5.0SP1"),
    ("FIX.5.0SP2", "FIX.5.0SP2"),
    ("FIXT.1.1", "FIXT.1.1"),
];

#[test]
fn every_vendored_fix_repository_source_converts_cleanly() {
    let dir = scratch_dir();
    for (source_dir, version) in VERSIONS {
        let source = format!("dict-src/fix-repository/{source_dir}");
        let out_path = dir.join(format!("{source_dir}.fixdict"));
        let out = run(&[
            "generate-dict",
            "--format",
            "fix-repository",
            "--source",
            &source,
            "--version",
            version,
            "--out",
            out_path.to_str().unwrap(),
        ]);
        assert!(
            out.status.success(),
            "{source_dir}: stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = std::fs::read_to_string(&out_path).unwrap();
        let dict = truefix_dict::parse(&text).unwrap();
        assert_eq!(dict.version(), *version);
    }
}

/// A standalone sanity check: every regenerated dictionary has a field/message count in the same
/// order of magnitude as QuickFIX/J's own real bundled dictionaries (GAP-33) — not the handful of
/// hand-picked fields the pre-US9 bundled dictionaries shipped with.
#[test]
fn regenerated_dictionaries_have_real_qfj_scale_coverage() {
    let dir = scratch_dir();
    // (source dir name, version, minimum expected field count, minimum expected message count) —
    // thresholds set comfortably below each version's actual scale (FIX.4.0: 138 fields/27
    // messages; FIX.5.0SP2: 1452 fields/116 messages in the vendored "Base edition" Repository
    // data), just enough to catch a regression back to a thin, hand-picked subset.
    let expectations: &[(&str, &str, usize, usize)] = &[
        ("FIX.4.0", "FIX.4.0", 100, 20),
        ("FIX.5.0SP2", "FIX.5.0SP2", 1400, 100),
    ];
    for (source_dir, version, min_fields, min_messages) in expectations {
        let source = format!("dict-src/fix-repository/{source_dir}");
        let out_path = dir.join(format!("{source_dir}.fixdict"));
        let out = run(&[
            "generate-dict",
            "--format",
            "fix-repository",
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
            "{source_dir}: expected >= {min_fields} fields, got {}",
            dict.field_count()
        );
        assert!(
            dict.message_count() >= *min_messages,
            "{source_dir}: expected >= {min_messages} messages, got {}",
            dict.message_count()
        );
    }
}
