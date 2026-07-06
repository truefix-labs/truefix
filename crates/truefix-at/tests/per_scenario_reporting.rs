use truefix_at::runner::{ScenarioResult, per_scenario_report};

fn result(name: &str, version: &str, outcome: Result<(), String>) -> ScenarioResult {
    ScenarioResult {
        name: name.to_owned(),
        version: version.to_owned(),
        outcome,
    }
}

#[test]
fn per_scenario_report_lists_every_run_individually_not_one_boolean() {
    let results = vec![
        result("2a_MsgSeqNumCorrect", "FIX.4.4", Ok(())),
        result("2b_MsgSeqNumTooHigh", "FIX.4.4", Err("boom".to_owned())),
        result("2a_MsgSeqNumCorrect", "FIX.4.2", Ok(())),
    ];
    let report = per_scenario_report(&results);
    let lines: Vec<&str> = report.lines().collect();

    // Every run gets its own line — a single overall pass/fail boolean would collapse these 3
    // results down to just "failed", losing which ones actually passed.
    assert_eq!(
        lines.len(),
        3,
        "expected one report line per scenario run: {report}"
    );
    assert!(
        lines[0].starts_with("PASS")
            && lines[0].contains("2a_MsgSeqNumCorrect")
            && lines[0].contains("FIX.4.4"),
        "line 0 should report the first pass: {report}"
    );
    assert!(
        lines[1].starts_with("FAIL")
            && lines[1].contains("2b_MsgSeqNumTooHigh")
            && lines[1].contains("boom"),
        "line 1 should report the failure with its reason: {report}"
    );
    assert!(
        lines[2].starts_with("PASS") && lines[2].contains("FIX.4.2"),
        "line 2 should report the second pass, distinguished by version: {report}"
    );
}

#[test]
fn per_scenario_report_of_all_passing_results_has_no_fail_lines() {
    let results = vec![
        result("a", "FIX.4.4", Ok(())),
        result("b", "FIX.4.4", Ok(())),
    ];
    let report = per_scenario_report(&results);
    assert!(!report.contains("FAIL"), "expected no FAIL lines: {report}");
}
