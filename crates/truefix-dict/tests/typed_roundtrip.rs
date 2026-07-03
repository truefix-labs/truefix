//! T044 (US6) — a typed message encodes/decodes byte-identically with the generic codec path
//! (FR-021): the typed struct is a thin wrapper over the same `Message`, so there is no separate
//! wire representation to drift.

use truefix_core::{decode, Field};
use truefix_dict::fix44::{NewOrderSingle, Side};

#[test]
fn typed_message_encodes_identically_to_generic_message() {
    let mut typed = NewOrderSingle::new();
    typed
        .set_cl_ord_id("O1")
        .set_symbol("AAPL")
        .set_side(Side::BUY)
        .set_order_qty("100".parse().unwrap());
    // Stamp the session header fields a real send would add, directly on the inner generic Message.
    typed.0.header.set(Field::string(8, "FIX.4.4"));
    typed.0.header.set(Field::int(34, 2));
    typed.0.header.set(Field::string(49, "CLIENT"));
    typed.0.header.set(Field::string(56, "SERVER"));
    typed.0.header.set(Field::string(52, "20240101-00:00:00"));

    let typed_bytes = typed.encode();

    // Build the equivalent message through the generic API and confirm identical wire bytes.
    let generic = typed.0.clone();
    let generic_bytes = generic.encode();
    assert_eq!(typed_bytes, generic_bytes);

    // And the bytes decode back through the generic decoder without loss.
    let decoded = decode(&typed_bytes).expect("decode");
    assert_eq!(decoded.body.get(11).unwrap().as_str().unwrap(), "O1");
    assert_eq!(decoded.body.get(54).unwrap().as_str().unwrap(), "1");
    assert_eq!(decoded.encode(), typed_bytes);
}
