//! T056 (US1 closeout) — AT suite-completeness regression floor (SC-001).
//!
//! `docs/todo-gap-analysis.md`'s TODO-01 is the authoritative, item-by-item record of exactly
//! which of QuickFIX/J's published scenarios are covered vs. explicitly deferred (with reasons —
//! e.g. harness limitations, architectural mismatches, or ambiguous reference semantics this
//! project won't guess at without copying QFJ source, per Principle III). This file does not
//! re-derive that enumeration; it's a CI-enforced regression floor so a future change can't
//! silently shrink the corpus (drop a scenario, a version, or a special suite) without a test
//! failure calling it out.

use std::collections::BTreeSet;

use truefix_at::scenarios::{
    resynch_suite, server_suite, timestamps_suite, validate_checksum_suite, SUITE_VERSIONS,
};

#[test]
fn suite_versions_cover_all_nine_targeted_fix_versions() {
    assert_eq!(
        SUITE_VERSIONS.len(),
        9,
        "the targeted-version matrix (spec Assumptions) is 9 versions"
    );
    assert!(
        SUITE_VERSIONS.contains(&"FIX.Latest"),
        "US9's FIX Latest must be in the matrix"
    );
    let unique: BTreeSet<_> = SUITE_VERSIONS.iter().collect();
    assert_eq!(
        unique.len(),
        SUITE_VERSIONS.len(),
        "SUITE_VERSIONS must not contain duplicates"
    );
}

#[test]
fn server_suite_scenario_run_count_does_not_regress() {
    let scenarios = server_suite();
    let total_runs: usize = scenarios.iter().map(|s| s.versions.len()).sum();
    assert!(
        total_runs >= 353,
        "server_suite() produced {total_runs} scenario runs, below the 353-run floor established \
         at US1 closeout (003) — a drop usually means a scenario or version was accidentally \
         dropped rather than intentionally deferred (deferrals are tracked in \
         docs/todo-gap-analysis.md's TODO-01, not by shrinking this suite)"
    );
}

#[test]
fn all_three_special_category_suites_are_non_empty_and_have_distinct_scenario_names() {
    let vc = validate_checksum_suite();
    let ts = timestamps_suite();
    let rs = resynch_suite();
    assert!(!vc.is_empty(), "validateChecksum suite must be non-empty");
    assert!(!ts.is_empty(), "timestamps suite must be non-empty");
    assert!(!rs.is_empty(), "resynch suite must be non-empty");

    let names: BTreeSet<_> = vc
        .iter()
        .chain(&ts)
        .chain(&rs)
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(
        names.len(),
        vc.len() + ts.len() + rs.len(),
        "the three special-category suites should not silently reuse each other's scenario names"
    );
}
