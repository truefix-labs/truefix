use truefix_session::SessionId;

/// T170/T171 (feature 009, NEW-39): two sessions differing only by SubID/LocationID must render
/// distinguishably via `Display`, not identically.
#[test]
fn two_sessions_differing_only_by_subid_render_distinguishably() {
    let a = SessionId::new_full(
        "FIX.4.4",
        "SERVER",
        Some("SUB1".to_owned()),
        None,
        "CLIENT",
        None,
        None,
        None,
    );
    let b = SessionId::new_full(
        "FIX.4.4",
        "SERVER",
        Some("SUB2".to_owned()),
        None,
        "CLIENT",
        None,
        None,
        None,
    );
    assert_ne!(
        a.to_string(),
        b.to_string(),
        "sessions differing only by SenderSubID must not render identically"
    );
    assert!(a.to_string().contains("SUB1"));
    assert!(b.to_string().contains("SUB2"));
}

#[test]
fn two_sessions_differing_only_by_location_id_render_distinguishably() {
    let a = SessionId::new_full(
        "FIX.4.4",
        "SERVER",
        None,
        Some("LOC1".to_owned()),
        "CLIENT",
        None,
        None,
        None,
    );
    let b = SessionId::new_full(
        "FIX.4.4",
        "SERVER",
        None,
        Some("LOC2".to_owned()),
        "CLIENT",
        None,
        None,
        None,
    );
    assert_ne!(
        a.to_string(),
        b.to_string(),
        "sessions differing only by SenderLocationID must not render identically"
    );
}

#[test]
fn a_session_with_no_sub_id_or_location_id_renders_the_same_as_before() {
    let id = SessionId::new("FIX.4.4", "SERVER", "CLIENT");
    assert_eq!(id.to_string(), "FIX.4.4:SERVER->CLIENT");
}
