//! T071/T072 (US2, feature 009, `NEW-26`): initiator hostnames survive configuration
//! resolution unchanged. DNS belongs to each transport connection attempt, not config loading.

use truefix_config::SessionSettings;

#[test]
fn initiator_hostname_is_preserved_without_dns_lookup_during_config_resolution() {
    let cfg = "[DEFAULT]\n\
               BeginString=FIX.4.4\n\
               ConnectionType=initiator\n\
               SocketConnectHost=changes-between-connects.invalid\n\
               SocketConnectPort=5001\n\
               [SESSION]\n\
               SenderCompID=S\n\
               TargetCompID=T\n";

    let session = SessionSettings::parse(cfg)
        .expect("parse")
        .resolve()
        .expect("config resolution must not perform DNS")
        .pop()
        .expect("one session");

    assert_eq!(session.address.host(), "changes-between-connects.invalid");
    assert_eq!(session.address.port(), 5001);
}
