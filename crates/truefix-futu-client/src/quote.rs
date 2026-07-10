use std::sync::Arc;

use prost::Message;

use crate::client::ClientCore;
use crate::error::{FutuError, FutuResult};
use crate::pb;
use crate::proto_id;
use crate::rpc::ensure_ok;

#[derive(Clone)]
pub struct QuoteClient {
    pub(crate) core: Arc<ClientCore>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubscribeRequest {
    pub security_list: Vec<pb::qot_common::Security>,
    pub sub_type_list: Vec<i32>,
    pub is_sub_or_un_sub: bool,
    pub is_reg_or_un_reg_push: Option<bool>,
    pub reg_push_rehab_type_list: Vec<i32>,
    pub is_first_push: Option<bool>,
    pub is_unsub_all: Option<bool>,
    pub is_sub_order_book_detail: Option<bool>,
    pub extended_time: Option<bool>,
    pub session: Option<i32>,
    pub header: Option<pb::qot_common::QotHeader>,
}

#[derive(Debug, Clone, Default)]
pub struct GetKlRequest {
    pub rehab_type: i32,
    pub kl_type: i32,
    pub security: Option<pb::qot_common::Security>,
    pub req_num: i32,
    pub header: Option<pb::qot_common::QotHeader>,
}

#[derive(Debug, Clone, Default)]
pub struct GetTickerRequest {
    pub security: Option<pb::qot_common::Security>,
    pub max_ret_num: i32,
    pub header: Option<pb::qot_common::QotHeader>,
}

#[derive(Debug, Clone, Default)]
pub struct GetOrderBookRequest {
    pub security: Option<pb::qot_common::Security>,
    pub num: i32,
    pub order_book_type: Option<i32>,
    pub header: Option<pb::qot_common::QotHeader>,
}

#[derive(Debug, Clone, Default)]
pub struct GetBasicQotRequest {
    pub security_list: Vec<pb::qot_common::Security>,
    pub header: Option<pb::qot_common::QotHeader>,
}

#[derive(Debug, Clone, Default)]
pub struct GetStaticInfoRequest {
    pub market: Option<i32>,
    pub sec_type: Option<i32>,
    pub security_list: Vec<pb::qot_common::Security>,
    pub header: Option<pb::qot_common::QotHeader>,
}

#[derive(Debug, Clone, Default)]
pub struct GetSecuritySnapshotRequest {
    pub security_list: Vec<pb::qot_common::Security>,
    pub header: Option<pb::qot_common::QotHeader>,
}

macro_rules! quote_passthrough {
    ($fn_name:ident, $proto_const:ident, $module:ident) => {
        pub async fn $fn_name(
            &self,
            request: crate::pb::$module::Request,
        ) -> crate::error::FutuResult<crate::pb::$module::S2c> {
            let resp = self
                .request::<_, crate::pb::$module::Response>(crate::proto_id::$proto_const, &request)
                .await?;
            ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
            resp.s2c.ok_or_else(|| crate::error::FutuError::OpenDError {
                ret_type: resp.ret_type,
                ret_msg: resp.ret_msg.clone(),
            })
        }
    };
}

macro_rules! skill_wrap_unusual {
    ($fn_name:ident, $proto_const:ident, $req_ty:ident, $rsp_ty:ident) => {
        pub async fn $fn_name(
            &self,
            request: crate::pb::skill_wrap_api::$req_ty,
        ) -> FutuResult<crate::pb::skill_wrap_api::$rsp_ty> {
            let resp: crate::pb::skill_wrap_api::$rsp_ty =
                self.request(crate::proto_id::$proto_const, &request).await?;
            if resp.ret_type == 0 {
                Ok(resp)
            } else {
                Err(FutuError::OpenDError {
                    ret_type: resp.ret_type,
                    ret_msg: resp.ret_msg.clone(),
                })
            }
        }
    };
}

impl QuoteClient {
    pub async fn close(&self) -> FutuResult<()> {
        self.core.shutdown().await
    }

    pub fn get_security_firm(&self) -> Option<pb::trd_common::SecurityFirm> {
        self.core.get_security_firm()
    }

    pub fn on_api_socket_reconnected(&self) -> FutuResult<()> {
        Ok(())
    }

    pub async fn subscribe(&self, request: SubscribeRequest) -> FutuResult<()> {
        let replay_request = request.clone();
        let req = pb::qot_sub::Request {
            c2s: pb::qot_sub::C2s {
                security_list: request.security_list,
                sub_type_list: request.sub_type_list,
                is_sub_or_un_sub: request.is_sub_or_un_sub,
                is_reg_or_un_reg_push: request.is_reg_or_un_reg_push,
                reg_push_rehab_type_list: request.reg_push_rehab_type_list,
                is_first_push: request.is_first_push,
                is_unsub_all: request.is_unsub_all,
                is_sub_order_book_detail: request.is_sub_order_book_detail,
                extended_time: request.extended_time,
                session: request.session,
                header: request.header,
            },
        };
        let resp = self.request::<_, pb::qot_sub::Response>(proto_id::QOT_SUB, &req).await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        self.core.remember_subscription(&replay_request).await;
        Ok(())
    }

    pub async fn get_global_state(
        &self,
        request: pb::get_global_state::Request,
    ) -> FutuResult<pb::get_global_state::S2c> {
        let resp = self
            .request::<_, pb::get_global_state::Response>(proto_id::GET_GLOBAL_STATE, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn test_cmd(
        &self,
        request: pb::test_cmd::Request,
    ) -> FutuResult<pb::test_cmd::S2c> {
        let resp = self
            .request::<_, pb::test_cmd::Response>(proto_id::TEST_CMD, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn request_trading_days(
        &self,
        request: pb::qot_request_trade_date::Request,
    ) -> FutuResult<pb::qot_request_trade_date::S2c> {
        let resp = self
            .request::<_, pb::qot_request_trade_date::Response>(
                proto_id::QOT_REQUEST_TRADE_DATE,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_market_state(
        &self,
        request: pb::qot_get_market_state::Request,
    ) -> FutuResult<pb::qot_get_market_state::S2c> {
        let resp = self
            .request::<_, pb::qot_get_market_state::Response>(
                proto_id::QOT_GET_MARKET_STATE,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_indicator_list(
        &self,
        request: pb::qot_get_indicator_list::Request,
    ) -> FutuResult<pb::qot_get_indicator_list::S2c> {
        let resp = self
            .request::<_, pb::qot_get_indicator_list::Response>(
                proto_id::QOT_GET_INDICATOR_LIST,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn request_indicator_calc_async(
        &self,
        request: pb::qot_request_indicator_calc::Request,
    ) -> FutuResult<pb::qot_request_indicator_calc::S2c> {
        let resp = self
            .request::<_, pb::qot_request_indicator_calc::Response>(
                proto_id::QOT_REQUEST_INDICATOR_CALC,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_basic_qot(
        &self,
        request: GetBasicQotRequest,
    ) -> FutuResult<pb::qot_get_basic_qot::S2c> {
        let req = pb::qot_get_basic_qot::Request {
            c2s: pb::qot_get_basic_qot::C2s {
                security_list: request.security_list,
                header: request.header,
            },
        };
        let resp = self
            .request::<_, pb::qot_get_basic_qot::Response>(proto_id::QOT_GET_BASIC_QOT, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_kl(&self, request: GetKlRequest) -> FutuResult<pb::qot_get_kl::S2c> {
        let req = pb::qot_get_kl::Request {
            c2s: pb::qot_get_kl::C2s {
                rehab_type: request.rehab_type,
                kl_type: request.kl_type,
                security: request.security.ok_or_else(|| {
                    FutuError::Crypto("get_kl requires a security".to_owned())
                })?,
                req_num: request.req_num,
                header: request.header,
            },
        };
        let resp = self.request::<_, pb::qot_get_kl::Response>(proto_id::QOT_GET_KL, &req).await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_ticker(
        &self,
        request: GetTickerRequest,
    ) -> FutuResult<pb::qot_get_ticker::S2c> {
        let req = pb::qot_get_ticker::Request {
            c2s: pb::qot_get_ticker::C2s {
                security: request.security.ok_or_else(|| {
                    FutuError::Crypto("get_ticker requires a security".to_owned())
                })?,
                max_ret_num: request.max_ret_num,
                header: request.header,
            },
        };
        let resp = self
            .request::<_, pb::qot_get_ticker::Response>(proto_id::QOT_GET_TICKER, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_order_book(
        &self,
        request: GetOrderBookRequest,
    ) -> FutuResult<pb::qot_get_order_book::S2c> {
        let req = pb::qot_get_order_book::Request {
            c2s: pb::qot_get_order_book::C2s {
                security: request.security.ok_or_else(|| {
                    FutuError::Crypto("get_order_book requires a security".to_owned())
                })?,
                num: request.num,
                order_book_type: request.order_book_type,
                header: request.header,
            },
        };
        let resp = self
            .request::<_, pb::qot_get_order_book::Response>(proto_id::QOT_GET_ORDER_BOOK, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_static_info(
        &self,
        request: GetStaticInfoRequest,
    ) -> FutuResult<pb::qot_get_static_info::S2c> {
        let req = pb::qot_get_static_info::Request {
            c2s: pb::qot_get_static_info::C2s {
                market: request.market,
                sec_type: request.sec_type,
                security_list: request.security_list,
                header: request.header,
            },
        };
        let resp = self
            .request::<_, pb::qot_get_static_info::Response>(proto_id::QOT_GET_STATIC_INFO, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_security_snapshot(
        &self,
        request: GetSecuritySnapshotRequest,
    ) -> FutuResult<pb::qot_get_security_snapshot::S2c> {
        let req = pb::qot_get_security_snapshot::Request {
            c2s: pb::qot_get_security_snapshot::C2s {
                security_list: request.security_list,
                header: request.header,
            },
        };
        let resp = self
            .request::<_, pb::qot_get_security_snapshot::Response>(
                proto_id::QOT_GET_SECURITY_SNAPSHOT,
                &req,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn sub(&self, request: SubscribeRequest) -> FutuResult<()> {
        self.subscribe(request).await
    }

    pub async fn unsubscribe(&self, request: SubscribeRequest) -> FutuResult<()> {
        let security_list = request.security_list.clone();
        let sub_type_list = request.sub_type_list.clone();
        let reg_push_rehab_type_list = request.reg_push_rehab_type_list.clone();
        let header = request.header;
        self.subscribe(SubscribeRequest {
            is_sub_or_un_sub: false,
            ..request
        })
        .await?;

        // Match the official client behavior: after unsubscribing, also unregister push delivery
        // for the same security/subtype tuple on this connection.
        if !security_list.is_empty() && !sub_type_list.is_empty() {
            self.reg_qot_push(
                security_list,
                sub_type_list,
                reg_push_rehab_type_list,
                false,
                Some(false),
                header,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn unsub(&self, request: SubscribeRequest) -> FutuResult<()> {
        self.unsubscribe(request).await
    }

    pub async fn unsubscribe_all(&self, mut request: SubscribeRequest) -> FutuResult<()> {
        request.is_sub_or_un_sub = false;
        request.is_unsub_all = Some(true);
        self.subscribe(request).await
    }

    pub async fn unsub_all(&self, request: SubscribeRequest) -> FutuResult<()> {
        self.unsubscribe_all(request).await
    }

    async fn reg_qot_push(
        &self,
        security_list: Vec<pb::qot_common::Security>,
        sub_type_list: Vec<i32>,
        rehab_type_list: Vec<i32>,
        is_reg_or_un_reg: bool,
        is_first_push: Option<bool>,
        header: Option<pb::qot_common::QotHeader>,
    ) -> FutuResult<()> {
        let req = pb::qot_reg_qot_push::Request {
            c2s: pb::qot_reg_qot_push::C2s {
                security_list,
                sub_type_list,
                rehab_type_list,
                is_reg_or_un_reg,
                is_first_push,
                header,
            },
        };
        let resp = self
            .request::<_, pb::qot_reg_qot_push::Response>(proto_id::QOT_REG_QOT_PUSH, &req)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        Ok(())
    }

    pub async fn get_rt_data(
        &self,
        request: pb::qot_get_rt::Request,
    ) -> FutuResult<pb::qot_get_rt::S2c> {
        let resp = self
            .request::<_, pb::qot_get_rt::Response>(proto_id::QOT_GET_RT, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_broker_queue(
        &self,
        request: pb::qot_get_broker::Request,
    ) -> FutuResult<pb::qot_get_broker::S2c> {
        let resp = self
            .request::<_, pb::qot_get_broker::Response>(proto_id::QOT_GET_BROKER, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_stock_filter(
        &self,
        request: pb::qot_stock_filter::Request,
    ) -> FutuResult<pb::qot_stock_filter::S2c> {
        let resp = self
            .request::<_, pb::qot_stock_filter::Response>(proto_id::QOT_STOCK_FILTER, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_chain(
        &self,
        request: pb::qot_get_option_chain::Request,
    ) -> FutuResult<pb::qot_get_option_chain::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_chain::Response>(proto_id::QOT_GET_OPTION_CHAIN, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_warrant(
        &self,
        request: pb::qot_get_warrant::Request,
    ) -> FutuResult<pb::qot_get_warrant::S2c> {
        let resp = self
            .request::<_, pb::qot_get_warrant::Response>(proto_id::QOT_GET_WARRANT, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_quote(
        &self,
        request: pb::qot_get_option_quote::Request,
    ) -> FutuResult<pb::qot_get_option_quote::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_quote::Response>(proto_id::QOT_GET_OPTION_QUOTE, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_search_quote(
        &self,
        request: pb::qot_get_search_quote::Request,
    ) -> FutuResult<pb::qot_get_search_quote::S2c> {
        let resp = self
            .request::<_, pb::qot_get_search_quote::Response>(proto_id::QOT_GET_SEARCH_QUOTE, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_search_news(
        &self,
        request: pb::qot_get_search_news::Request,
    ) -> FutuResult<pb::qot_get_search_news::S2c> {
        let resp = self
            .request::<_, pb::qot_get_search_news::Response>(proto_id::QOT_GET_SEARCH_NEWS, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_stock_screen(
        &self,
        request: pb::qot_stock_screen::Request,
    ) -> FutuResult<pb::qot_stock_screen::S2c> {
        let resp = self
            .request::<_, pb::qot_stock_screen::Response>(proto_id::QOT_STOCK_SCREEN, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_warrant_screen(
        &self,
        request: pb::qot_warrant_screen::Request,
    ) -> FutuResult<pb::qot_warrant_screen::S2c> {
        let resp = self
            .request::<_, pb::qot_warrant_screen::Response>(proto_id::QOT_WARRANT_SCREEN, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_screen(
        &self,
        request: pb::qot_option_screen::Request,
    ) -> FutuResult<pb::qot_option_screen::S2c> {
        let resp = self
            .request::<_, pb::qot_option_screen::Response>(proto_id::QOT_OPTION_SCREEN, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_expiration_date(
        &self,
        request: pb::qot_get_option_expiration_date::Request,
    ) -> FutuResult<pb::qot_get_option_expiration_date::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_expiration_date::Response>(
                proto_id::QOT_GET_OPTION_EXPIRATION_DATE,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_market_statistic(
        &self,
        request: pb::qot_get_option_market_statistic::Request,
    ) -> FutuResult<pb::qot_get_option_market_statistic::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_market_statistic::Response>(
                proto_id::QOT_GET_OPTION_MARKET_STATISTIC,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_underlying_his_statistic(
        &self,
        request: pb::qot_get_option_underlying_his_statistic::Request,
    ) -> FutuResult<pb::qot_get_option_underlying_his_statistic::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_underlying_his_statistic::Response>(
                proto_id::QOT_GET_OPTION_UNDERLYING_HIS_STATISTIC,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_underlying_overview(
        &self,
        request: pb::qot_get_option_underlying_overview::Request,
    ) -> FutuResult<pb::qot_get_option_underlying_overview::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_underlying_overview::Response>(
                proto_id::QOT_GET_OPTION_UNDERLYING_OVERVIEW,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_underlying_his_volatility(
        &self,
        request: pb::qot_get_option_underlying_his_volatility::Request,
    ) -> FutuResult<pb::qot_get_option_underlying_his_volatility::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_underlying_his_volatility::Response>(
                proto_id::QOT_GET_OPTION_UNDERLYING_HIS_VOLATILITY,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_underlying_rank(
        &self,
        request: pb::qot_get_option_underlying_rank::Request,
    ) -> FutuResult<pb::qot_get_option_underlying_rank::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_underlying_rank::Response>(
                proto_id::QOT_GET_OPTION_UNDERLYING_RANK,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_rank(
        &self,
        request: pb::qot_get_option_rank::Request,
    ) -> FutuResult<pb::qot_get_option_rank::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_rank::Response>(proto_id::QOT_GET_OPTION_RANK, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_event(
        &self,
        request: pb::qot_get_option_event::Request,
    ) -> FutuResult<pb::qot_get_option_event::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_event::Response>(proto_id::QOT_GET_OPTION_EVENT, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_event_alert(
        &self,
        request: pb::qot_get_option_event_alert::Request,
    ) -> FutuResult<pb::qot_get_option_event_alert::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_event_alert::Response>(
                proto_id::QOT_GET_OPTION_EVENT_ALERT,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn set_option_event_alert(
        &self,
        request: pb::qot_set_option_event_alert::Request,
    ) -> FutuResult<()> {
        let resp = self
            .request::<_, pb::qot_set_option_event_alert::Response>(
                proto_id::QOT_SET_OPTION_EVENT_ALERT,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        Ok(())
    }

    pub async fn get_option_zero_dte_screener(
        &self,
        request: pb::qot_get_option_zero_dte_screener::Request,
    ) -> FutuResult<pb::qot_get_option_zero_dte_screener::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_zero_dte_screener::Response>(
                proto_id::QOT_GET_OPTION_ZERO_DTE_SCREENER,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_zero_dte_contract(
        &self,
        request: pb::qot_get_option_zero_dte_contract::Request,
    ) -> FutuResult<pb::qot_get_option_zero_dte_contract::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_zero_dte_contract::Response>(
                proto_id::QOT_GET_OPTION_ZERO_DTE_CONTRACT,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_earnings_screener(
        &self,
        request: pb::qot_get_option_earnings_screener::Request,
    ) -> FutuResult<pb::qot_get_option_earnings_screener::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_earnings_screener::Response>(
                proto_id::QOT_GET_OPTION_EARNINGS_SCREENER,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_option_seller_screener(
        &self,
        request: pb::qot_get_option_seller_screener::Request,
    ) -> FutuResult<pb::qot_get_option_seller_screener::S2c> {
        let resp = self
            .request::<_, pb::qot_get_option_seller_screener::Response>(
                proto_id::QOT_GET_OPTION_SELLER_SCREENER,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_sub_list(
        &self,
        request: pb::qot_get_sub_info::Request,
    ) -> FutuResult<pb::qot_get_sub_info::S2c> {
        let resp = self
            .request::<_, pb::qot_get_sub_info::Response>(proto_id::QOT_GET_SUB_INFO, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn query_subscription(
        &self,
        request: pb::qot_get_sub_info::Request,
    ) -> FutuResult<pb::qot_get_sub_info::S2c> {
        self.get_sub_list(request).await
    }

    pub async fn stock_quote(
        &self,
        request: GetBasicQotRequest,
    ) -> FutuResult<pb::qot_get_basic_qot::S2c> {
        self.get_basic_qot(request).await
    }

    pub async fn get_stock_quote(
        &self,
        request: GetBasicQotRequest,
    ) -> FutuResult<pb::qot_get_basic_qot::S2c> {
        self.get_basic_qot(request).await
    }

    pub async fn get_market_snapshot(
        &self,
        request: GetSecuritySnapshotRequest,
    ) -> FutuResult<pb::qot_get_security_snapshot::S2c> {
        self.get_security_snapshot(request).await
    }

    pub async fn get_cur_kline(&self, request: GetKlRequest) -> FutuResult<pb::qot_get_kl::S2c> {
        self.get_kl(request).await
    }

    pub async fn get_rt_ticker(
        &self,
        request: GetTickerRequest,
    ) -> FutuResult<pb::qot_get_ticker::S2c> {
        self.get_ticker(request).await
    }

    pub async fn get_stock_basicinfo(
        &self,
        request: GetStaticInfoRequest,
    ) -> FutuResult<pb::qot_get_static_info::S2c> {
        self.get_static_info(request).await
    }

    pub async fn request_history_kline(
        &self,
        request: pb::qot_request_history_kl::Request,
    ) -> FutuResult<pb::qot_request_history_kl::S2c> {
        let resp = self
            .request::<_, pb::qot_request_history_kl::Response>(
                proto_id::QOT_REQUEST_HISTORY_KL,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn request_history_kl_quota(
        &self,
        request: pb::qot_request_history_kl_quota::Request,
    ) -> FutuResult<pb::qot_request_history_kl_quota::S2c> {
        let resp = self
            .request::<_, pb::qot_request_history_kl_quota::Response>(
                proto_id::QOT_REQUEST_HISTORY_KL_QUOTA,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_rehab(
        &self,
        request: pb::qot_request_rehab::Request,
    ) -> FutuResult<pb::qot_request_rehab::S2c> {
        let resp = self
            .request::<_, pb::qot_request_rehab::Response>(proto_id::QOT_REQUEST_REHAB, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_user_info(
        &self,
        request: pb::get_user_info::Request,
    ) -> FutuResult<pb::get_user_info::S2c> {
        let resp = self
            .request::<_, pb::get_user_info::Response>(proto_id::GET_USER_INFO, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_price_reminder(
        &self,
        request: pb::qot_get_price_reminder::Request,
    ) -> FutuResult<pb::qot_get_price_reminder::S2c> {
        let resp = self
            .request::<_, pb::qot_get_price_reminder::Response>(
                proto_id::QOT_GET_PRICE_REMINDER,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn set_price_reminder(
        &self,
        request: pb::qot_set_price_reminder::Request,
    ) -> FutuResult<u64> {
        let resp = self
            .request::<_, pb::qot_set_price_reminder::Response>(
                proto_id::QOT_SET_PRICE_REMINDER,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        let s2c = resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })?;
        Ok(s2c.key.max(0) as u64)
    }

    pub async fn get_user_security(
        &self,
        request: pb::qot_get_user_security::Request,
    ) -> FutuResult<pb::qot_get_user_security::S2c> {
        let resp = self
            .request::<_, pb::qot_get_user_security::Response>(
                proto_id::QOT_GET_USER_SECURITY,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn modify_user_security(
        &self,
        request: pb::qot_modify_user_security::Request,
    ) -> FutuResult<pb::qot_modify_user_security::S2c> {
        let resp = self
            .request::<_, pb::qot_modify_user_security::Response>(
                proto_id::QOT_MODIFY_USER_SECURITY,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_user_security_group(
        &self,
        request: pb::qot_get_user_security_group::Request,
    ) -> FutuResult<pb::qot_get_user_security_group::S2c> {
        let resp = self
            .request::<_, pb::qot_get_user_security_group::Response>(
                proto_id::QOT_GET_USER_SECURITY_GROUP,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn verification(
        &self,
        request: pb::verification::Request,
    ) -> FutuResult<pb::verification::S2c> {
        let resp = self
            .request::<_, pb::verification::Response>(proto_id::VERIFICATION, &request)
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    pub async fn get_delay_statistics(
        &self,
        request: pb::get_delay_statistics::Request,
    ) -> FutuResult<pb::get_delay_statistics::S2c> {
        let resp = self
            .request::<_, pb::get_delay_statistics::Response>(
                proto_id::GET_DELAY_STATISTICS,
                &request,
            )
            .await?;
        ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        resp.s2c.ok_or_else(|| FutuError::OpenDError {
            ret_type: resp.ret_type,
            ret_msg: resp.ret_msg.clone(),
        })
    }

    quote_passthrough!(get_financials_earnings_price_move, QOT_GET_FINANCIALS_EARNINGS_PRICE_MOVE, qot_get_financials_earnings_price_move);
    quote_passthrough!(get_financials_earnings_price_history, QOT_GET_FINANCIALS_EARNINGS_PRICE_HISTORY, qot_get_financials_earnings_price_history);
    quote_passthrough!(get_financials_statements, QOT_GET_FINANCIALS_STATEMENTS, qot_get_financials_statements);
    quote_passthrough!(get_financials_revenue_breakdown, QOT_GET_FINANCIALS_REVENUE_BREAKDOWN, qot_get_financials_revenue_breakdown);
    quote_passthrough!(get_research_analyst_consensus, QOT_GET_RESEARCH_ANALYST_CONSENSUS, qot_get_research_analyst_consensus);
    quote_passthrough!(get_research_rating_summary, QOT_GET_RESEARCH_RATING_SUMMARY, qot_get_research_rating_summary);
    quote_passthrough!(get_research_morningstar_report, QOT_GET_RESEARCH_MORNINGSTAR_REPORT, qot_get_research_morningstar_report);
    quote_passthrough!(get_valuation_detail, QOT_GET_VALUATION_DETAIL, qot_get_valuation_detail);
    quote_passthrough!(get_valuation_plate_stock_list, QOT_GET_VALUATION_PLATE_STOCK_LIST, qot_get_valuation_plate_stock_list);
    quote_passthrough!(get_corporate_actions_dividends, QOT_GET_CORPORATE_ACTIONS_DIVIDENDS, qot_get_corporate_actions_dividends);
    quote_passthrough!(get_corporate_actions_buybacks, QOT_GET_CORPORATE_ACTIONS_BUYBACKS, qot_get_corporate_actions_buybacks);
    quote_passthrough!(get_corporate_actions_stock_splits, QOT_GET_CORPORATE_ACTIONS_STOCK_SPLITS, qot_get_corporate_actions_stock_splits);
    quote_passthrough!(get_shareholders_overview, QOT_GET_SHAREHOLDERS_OVERVIEW, qot_get_shareholders_overview);
    quote_passthrough!(get_shareholders_holding_changes, QOT_GET_SHAREHOLDERS_HOLDING_CHANGES, qot_get_shareholders_holding_changes);
    quote_passthrough!(get_shareholders_holder_detail, QOT_GET_SHAREHOLDERS_HOLDER_DETAIL, qot_get_shareholders_holder_detail);
    quote_passthrough!(get_shareholders_institutional, QOT_GET_SHAREHOLDERS_INSTITUTIONAL, qot_get_shareholders_institutional);
    quote_passthrough!(get_insider_holder_list, QOT_GET_INSIDER_HOLDER_LIST, qot_get_insider_holder_list);
    quote_passthrough!(get_insider_trade_list, QOT_GET_INSIDER_TRADE_LIST, qot_get_insider_trade_list);
    quote_passthrough!(get_company_profile, QOT_GET_COMPANY_PROFILE, qot_get_company_profile);
    quote_passthrough!(get_company_executives, QOT_GET_COMPANY_EXECUTIVES, qot_get_company_executives);
    quote_passthrough!(get_company_executive_background, QOT_GET_COMPANY_EXECUTIVE_BACKGROUND, qot_get_company_executive_background);
    quote_passthrough!(get_company_operational_efficiency, QOT_GET_COMPANY_OPERATIONAL_EFFICIENCY, qot_get_company_operational_efficiency);
    quote_passthrough!(get_top_ten_buy_sell_brokers, QOT_GET_TOP_TEN_BUY_SELL_BROKERS, qot_get_top_ten_buy_sell_brokers);
    quote_passthrough!(get_daily_short_volume, QOT_GET_DAILY_SHORT_VOLUME, qot_get_daily_short_volume);
    quote_passthrough!(get_short_interest, QOT_GET_SHORT_INTEREST, qot_get_short_interest);
    quote_passthrough!(get_option_exercise_probability, QOT_GET_OPTION_EXERCISE_PROBABILITY, qot_get_option_exercise_probability);
    quote_passthrough!(get_option_volatility, QOT_GET_OPTION_VOLATILITY, qot_get_option_volatility);
    quote_passthrough!(get_earnings_calendar, QOT_GET_EARNINGS_CALENDAR, qot_get_earnings_calendar);
    quote_passthrough!(get_macro_indicator_list, QOT_GET_MACRO_INDICATOR_LIST, qot_get_macro_indicator_list);
    quote_passthrough!(get_macro_indicator_history, QOT_GET_MACRO_INDICATOR_HISTORY, qot_get_macro_indicator_history);
    quote_passthrough!(get_fed_watch_target_rate, QOT_GET_FED_WATCH_TARGET_RATE, qot_get_fed_watch_target_rate);
    quote_passthrough!(get_fed_watch_dot_plot, QOT_GET_FED_WATCH_DOT_PLOT, qot_get_fed_watch_dot_plot);
    quote_passthrough!(get_economic_calendar, QOT_GET_ECONOMIC_CALENDAR, qot_get_economic_calendar);
    quote_passthrough!(get_earnings_beat_rank, QOT_GET_EARNINGS_BEAT_RANK, qot_get_earnings_beat_rank);
    quote_passthrough!(get_dividend_rank, QOT_GET_DIVIDEND_RANK, qot_get_dividend_rank);
    quote_passthrough!(get_dividend_calendar, QOT_GET_DIVIDEND_CALENDAR, qot_get_dividend_calendar);
    quote_passthrough!(get_us_pre_market_rank, QOT_GET_US_PRE_MARKET_RANK, qot_get_us_pre_market_rank);
    quote_passthrough!(get_us_after_hours_rank, QOT_GET_US_AFTER_HOURS_RANK, qot_get_us_after_hours_rank);
    quote_passthrough!(get_us_overnight_rank, QOT_GET_US_OVERNIGHT_RANK, qot_get_us_overnight_rank);
    quote_passthrough!(get_top_movers_rank, QOT_GET_TOP_MOVERS_RANK, qot_get_top_movers_rank);
    quote_passthrough!(get_hot_list, QOT_GET_HOT_LIST, qot_get_hot_list);
    quote_passthrough!(get_short_selling_rank, QOT_GET_SHORT_SELLING_RANK, qot_get_short_selling_rank);
    quote_passthrough!(get_period_change_rank, QOT_GET_PERIOD_CHANGE_RANK, qot_get_period_change_rank);
    quote_passthrough!(get_high_dividend_soe_rank, QOT_GET_HIGH_DIVIDEND_SOE_RANK, qot_get_high_dividend_soe_rank);
    quote_passthrough!(get_institution_list, QOT_GET_INSTITUTION_LIST, qot_get_institution_list);
    quote_passthrough!(get_institution_profile, QOT_GET_INSTITUTION_PROFILE, qot_get_institution_profile);
    quote_passthrough!(get_institution_distribution, QOT_GET_INSTITUTION_DISTRIBUTION, qot_get_institution_distribution);
    quote_passthrough!(get_institution_holding_change, QOT_GET_INSTITUTION_HOLDING_CHANGE, qot_get_institution_holding_change);
    quote_passthrough!(get_institution_holding_list, QOT_GET_INSTITUTION_HOLDING_LIST, qot_get_institution_holding_list);
    quote_passthrough!(get_ark_fund_holding, QOT_GET_ARK_FUND_HOLDING, qot_get_ark_fund_holding);
    quote_passthrough!(get_ark_stock_dynamic, QOT_GET_ARK_STOCK_DYNAMIC, qot_get_ark_stock_dynamic);
    quote_passthrough!(get_ark_active_transaction, QOT_GET_ARK_ACTIVE_TRANSACTION, qot_get_ark_active_transaction);
    quote_passthrough!(get_rating_change, QOT_GET_RATING_CHANGE, qot_get_rating_change);
    quote_passthrough!(get_industrial_chain_list, QOT_GET_INDUSTRIAL_CHAIN_LIST, qot_get_industrial_chain_list);
    quote_passthrough!(get_industrial_chain_detail, QOT_GET_INDUSTRIAL_CHAIN_DETAIL, qot_get_industrial_chain_detail);
    quote_passthrough!(get_industrial_chain_by_plate, QOT_GET_INDUSTRIAL_CHAIN_BY_PLATE, qot_get_industrial_chain_by_plate);
    quote_passthrough!(get_industrial_plate_info, QOT_GET_INDUSTRIAL_PLATE_INFO, qot_get_industrial_plate_info);
    quote_passthrough!(get_industrial_plate_stock, QOT_GET_INDUSTRIAL_PLATE_STOCK, qot_get_industrial_plate_stock);
    quote_passthrough!(get_heat_map_data, QOT_GET_HEAT_MAP_DATA, qot_get_heat_map_data);
    quote_passthrough!(get_rise_fall_distribution, QOT_GET_RISE_FALL_DISTRIBUTION, qot_get_rise_fall_distribution);
    quote_passthrough!(get_reference, QOT_GET_REFERENCE, qot_get_reference);
    quote_passthrough!(get_owner_plate, QOT_GET_OWNER_PLATE, qot_get_owner_plate);
    quote_passthrough!(get_holding_change_list, QOT_GET_HOLDING_CHANGE_LIST, qot_get_holding_change_list);
    quote_passthrough!(get_plate_list, QOT_GET_PLATE_SET, qot_get_plate_set);
    quote_passthrough!(get_plate_stock, QOT_GET_PLATE_SECURITY, qot_get_plate_security);
    quote_passthrough!(get_code_change, QOT_GET_CODE_CHANGE, qot_get_code_change);
    quote_passthrough!(get_capital_flow, QOT_GET_CAPITAL_FLOW, qot_get_capital_flow);
    quote_passthrough!(get_capital_distribution, QOT_GET_CAPITAL_DISTRIBUTION, qot_get_capital_distribution);
    quote_passthrough!(get_future_info, QOT_GET_FUTURE_INFO, qot_get_future_info);
    quote_passthrough!(get_ipo_list, QOT_GET_IPO_LIST, qot_get_ipo_list);
    quote_passthrough!(get_option_strategy, QOT_GET_OPTION_STRATEGY, qot_get_option_strategy);
    quote_passthrough!(get_option_strategy_analysis, QOT_GET_OPTION_STRATEGY_ANALYSIS, qot_get_option_strategy_analysis);
    quote_passthrough!(get_option_strategy_spread, QOT_GET_OPTION_STRATEGY_SPREAD, qot_get_option_strategy_spread);
    skill_wrap_unusual!(get_technical_unusual, SKILL_WRAP_TECHNICAL_UNUSUAL, TechnicalUnusualReq, TechnicalUnusualRsp);
    skill_wrap_unusual!(get_financial_unusual, SKILL_WRAP_FINANCIAL_UNUSUAL, FinancialUnusualReq, FinancialUnusualRsp);
    skill_wrap_unusual!(get_derivative_unusual, SKILL_WRAP_DERIVATIVE_UNUSUAL, DerivativeUnusualReq, DerivativeUnusualRsp);

    pub async fn get_history_kl_quota(
        &self,
        request: pb::qot_request_history_kl_quota::Request,
    ) -> FutuResult<pb::qot_request_history_kl_quota::S2c> {
        self.request_history_kl_quota(request).await
    }

    pub async fn get_referencestock_list(
        &self,
        request: pb::qot_get_reference::Request,
    ) -> FutuResult<pb::qot_get_reference::S2c> {
        self.get_reference(request).await
    }

    async fn request<Req, Resp>(&self, proto_id: u32, request: &Req) -> FutuResult<Resp>
    where
        Req: Message,
        Resp: Message + Default,
    {
        self.core.request(proto_id, request).await
    }
}
