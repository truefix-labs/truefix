use truefix_config::{ConfigError, SessionSettings};
use truefix_session::Protocol;

fn resolve_one(cfg: &str) -> Result<truefix_config::ResolvedSession, ConfigError> {
    let settings = SessionSettings::parse(cfg)?;
    let mut sessions = settings.resolve()?;
    Ok(sessions.remove(0))
}

fn base(extra: &str) -> String {
    format!(
        "[SESSION]\nBeginString=FIX.4.4\nSenderCompID=S\nTargetCompID=T\nConnectionType=acceptor\nSocketAcceptPort=5001\n{extra}"
    )
}

#[test]
fn protocol_defaults_to_soh() {
    let rs = resolve_one(&base("")).expect("resolve");
    assert_eq!(rs.session.protocol, Protocol::Soh);
    assert_eq!(rs.session.fast_template_path, None);
    assert_eq!(rs.session.sbe_schema_path, None);
}

#[test]
fn parses_fast_and_sbe_protocol_values_case_insensitively() {
    let fast = resolve_one(&base("Protocol=fast\nFastTemplatePath=fast.xml\n")).expect("fast");
    assert_eq!(fast.session.protocol, Protocol::Fast);
    assert_eq!(fast.session.fast_template_path.as_deref(), Some("fast.xml"));

    let sbe = resolve_one(&base("Protocol=SBE\nSbeSchemaPath=sbe.xml\n")).expect("sbe");
    assert_eq!(sbe.session.protocol, Protocol::Sbe);
    assert_eq!(sbe.session.sbe_schema_path.as_deref(), Some("sbe.xml"));
}

#[test]
fn binary_protocol_missing_path_fails_fast() {
    let err = resolve_one(&base("Protocol=FAST\n")).expect_err("missing path");
    assert!(matches!(err, ConfigError::MissingRequired { key, .. } if key == "FastTemplatePath"));

    let err = resolve_one(&base("Protocol=SBE\n")).expect_err("missing path");
    assert!(matches!(err, ConfigError::MissingRequired { key, .. } if key == "SbeSchemaPath"));
}
