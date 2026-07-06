use truefix_core::Message;

fn build(body_fields: &[u8]) -> Vec<u8> {
    let begin = b"8=FIX.5.0SP2\x01";
    let mut middle = Vec::new();
    middle.extend_from_slice(b"35=D\x01");
    middle.extend_from_slice(body_fields);
    let bl = middle.len();
    let mut out = Vec::new();
    out.extend_from_slice(begin);
    out.extend_from_slice(format!("9={}\x01", bl).as_bytes());
    out.extend_from_slice(&middle);
    let cs: u32 = out.iter().map(|&b| b as u32).sum::<u32>() % 256;
    out.extend_from_slice(format!("10={:03}\x01", cs).as_bytes());
    out
}

#[test]
fn missing_len_data_pair_corrupts_message() {
    // tag 1401 = EncryptedPasswordLen, 1402 = EncryptedPassword (DATA), with an embedded SOH.
    let mut body = Vec::new();
    body.extend_from_slice(b"1401=3\x01");
    body.push(b'1');
    body.push(b'4');
    body.push(b'0');
    body.push(b'2');
    body.push(b'=');
    body.push(b'A');
    body.push(0x01); // embedded SOH inside the intended 3-byte data value
    body.push(b'B');
    // (no trailing SOH here deliberately -- the *real* field-ending SOH for a 3-byte value)
    let msg_bytes = build(&body);
    println!("raw: {:?}", String::from_utf8_lossy(&msg_bytes));
    match Message::decode(&msg_bytes) {
        Ok(m) => {
            println!("DECODED OK (should NOT be, data got split): body fields:");
            for f in m.body.fields() {
                println!("  tag={} value={:?}", f.tag(), f.as_str());
            }
        }
        Err(e) => println!("decode error: {e}"),
    }
}
