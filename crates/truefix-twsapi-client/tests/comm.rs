use truefix_twsapi_client::comm;
use truefix_twsapi_client::constants::{UNSET_DOUBLE, UNSET_INTEGER};
use truefix_twsapi_client::message::{Outgoing, PROTOBUF_MSG_ID};
use truefix_twsapi_client::server_versions::{MAX_CLIENT_VER, MIN_CLIENT_VER};

#[test]
fn enhanced_handshake_matches_python_client_shape() {
    let bytes = comm::make_client_handshake(MIN_CLIENT_VER, MAX_CLIENT_VER, None);
    let expected_text = format!("v{MIN_CLIENT_VER}..{MAX_CLIENT_VER}");
    let expected_len = (expected_text.len() as u32).to_be_bytes();

    assert_eq!(&bytes[0..4], b"API\0");
    assert_eq!(&bytes[4..8], &expected_len);
    assert_eq!(&bytes[8..], expected_text.as_bytes());
}

#[test]
fn make_msg_encodes_text_message_id_until_protobuf_server_version() {
    let fields = format!(
        "{}{}",
        comm::make_field(1).unwrap(),
        comm::make_field(7).unwrap()
    );
    let msg = comm::make_msg(Outgoing::ReqIds.id(), false, &fields).unwrap();

    assert_eq!(&msg[0..4], 6_u32.to_be_bytes());
    assert_eq!(&msg[4..], &[b'8', 0, b'1', 0, b'7', 0]);
}

#[test]
fn make_msg_encodes_raw_int_message_id_for_newer_server_versions() {
    let fields = comm::make_field(1).unwrap();
    let msg = comm::make_msg(Outgoing::ReqPositions.id(), true, &fields).unwrap();

    assert_eq!(&msg[0..4], 6_u32.to_be_bytes());
    assert_eq!(&msg[4..8], &61_i32.to_be_bytes());
    assert_eq!(&msg[8..], b"1\0");
}

#[test]
fn make_msg_proto_prefixes_raw_message_id() {
    let msg = comm::make_msg_proto(Outgoing::ReqCurrentTime.protobuf_id(), b"abc");

    assert_eq!(&msg[0..4], 7_u32.to_be_bytes());
    assert_eq!(
        &msg[4..8],
        &(Outgoing::ReqCurrentTime.id() + PROTOBUF_MSG_ID).to_be_bytes()
    );
    assert_eq!(&msg[8..], b"abc");
}

#[test]
fn field_encoding_matches_python_bool_and_empty_sentinel_rules() {
    assert_eq!(comm::make_field(true).unwrap(), "1\0");
    assert_eq!(comm::make_field(false).unwrap(), "0\0");
    assert_eq!(comm::make_field_handle_empty(UNSET_INTEGER).unwrap(), "\0");
    assert_eq!(comm::make_field_handle_empty(UNSET_DOUBLE).unwrap(), "\0");
    assert_eq!(comm::make_field(100.0_f64).unwrap(), "100.0\0");
    assert_eq!(comm::make_field(-0.0_f64).unwrap(), "-0.0\0");
    assert_eq!(comm::make_field(f64::INFINITY).unwrap(), "inf\0");
    assert_eq!(
        comm::make_field_handle_empty(f64::INFINITY).unwrap(),
        "Infinity\0"
    );
}

#[test]
fn read_msg_and_fields_return_complete_frame_and_rest() {
    let mut buf = Vec::new();
    buf.extend_from_slice(&4_u32.to_be_bytes());
    buf.extend_from_slice(b"1\0x\0");
    buf.extend_from_slice(b"rest");

    let frame = comm::read_msg(&buf).unwrap().unwrap();
    assert_eq!(frame.size, 4);
    assert_eq!(frame.payload, b"1\0x\0");
    assert_eq!(frame.rest, b"rest");
    assert_eq!(
        comm::read_fields(frame.payload),
        vec![b"1".as_slice(), b"x".as_slice()]
    );
}

#[test]
fn read_msg_returns_none_for_partial_data_like_python_zero_size_tuple() {
    assert!(comm::read_msg(b"\0\0").unwrap().is_none());

    let mut partial = Vec::new();
    partial.extend_from_slice(&10_u32.to_be_bytes());
    partial.extend_from_slice(b"abc");
    assert!(comm::read_msg(&partial).unwrap().is_none());
}
