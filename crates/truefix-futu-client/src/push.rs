use bytes::Bytes;
use prost::Message;

use crate::error::{FutuError, FutuResult};
use crate::pb;
use crate::proto_id;

#[derive(Debug, Clone)]
pub enum Push {
    Notify(pb::notify::S2c),
    UpdateOrder(pb::trd_update_order::S2c),
    UpdateOrderFill(pb::trd_update_order_fill::S2c),
    UpdateBasicQot(pb::qot_update_basic_qot::S2c),
    UpdateKl(pb::qot_update_kl::S2c),
    UpdateRt(pb::qot_update_rt::S2c),
    UpdateTicker(pb::qot_update_ticker::S2c),
    UpdateOrderBook(pb::qot_update_order_book::S2c),
    UpdateBroker(pb::qot_update_broker::S2c),
    UpdatePriceReminder(pb::qot_update_price_reminder::S2c),
    UpdateOptionEvent(pb::qot_update_option_event::S2c),
    PushIndicatorCalc(pb::qot_push_indicator_calc::S2c),
    Unknown { proto_id: u32, body: Bytes },
}

pub fn decode_push(proto_id_value: u32, body: &[u8]) -> FutuResult<Push> {
    macro_rules! decode_response_s2c {
        ($response:ty) => {{
            let response = <$response>::decode(body)?;
            response.s2c.ok_or_else(|| FutuError::OpenDError {
                ret_type: response.ret_type,
                ret_msg: response.ret_msg,
            })?
        }};
    }

    let push = match proto_id_value {
        proto_id::NOTIFY => Push::Notify(decode_response_s2c!(pb::notify::Response)),
        proto_id::TRD_UPDATE_ORDER => {
            Push::UpdateOrder(decode_response_s2c!(pb::trd_update_order::Response))
        }
        proto_id::TRD_UPDATE_ORDER_FILL => {
            Push::UpdateOrderFill(decode_response_s2c!(pb::trd_update_order_fill::Response))
        }
        proto_id::QOT_UPDATE_BASIC_QOT => {
            Push::UpdateBasicQot(decode_response_s2c!(pb::qot_update_basic_qot::Response))
        }
        proto_id::QOT_UPDATE_KL => {
            Push::UpdateKl(decode_response_s2c!(pb::qot_update_kl::Response))
        }
        proto_id::QOT_UPDATE_RT => {
            Push::UpdateRt(decode_response_s2c!(pb::qot_update_rt::Response))
        }
        proto_id::QOT_UPDATE_TICKER => {
            Push::UpdateTicker(decode_response_s2c!(pb::qot_update_ticker::Response))
        }
        proto_id::QOT_UPDATE_ORDER_BOOK => {
            Push::UpdateOrderBook(decode_response_s2c!(pb::qot_update_order_book::Response))
        }
        proto_id::QOT_UPDATE_BROKER => {
            Push::UpdateBroker(decode_response_s2c!(pb::qot_update_broker::Response))
        }
        proto_id::QOT_UPDATE_PRICE_REMINDER => Push::UpdatePriceReminder(decode_response_s2c!(
            pb::qot_update_price_reminder::Response
        )),
        proto_id::QOT_UPDATE_OPTION_EVENT => {
            Push::UpdateOptionEvent(decode_response_s2c!(pb::qot_update_option_event::Response))
        }
        proto_id::QOT_PUSH_INDICATOR_CALC => {
            Push::PushIndicatorCalc(decode_response_s2c!(pb::qot_push_indicator_calc::Response))
        }
        _ => Push::Unknown {
            proto_id: proto_id_value,
            body: Bytes::copy_from_slice(body),
        },
    };
    Ok(push)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_basic_quote_push_response_wrapper() {
        let body = pb::qot_update_basic_qot::Response {
            ret_type: 0,
            ret_msg: None,
            err_code: None,
            s2c: Some(pb::qot_update_basic_qot::S2c {
                basic_qot_list: Vec::new(),
            }),
        }
        .encode_to_vec();

        let push = decode_push(proto_id::QOT_UPDATE_BASIC_QOT, &body).unwrap();
        assert!(matches!(push, Push::UpdateBasicQot(_)));
    }

    #[test]
    fn missing_push_payload_is_an_opend_error() {
        let body = pb::qot_update_ticker::Response {
            ret_type: 0,
            ret_msg: Some("missing payload".to_owned()),
            err_code: None,
            s2c: None,
        }
        .encode_to_vec();

        let error = decode_push(proto_id::QOT_UPDATE_TICKER, &body).unwrap_err();
        assert!(matches!(
            error,
            FutuError::OpenDError {
                ret_type: 0,
                ret_msg: Some(_)
            }
        ));
    }
}
