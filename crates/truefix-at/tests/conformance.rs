//! T088 — run the AT suite across all targeted versions; every in-scope scenario must pass.

use truefix_at::run_report;
use truefix_at::scenarios::server_suite;

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
