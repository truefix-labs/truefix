use std::sync::Arc;

use prost::Message;

use crate::client::ClientCore;
use crate::error::{FutuError, FutuResult};
use crate::pb;
use crate::proto_id;
use crate::rpc::ResponseWithS2C;
use crate::rpc::{ensure_ok, impl_response_with_s2c};

#[derive(Clone)]
pub struct TradeClient {
    pub(crate) core: Arc<ClientCore>,
}

#[derive(Debug, Clone, Default)]
pub struct TradeHeader {
    pub trd_env: i32,
    pub acc_id: u64,
    pub trd_market: i32,
    pub jp_acc_type: Option<i32>,
}

impl TradeHeader {
    pub fn into_proto(self) -> pb::trd_common::TrdHeader {
        pb::trd_common::TrdHeader {
            trd_env: self.trd_env,
            acc_id: self.acc_id,
            trd_market: self.trd_market,
            jp_acc_type: self.jp_acc_type,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UnlockTradeRequest {
    pub unlock: bool,
    pub pwd_md5: Option<String>,
    pub security_firm: Option<pb::trd_common::SecurityFirm>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubAccPushRequest {
    pub acc_id_list: Vec<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct GetAccListRequest {
    pub user_id: u64,
    pub trd_category: Option<i32>,
    pub need_general_sec_account: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct GetFundsRequest {
    pub header: Option<pb::trd_common::TrdHeader>,
    pub refresh_cache: Option<bool>,
    pub currency: Option<i32>,
    pub asset_category: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct GetPositionListRequest {
    pub header: Option<pb::trd_common::TrdHeader>,
    pub filter_conditions: Option<pb::trd_common::TrdFilterConditions>,
    pub filter_pl_ratio_min: Option<f64>,
    pub filter_pl_ratio_max: Option<f64>,
    pub refresh_cache: Option<bool>,
    pub asset_category: Option<i32>,
    pub currency: Option<i32>,
    pub option_strategy_view: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct GetOrderListRequest {
    pub header: Option<pb::trd_common::TrdHeader>,
    pub filter_conditions: Option<pb::trd_common::TrdFilterConditions>,
    pub filter_status_list: Vec<i32>,
    pub refresh_cache: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct GetOrderFillListRequest {
    pub header: Option<pb::trd_common::TrdHeader>,
    pub filter_conditions: Option<pb::trd_common::TrdFilterConditions>,
    pub refresh_cache: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct GetHistoryOrderListRequest {
    pub header: Option<pb::trd_common::TrdHeader>,
    pub filter_conditions: Option<pb::trd_common::TrdFilterConditions>,
    pub filter_status_list: Vec<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct CancelAllOrderRequest {
    pub header: Option<pb::trd_common::TrdHeader>,
    pub trd_market: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct PlaceOrderRequest {
    pub header: pb::trd_common::TrdHeader,
    pub trd_side: i32,
    pub order_type: i32,
    pub code: String,
    pub qty: f64,
    pub price: Option<f64>,
    pub adjust_price: Option<bool>,
    pub adjust_side_and_limit: Option<f64>,
    pub sec_market: Option<i32>,
    pub remark: Option<String>,
    pub time_in_force: Option<i32>,
    pub fill_outside_rth: Option<bool>,
    pub aux_price: Option<f64>,
    pub trail_type: Option<i32>,
    pub trail_value: Option<f64>,
    pub trail_spread: Option<f64>,
    pub session: Option<i32>,
    pub position_id: Option<u64>,
    pub expire_time: Option<String>,
}

impl Default for PlaceOrderRequest {
    fn default() -> Self {
        Self {
            header: pb::trd_common::TrdHeader {
                trd_env: 0,
                acc_id: 0,
                trd_market: 0,
                jp_acc_type: None,
            },
            trd_side: 0,
            order_type: 0,
            code: String::new(),
            qty: 0.0,
            price: None,
            adjust_price: None,
            adjust_side_and_limit: None,
            sec_market: None,
            remark: None,
            time_in_force: None,
            fill_outside_rth: None,
            aux_price: None,
            trail_type: None,
            trail_value: None,
            trail_spread: None,
            session: None,
            position_id: None,
            expire_time: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModifyOrderRequest {
    pub header: pb::trd_common::TrdHeader,
    pub order_id: u64,
    pub modify_order_op: i32,
    pub for_all: Option<bool>,
    pub trd_market: Option<i32>,
    pub qty: Option<f64>,
    pub price: Option<f64>,
    pub adjust_price: Option<bool>,
    pub adjust_side_and_limit: Option<f64>,
    pub aux_price: Option<f64>,
    pub trail_type: Option<i32>,
    pub trail_value: Option<f64>,
    pub trail_spread: Option<f64>,
    pub order_id_ex: Option<String>,
}

impl Default for ModifyOrderRequest {
    fn default() -> Self {
        Self {
            header: pb::trd_common::TrdHeader {
                trd_env: 0,
                acc_id: 0,
                trd_market: 0,
                jp_acc_type: None,
            },
            order_id: 0,
            modify_order_op: 0,
            for_all: None,
            trd_market: None,
            qty: None,
            price: None,
            adjust_price: None,
            adjust_side_and_limit: None,
            aux_price: None,
            trail_type: None,
            trail_value: None,
            trail_spread: None,
            order_id_ex: None,
        }
    }
}

impl TradeClient {
    pub async fn close(&self) -> FutuResult<()> {
        self.core.shutdown().await
    }

    pub fn get_security_firm(&self) -> Option<pb::trd_common::SecurityFirm> {
        self.core.get_security_firm()
    }

    pub fn on_api_socket_reconnected(&self) -> FutuResult<()> {
        Ok(())
    }

    pub fn on_async_sub_acc_push(&self) -> FutuResult<()> {
        Ok(())
    }

    pub async fn get_acc_list(
        &self,
        request: GetAccListRequest,
    ) -> FutuResult<pb::trd_get_acc_list::S2c> {
        let req = pb::trd_get_acc_list::Request {
            c2s: pb::trd_get_acc_list::C2s {
                user_id: request.user_id,
                trd_category: request.trd_category,
                need_general_sec_account: request.need_general_sec_account,
            },
        };
        self.decode_s2c::<_, pb::trd_get_acc_list::Response>(proto_id::TRD_GET_ACC_LIST, &req)
            .await
    }

    pub async fn accinfo_query(
        &self,
        request: GetFundsRequest,
    ) -> FutuResult<pb::trd_get_funds::S2c> {
        self.get_funds(request).await
    }

    pub async fn position_list_query(
        &self,
        request: GetPositionListRequest,
    ) -> FutuResult<pb::trd_get_position_list::S2c> {
        self.get_position_list(request).await
    }

    pub async fn order_list_query(
        &self,
        request: GetOrderListRequest,
    ) -> FutuResult<pb::trd_get_order_list::S2c> {
        self.get_order_list(request).await
    }

    pub async fn history_order_list_query(
        &self,
        request: GetHistoryOrderListRequest,
    ) -> FutuResult<pb::trd_get_history_order_list::S2c> {
        self.get_history_order_list(request).await
    }

    pub async fn deal_list_query(
        &self,
        request: GetOrderFillListRequest,
    ) -> FutuResult<pb::trd_get_order_fill_list::S2c> {
        self.get_order_fill_list(request).await
    }

    pub async fn history_deal_list_query(
        &self,
        request: pb::trd_get_history_order_fill_list::Request,
    ) -> FutuResult<pb::trd_get_history_order_fill_list::S2c> {
        self.get_history_order_fill_list(request).await
    }

    pub async fn order_fee_query(
        &self,
        request: pb::trd_get_order_fee::Request,
    ) -> FutuResult<pb::trd_get_order_fee::S2c> {
        self.get_order_fee(request).await
    }

    pub async fn comboorder_tradinginfo_query(
        &self,
        request: pb::trd_get_combo_max_trd_qtys::Request,
    ) -> FutuResult<pb::trd_get_combo_max_trd_qtys::S2c> {
        self.get_combo_max_trd_qtys(request).await
    }

    pub async fn get_max_trd_qtys(
        &self,
        request: pb::trd_get_max_trd_qtys::Request,
    ) -> FutuResult<pb::trd_get_max_trd_qtys::S2c> {
        let resp = self
            .request::<_, pb::trd_get_max_trd_qtys::Response>(
                proto_id::TRD_GET_MAX_TRD_QTYS,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn unlock_trade(&self, request: UnlockTradeRequest) -> FutuResult<()> {
        let replay_request = request.clone();
        let req = pb::trd_unlock_trade::Request {
            c2s: pb::trd_unlock_trade::C2s {
                unlock: request.unlock,
                pwd_md5: request.pwd_md5,
                security_firm: request.security_firm.map(|firm| firm as i32),
            },
        };
        let resp = self
            .request::<_, pb::trd_unlock_trade::Response>(proto_id::TRD_UNLOCK_TRADE, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        self.core.remember_unlock_trade(&replay_request).await;
        Ok(())
    }

    pub async fn sub_acc_push(&self, request: SubAccPushRequest) -> FutuResult<()> {
        let replay_request = request.clone();
        let req = pb::trd_sub_acc_push::Request {
            c2s: pb::trd_sub_acc_push::C2s {
                acc_id_list: request.acc_id_list,
            },
        };
        let resp = self
            .request::<_, pb::trd_sub_acc_push::Response>(proto_id::TRD_SUB_ACC_PUSH, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        self.core.remember_sub_acc_push(&replay_request).await;
        Ok(())
    }

    pub async fn get_funds(&self, request: GetFundsRequest) -> FutuResult<pb::trd_get_funds::S2c> {
        let req = pb::trd_get_funds::Request {
            c2s: pb::trd_get_funds::C2s {
                header: request.header.ok_or_else(|| {
                    FutuError::Crypto("get_funds requires a trade header".to_owned())
                })?,
                refresh_cache: request.refresh_cache,
                currency: request.currency,
                asset_category: request.asset_category,
            },
        };
        self.decode_s2c::<_, pb::trd_get_funds::Response>(proto_id::TRD_GET_FUNDS, &req)
            .await
    }

    pub async fn acctradinginfo_query(
        &self,
        request: pb::trd_get_max_trd_qtys::Request,
    ) -> FutuResult<pb::trd_get_max_trd_qtys::S2c> {
        self.get_max_trd_qtys(request).await
    }

    pub async fn get_position_list(
        &self,
        request: GetPositionListRequest,
    ) -> FutuResult<pb::trd_get_position_list::S2c> {
        let req = pb::trd_get_position_list::Request {
            c2s: pb::trd_get_position_list::C2s {
                header: request.header.ok_or_else(|| {
                    FutuError::Crypto("get_position_list requires a trade header".to_owned())
                })?,
                filter_conditions: request.filter_conditions,
                filter_pl_ratio_min: request.filter_pl_ratio_min,
                filter_pl_ratio_max: request.filter_pl_ratio_max,
                refresh_cache: request.refresh_cache,
                asset_category: request.asset_category,
                currency: request.currency,
                option_strategy_view: request.option_strategy_view,
            },
        };
        self.decode_s2c::<_, pb::trd_get_position_list::Response>(
            proto_id::TRD_GET_POSITION_LIST,
            &req,
        )
        .await
    }

    pub async fn get_order_list(
        &self,
        request: GetOrderListRequest,
    ) -> FutuResult<pb::trd_get_order_list::S2c> {
        let req = pb::trd_get_order_list::Request {
            c2s: pb::trd_get_order_list::C2s {
                header: request.header.ok_or_else(|| {
                    FutuError::Crypto("get_order_list requires a trade header".to_owned())
                })?,
                filter_conditions: request.filter_conditions,
                filter_status_list: request.filter_status_list,
                refresh_cache: request.refresh_cache,
            },
        };
        self.decode_s2c::<_, pb::trd_get_order_list::Response>(proto_id::TRD_GET_ORDER_LIST, &req)
            .await
    }

    pub async fn place_order(&self, request: PlaceOrderRequest) -> FutuResult<u64> {
        let serial_no = self.core.next_serial()?;
        let req = pb::trd_place_order::Request {
            c2s: pb::trd_place_order::C2s {
                packet_id: self.core.packet_id_for(serial_no),
                header: request.header,
                trd_side: request.trd_side,
                order_type: request.order_type,
                code: request.code,
                qty: request.qty,
                price: request.price,
                adjust_price: request.adjust_price,
                adjust_side_and_limit: request.adjust_side_and_limit,
                sec_market: request.sec_market,
                remark: request.remark,
                time_in_force: request.time_in_force,
                fill_outside_rth: request.fill_outside_rth,
                aux_price: request.aux_price,
                trail_type: request.trail_type,
                trail_value: request.trail_value,
                trail_spread: request.trail_spread,
                session: request.session,
                position_id: request.position_id,
                expire_time: request.expire_time,
            },
        };
        let resp = self
            .request_with_serial::<_, pb::trd_place_order::Response>(
                proto_id::TRD_PLACE_ORDER,
                serial_no,
                &req,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        let s2c = resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })?;
        Ok(s2c.order_id.unwrap_or_default())
    }

    pub async fn modify_order(&self, request: ModifyOrderRequest) -> FutuResult<u64> {
        let serial_no = self.core.next_serial()?;
        let req = pb::trd_modify_order::Request {
            c2s: pb::trd_modify_order::C2s {
                packet_id: self.core.packet_id_for(serial_no),
                header: request.header,
                order_id: request.order_id,
                modify_order_op: request.modify_order_op,
                for_all: request.for_all,
                trd_market: request.trd_market,
                qty: request.qty,
                price: request.price,
                adjust_price: request.adjust_price,
                adjust_side_and_limit: request.adjust_side_and_limit,
                aux_price: request.aux_price,
                trail_type: request.trail_type,
                trail_value: request.trail_value,
                trail_spread: request.trail_spread,
                order_id_ex: request.order_id_ex,
            },
        };
        let resp = self
            .request_with_serial::<_, pb::trd_modify_order::Response>(
                proto_id::TRD_MODIFY_ORDER,
                serial_no,
                &req,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        let s2c = resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })?;
        Ok(s2c.order_id)
    }

    pub async fn get_order_fill_list(
        &self,
        request: GetOrderFillListRequest,
    ) -> FutuResult<pb::trd_get_order_fill_list::S2c> {
        let req = pb::trd_get_order_fill_list::Request {
            c2s: pb::trd_get_order_fill_list::C2s {
                header: request.header.ok_or_else(|| {
                    FutuError::Crypto("get_order_fill_list requires a trade header".to_owned())
                })?,
                filter_conditions: request.filter_conditions,
                refresh_cache: request.refresh_cache,
            },
        };
        self.decode_s2c::<_, pb::trd_get_order_fill_list::Response>(
            proto_id::TRD_GET_ORDER_FILL_LIST,
            &req,
        )
        .await
    }

    pub async fn get_history_order_list(
        &self,
        request: GetHistoryOrderListRequest,
    ) -> FutuResult<pb::trd_get_history_order_list::S2c> {
        let req = pb::trd_get_history_order_list::Request {
            c2s: pb::trd_get_history_order_list::C2s {
                header: request.header.ok_or_else(|| {
                    FutuError::Crypto("get_history_order_list requires a trade header".to_owned())
                })?,
                filter_conditions: request.filter_conditions.unwrap_or_default(),
                filter_status_list: request.filter_status_list,
            },
        };
        self.decode_s2c::<_, pb::trd_get_history_order_list::Response>(
            proto_id::TRD_GET_HISTORY_ORDER_LIST,
            &req,
        )
        .await
    }

    pub async fn change_order(&self, request: ModifyOrderRequest) -> FutuResult<u64> {
        self.modify_order(request).await
    }

    pub async fn cancel_all_order(&self, request: CancelAllOrderRequest) -> FutuResult<()> {
        let header = request.header.ok_or_else(|| {
            FutuError::Crypto("cancel_all_order requires a trade header".to_owned())
        })?;
        let serial_no = self.core.next_serial()?;
        let req = pb::trd_modify_order::Request {
            c2s: pb::trd_modify_order::C2s {
                packet_id: self.core.packet_id_for(serial_no),
                header,
                order_id: 0,
                modify_order_op: pb::trd_common::ModifyOrderOp::Cancel as i32,
                for_all: Some(true),
                trd_market: request.trd_market,
                qty: None,
                price: None,
                adjust_price: None,
                adjust_side_and_limit: None,
                aux_price: None,
                trail_type: None,
                trail_value: None,
                trail_spread: None,
                order_id_ex: None,
            },
        };
        let resp = self
            .request_with_serial::<_, pb::trd_modify_order::Response>(
                proto_id::TRD_MODIFY_ORDER,
                serial_no,
                &req,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        Ok(())
    }

    pub fn is_futures_market_sim(
        trd_market: pb::trd_common::TrdMarket,
        trd_mkt_list: &[pb::trd_common::TrdMarket],
    ) -> bool {
        if trd_market != pb::trd_common::TrdMarket::Futures {
            return false;
        }
        trd_mkt_list.iter().any(|trd_mkt| {
            matches!(
                trd_mkt,
                pb::trd_common::TrdMarket::FuturesSimulateHk
                    | pb::trd_common::TrdMarket::FuturesSimulateUs
                    | pb::trd_common::TrdMarket::FuturesSimulateJp
                    | pb::trd_common::TrdMarket::FuturesSimulateSg
            )
        })
    }

    pub async fn get_history_order_fill_list(
        &self,
        request: pb::trd_get_history_order_fill_list::Request,
    ) -> FutuResult<pb::trd_get_history_order_fill_list::S2c> {
        self.decode_s2c::<_, pb::trd_get_history_order_fill_list::Response>(
            proto_id::TRD_GET_HISTORY_ORDER_FILL_LIST,
            &request,
        )
        .await
    }

    pub async fn get_margin_ratio(
        &self,
        request: pb::trd_get_margin_ratio::Request,
    ) -> FutuResult<pb::trd_get_margin_ratio::S2c> {
        self.decode_s2c::<_, pb::trd_get_margin_ratio::Response>(
            proto_id::TRD_GET_MARGIN_RATIO,
            &request,
        )
        .await
    }

    pub async fn get_order_fee(
        &self,
        request: pb::trd_get_order_fee::Request,
    ) -> FutuResult<pb::trd_get_order_fee::S2c> {
        self.decode_s2c::<_, pb::trd_get_order_fee::Response>(proto_id::TRD_GET_ORDER_FEE, &request)
            .await
    }

    pub async fn get_acc_cash_flow(
        &self,
        request: pb::trd_flow_summary::Request,
    ) -> FutuResult<pb::trd_flow_summary::S2c> {
        self.decode_s2c::<_, pb::trd_flow_summary::Response>(proto_id::TRD_FLOW_SUMMARY, &request)
            .await
    }

    pub async fn get_combo_max_trd_qtys(
        &self,
        request: pb::trd_get_combo_max_trd_qtys::Request,
    ) -> FutuResult<pb::trd_get_combo_max_trd_qtys::S2c> {
        self.decode_s2c::<_, pb::trd_get_combo_max_trd_qtys::Response>(
            proto_id::TRD_GET_COMBO_MAX_TRD_QTYS,
            &request,
        )
        .await
    }

    pub async fn place_combo_order(
        &self,
        request: pb::trd_place_combo_order::Request,
    ) -> FutuResult<String> {
        let resp = self
            .request::<_, pb::trd_place_combo_order::Response>(
                proto_id::TRD_PLACE_COMBO_ORDER,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        let s2c = resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })?;
        Ok(s2c.order_id_ex.unwrap_or_default())
    }

    async fn decode_s2c<Req, Resp>(&self, proto_id: u32, request: &Req) -> FutuResult<Resp::S2c>
    where
        Req: Message,
        Resp: Message + Default + ResponseWithS2C,
    {
        let resp: Resp = self.core.request(proto_id, request).await?;
        ensure_ok(resp.ret_type(), resp.ret_msg())?;
        let ret_type = resp.ret_type();
        let ret_msg = resp.ret_msg();
        resp.s2c()
            .ok_or(FutuError::OpenDError { ret_type, ret_msg })
    }

    async fn request<Req, Resp>(&self, proto_id: u32, request: &Req) -> FutuResult<Resp>
    where
        Req: Message,
        Resp: Message + Default,
    {
        self.core.request(proto_id, request).await
    }

    async fn request_with_serial<Req, Resp>(
        &self,
        proto_id: u32,
        serial_no: u32,
        request: &Req,
    ) -> FutuResult<Resp>
    where
        Req: Message,
        Resp: Message + Default,
    {
        self.core
            .request_with_serial(proto_id, serial_no, request, false)
            .await
    }
}

impl_response_with_s2c!(pb::trd_get_acc_list::Response, pb::trd_get_acc_list::S2c);
impl_response_with_s2c!(pb::trd_get_funds::Response, pb::trd_get_funds::S2c);
impl_response_with_s2c!(
    pb::trd_get_position_list::Response,
    pb::trd_get_position_list::S2c
);
impl_response_with_s2c!(
    pb::trd_get_order_list::Response,
    pb::trd_get_order_list::S2c
);
impl_response_with_s2c!(
    pb::trd_get_order_fill_list::Response,
    pb::trd_get_order_fill_list::S2c
);
impl_response_with_s2c!(
    pb::trd_get_history_order_list::Response,
    pb::trd_get_history_order_list::S2c
);
impl_response_with_s2c!(
    pb::trd_get_history_order_fill_list::Response,
    pb::trd_get_history_order_fill_list::S2c
);
impl_response_with_s2c!(
    pb::trd_get_margin_ratio::Response,
    pb::trd_get_margin_ratio::S2c
);
impl_response_with_s2c!(pb::trd_get_order_fee::Response, pb::trd_get_order_fee::S2c);
impl_response_with_s2c!(pb::trd_flow_summary::Response, pb::trd_flow_summary::S2c);
impl_response_with_s2c!(
    pb::trd_get_combo_max_trd_qtys::Response,
    pb::trd_get_combo_max_trd_qtys::S2c
);
impl_response_with_s2c!(
    pb::trd_place_combo_order::Response,
    pb::trd_place_combo_order::S2c
);
