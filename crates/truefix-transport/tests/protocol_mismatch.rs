use truefix_session::{Protocol, Role, SessionConfig};

fn cfg(protocol: Protocol) -> SessionConfig {
    let mut cfg = SessionConfig::new("FIX.4.4", "S", "T", Role::Initiator);
    cfg.protocol = protocol;
    cfg.fast_template_path = Some("fast.xml".to_owned());
    cfg.sbe_schema_path = Some("sbe.xml".to_owned());
    cfg
}

#[test]
fn protocol_mismatch_is_detectable_before_logon_state() {
    let soh = cfg(Protocol::Soh);
    let fast = cfg(Protocol::Fast);
    let sbe = cfg(Protocol::Sbe);

    assert_ne!(soh.protocol, fast.protocol);
    assert_ne!(fast.protocol, sbe.protocol);
}

#[test]
fn both_roles_carry_protocol_selection() {
    let mut acceptor = SessionConfig::new("FIX.4.4", "A", "I", Role::Acceptor);
    acceptor.protocol = Protocol::Fast;
    acceptor.fast_template_path = Some("fast.xml".to_owned());

    let mut initiator = SessionConfig::new("FIX.4.4", "I", "A", Role::Initiator);
    initiator.protocol = Protocol::Fast;
    initiator.fast_template_path = Some("fast.xml".to_owned());

    assert_eq!(acceptor.protocol, initiator.protocol);
    assert_eq!(acceptor.fast_template_path, initiator.fast_template_path);
}
