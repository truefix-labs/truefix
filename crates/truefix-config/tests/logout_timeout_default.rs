//! T161 (NEW-89) — `.cfg`-parsing default for `LogoutTimeout` is 2 seconds (matching QuickFIX/J),
//! not the previous 10-second value. Unlike `NEW-73`'s `ReconnectInterval` default (a test-only
//! discrepancy), this is real `.cfg`-driven deployment behavior — flagged prominently here per
//! the task's own instruction.

use truefix_config::SessionSettings;

fn base(extra: &str) -> String {
    format!(
        "[DEFAULT]\nBeginString=FIX.4.4\nConnectionType=acceptor\nSocketAcceptPort=5001\n\
         [SESSION]\nSenderCompID=S\nTargetCompID=T\n{extra}"
    )
}

#[test]
fn logout_timeout_defaults_to_two_seconds_when_unset() {
    let resolved = SessionSettings::parse(&base(""))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolved[0].session.logout_timeout, 2);
}

#[test]
fn logout_timeout_is_still_configurable() {
    let resolved = SessionSettings::parse(&base("LogoutTimeout=5\n"))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolved[0].session.logout_timeout, 5);
}
