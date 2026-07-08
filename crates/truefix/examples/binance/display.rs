//! Human-readable pretty-printers for inbound Binance FIX messages.
//!
//! Called in addition to (not instead of) the raw wire-traffic log line every admin/app message
//! already gets — these summarize the fields a human actually cares about, decoding Binance's enum
//! values to names. Repeating groups are read via `FieldMap::group`, structured on receive by the
//! FIX data dictionary configured in `config/binance-testnet.cfg` (`DataDictionary=`).

use truefix::{FieldMap, Message};

fn field_str(fm: &FieldMap, tag: u32) -> String {
    fm.get(tag)
        .and_then(|f| f.as_str().ok())
        .unwrap_or_default()
        .to_owned()
}

fn opt_field_str(fm: &FieldMap, tag: u32) -> Option<String> {
    fm.get(tag).and_then(|f| f.as_str().ok()).map(str::to_owned)
}

fn ord_status_name(code: &str) -> &'static str {
    match code {
        "0" => "NEW",
        "1" => "PARTIALLY_FILLED",
        "2" => "FILLED",
        "4" => "CANCELED",
        "6" => "PENDING_CANCEL",
        "8" => "REJECTED",
        "A" => "PENDING_NEW",
        "C" => "EXPIRED",
        _ => "UNKNOWN",
    }
}

fn exec_type_name(code: &str) -> &'static str {
    match code {
        "0" => "NEW",
        "4" => "CANCELED",
        "5" => "REPLACED",
        "8" => "REJECTED",
        "F" => "TRADE",
        "C" => "EXPIRED",
        _ => "UNKNOWN",
    }
}

fn side_name(code: &str) -> &'static str {
    match code {
        "1" => "BUY",
        "2" => "SELL",
        _ => "UNKNOWN",
    }
}

fn md_entry_type_name(code: &str) -> &'static str {
    match code {
        "0" => "BID",
        "1" => "OFFER",
        "2" => "TRADE",
        _ => "UNKNOWN",
    }
}

fn md_update_action_name(code: &str) -> &'static str {
    match code {
        "0" => "NEW",
        "1" => "CHANGE",
        "2" => "DELETE",
        _ => "UNKNOWN",
    }
}

/// Dispatch one decoded message to its pretty-printer by `MsgType`, logging a human-readable
/// summary via `tracing::info!`. No-op for message types with nothing extra worth summarizing
/// beyond the raw wire dump (e.g. plain Heartbeats).
pub fn pretty_print(session: &str, message: &Message) {
    match message.msg_type() {
        Some("8") => execution_report(session, message),
        Some("9") => order_cancel_reject(session, message),
        Some("r") => order_mass_cancel_report(session, message),
        Some("N") => list_status(session, message),
        Some("XAR") => order_amend_reject(session, message),
        Some("XLR") => limit_response(session, message),
        Some("W") => market_data_snapshot(session, message),
        Some("X") => market_data_incremental_refresh(session, message),
        Some("Y") => market_data_request_reject(session, message),
        Some("y") => instrument_list(session, message),
        Some("B") => news(session, message),
        Some("3") => reject(session, message),
        _ => {}
    }
}

fn execution_report(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        cl_ord_id = %field_str(b, 11),
        order_id = %field_str(b, 37),
        exec_type = %exec_type_name(&field_str(b, 150)),
        ord_status = %ord_status_name(&field_str(b, 39)),
        symbol = %field_str(b, 55),
        side = %side_name(&field_str(b, 54)),
        order_qty = %field_str(b, 38),
        cum_qty = %field_str(b, 14),
        cum_quote_qty = %field_str(b, 25017),
        leaves_qty = %field_str(b, 151),
        price = %field_str(b, 44),
        last_px = %field_str(b, 31),
        last_qty = %field_str(b, 32),
        working_indicator = %field_str(b, 636),
        working_time = %field_str(b, 25023),
        text = %field_str(b, 58),
        "ExecutionReport"
    );
    if let Some(prevented_match_id) = opt_field_str(b, 25024) {
        tracing::info!(
            session,
            prevented_match_id = %prevented_match_id,
            prevented_price = %field_str(b, 25025),
            prevented_qty = %field_str(b, 25026),
            "  SelfTradePrevention"
        );
    }
    for fee in b.group(136).unwrap_or(&[]) {
        tracing::info!(
            session,
            amt = %field_str(fee, 137),
            curr = %field_str(fee, 138),
            fee_type = %field_str(fee, 139),
            "  MiscFee"
        );
    }
}

fn order_cancel_reject(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        cl_ord_id = %field_str(b, 11),
        orig_cl_ord_id = %field_str(b, 41),
        order_id = %field_str(b, 37),
        symbol = %field_str(b, 55),
        cxl_rej_response_to = %field_str(b, 434),
        error_code = %field_str(b, 25016),
        text = %field_str(b, 58),
        "OrderCancelReject"
    );
}

fn order_mass_cancel_report(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        symbol = %field_str(b, 55),
        cl_ord_id = %field_str(b, 11),
        response = %field_str(b, 531),
        total_affected_orders = %field_str(b, 533),
        error_code = %field_str(b, 25016),
        text = %field_str(b, 58),
        "OrderMassCancelReport"
    );
}

fn list_status(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        symbol = %field_str(b, 55),
        list_id = %field_str(b, 66),
        cl_list_id = %field_str(b, 25014),
        contingency_type = %field_str(b, 1385),
        list_status_type = %field_str(b, 429),
        list_order_status = %field_str(b, 431),
        "ListStatus"
    );
    for order in b.group(73).unwrap_or(&[]) {
        tracing::info!(
            session,
            cl_ord_id = %field_str(order, 11),
            symbol = %field_str(order, 55),
            order_id = %field_str(order, 37),
            text = %field_str(order, 58),
            "  ListStatus order"
        );
        // NoListTriggeringInstructions is nested inside this NoOrders entry.
        for instruction in order.group(25010).unwrap_or(&[]) {
            tracing::info!(
                session,
                trigger_type = %field_str(instruction, 25011),
                trigger_index = %field_str(instruction, 25012),
                trigger_action = %field_str(instruction, 25013),
                "    TriggeringInstruction"
            );
        }
    }
}

fn order_amend_reject(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        cl_ord_id = %field_str(b, 11),
        orig_cl_ord_id = %field_str(b, 41),
        order_id = %field_str(b, 37),
        symbol = %field_str(b, 55),
        order_qty = %field_str(b, 38),
        error_code = %field_str(b, 25016),
        text = %field_str(b, 58),
        "OrderAmendReject"
    );
}

fn limit_response(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(session, req_id = %field_str(b, 6136), "LimitResponse");
    for limit in b.group(25003).unwrap_or(&[]) {
        tracing::info!(
            session,
            limit_type = %field_str(limit, 25004),
            count = %field_str(limit, 25005),
            max = %field_str(limit, 25006),
            reset_interval = %field_str(limit, 25007),
            reset_resolution = %field_str(limit, 25008),
            "  Limit"
        );
    }
}

fn market_data_snapshot(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        md_req_id = %field_str(b, 262),
        symbol = %field_str(b, 55),
        "MarketDataSnapshot"
    );
    for entry in b.group(268).unwrap_or(&[]) {
        tracing::info!(
            session,
            entry_type = %md_entry_type_name(&field_str(entry, 269)),
            px = %field_str(entry, 270),
            size = %field_str(entry, 271),
            "  MDEntry"
        );
    }
}

fn market_data_incremental_refresh(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(session, md_req_id = %field_str(b, 262), "MarketDataIncrementalRefresh");
    for entry in b.group(268).unwrap_or(&[]) {
        tracing::info!(
            session,
            action = %md_update_action_name(&field_str(entry, 279)),
            entry_type = %entry.get(269).and_then(|f| f.as_str().ok()).map(md_entry_type_name).unwrap_or(""),
            symbol = %field_str(entry, 55),
            px = %field_str(entry, 270),
            size = %field_str(entry, 271),
            "  MDEntry"
        );
    }
}

fn market_data_request_reject(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(
        session,
        md_req_id = %field_str(b, 262),
        reason = %field_str(b, 281),
        error_code = %field_str(b, 25016),
        text = %field_str(b, 58),
        "MarketDataRequestReject"
    );
}

fn instrument_list(session: &str, message: &Message) {
    let b = &message.body;
    tracing::info!(session, req_id = %field_str(b, 320), "InstrumentList");
    for instrument in b.group(146).unwrap_or(&[]) {
        tracing::info!(
            session,
            symbol = %field_str(instrument, 55),
            currency = %field_str(instrument, 15),
            min_trade_vol = %field_str(instrument, 562),
            max_trade_vol = %field_str(instrument, 1140),
            min_qty_increment = %field_str(instrument, 25039),
            min_price_increment = %field_str(instrument, 969),
            "  Instrument"
        );
    }
}

fn news(session: &str, message: &Message) {
    tracing::info!(session, headline = %field_str(&message.body, 148), "News");
}

fn reject(session: &str, message: &Message) {
    let b = &message.body;
    if let Some(reason) = opt_field_str(b, 373).or_else(|| opt_field_str(b, 25016)) {
        tracing::warn!(
            session,
            ref_seq_num = %field_str(b, 45),
            ref_tag_id = %field_str(b, 371),
            reason = %reason,
            text = %field_str(b, 58),
            "Reject"
        );
    }
}
