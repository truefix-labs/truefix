//! T045 (US6) — the generated `crack_<version>` dispatcher routes an inbound message to the
//! typed handler method matching its MsgType, by BeginString/version (FR-022).

use truefix::dict::fix44::{self, FIX44MessageHandler, NewOrderSingle};

#[derive(Default)]
struct Handler {
    saw_heartbeat: bool,
    saw_order: Option<String>,
}

impl FIX44MessageHandler for Handler {
    fn on_heartbeat(&mut self, _msg: &fix44::Heartbeat) {
        self.saw_heartbeat = true;
    }
    fn on_new_order_single(&mut self, msg: &NewOrderSingle) {
        self.saw_order = msg.cl_ord_id().map(str::to_owned);
    }
}

#[test]
fn dispatches_to_the_typed_handler_for_its_msg_type() {
    let mut handler = Handler::default();

    let mut hb = fix44::Heartbeat::new();
    hb.0.header.set(truefix_core::Field::string(8, "FIX.4.4"));
    assert!(fix44::crack_fix44(&hb.0, &mut handler));
    assert!(handler.saw_heartbeat);

    let mut order = NewOrderSingle::new();
    order.set_cl_ord_id("O-42");
    order
        .0
        .header
        .set(truefix_core::Field::string(8, "FIX.4.4"));
    assert!(fix44::crack_fix44(&order.0, &mut handler));
    assert_eq!(handler.saw_order.as_deref(), Some("O-42"));
}

#[test]
fn refuses_to_dispatch_a_message_from_a_different_version() {
    let mut handler = Handler::default();
    let mut hb = fix44::Heartbeat::new();
    hb.0.header.set(truefix_core::Field::string(8, "FIX.4.2")); // wrong BeginString
    assert!(!fix44::crack_fix44(&hb.0, &mut handler));
    assert!(!handler.saw_heartbeat);
}
