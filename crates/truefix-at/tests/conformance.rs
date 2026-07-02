//! T088 — run the AT suite across all targeted versions; every in-scope scenario must pass.

use truefix_at::run_report;
use truefix_at::scenarios::{
    resynch_suite, server_suite, timestamps_suite, validate_checksum_suite,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_acceptance_suite_passes() {
    let scenarios = server_suite();
    assert!(!scenarios.is_empty());

    let results = run_report(&scenarios).await;
    assert!(!results.is_empty(), "no AT results produced");

    let failures: Vec<_> = results.iter().filter(|r| r.outcome.is_err()).collect();
    for f in &failures {
        eprintln!("AT FAIL  {} [{}]: {:?}", f.name, f.version, f.outcome);
    }
    let passed = results.len() - failures.len();
    eprintln!("AT report: {passed}/{} scenario runs passed", results.len());
    assert!(
        failures.is_empty(),
        "{} AT scenario run(s) failed",
        failures.len()
    );
}

/// T022/US1 — the `validateChecksum` special-category suite (its own AT gate, distinct from the
/// server suite, per the audit's "73 scenarios + 3 special suites" framing).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_checksum_suite_passes() {
    let scenarios = validate_checksum_suite();
    assert!(!scenarios.is_empty());
    let results = run_report(&scenarios).await;
    let failures: Vec<_> = results.iter().filter(|r| r.outcome.is_err()).collect();
    for f in &failures {
        eprintln!("AT FAIL  {} [{}]: {:?}", f.name, f.version, f.outcome);
    }
    assert!(
        failures.is_empty(),
        "{} validateChecksum scenario run(s) failed",
        failures.len()
    );
}

/// T054/US1 — the `timestamps` special-category suite.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timestamps_suite_passes() {
    let scenarios = timestamps_suite();
    assert!(!scenarios.is_empty());
    let results = run_report(&scenarios).await;
    let failures: Vec<_> = results.iter().filter(|r| r.outcome.is_err()).collect();
    for f in &failures {
        eprintln!("AT FAIL  {} [{}]: {:?}", f.name, f.version, f.outcome);
    }
    assert!(
        failures.is_empty(),
        "{} timestamps scenario run(s) failed",
        failures.len()
    );
}

/// T054/US1 — the `resynch` special-category suite.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resynch_suite_passes() {
    let scenarios = resynch_suite();
    assert!(!scenarios.is_empty());
    let results = run_report(&scenarios).await;
    let failures: Vec<_> = results.iter().filter(|r| r.outcome.is_err()).collect();
    for f in &failures {
        eprintln!("AT FAIL  {} [{}]: {:?}", f.name, f.version, f.outcome);
    }
    assert!(
        failures.is_empty(),
        "{} resynch scenario run(s) failed",
        failures.len()
    );
}
