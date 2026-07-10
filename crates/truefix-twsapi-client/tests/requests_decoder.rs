use prost::Message;
use truefix_twsapi_client::comm;
use truefix_twsapi_client::decoder;
use truefix_twsapi_client::events::Event;
use truefix_twsapi_client::message::{Incoming, Outgoing, PROTOBUF_MSG_ID};
use truefix_twsapi_client::protobuf;
use truefix_twsapi_client::requests::{
    AccountSummaryRequest, AccountUpdatesMultiRequest, CalculateImpliedVolatilityRequest,
    CalculateOptionPriceRequest, CancelMarketDepthRequest, CancelOrderRequest,
    CompletedOrdersRequest, ContractDetailsRequest, EmptyRequest, EncodableRequest,
    ExecutionRequest, ExerciseOptionsRequest, FieldSink, FinancialAdvisorRequest,
    GlobalCancelRequest, HeadTimestampRequest, HistogramDataRequest, HistoricalDataRequest,
    HistoricalTicksRequest, IdRequest, MarketDataRequest, MarketDepthRequest, NewsArticleRequest,
    PlaceOrderRequest, PnlSingleRequest, RealTimeBarsRequest, ReplaceFinancialAdvisorRequest,
    ScannerSubscriptionRequest, StartApiRequest, SubscribeToGroupEventsRequest, TickByTickRequest,
    UpdateDisplayGroupRequest, VerifyAndAuthMessageRequest, VerifyAndAuthRequest,
    VerifyMessageRequest, VerifyRequest, VersionedRequest, WshEventDataRequest,
    encode_request_frame, encode_request_frame_with_protobuf, protobuf_min_server_version,
};
use truefix_twsapi_client::server_versions::{
    MAX_CLIENT_VER, MIN_SERVER_VER_LINKING, MIN_SERVER_VER_MANUAL_ORDER_TIME,
    MIN_SERVER_VER_MKT_DEPTH_PRIM_EXCHANGE, MIN_SERVER_VER_PROTOBUF_MARKET_DATA,
    MIN_SERVER_VER_REPLACE_FA_END, MIN_SERVER_VER_RFQ_FIELDS, MIN_SERVER_VER_SCANNER_GENERIC_OPTS,
    MIN_SERVER_VER_SMART_DEPTH, MIN_SERVER_VER_TRADING_CLASS,
};
use truefix_twsapi_client::types::{
    Contract, DepthMarketDataDescription, ExecutionFilter, FamilyCode, HistogramEntry,
    NewsProvider, Order, OrderCancel, Origin, PriceIncrement, ScannerSubscription, TagValue,
    TickByTick as TickByTickPayload,
};

#[test]
fn protobuf_offset_matches_official_common_py() {
    assert_eq!(PROTOBUF_MSG_ID, 200);
    assert_eq!(Outgoing::ReqCurrentTime.protobuf_id(), 249);
}

#[test]
fn protobuf_gate_matches_market_data_mapping() {
    assert_eq!(
        protobuf_min_server_version(Outgoing::ReqMktData),
        Some(MIN_SERVER_VER_PROTOBUF_MARKET_DATA)
    );
}

#[test]
fn market_data_request_encodes_core_contract_fields() {
    let request = MarketDataRequest {
        req_id: 42,
        contract: Contract {
            con_id: 265598,
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        generic_tick_list: "233".to_owned(),
        snapshot: false,
        regulatory_snapshot: false,
        market_data_options: Vec::new(),
    };

    let mut fields = FieldSink::default();
    request
        .encode_fields_for_server_version(&mut fields, MAX_CLIENT_VER)
        .unwrap();
    let encoded = fields.into_string();
    let parts = field_strings(&encoded);

    assert_eq!(
        parts,
        vec![
            "11", "42", "265598", "AAPL", "STK", "", "", "", "", "SMART", "", "USD", "", "", "0",
            "233", "0", "0", ""
        ]
    );
}

#[test]
fn request_frame_uses_protobuf_when_server_supports_it() {
    let request = MarketDataRequest {
        req_id: 42,
        contract: Contract {
            con_id: 265598,
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        generic_tick_list: "233".to_owned(),
        snapshot: false,
        regulatory_snapshot: false,
        market_data_options: Vec::new(),
    };

    let frame = encode_request_frame(&request, MIN_SERVER_VER_PROTOBUF_MARKET_DATA).unwrap();
    assert_eq!(
        &frame[4..8],
        &Outgoing::ReqMktData.protobuf_id().to_be_bytes()
    );

    let decoded = protobuf::MarketDataRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(42));
    assert_eq!(
        decoded
            .contract
            .as_ref()
            .and_then(|contract| contract.symbol.as_deref()),
        Some("AAPL")
    );
}

#[test]
fn request_frame_uses_field_encoding_before_protobuf_min_version() {
    let request = MarketDataRequest {
        req_id: 42,
        contract: Contract {
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        generic_tick_list: "233".to_owned(),
        snapshot: false,
        regulatory_snapshot: false,
        market_data_options: Vec::new(),
    };

    let frame = encode_request_frame(&request, 100).unwrap();
    let payload = &frame[4..];
    assert!(payload.starts_with(b"1\0"));
    let fields = field_strings(&String::from_utf8_lossy(&payload[2..]));
    assert_eq!(fields.first().map(String::as_str), Some("11"));
}

#[test]
fn request_frame_can_force_field_encoding_before_api_ready() {
    let request = MarketDataRequest {
        req_id: 42,
        contract: Contract {
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        generic_tick_list: String::new(),
        snapshot: false,
        regulatory_snapshot: false,
        market_data_options: Vec::new(),
    };

    let frame = encode_request_frame_with_protobuf(&request, MAX_CLIENT_VER, false).unwrap();
    let payload = &frame[4..];

    assert_eq!(&payload[..4], &Outgoing::ReqMktData.id().to_be_bytes());
    assert_ne!(
        &payload[..4],
        &Outgoing::ReqMktData.protobuf_id().to_be_bytes()
    );
    assert_eq!(
        field_strings(&String::from_utf8_lossy(&payload[4..]))
            .first()
            .map(String::as_str),
        Some("11")
    );
}

#[test]
fn simple_request_frames_use_protobuf_when_supported() {
    let frame = encode_request_frame(
        &VersionedRequest {
            message: Outgoing::ReqCurrentTime,
            version: 1,
        },
        MAX_CLIENT_VER,
    )
    .unwrap();
    assert_eq!(
        &frame[4..8],
        &Outgoing::ReqCurrentTime.protobuf_id().to_be_bytes()
    );
    protobuf::CurrentTimeRequest::decode(&frame[8..]).unwrap();

    let frame = encode_request_frame(
        &IdRequest {
            message: Outgoing::CancelMktData,
            version: Some(1),
            req_id: 91,
        },
        MAX_CLIENT_VER,
    )
    .unwrap();
    assert_eq!(
        &frame[4..8],
        &Outgoing::CancelMktData.protobuf_id().to_be_bytes()
    );
    let decoded = protobuf::CancelMarketData::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(91));
}

#[test]
fn start_api_request_stays_field_encoded_until_api_is_accepted() {
    let frame = encode_request_frame(
        &StartApiRequest {
            client_id: 1002,
            optional_capabilities: None,
            include_optional_capabilities: false,
        },
        MAX_CLIENT_VER,
    )
    .unwrap();
    let payload = &frame[4..];

    assert_eq!(&payload[..4], &Outgoing::StartApi.id().to_be_bytes());
    assert_ne!(
        &payload[..4],
        &Outgoing::StartApi.protobuf_id().to_be_bytes()
    );
    assert_eq!(
        field_strings(&String::from_utf8_lossy(&payload[4..])),
        vec!["2", "1002"]
    );
    assert_eq!(protobuf_min_server_version(Outgoing::StartApi), None);
}

#[test]
fn typed_request_frames_use_protobuf_when_supported() {
    let depth = MarketDepthRequest {
        req_id: 21,
        contract: Contract {
            symbol: "MSFT".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        num_rows: 10,
        is_smart_depth: true,
        market_depth_options: vec![TagValue {
            tag: "exchange".to_owned(),
            value: "ISLAND".to_owned(),
        }],
    };
    let frame = encode_request_frame(&depth, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::MarketDepthRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(21));
    assert_eq!(decoded.num_rows, Some(10));
    assert_eq!(decoded.is_smart_depth, Some(true));

    let cancel = CancelOrderRequest {
        order_id: 7,
        order_cancel: OrderCancel {
            manual_order_cancel_time: "20260708-12:00:00".to_owned(),
            ext_operator: "op".to_owned(),
            manual_order_indicator: 1,
        },
    };
    let frame = encode_request_frame(&cancel, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::CancelOrderRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.order_id, Some(7));
    assert_eq!(
        decoded.order_cancel.and_then(|cancel| cancel.ext_operator),
        Some("op".to_owned())
    );

    let execution = ExecutionRequest {
        req_id: 8,
        filter: ExecutionFilter {
            client_id: 2,
            acct_code: "DU123".to_owned(),
            symbol: "AAPL".to_owned(),
            last_n_days: 5,
            specific_dates: vec![20260701, 20260702],
            ..ExecutionFilter::default()
        },
    };
    let frame = encode_request_frame(&execution, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::ExecutionRequest::decode(&frame[8..]).unwrap();
    let filter = decoded.execution_filter.unwrap();
    assert_eq!(filter.symbol.as_deref(), Some("AAPL"));
    assert_eq!(filter.last_n_days, Some(5));
    assert_eq!(filter.specific_dates, [20260701, 20260702]);

    let frame = encode_request_frame_with_protobuf(&execution, 200, false).unwrap();
    let fields = field_strings(&String::from_utf8_lossy(&frame[8..]));
    assert!(fields.ends_with(&[
        "5".to_owned(),
        "2".to_owned(),
        "20260701".to_owned(),
        "20260702".to_owned(),
    ]));
}

#[test]
fn scanner_subscription_request_uses_protobuf_when_supported() {
    let request = ScannerSubscriptionRequest {
        req_id: 33,
        subscription: ScannerSubscription {
            number_of_rows: 50,
            instrument: "STK".to_owned(),
            location_code: "STK.US.MAJOR".to_owned(),
            scan_code: "TOP_PERC_GAIN".to_owned(),
            above_price: 5.0,
            ..ScannerSubscription::default()
        },
        scanner_subscription_options: vec![TagValue {
            tag: "opt".to_owned(),
            value: "1".to_owned(),
        }],
        scanner_subscription_filter_options: vec![TagValue {
            tag: "filter".to_owned(),
            value: "yes".to_owned(),
        }],
    };

    let frame = encode_request_frame(&request, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::ScannerSubscriptionRequest::decode(&frame[8..]).unwrap();
    let subscription = decoded.scanner_subscription.unwrap();
    assert_eq!(decoded.req_id, Some(33));
    assert_eq!(subscription.instrument, Some("STK".to_owned()));
    assert_eq!(
        subscription.scanner_subscription_options.get("opt"),
        Some(&"1".to_owned())
    );
    assert_eq!(
        subscription
            .scanner_subscription_filter_options
            .get("filter"),
        Some(&"yes".to_owned())
    );
}

#[test]
fn empty_request_keeps_old_field_payload_empty_and_new_payload_protobuf() {
    let request = EmptyRequest {
        message: Outgoing::ReqNewsProviders,
    };

    let old_frame = encode_request_frame(&request, 100).unwrap();
    assert_eq!(&old_frame[4..], b"85\0");

    let new_frame = encode_request_frame(&request, MAX_CLIENT_VER).unwrap();
    assert_eq!(
        &new_frame[4..8],
        &Outgoing::ReqNewsProviders.protobuf_id().to_be_bytes()
    );
    protobuf::NewsProvidersRequest::decode(&new_frame[8..]).unwrap();
}

#[test]
fn account_and_pnl_requests_use_protobuf_when_supported() {
    let account = AccountSummaryRequest {
        req_id: 7,
        group_name: "All".to_owned(),
        tags: "NetLiquidation".to_owned(),
    };
    let frame = encode_request_frame(&account, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::AccountSummaryRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(7));
    assert_eq!(decoded.group.as_deref(), Some("All"));

    let updates = AccountUpdatesMultiRequest {
        req_id: 8,
        account: "DU123".to_owned(),
        model_code: "MODEL".to_owned(),
        ledger_and_nlv: true,
    };
    let frame = encode_request_frame(&updates, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::AccountUpdatesMultiRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(8));
    assert_eq!(decoded.ledger_and_nlv, Some(true));

    let pnl = PnlSingleRequest {
        req_id: 9,
        account: "DU123".to_owned(),
        model_code: String::new(),
        con_id: 265598,
    };
    let frame = encode_request_frame(&pnl, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::PnLSingleRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(9));
    assert_eq!(decoded.con_id, Some(265598));
}

#[test]
fn news_and_completed_order_requests_use_protobuf_when_supported() {
    let news = NewsArticleRequest {
        req_id: 1,
        provider_code: "BRFG".to_owned(),
        article_id: "A1".to_owned(),
        options: vec![TagValue {
            tag: "format".to_owned(),
            value: "text".to_owned(),
        }],
    };
    let frame = encode_request_frame(&news, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::NewsArticleRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(1));
    assert_eq!(
        decoded
            .news_article_options
            .get("format")
            .map(String::as_str),
        Some("text")
    );

    let completed = CompletedOrdersRequest { api_only: true };
    let frame = encode_request_frame(&completed, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::CompletedOrdersRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.api_only, Some(true));
}

#[test]
fn fa_display_and_verify_requests_use_protobuf_when_supported() {
    let fa = FinancialAdvisorRequest { fa_data_type: 1 };
    let frame = encode_request_frame(&fa, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::FaRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.fa_data_type, Some(1));

    let replace = ReplaceFinancialAdvisorRequest {
        req_id: 2,
        fa_data_type: 3,
        xml: "<xml/>".to_owned(),
    };
    let frame = encode_request_frame(&replace, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::FaReplace::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(2));
    assert_eq!(decoded.xml.as_deref(), Some("<xml/>"));

    let subscribe = SubscribeToGroupEventsRequest {
        req_id: 4,
        group_id: 5,
    };
    let frame = encode_request_frame(&subscribe, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::SubscribeToGroupEventsRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.group_id, Some(5));

    let update = UpdateDisplayGroupRequest {
        req_id: 6,
        contract_info: "AAPL@SMART".to_owned(),
    };
    let frame = encode_request_frame(&update, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::UpdateDisplayGroupRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.contract_info.as_deref(), Some("AAPL@SMART"));

    let verify = VerifyRequest {
        api_name: "api".to_owned(),
        api_version: "1".to_owned(),
    };
    let frame = encode_request_frame(&verify, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::VerifyRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.api_name.as_deref(), Some("api"));

    let verify_message = VerifyMessageRequest {
        api_data: "payload".to_owned(),
    };
    let frame = encode_request_frame(&verify_message, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::VerifyMessageRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.api_data.as_deref(), Some("payload"));
}

#[test]
fn historical_realtime_and_wsh_requests_use_protobuf_when_supported() {
    let contract = Contract {
        con_id: 265598,
        symbol: "AAPL".to_owned(),
        sec_type: "STK".to_owned(),
        exchange: "SMART".to_owned(),
        currency: "USD".to_owned(),
        ..Contract::default()
    };

    let head = HeadTimestampRequest {
        req_id: 10,
        contract: contract.clone(),
        use_rth: true,
        what_to_show: "TRADES".to_owned(),
        format_date: 1,
    };
    let frame = encode_request_frame(&head, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::HeadTimestampRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(10));
    assert_eq!(decoded.what_to_show.as_deref(), Some("TRADES"));

    let realtime = RealTimeBarsRequest {
        req_id: 11,
        contract,
        bar_size: 5,
        what_to_show: "MIDPOINT".to_owned(),
        use_rth: false,
        options: vec![TagValue {
            tag: "opt".to_owned(),
            value: "1".to_owned(),
        }],
    };
    let frame = encode_request_frame(&realtime, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::RealTimeBarsRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(11));
    assert_eq!(
        decoded
            .real_time_bars_options
            .get("opt")
            .map(String::as_str),
        Some("1")
    );

    let wsh = WshEventDataRequest {
        req_id: 12,
        con_id: 265598,
        filter: "{\"watchlist\":true}".to_owned(),
        fill_watchlist: true,
        fill_portfolio: false,
        fill_competitors: true,
        start_date: "20260701".to_owned(),
        end_date: "20260709".to_owned(),
        total_limit: 100,
    };
    let frame = encode_request_frame(&wsh, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::WshEventDataRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(12));
    assert_eq!(decoded.con_id, Some(265598));
    assert_eq!(decoded.fill_watchlist, Some(true));
}

#[test]
fn complex_contract_requests_match_python_field_order() {
    let contract = Contract {
        con_id: 265598,
        symbol: "AAPL".to_owned(),
        sec_type: "STK".to_owned(),
        exchange: "SMART".to_owned(),
        primary_exchange: "NASDAQ".to_owned(),
        currency: "USD".to_owned(),
        local_symbol: "AAPL".to_owned(),
        trading_class: "NMS".to_owned(),
        include_expired: true,
        ..Contract::default()
    };

    let tick = TickByTickRequest {
        req_id: 1,
        contract: contract.clone(),
        tick_type: "Last".to_owned(),
        number_of_ticks: 10,
        ignore_size: true,
    };
    let mut fields = FieldSink::default();
    tick.encode_fields_for_server_version(&mut fields, MAX_CLIENT_VER)
        .unwrap();
    assert_eq!(
        field_strings(&fields.into_string()),
        vec![
            "1", "265598", "AAPL", "STK", "", "", "", "", "SMART", "NASDAQ", "USD", "AAPL", "NMS",
            "Last", "10", "1"
        ]
    );

    let historical_ticks = HistoricalTicksRequest {
        req_id: 2,
        contract,
        start_date_time: "20260708 09:30:00 US/Eastern".to_owned(),
        end_date_time: "20260708 16:00:00 US/Eastern".to_owned(),
        number_of_ticks: 100,
        what_to_show: "TRADES".to_owned(),
        use_rth: true,
        ignore_size: false,
        misc_options: vec![TagValue {
            tag: "opt".to_owned(),
            value: "1".to_owned(),
        }],
    };
    let mut fields = FieldSink::default();
    historical_ticks
        .encode_fields_for_server_version(&mut fields, MAX_CLIENT_VER)
        .unwrap();
    assert_eq!(
        field_strings(&fields.into_string()),
        vec![
            "2",
            "265598",
            "AAPL",
            "STK",
            "",
            "",
            "",
            "",
            "SMART",
            "NASDAQ",
            "USD",
            "AAPL",
            "NMS",
            "1",
            "20260708 09:30:00 US/Eastern",
            "20260708 16:00:00 US/Eastern",
            "100",
            "TRADES",
            "1",
            "0",
            "opt=1;"
        ]
    );
}

#[test]
fn complex_contract_requests_use_protobuf_when_supported() {
    let contract = Contract {
        con_id: 265598,
        symbol: "AAPL".to_owned(),
        sec_type: "STK".to_owned(),
        exchange: "SMART".to_owned(),
        currency: "USD".to_owned(),
        ..Contract::default()
    };

    let implied = CalculateImpliedVolatilityRequest {
        req_id: 3,
        contract: contract.clone(),
        option_price: 1.25,
        under_price: 190.0,
        options: vec![TagValue {
            tag: "model".to_owned(),
            value: "default".to_owned(),
        }],
    };
    let frame = encode_request_frame(&implied, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::CalculateImpliedVolatilityRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(3));
    assert_eq!(decoded.option_price, Some(1.25));
    assert_eq!(
        decoded
            .implied_volatility_options
            .get("model")
            .map(String::as_str),
        Some("default")
    );

    let option_price = CalculateOptionPriceRequest {
        req_id: 4,
        contract: contract.clone(),
        volatility: 0.2,
        under_price: 190.0,
        options: Vec::new(),
    };
    let frame = encode_request_frame(&option_price, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::CalculateOptionPriceRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(4));
    assert_eq!(decoded.volatility, Some(0.2));

    let exercise = ExerciseOptionsRequest {
        order_id: 5,
        contract: contract.clone(),
        exercise_action: 1,
        exercise_quantity: 2,
        account: "DU123".to_owned(),
        override_system_action: true,
        manual_order_time: "20260708-12:00:00".to_owned(),
        customer_account: "CUST".to_owned(),
        professional_customer: true,
    };
    let frame = encode_request_frame(&exercise, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::ExerciseOptionsRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.order_id, Some(5));
    assert_eq!(decoded.r#override, Some(true));

    let ticks = HistoricalTicksRequest {
        req_id: 6,
        contract,
        start_date_time: "20260708 09:30:00 US/Eastern".to_owned(),
        end_date_time: String::new(),
        number_of_ticks: 25,
        what_to_show: "BID_ASK".to_owned(),
        use_rth: false,
        ignore_size: true,
        misc_options: Vec::new(),
    };
    let frame = encode_request_frame(&ticks, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::HistoricalTicksRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.req_id, Some(6));
    assert_eq!(decoded.ignore_size, Some(true));
}

#[test]
fn verify_and_auth_requests_remain_field_encoded() {
    let request = VerifyAndAuthRequest {
        api_name: "api".to_owned(),
        api_version: "1".to_owned(),
        opaque_isv_key: "key".to_owned(),
    };
    let frame = encode_request_frame(&request, MAX_CLIENT_VER).unwrap();
    assert_eq!(
        &frame[4..8],
        &Outgoing::VerifyAndAuthRequest.id().to_be_bytes()
    );
    assert_eq!(
        field_strings(&String::from_utf8_lossy(&frame[8..])),
        vec!["1", "api", "1", "key"]
    );

    let request = VerifyAndAuthMessageRequest {
        api_data: "payload".to_owned(),
        xyz_response: "response".to_owned(),
    };
    let frame = encode_request_frame(&request, MAX_CLIENT_VER).unwrap();
    assert_eq!(
        &frame[4..8],
        &Outgoing::VerifyAndAuthMessage.id().to_be_bytes()
    );
    assert_eq!(
        field_strings(&String::from_utf8_lossy(&frame[8..])),
        vec!["1", "payload", "response"]
    );
}

#[test]
fn decoder_reads_more_protobuf_market_and_scanner_events() {
    let depth = protobuf::MarketDepth {
        req_id: Some(1),
        market_depth_data: Some(protobuf::MarketDepthData {
            position: Some(2),
            operation: Some(0),
            side: Some(1),
            price: Some(123.45),
            size: Some("100".to_owned()),
            market_maker: Some("MM".to_owned()),
            is_smart_depth: Some(true),
        }),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::MarketDepth as i32, &depth.encode_to_vec())
            .unwrap();
    match event {
        Event::MarketDepth {
            req_id,
            position,
            market_maker,
            is_smart_depth,
            ..
        } => {
            assert_eq!(req_id, 1);
            assert_eq!(position, 2);
            assert_eq!(market_maker, "MM");
            assert!(is_smart_depth);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let scanner = protobuf::ScannerData {
        req_id: Some(3),
        scanner_data_element: vec![protobuf::ScannerDataElement {
            rank: Some(1),
            contract: Some(protobuf::Contract {
                symbol: Some("AAPL".to_owned()),
                sec_type: Some("STK".to_owned()),
                ..protobuf::Contract::default()
            }),
            market_name: Some("NASDAQ".to_owned()),
            distance: Some("0".to_owned()),
            benchmark: Some("SPX".to_owned()),
            projection: Some("proj".to_owned()),
            combo_key: Some("combo".to_owned()),
        }],
    };
    let event =
        decoder::decode_protobuf_event(Incoming::ScannerData as i32, &scanner.encode_to_vec())
            .unwrap();
    match event {
        Event::ScannerData { req_id, rows } => {
            assert_eq!(req_id, 3);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].contract.symbol, "AAPL");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_more_protobuf_news_fa_and_verify_events() {
    let providers = protobuf::NewsProviders {
        news_providers: vec![protobuf::NewsProvider {
            provider_code: Some("BRFG".to_owned()),
            provider_name: Some("Briefing".to_owned()),
        }],
    };
    let event =
        decoder::decode_protobuf_event(Incoming::NewsProviders as i32, &providers.encode_to_vec())
            .unwrap();
    assert_eq!(
        event,
        Event::NewsProviders {
            providers: vec![NewsProvider {
                code: "BRFG".to_owned(),
                name: "Briefing".to_owned(),
            }],
        }
    );

    let receive_fa = protobuf::ReceiveFa {
        fa_data_type: Some(1),
        xml: Some("<fa/>".to_owned()),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::ReceiveFa as i32, &receive_fa.encode_to_vec())
            .unwrap();
    assert_eq!(
        event,
        Event::ReceiveFa {
            fa_data_type: 1,
            xml: "<fa/>".to_owned(),
        }
    );

    let verify = protobuf::VerifyCompleted {
        is_successful: Some(true),
        error_text: Some(String::new()),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::VerifyCompleted as i32, &verify.encode_to_vec())
            .unwrap();
    assert_eq!(
        event,
        Event::VerifyCompleted {
            is_successful: true,
            error_text: String::new(),
        }
    );
}

#[test]
fn decoder_reads_more_protobuf_rules_and_config_events() {
    let rule = protobuf::MarketRule {
        market_rule_id: Some(26),
        price_increments: vec![protobuf::PriceIncrement {
            low_edge: Some(0.0),
            increment: Some(0.01),
        }],
    };
    let event =
        decoder::decode_protobuf_event(Incoming::MarketRule as i32, &rule.encode_to_vec()).unwrap();
    assert_eq!(
        event,
        Event::MarketRule {
            market_rule_id: 26,
            price_increments: vec![PriceIncrement {
                low_edge: 0.0,
                increment: 0.01,
            }],
        }
    );

    let update = protobuf::UpdateConfigResponse {
        req_id: Some(5),
        status: Some("OK".to_owned()),
        message: Some("updated".to_owned()),
        changed_fields: vec!["api".to_owned()],
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let event = decoder::decode_protobuf_event(
        Incoming::UpdateConfigResponse as i32,
        &update.encode_to_vec(),
    )
    .unwrap();
    assert_eq!(
        event,
        Event::UpdateConfigResponse {
            req_id: 5,
            status: "OK".to_owned(),
            message: "updated".to_owned(),
            changed_fields: vec!["api".to_owned()],
            errors: Vec::new(),
        }
    );
}

#[test]
fn contract_details_request_matches_python_latest_field_order() {
    let request = ContractDetailsRequest {
        req_id: 11,
        contract: Contract {
            con_id: 265598,
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            last_trade_date_or_contract_month: "202612".to_owned(),
            exchange: "SMART".to_owned(),
            primary_exchange: "NASDAQ".to_owned(),
            currency: "USD".to_owned(),
            local_symbol: "AAPL".to_owned(),
            trading_class: "NMS".to_owned(),
            include_expired: true,
            sec_id_type: "ISIN".to_owned(),
            sec_id: "US0378331005".to_owned(),
            issuer_id: "issuer".to_owned(),
            ..Contract::default()
        },
    };

    let mut fields = FieldSink::default();
    request
        .encode_fields_for_server_version(&mut fields, MAX_CLIENT_VER)
        .unwrap();
    let encoded = fields.into_string();

    assert_eq!(
        field_strings(&encoded),
        vec![
            "8",
            "11",
            "265598",
            "AAPL",
            "STK",
            "202612",
            "",
            "",
            "",
            "SMART",
            "NASDAQ",
            "USD",
            "AAPL",
            "NMS",
            "1",
            "ISIN",
            "US0378331005",
            "issuer"
        ]
    );
}

#[test]
fn historical_data_request_matches_python_latest_field_order() {
    let request = HistoricalDataRequest {
        req_id: 12,
        contract: Contract {
            con_id: 265598,
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            primary_exchange: "NASDAQ".to_owned(),
            currency: "USD".to_owned(),
            local_symbol: "AAPL".to_owned(),
            trading_class: "NMS".to_owned(),
            include_expired: false,
            ..Contract::default()
        },
        end_date_time: "20260708 12:00:00 US/Eastern".to_owned(),
        duration_str: "1 D".to_owned(),
        bar_size_setting: "1 min".to_owned(),
        what_to_show: "TRADES".to_owned(),
        use_rth: 1,
        format_date: 1,
        keep_up_to_date: false,
        chart_options: Vec::new(),
    };

    let mut fields = FieldSink::default();
    request
        .encode_fields_for_server_version(&mut fields, MAX_CLIENT_VER)
        .unwrap();
    let encoded = fields.into_string();

    assert_eq!(
        field_strings(&encoded),
        vec![
            "12",
            "265598",
            "AAPL",
            "STK",
            "",
            "",
            "",
            "",
            "SMART",
            "NASDAQ",
            "USD",
            "AAPL",
            "NMS",
            "0",
            "20260708 12:00:00 US/Eastern",
            "1 min",
            "1 D",
            "1",
            "TRADES",
            "1",
            "0",
            ""
        ]
    );
}

#[test]
fn place_order_request_keeps_rust_model_fields_and_extra_tail() {
    let request = PlaceOrderRequest {
        order_id: 7,
        contract: Contract {
            symbol: "MSFT".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        order: Order {
            action: "BUY".to_owned(),
            order_type: "LMT".to_owned(),
            tif: "DAY".to_owned(),
            ..Order::default()
        },
        extra_fields: comm::make_field("tail").unwrap(),
    };

    let mut fields = FieldSink::default();
    request.encode_fields(&mut fields).unwrap();
    let encoded = fields.into_string();
    let parts = comm::read_fields(encoded.as_bytes());

    assert_eq!(parts.first(), Some(&b"7".as_slice()));
    assert!(parts.contains(&b"MSFT".as_slice()));
    assert!(parts.contains(&b"BUY".as_slice()));
    assert!(parts.contains(&b"LMT".as_slice()));
    assert!(encoded.ends_with("tail\0"));
}

#[test]
fn place_order_request_encodes_extended_order_fields_to_field_protocol() {
    let request = PlaceOrderRequest {
        order_id: 7001,
        contract: Contract {
            con_id: 265598,
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            primary_exchange: "NASDAQ".to_owned(),
            currency: "USD".to_owned(),
            local_symbol: "AAPL".to_owned(),
            trading_class: "NMS".to_owned(),
            ..Contract::default()
        },
        order: Order {
            action: "BUY".to_owned(),
            total_quantity: "10".parse().unwrap(),
            order_type: "LMT".to_owned(),
            limit_price: 175.5,
            tif: "DAY".to_owned(),
            account: "DU123".to_owned(),
            fa_group: "group".to_owned(),
            model_code: "model-1".to_owned(),
            hedge_type: "D".to_owned(),
            hedge_param: "delta=0.5".to_owned(),
            algo_strategy: "Adaptive".to_owned(),
            algo_params: vec![TagValue {
                tag: "adaptivePriority".to_owned(),
                value: "Normal".to_owned(),
            }],
            algo_id: "algo-1".to_owned(),
            cash_qty: 1000.0,
            manual_order_indicator: 1,
            include_overnight: true,
            ..Order::default()
        },
        extra_fields: String::new(),
    };

    let mut fields = FieldSink::default();
    request
        .encode_fields_for_server_version(&mut fields, MAX_CLIENT_VER)
        .unwrap();
    let encoded = fields.into_string();
    let parts = field_strings(&encoded);

    assert!(parts.contains(&"group".to_owned()));
    assert!(parts.contains(&"model-1".to_owned()));
    assert!(parts.contains(&"D".to_owned()));
    assert!(parts.contains(&"delta=0.5".to_owned()));
    assert!(parts.contains(&"Adaptive".to_owned()));
    assert!(parts.contains(&"adaptivePriority".to_owned()));
    assert!(parts.contains(&"Normal".to_owned()));
    assert!(parts.contains(&"algo-1".to_owned()));
    assert!(parts.contains(&"1000".to_owned()));
    assert!(parts.contains(&"1".to_owned()));
}

#[test]
fn place_order_field_protocol_keeps_conditional_fields_in_wire_order() {
    let encode = |contract: Contract, order: Order, server_version| {
        let request = PlaceOrderRequest {
            order_id: 1,
            contract,
            order,
            extra_fields: String::new(),
        };
        let mut fields = FieldSink::default();
        request
            .encode_fields_for_server_version(&mut fields, server_version)
            .unwrap();
        field_strings(&fields.into_string())
    };
    let has_sequence = |fields: &[String], expected: &[&str]| {
        fields.windows(expected.len()).any(|window| {
            window
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied())
        })
    };

    let benchmark = encode(
        Contract {
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        Order {
            action: "BUY".to_owned(),
            total_quantity: "1".parse().unwrap(),
            order_type: "PEG BENCH".to_owned(),
            reference_contract_id: 42,
            is_pegged_change_amount_decrease: true,
            pegged_change_amount: 1.25,
            reference_change_amount: 2.5,
            reference_exchange_id: "NYSE".to_owned(),
            ..Order::default()
        },
        MAX_CLIENT_VER,
    );
    assert!(has_sequence(
        &benchmark,
        &["42", "1", "1.25", "2.5", "NYSE", "0"],
    ));

    let peg_best = encode(
        Contract {
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "IBKRATS".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        Order {
            action: "BUY".to_owned(),
            total_quantity: "1".parse().unwrap(),
            order_type: "PEG BEST".to_owned(),
            min_trade_qty: 3,
            min_compete_size: 4,
            compete_against_best_offset: f64::INFINITY,
            mid_offset_at_whole: 0.1,
            mid_offset_at_half: 0.2,
            customer_account: "customer".to_owned(),
            professional_customer: true,
            ..Order::default()
        },
        MIN_SERVER_VER_RFQ_FIELDS,
    );
    assert!(has_sequence(
        &peg_best,
        &["3", "4", "Infinity", "0.1", "0.2", "customer", "1", "", ""],
    ));
}

#[test]
fn legacy_field_encoders_follow_tws_version_gates() {
    let cancel = CancelOrderRequest {
        order_id: 7,
        order_cancel: OrderCancel {
            manual_order_cancel_time: "20260710-12:00:00".to_owned(),
            ext_operator: "operator".to_owned(),
            manual_order_indicator: 1,
        },
    };
    let encode_cancel = |server_version| {
        let mut fields = FieldSink::default();
        cancel
            .encode_fields_for_server_version(&mut fields, server_version)
            .unwrap();
        field_strings(&fields.into_string())
    };
    assert_eq!(encode_cancel(168), ["1", "7"]);
    assert_eq!(
        encode_cancel(MIN_SERVER_VER_MANUAL_ORDER_TIME),
        ["1", "7", "20260710-12:00:00"]
    );
    assert_eq!(
        encode_cancel(MIN_SERVER_VER_RFQ_FIELDS),
        ["1", "7", "20260710-12:00:00", "", "", ""]
    );
    assert_eq!(
        encode_cancel(192),
        ["7", "20260710-12:00:00", "operator", "1"]
    );

    let global_cancel = GlobalCancelRequest {
        order_cancel: cancel.order_cancel.clone(),
    };
    let encode_global_cancel = |server_version| {
        let mut fields = FieldSink::default();
        global_cancel
            .encode_fields_for_server_version(&mut fields, server_version)
            .unwrap();
        field_strings(&fields.into_string())
    };
    assert_eq!(encode_global_cancel(191), ["1"]);
    assert_eq!(encode_global_cancel(192), ["operator", "1"]);

    let cancel_depth = CancelMarketDepthRequest {
        req_id: 9,
        is_smart_depth: true,
    };
    let encode_cancel_depth = |server_version| {
        let mut fields = FieldSink::default();
        cancel_depth
            .encode_fields_for_server_version(&mut fields, server_version)
            .unwrap();
        field_strings(&fields.into_string())
    };
    assert_eq!(encode_cancel_depth(145), ["1", "9"]);
    assert_eq!(
        encode_cancel_depth(MIN_SERVER_VER_SMART_DEPTH),
        ["1", "9", "1"]
    );

    let replace = ReplaceFinancialAdvisorRequest {
        req_id: 8,
        fa_data_type: 3,
        xml: "<xml/>".to_owned(),
    };
    let encode_replace = |server_version| {
        let mut fields = FieldSink::default();
        replace
            .encode_fields_for_server_version(&mut fields, server_version)
            .unwrap();
        field_strings(&fields.into_string())
    };
    assert_eq!(encode_replace(156), ["1", "3", "<xml/>"]);
    assert_eq!(
        encode_replace(MIN_SERVER_VER_REPLACE_FA_END),
        ["1", "3", "<xml/>", "8"]
    );

    let depth = MarketDepthRequest {
        req_id: 9,
        contract: Contract {
            con_id: 265598,
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            last_trade_date_or_contract_month: "202609".to_owned(),
            strike: 175.0,
            right: "C".to_owned(),
            multiplier: "100".to_owned(),
            exchange: "SMART".to_owned(),
            primary_exchange: "NASDAQ".to_owned(),
            currency: "USD".to_owned(),
            local_symbol: "AAPL".to_owned(),
            trading_class: "NMS".to_owned(),
            include_expired: true,
            sec_id_type: "ISIN".to_owned(),
            sec_id: "US0378331005".to_owned(),
            ..Contract::default()
        },
        num_rows: 10,
        is_smart_depth: true,
        market_depth_options: vec![TagValue {
            tag: "exchange".to_owned(),
            value: "ISLAND".to_owned(),
        }],
    };
    let encode_depth = |server_version| {
        let mut fields = FieldSink::default();
        depth
            .encode_fields_for_server_version(&mut fields, server_version)
            .unwrap();
        field_strings(&fields.into_string())
    };
    assert_eq!(
        encode_depth(MIN_SERVER_VER_SMART_DEPTH - 1),
        [
            "5",
            "9",
            "265598",
            "AAPL",
            "STK",
            "202609",
            "175",
            "C",
            "100",
            "SMART",
            "USD",
            "AAPL",
            "NMS",
            "10",
            "exchange=ISLAND;"
        ]
    );
    assert_eq!(
        encode_depth(MIN_SERVER_VER_SMART_DEPTH),
        [
            "5",
            "9",
            "265598",
            "AAPL",
            "STK",
            "202609",
            "175",
            "C",
            "100",
            "SMART",
            "USD",
            "AAPL",
            "NMS",
            "10",
            "1",
            "exchange=ISLAND;"
        ]
    );
    assert_eq!(
        encode_depth(MIN_SERVER_VER_MKT_DEPTH_PRIM_EXCHANGE),
        [
            "5",
            "9",
            "265598",
            "AAPL",
            "STK",
            "202609",
            "175",
            "C",
            "100",
            "SMART",
            "NASDAQ",
            "USD",
            "AAPL",
            "NMS",
            "10",
            "1",
            "exchange=ISLAND;"
        ]
    );
}

#[test]
fn scanner_realtime_and_contract_field_encoders_match_python_layouts() {
    fn encode_fields<R: EncodableRequest>(request: &R) -> Vec<String> {
        let mut sink = FieldSink::default();
        request.encode_fields(&mut sink).unwrap();
        field_strings(&sink.into_string())
    }

    let contract = Contract {
        con_id: 265598,
        symbol: "AAPL".to_owned(),
        sec_type: "STK".to_owned(),
        last_trade_date_or_contract_month: "202609".to_owned(),
        strike: 175.0,
        right: "C".to_owned(),
        multiplier: "100".to_owned(),
        exchange: "SMART".to_owned(),
        primary_exchange: "NASDAQ".to_owned(),
        currency: "USD".to_owned(),
        local_symbol: "AAPL".to_owned(),
        trading_class: "NMS".to_owned(),
        include_expired: true,
        sec_id_type: "ISIN".to_owned(),
        sec_id: "US0378331005".to_owned(),
        ..Contract::default()
    };

    let head = HeadTimestampRequest {
        req_id: 1,
        contract: contract.clone(),
        use_rth: true,
        what_to_show: "TRADES".to_owned(),
        format_date: 2,
    };
    let histogram = HistogramDataRequest {
        req_id: 2,
        contract: contract.clone(),
        use_rth: true,
        time_period: "3 days".to_owned(),
    };
    for fields in [encode_fields(&head), encode_fields(&histogram)] {
        assert!(fields.contains(&"265598".to_owned()));
        assert!(fields.contains(&"NMS".to_owned()));
        assert!(fields.contains(&"1".to_owned()));
        assert!(!fields.contains(&"ISIN".to_owned()));
        assert!(!fields.contains(&"US0378331005".to_owned()));
    }

    let realtime = RealTimeBarsRequest {
        req_id: 3,
        contract: contract.clone(),
        bar_size: 5,
        what_to_show: "TRADES".to_owned(),
        use_rth: true,
        options: vec![TagValue {
            tag: "source".to_owned(),
            value: "api".to_owned(),
        }],
    };
    let encode_realtime = |server_version| {
        let mut sink = FieldSink::default();
        realtime
            .encode_fields_for_server_version(&mut sink, server_version)
            .unwrap();
        field_strings(&sink.into_string())
    };
    let before_trading_class = encode_realtime(MIN_SERVER_VER_TRADING_CLASS - 1);
    assert_eq!(before_trading_class[0..3], ["3", "3", "AAPL"]);
    assert!(!before_trading_class.contains(&"265598".to_owned()));
    assert!(!before_trading_class.contains(&"NMS".to_owned()));
    assert!(!before_trading_class.contains(&"source=api;".to_owned()));
    let with_trading_class = encode_realtime(MIN_SERVER_VER_TRADING_CLASS);
    assert_eq!(with_trading_class[0..4], ["3", "3", "265598", "AAPL"]);
    assert!(with_trading_class.contains(&"NMS".to_owned()));
    let with_options = encode_realtime(MIN_SERVER_VER_LINKING);
    assert_eq!(with_options.last().map(String::as_str), Some("source=api;"));

    let scanner = ScannerSubscriptionRequest {
        req_id: 4,
        subscription: ScannerSubscription {
            number_of_rows: 10,
            instrument: "STK".to_owned(),
            location_code: "STK.US".to_owned(),
            scan_code: "TOP_PERC_GAIN".to_owned(),
            ..ScannerSubscription::default()
        },
        scanner_subscription_options: vec![TagValue {
            tag: "option".to_owned(),
            value: "value".to_owned(),
        }],
        scanner_subscription_filter_options: vec![TagValue {
            tag: "filter".to_owned(),
            value: "value".to_owned(),
        }],
    };
    let encode_scanner = |server_version| {
        let mut sink = FieldSink::default();
        scanner
            .encode_fields_for_server_version(&mut sink, server_version)
            .unwrap();
        field_strings(&sink.into_string())
    };
    let before_generic_options = encode_scanner(MIN_SERVER_VER_SCANNER_GENERIC_OPTS - 1);
    assert_eq!(
        before_generic_options.first().map(String::as_str),
        Some("4")
    );
    assert!(!before_generic_options.contains(&"filter=value;".to_owned()));
    assert_eq!(
        before_generic_options.last().map(String::as_str),
        Some("option=value;")
    );
    let generic_options = encode_scanner(MIN_SERVER_VER_SCANNER_GENERIC_OPTS);
    assert_eq!(generic_options.first().map(String::as_str), Some("4"));
    assert!(generic_options.ends_with(&["filter=value;".to_owned(), "option=value;".to_owned()]));
}

#[test]
fn place_order_request_encodes_extended_order_fields_to_protobuf() {
    let request = PlaceOrderRequest {
        order_id: 7001,
        contract: Contract {
            symbol: "AAPL".to_owned(),
            sec_type: "STK".to_owned(),
            exchange: "SMART".to_owned(),
            currency: "USD".to_owned(),
            ..Contract::default()
        },
        order: Order {
            action: "BUY".to_owned(),
            total_quantity: "10".parse().unwrap(),
            order_type: "LMT".to_owned(),
            limit_price: 175.5,
            tif: "DAY".to_owned(),
            active_start_time: "20260709 09:30:00 US/Eastern".to_owned(),
            active_stop_time: "20260709 16:00:00 US/Eastern".to_owned(),
            fa_group: "group".to_owned(),
            fa_method: "PctChange".to_owned(),
            fa_percentage: "100".to_owned(),
            settling_firm: "IB".to_owned(),
            clearing_account: "DU123".to_owned(),
            clearing_intent: "IB".to_owned(),
            delta_neutral_order_type: "MKT".to_owned(),
            delta_neutral_aux_price: 1.25,
            scale_init_level_size: 100,
            hedge_type: "D".to_owned(),
            hedge_param: "delta=0.5".to_owned(),
            algo_id: "algo-1".to_owned(),
            what_if: true,
            not_held: true,
            solicited: true,
            reference_contract_id: 265598,
            adjusted_order_type: "STP".to_owned(),
            ext_operator: "operator".to_owned(),
            cash_qty: 1000.0,
            mifid2_decision_maker: "maker".to_owned(),
            is_oms_container: true,
            auto_cancel_date: "20260710".to_owned(),
            route_marketable_to_bbo: 1,
            parent_perm_id: 123456,
            use_price_mgmt_algo: 1,
            min_trade_qty: 5,
            compete_against_best_offset: 0.05,
            customer_account: "cust".to_owned(),
            professional_customer: true,
            include_overnight: true,
            manual_order_indicator: 1,
            submitter: "submitter".to_owned(),
            post_only: true,
            allow_pre_open: true,
            ignore_open_auction: true,
            seek_price_improvement: 1,
            what_if_type: 2,
            hedge_max_size: 10,
            stop_loss_order_id: 7002,
            stop_loss_order_type: "STP".to_owned(),
            profit_taker_order_id: 7003,
            profit_taker_order_type: "LMT".to_owned(),
            ..Order::default()
        },
        extra_fields: String::new(),
    };

    let frame = encode_request_frame(&request, MAX_CLIENT_VER).unwrap();
    let decoded = protobuf::PlaceOrderRequest::decode(&frame[8..]).unwrap();
    let order = decoded.order.unwrap();
    let attached = decoded.attached_orders.unwrap();

    assert_eq!(
        order.active_start_time.as_deref(),
        Some("20260709 09:30:00 US/Eastern")
    );
    assert_eq!(order.fa_group.as_deref(), Some("group"));
    assert_eq!(order.clearing_account.as_deref(), Some("DU123"));
    assert_eq!(order.delta_neutral_order_type.as_deref(), Some("MKT"));
    assert_eq!(order.scale_init_level_size, Some(100));
    assert_eq!(order.hedge_type.as_deref(), Some("D"));
    assert_eq!(order.algo_id.as_deref(), Some("algo-1"));
    assert_eq!(order.what_if, Some(true));
    assert_eq!(order.reference_contract_id, Some(265598));
    assert_eq!(order.ext_operator.as_deref(), Some("operator"));
    assert_eq!(order.cash_qty, Some(1000.0));
    assert_eq!(order.mifid2_decision_maker.as_deref(), Some("maker"));
    assert_eq!(order.route_marketable_to_bbo, Some(1));
    assert_eq!(order.parent_perm_id, Some(123456));
    assert_eq!(order.min_trade_qty, Some(5));
    assert_eq!(order.professional_customer, Some(true));
    assert_eq!(order.manual_order_indicator, Some(1));
    assert_eq!(order.hedge_max_size, Some(10));
    assert_eq!(attached.sl_order_id, Some(7002));
    assert_eq!(attached.pt_order_type.as_deref(), Some("LMT"));
}

#[test]
fn incoming_message_catalog_roundtrips_and_documents_protocol_exceptions() {
    let mut ids = Incoming::ALL
        .iter()
        .map(|incoming| *incoming as i32)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();

    assert_eq!(Incoming::ALL.len(), 85);
    assert_eq!(ids.len(), Incoming::ALL.len());
    for incoming in Incoming::ALL {
        assert_eq!(Incoming::try_from(*incoming as i32), Ok(*incoming));
    }

    let protobuf_only_field_exceptions = [Incoming::ConfigResponse, Incoming::UpdateConfigResponse];
    let field_only_protobuf_exceptions = [
        Incoming::TickEfp,
        Incoming::DeltaNeutralValidation,
        Incoming::VerifyAndAuthMessageApi,
        Incoming::VerifyAndAuthCompleted,
        Incoming::SecurityDefinitionOptionParameter,
        Incoming::SecurityDefinitionOptionParameterEnd,
    ];

    assert_eq!(protobuf_only_field_exceptions.len(), 2);
    assert_eq!(field_only_protobuf_exceptions.len(), 6);
}

fn field_strings(encoded: &str) -> Vec<String> {
    comm::read_fields(encoded.as_bytes())
        .into_iter()
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect()
}

fn field_payload(message_id: Incoming, fields: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice((message_id as i32).to_string().as_bytes());
    payload.push(0);
    for field in fields {
        payload.extend_from_slice(field.as_bytes());
        payload.push(0);
    }
    payload
}

#[test]
fn decoder_reads_text_message_id_events() {
    let payload = &[b'9', 0, b'1', b'2', b'3', b'4', b'5', 0];
    let event = decoder::decode_payload(false, payload).unwrap();
    assert_eq!(event, Event::NextValidId { order_id: 12345 });
}

#[test]
fn decoder_reads_raw_int_message_id_events() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(49_i32).to_be_bytes());
    payload.extend_from_slice(b"1700000000\0");
    let event = decoder::decode_payload(true, &payload).unwrap();
    assert_eq!(
        event,
        Event::CurrentTime {
            time: 1_700_000_000
        }
    );
}

#[test]
fn decoder_preserves_unknown_messages() {
    let payload = b"199\0alpha\0beta\0";
    let event = decoder::decode_payload(false, payload).unwrap();
    assert_eq!(
        event,
        Event::Raw {
            msg_id: 199,
            fields: vec!["alpha".to_owned(), "beta".to_owned()]
        }
    );
}

#[test]
fn decoder_reads_remaining_field_based_callbacks() {
    let tick_efp = field_payload(
        Incoming::TickEfp,
        &[
            "1001", "38", "1.5", "1.50", "2.25", "7", "20260918", "0.12", "0.34",
        ],
    );
    let event = decoder::decode_payload(false, &tick_efp).unwrap();
    assert_eq!(
        event,
        Event::TickEfp {
            req_id: 1001,
            tick_type: 38,
            basis_points: 1.5,
            formatted_basis_points: "1.50".to_owned(),
            total_dividends: 2.25,
            hold_days: 7,
            future_last_trade_date: "20260918".to_owned(),
            dividend_impact: 0.12,
            dividends_to_last_trade_date: 0.34,
        }
    );

    let delta = field_payload(
        Incoming::DeltaNeutralValidation,
        &["1", "1002", "265598", "0.5", "175.25"],
    );
    let event = decoder::decode_payload(false, &delta).unwrap();
    match event {
        Event::DeltaNeutralValidation {
            req_id,
            delta_neutral_contract,
        } => {
            assert_eq!(req_id, 1002);
            assert_eq!(delta_neutral_contract.con_id, 265598);
            assert_eq!(delta_neutral_contract.delta, 0.5);
            assert_eq!(delta_neutral_contract.price, 175.25);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let commission = field_payload(
        Incoming::CommissionAndFeesReport,
        &["1", "E1", "1.25", "USD", "2.5", "3.5", "20260709"],
    );
    let event = decoder::decode_payload(false, &commission).unwrap();
    match event {
        Event::CommissionAndFeesReport { report } => {
            assert_eq!(report.exec_id, "E1");
            assert_eq!(report.commission_and_fees, 1.25);
            assert_eq!(report.currency, "USD");
            assert_eq!(report.realized_pnl, 2.5);
            assert_eq!(report.bond_yield, 3.5);
            assert_eq!(report.yield_redemption_date, "20260709");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_verify_and_secdef_callbacks() {
    let verify = field_payload(Incoming::VerifyMessageApi, &["api-data"]);
    assert_eq!(
        decoder::decode_payload(false, &verify).unwrap(),
        Event::VerifyMessageApi {
            api_data: "api-data".to_owned()
        }
    );

    let verify_done = field_payload(Incoming::VerifyCompleted, &["1", ""]);
    assert_eq!(
        decoder::decode_payload(false, &verify_done).unwrap(),
        Event::VerifyCompleted {
            is_successful: true,
            error_text: String::new()
        }
    );

    let auth = field_payload(
        Incoming::VerifyAndAuthMessageApi,
        &["api-data", "challenge"],
    );
    assert_eq!(
        decoder::decode_payload(false, &auth).unwrap(),
        Event::VerifyAndAuthMessageApi {
            api_data: "api-data".to_owned(),
            challenge: "challenge".to_owned(),
        }
    );

    let auth_done = field_payload(Incoming::VerifyAndAuthCompleted, &["0", "denied"]);
    assert_eq!(
        decoder::decode_payload(false, &auth_done).unwrap(),
        Event::VerifyAndAuthCompleted {
            is_successful: false,
            error_text: "denied".to_owned(),
        }
    );

    let secdef = field_payload(
        Incoming::SecurityDefinitionOptionParameter,
        &[
            "1003", "SMART", "265598", "AAPL", "100", "2", "20260918", "20261218", "2", "175.0",
            "180.0",
        ],
    );
    let event = decoder::decode_payload(false, &secdef).unwrap();
    match event {
        Event::SecurityDefinitionOptionParameter {
            req_id,
            exchange,
            underlying_con_id,
            trading_class,
            multiplier,
            expirations,
            strikes,
        } => {
            assert_eq!(req_id, 1003);
            assert_eq!(exchange, "SMART");
            assert_eq!(underlying_con_id, 265598);
            assert_eq!(trading_class, "AAPL");
            assert_eq!(multiplier, "100");
            assert_eq!(expirations, ["20260918", "20261218"]);
            assert_eq!(strikes, [175.0, 180.0]);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let secdef_end = field_payload(Incoming::SecurityDefinitionOptionParameterEnd, &["1003"]);
    assert_eq!(
        decoder::decode_payload(false, &secdef_end).unwrap(),
        Event::SecurityDefinitionOptionParameterEnd { req_id: 1003 }
    );
}

#[test]
fn decoder_reads_more_field_based_account_and_position_callbacks() {
    let account_value = field_payload(
        Incoming::AccountValue,
        &["NetLiquidation", "100000", "USD", "DU123"],
    );
    assert_eq!(
        decoder::decode_payload(false, &account_value).unwrap(),
        Event::AccountValue {
            key: "NetLiquidation".to_owned(),
            value: "100000".to_owned(),
            currency: "USD".to_owned(),
            account_name: "DU123".to_owned(),
        }
    );

    let portfolio = field_payload(
        Incoming::PortfolioValue,
        &[
            "8", "265598", "AAPL", "STK", "", "0", "", "1", "NASDAQ", "USD", "AAPL", "NMS", "10",
            "175.5", "1755.0", "150.0", "255.0", "0.0", "DU123",
        ],
    );
    let event = decoder::decode_payload(false, &portfolio).unwrap();
    match event {
        Event::PortfolioValue {
            contract,
            position,
            market_price,
            account_name,
            ..
        } => {
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(contract.trading_class, "NMS");
            assert_eq!(position.to_string(), "10");
            assert_eq!(market_price, 175.5);
            assert_eq!(account_name, "DU123");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let position = field_payload(
        Incoming::PositionData,
        &[
            "3", "DU123", "265598", "AAPL", "STK", "", "0", "", "1", "SMART", "USD", "AAPL", "NMS",
            "10", "150.25",
        ],
    );
    let event = decoder::decode_payload(false, &position).unwrap();
    match event {
        Event::Position {
            account,
            contract,
            position,
            avg_cost,
        } => {
            assert_eq!(account, "DU123");
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(contract.trading_class, "NMS");
            assert_eq!(position.to_string(), "10");
            assert_eq!(avg_cost, 150.25);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let position_multi = field_payload(
        Incoming::PositionMulti,
        &[
            "1", "42", "DU123", "265598", "AAPL", "STK", "", "0", "", "1", "SMART", "USD", "AAPL",
            "NMS", "12", "151.5", "MODEL",
        ],
    );
    let event = decoder::decode_payload(false, &position_multi).unwrap();
    match event {
        Event::PositionMulti {
            req_id,
            account,
            model_code,
            contract,
            position,
            ..
        } => {
            assert_eq!(req_id, 42);
            assert_eq!(account, "DU123");
            assert_eq!(model_code, "MODEL");
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(position.to_string(), "12");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_more_field_based_misc_callbacks() {
    let news = field_payload(Incoming::NewsBulletins, &["1", "2", "headline", "NYSE"]);
    assert_eq!(
        decoder::decode_payload(false, &news).unwrap(),
        Event::NewsBulletin {
            news_msg_id: 1,
            news_msg_type: 2,
            news_message: "headline".to_owned(),
            originating_exch: "NYSE".to_owned(),
        }
    );

    let receive_fa = field_payload(Incoming::ReceiveFa, &["1", "<xml/>"]);
    assert_eq!(
        decoder::decode_payload(false, &receive_fa).unwrap(),
        Event::ReceiveFa {
            fa_data_type: 1,
            xml: "<xml/>".to_owned(),
        }
    );

    let scanner_params = field_payload(Incoming::ScannerParameters, &["<scan/>"]);
    assert_eq!(
        decoder::decode_payload(false, &scanner_params).unwrap(),
        Event::ScannerParameters {
            xml: "<scan/>".to_owned(),
        }
    );

    let display_groups = field_payload(Incoming::DisplayGroupList, &["7", "1|2|3"]);
    assert_eq!(
        decoder::decode_payload(false, &display_groups).unwrap(),
        Event::DisplayGroupList {
            req_id: 7,
            groups: "1|2|3".to_owned(),
        }
    );

    let account_multi = field_payload(
        Incoming::AccountUpdateMulti,
        &["8", "DU123", "MODEL", "NetLiquidation", "100000", "USD"],
    );
    assert_eq!(
        decoder::decode_payload(false, &account_multi).unwrap(),
        Event::AccountUpdateMulti {
            req_id: 8,
            account: "DU123".to_owned(),
            model_code: "MODEL".to_owned(),
            key: "NetLiquidation".to_owned(),
            value: "100000".to_owned(),
            currency: "USD".to_owned(),
        }
    );
}

#[test]
fn decoder_reads_field_based_market_depth_and_bar_callbacks() {
    let depth = field_payload(
        Incoming::MarketDepth,
        &["1", "77", "0", "1", "0", "175.5", "10"],
    );
    let event = decoder::decode_payload(false, &depth).unwrap();
    match event {
        Event::MarketDepth {
            req_id,
            position,
            operation,
            side,
            price,
            size,
            market_maker,
            is_smart_depth,
        } => {
            assert_eq!(req_id, 77);
            assert_eq!(position, 0);
            assert_eq!(operation, 1);
            assert_eq!(side, 0);
            assert_eq!(price, 175.5);
            assert_eq!(size.to_string(), "10");
            assert_eq!(market_maker, "");
            assert!(!is_smart_depth);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let depth_l2 = field_payload(
        Incoming::MarketDepthL2,
        &["1", "78", "2", "ISLAND", "0", "1", "175.6", "20", "1"],
    );
    let event = decoder::decode_payload(false, &depth_l2).unwrap();
    match event {
        Event::MarketDepth {
            req_id,
            market_maker,
            is_smart_depth,
            size,
            ..
        } => {
            assert_eq!(req_id, 78);
            assert_eq!(market_maker, "ISLAND");
            assert!(is_smart_depth);
            assert_eq!(size.to_string(), "20");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let realtime = field_payload(
        Incoming::RealTimeBars,
        &[
            "1",
            "79",
            "1700000000",
            "175.0",
            "176.0",
            "174.0",
            "175.5",
            "100",
            "175.25",
            "12",
        ],
    );
    let event = decoder::decode_payload(false, &realtime).unwrap();
    match event {
        Event::RealTimeBar { req_id, time, bar } => {
            assert_eq!(req_id, 79);
            assert_eq!(time, 1_700_000_000);
            assert_eq!(bar.close, 175.5);
            assert_eq!(bar.volume.to_string(), "100");
            assert_eq!(bar.bar_count, 12);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let historical = field_payload(
        Incoming::HistoricalData,
        &[
            "80", "1", "20260709", "175.0", "176.0", "174.0", "175.5", "100", "175.25", "12",
        ],
    );
    let event = decoder::decode_payload(false, &historical).unwrap();
    match event {
        Event::HistoricalDataBars { req_id, bars } => {
            assert_eq!(req_id, 80);
            assert_eq!(bars.len(), 1);
            assert_eq!(bars[0].date, "20260709");
            assert_eq!(bars[0].bar_count, 12);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let historical_update = field_payload(
        Incoming::HistoricalDataUpdate,
        &[
            "81", "12", "20260709", "175.0", "175.5", "176.0", "174.0", "175.25", "100",
        ],
    );
    let event = decoder::decode_payload(false, &historical_update).unwrap();
    match event {
        Event::HistoricalDataUpdate { req_id, bar } => {
            assert_eq!(req_id, 81);
            assert_eq!(bar.open, 175.0);
            assert_eq!(bar.close, 175.5);
            assert_eq!(bar.high, 176.0);
            assert_eq!(bar.low, 174.0);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_scanner_data_callback() {
    let scanner = field_payload(
        Incoming::ScannerData,
        &[
            "1",
            "90",
            "1",
            "0",
            "265598",
            "AAPL",
            "STK",
            "",
            "0",
            "",
            "SMART",
            "USD",
            "AAPL",
            "NASDAQ.NMS",
            "NMS",
            "0.1",
            "bench",
            "proj",
            "combo",
        ],
    );
    let event = decoder::decode_payload(false, &scanner).unwrap();
    match event {
        Event::ScannerData { req_id, rows } => {
            assert_eq!(req_id, 90);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].rank, 0);
            assert_eq!(rows[0].contract.symbol, "AAPL");
            assert_eq!(rows[0].market_name, "NASDAQ.NMS");
            assert_eq!(rows[0].combo_key, "combo");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_execution_data_callbacks() {
    let execution = field_payload(
        Incoming::ExecutionData,
        &[
            "91",
            "1001",
            "265598",
            "AAPL",
            "STK",
            "",
            "0",
            "",
            "1",
            "SMART",
            "USD",
            "AAPL",
            "NMS",
            "E1",
            "20260709 12:00:00",
            "DU123",
            "NASDAQ",
            "BOT",
            "10",
            "175.5",
            "123456",
            "7",
            "0",
            "10",
            "175.5",
            "ref",
            "ev",
            "1.5",
            "MODEL",
            "2",
            "1",
            "SUB",
        ],
    );
    let event = decoder::decode_payload(false, &execution).unwrap();
    match event {
        Event::ExecutionDetails {
            req_id,
            contract,
            execution,
        } => {
            assert_eq!(req_id, 91);
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(contract.trading_class, "NMS");
            assert_eq!(execution.order_id, 1001);
            assert_eq!(execution.exec_id, "E1");
            assert_eq!(execution.shares.to_string(), "10");
            assert_eq!(execution.perm_id, 123456);
            assert_eq!(execution.order_ref, "ref");
            assert_eq!(execution.ev_multiplier, 1.5);
            assert_eq!(execution.model_code, "MODEL");
            assert_eq!(execution.last_liquidity, 2);
            assert!(execution.pending_price_revision);
            assert_eq!(execution.submitter, "SUB");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let versioned_execution = field_payload(
        Incoming::ExecutionData,
        &[
            "10",
            "92",
            "1002",
            "265598",
            "AAPL",
            "STK",
            "",
            "0",
            "",
            "1",
            "SMART",
            "USD",
            "AAPL",
            "NMS",
            "E2",
            "20260709 12:01:00",
            "DU123",
            "NASDAQ",
            "SLD",
            "5",
            "176.0",
            "123457",
            "7",
            "0",
            "5",
            "176.0",
            "ref2",
            "ev2",
            "2.5",
        ],
    );
    let event = decoder::decode_payload(false, &versioned_execution).unwrap();
    match event {
        Event::ExecutionDetails {
            req_id, execution, ..
        } => {
            assert_eq!(req_id, 92);
            assert_eq!(execution.order_id, 1002);
            assert_eq!(execution.exec_id, "E2");
            assert_eq!(execution.side, "SLD");
            assert_eq!(execution.avg_price, 176.0);
            assert_eq!(execution.ev_rule, "ev2");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_contract_details_callback() {
    let contract_data = field_payload(
        Incoming::ContractData,
        &[
            "93",
            "AAPL",
            "STK",
            "202609",
            "20260918",
            "175.0",
            "C",
            "SMART",
            "USD",
            "AAPL",
            "NASDAQ.NMS",
            "NMS",
            "265598",
            "0.01",
            "100",
            "LMT,MKT",
            "SMART,NASDAQ",
            "1",
            "0",
            "Apple Inc",
            "NASDAQ",
            "202609",
            "Technology",
            "Computers",
            "Hardware",
            "US/Eastern",
            "20260709:0930-1600",
            "20260709:0930-1600",
            "ev",
            "1",
            "1",
            "ISIN",
            "US0378331005",
            "0",
            "AAPL",
            "STK",
            "26",
        ],
    );
    let event = decoder::decode_payload(false, &contract_data).unwrap();
    match event {
        Event::ContractDetails { req_id, details } => {
            assert_eq!(req_id, 93);
            assert_eq!(details.contract.symbol, "AAPL");
            assert_eq!(details.contract.last_trade_date, "20260918");
            assert_eq!(details.contract.strike, 175.0);
            assert_eq!(details.market_name, "NASDAQ.NMS");
            assert_eq!(details.contract.con_id, 265598);
            assert_eq!(details.min_tick, 0.01);
            assert_eq!(details.long_name, "Apple Inc");
            assert_eq!(details.ev_rule, "ev");
            assert_eq!(details.ev_multiplier, 1.0);
            assert_eq!(details.sec_id_list.len(), 1);
            assert_eq!(details.sec_id_list[0].tag, "ISIN");
            assert_eq!(details.sec_id_list[0].value, "US0378331005");
            assert_eq!(details.aggregate_group, 0);
            assert_eq!(details.under_symbol, "AAPL");
            assert_eq!(details.under_sec_type, "STK");
            assert_eq!(details.market_rule_ids, "26");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_bond_contract_details_callback() {
    let bond = field_payload(
        Incoming::BondContractData,
        &[
            "94",
            "9128285M8",
            "BOND",
            "9128285M8",
            "4.25",
            "20300115",
            "20200115",
            "AAA",
            "GOVT",
            "FIXED",
            "0",
            "1",
            "0",
            "desc",
            "SMART",
            "USD",
            "US-T",
            "USGOVT",
            "1001",
            "0.001",
            "LMT,MKT",
            "SMART",
            "20290115",
            "CALL",
            "1",
            "notes",
            "US Treasury",
            "US/Eastern",
            "20260709:0930-1600",
            "20260709:0930-1600",
            "ev",
            "1",
            "0",
            "0",
            "26",
        ],
    );
    let event = decoder::decode_payload(false, &bond).unwrap();
    match event {
        Event::ContractDetails { req_id, details } => {
            assert_eq!(req_id, 94);
            assert_eq!(details.contract.symbol, "9128285M8");
            assert_eq!(details.contract.sec_type, "BOND");
            assert_eq!(details.cusip, "9128285M8");
            assert_eq!(details.coupon, 4.25);
            assert_eq!(details.issue_date, "20200115");
            assert!(details.callable);
            assert!(!details.puttable);
            assert_eq!(details.next_option_type, "CALL");
            assert_eq!(details.bond_notes, "notes");
            assert_eq!(details.long_name, "US Treasury");
            assert_eq!(details.market_rule_ids, "26");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_open_order_core_callback() {
    let open_order = field_payload(
        Incoming::OpenOrder,
        &[
            "200",
            "1001",
            "265598",
            "AAPL",
            "STK",
            "",
            "0",
            "",
            "",
            "SMART",
            "USD",
            "AAPL",
            "NMS",
            "BUY",
            "10",
            "LMT",
            "175.5",
            "0",
            "DAY",
            "OCA",
            "DU123",
            "O",
            "0",
            "ref",
            "7",
            "123456",
            "1",
            "0",
            "0",
            "0",
            "",
            "",
            "",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "",
            "",
            "",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "",
            "",
            "",
            "0",
            "0",
            "0",
            "",
            "",
            "0",
            "",
            "",
            "0",
            "Submitted",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "",
            "",
            "",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "",
            "0",
            "",
            "0",
        ],
    );
    let event = decoder::decode_payload(false, &open_order).unwrap();
    match event {
        Event::OpenOrder {
            order_id,
            contract,
            order,
            order_state,
        } => {
            assert_eq!(order_id, 1001);
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(contract.trading_class, "NMS");
            assert_eq!(order.action, "BUY");
            assert_eq!(order.total_quantity.to_string(), "10");
            assert_eq!(order.limit_price, 175.5);
            assert_eq!(order.account, "DU123");
            assert_eq!(order.order_ref, "ref");
            assert_eq!(order.client_id, 7);
            assert_eq!(order.perm_id, 123456);
            assert!(order.outside_rth);
            assert!(!order.hidden);
            assert_eq!(order_state.status, "0");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_completed_order_core_callback() {
    let completed = field_payload(
        Incoming::CompletedOrder,
        &[
            "265598",
            "AAPL",
            "STK",
            "",
            "0",
            "",
            "",
            "SMART",
            "USD",
            "AAPL",
            "NMS",
            "SELL",
            "5",
            "MKT",
            "0",
            "0",
            "DAY",
            "",
            "DU123",
            "C",
            "0",
            "ref2",
            "123457",
            "0",
            "1",
            "0",
            "20260709 10:00:00",
            "",
            "",
            "",
            "",
            "0",
            "0",
            "",
            "0",
            "",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "",
            "",
            "",
            "",
            "0",
            "",
            "",
            "0",
            "",
            "",
            "0",
            "Submitted",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "",
            "",
            "",
            "",
            "0",
            "0",
            "0",
            "",
            "0",
            "",
            "0",
        ],
    );
    let event = decoder::decode_payload(false, &completed).unwrap();
    match event {
        Event::CompletedOrder {
            contract,
            order,
            order_state,
        } => {
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(contract.trading_class, "NMS");
            assert_eq!(order.action, "SELL");
            assert_eq!(order.total_quantity.to_string(), "5");
            assert_eq!(order.order_type, "MKT");
            assert_eq!(order.account, "DU123");
            assert_eq!(order.order_ref, "ref2");
            assert_eq!(order.perm_id, 123457);
            assert!(!order.outside_rth);
            assert!(order.hidden);
            assert_eq!(order.good_after_time, "20260709 10:00:00");
            assert_eq!(order_state.status, "");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_metadata_list_callbacks() {
    let tiers = field_payload(
        Incoming::SoftDollarTiers,
        &["1", "1", "tier", "value", "display"],
    );
    match decoder::decode_payload(false, &tiers).unwrap() {
        Event::SoftDollarTiers { req_id, tiers } => {
            assert_eq!(req_id, 1);
            assert_eq!(tiers[0].name, "tier");
            assert_eq!(tiers[0].value, "value");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let family = field_payload(Incoming::FamilyCodes, &["1", "DU123", "family"]);
    assert_eq!(
        decoder::decode_payload(false, &family).unwrap(),
        Event::FamilyCodes {
            family_codes: vec![FamilyCode {
                account_id: "DU123".to_owned(),
                family_code: "family".to_owned(),
            }],
        }
    );

    let symbol = field_payload(
        Incoming::SymbolSamples,
        &[
            "2",
            "1",
            "265598",
            "AAPL",
            "STK",
            "NASDAQ",
            "USD",
            "2",
            "OPT",
            "FUT",
            "Apple Inc",
            "ISSUER",
        ],
    );
    match decoder::decode_payload(false, &symbol).unwrap() {
        Event::SymbolSamples {
            req_id,
            descriptions,
        } => {
            assert_eq!(req_id, 2);
            assert_eq!(descriptions[0].contract.symbol, "AAPL");
            assert_eq!(descriptions[0].derivative_sec_types, ["OPT", "FUT"]);
            assert_eq!(descriptions[0].contract.issuer_id, "ISSUER");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let smart = field_payload(Incoming::SmartComponents, &["3", "1", "1", "ISLAND", "Q"]);
    match decoder::decode_payload(false, &smart).unwrap() {
        Event::SmartComponents { req_id, components } => {
            assert_eq!(req_id, 3);
            assert_eq!(components[0].exchange_letter, "Q");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let depth = field_payload(
        Incoming::MarketDepthExchanges,
        &["1", "ISLAND", "STK", "NASDAQ", "Deep2", "1"],
    );
    match decoder::decode_payload(false, &depth).unwrap() {
        Event::MarketDepthExchanges { descriptions } => {
            assert_eq!(
                descriptions[0],
                DepthMarketDataDescription {
                    exchange: "ISLAND".to_owned(),
                    security_type: "STK".to_owned(),
                    listing_exchange: "NASDAQ".to_owned(),
                    service_data_type: "Deep2".to_owned(),
                    aggregate_group: 1,
                }
            );
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_wsh_schedule_and_user_callbacks() {
    let tick_req = field_payload(Incoming::TickRequestParameters, &["4", "0.01", "BBO", "7"]);
    assert_eq!(
        decoder::decode_payload(false, &tick_req).unwrap(),
        Event::TickReqParams {
            req_id: 4,
            min_tick: "0.01".to_owned(),
            bbo_exchange: "BBO".to_owned(),
            snapshot_permissions: 7,
            last_price_precision: String::new(),
            last_size_precision: String::new(),
        }
    );

    let schedule = field_payload(
        Incoming::HistoricalSchedule,
        &[
            "5",
            "20260709 09:30:00",
            "20260709 16:00:00",
            "US/Eastern",
            "1",
            "20260709 09:30:00",
            "20260709 16:00:00",
            "20260709",
        ],
    );
    match decoder::decode_payload(false, &schedule).unwrap() {
        Event::HistoricalSchedule {
            req_id,
            time_zone,
            sessions,
            ..
        } => {
            assert_eq!(req_id, 5);
            assert_eq!(time_zone, "US/Eastern");
            assert_eq!(sessions[0].ref_date, "20260709");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let wsh = field_payload(Incoming::WshMetaData, &["6", "{\"ok\":true}"]);
    assert_eq!(
        decoder::decode_payload(false, &wsh).unwrap(),
        Event::WshMetaData {
            req_id: 6,
            data_json: "{\"ok\":true}".to_owned(),
        }
    );

    let user = field_payload(Incoming::UserInfo, &["7", "white"]);
    assert_eq!(
        decoder::decode_payload(false, &user).unwrap(),
        Event::UserInfo {
            req_id: 7,
            white_branding_id: "white".to_owned(),
        }
    );
}

#[test]
fn decoder_reads_field_based_pnl_news_and_rule_callbacks() {
    let head = field_payload(Incoming::HeadTimestamp, &["1", "20260709 09:30:00"]);
    assert_eq!(
        decoder::decode_payload(false, &head).unwrap(),
        Event::HeadTimestamp {
            req_id: 1,
            head_timestamp: "20260709 09:30:00".to_owned(),
        }
    );

    let histogram = field_payload(Incoming::HistogramData, &["2", "1", "175.5", "10"]);
    assert_eq!(
        decoder::decode_payload(false, &histogram).unwrap(),
        Event::HistogramData {
            req_id: 2,
            items: vec![HistogramEntry {
                price: 175.5,
                size: "10".parse().unwrap(),
            }],
        }
    );

    let market_rule = field_payload(Incoming::MarketRule, &["26", "1", "0", "0.01"]);
    assert_eq!(
        decoder::decode_payload(false, &market_rule).unwrap(),
        Event::MarketRule {
            market_rule_id: 26,
            price_increments: vec![PriceIncrement {
                low_edge: 0.0,
                increment: 0.01,
            }],
        }
    );

    let pnl = field_payload(Incoming::Pnl, &["3", "1.5", "2.5", "3.5"]);
    assert_eq!(
        decoder::decode_payload(false, &pnl).unwrap(),
        Event::Pnl {
            req_id: 3,
            daily_pnl: 1.5,
            unrealized_pnl: 2.5,
            realized_pnl: 3.5,
        }
    );

    let pnl_single = field_payload(
        Incoming::PnlSingle,
        &["4", "10", "1.5", "2.5", "3.5", "4.5"],
    );
    match decoder::decode_payload(false, &pnl_single).unwrap() {
        Event::PnlSingle {
            req_id,
            position,
            value,
            ..
        } => {
            assert_eq!(req_id, 4);
            assert_eq!(position.to_string(), "10");
            assert_eq!(value, 4.5);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let providers = field_payload(Incoming::NewsProviders, &["1", "BRFG", "Briefing"]);
    assert_eq!(
        decoder::decode_payload(false, &providers).unwrap(),
        Event::NewsProviders {
            providers: vec![NewsProvider {
                code: "BRFG".to_owned(),
                name: "Briefing".to_owned(),
            }],
        }
    );
}

#[test]
fn decoder_reads_field_based_news_and_order_bound_callbacks() {
    let article = field_payload(Incoming::NewsArticle, &["5", "0", "body"]);
    assert_eq!(
        decoder::decode_payload(false, &article).unwrap(),
        Event::NewsArticle {
            req_id: 5,
            article_type: 0,
            article_text: "body".to_owned(),
        }
    );

    let historical_news = field_payload(
        Incoming::HistoricalNews,
        &["6", "20260709", "BRFG", "A1", "headline"],
    );
    assert_eq!(
        decoder::decode_payload(false, &historical_news).unwrap(),
        Event::HistoricalNews {
            req_id: 6,
            time: "20260709".to_owned(),
            provider_code: "BRFG".to_owned(),
            article_id: "A1".to_owned(),
            headline: "headline".to_owned(),
        }
    );

    let historical_news_end = field_payload(Incoming::HistoricalNewsEnd, &["6", "1"]);
    assert_eq!(
        decoder::decode_payload(false, &historical_news_end).unwrap(),
        Event::HistoricalNewsEnd {
            req_id: 6,
            has_more: true,
        }
    );

    let tick_news = field_payload(
        Incoming::TickNews,
        &["7", "1700000000", "BRFG", "A1", "headline", "extra"],
    );
    assert_eq!(
        decoder::decode_payload(false, &tick_news).unwrap(),
        Event::TickNews {
            req_id: 7,
            timestamp: 1_700_000_000,
            provider_code: "BRFG".to_owned(),
            article_id: "A1".to_owned(),
            headline: "headline".to_owned(),
            extra_data: "extra".to_owned(),
        }
    );

    let order_bound = field_payload(Incoming::OrderBound, &["123456", "7", "1001"]);
    assert_eq!(
        decoder::decode_payload(false, &order_bound).unwrap(),
        Event::OrderBound {
            perm_id: 123456,
            client_id: 7,
            order_id: 1001,
        }
    );
}

#[test]
fn decoder_reads_field_based_historical_tick_callbacks() {
    let ticks = field_payload(
        Incoming::HistoricalTicks,
        &["1", "1", "1700000000", "", "175.5", "10", "1"],
    );
    match decoder::decode_payload(false, &ticks).unwrap() {
        Event::HistoricalTicks {
            req_id,
            ticks,
            done,
        } => {
            assert_eq!(req_id, 1);
            assert!(done);
            assert_eq!(ticks[0].time, 1_700_000_000);
            assert_eq!(ticks[0].size.to_string(), "10");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let bid_ask = field_payload(
        Incoming::HistoricalTicksBidAsk,
        &[
            "2",
            "1",
            "1700000001",
            "3",
            "175.4",
            "175.6",
            "20",
            "30",
            "0",
        ],
    );
    match decoder::decode_payload(false, &bid_ask).unwrap() {
        Event::HistoricalTicksBidAsk { ticks, done, .. } => {
            assert!(!done);
            assert!(ticks[0].tick_attrib_bid_ask.ask_past_high);
            assert!(ticks[0].tick_attrib_bid_ask.bid_past_low);
            assert_eq!(ticks[0].size_ask.to_string(), "30");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let last = field_payload(
        Incoming::HistoricalTicksLast,
        &[
            "3",
            "1",
            "1700000002",
            "3",
            "175.7",
            "40",
            "NASDAQ",
            "@",
            "1",
        ],
    );
    match decoder::decode_payload(false, &last).unwrap() {
        Event::HistoricalTicksLast { ticks, done, .. } => {
            assert!(done);
            assert!(ticks[0].tick_attrib_last.past_limit);
            assert!(ticks[0].tick_attrib_last.unreported);
            assert_eq!(ticks[0].exchange, "NASDAQ");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_field_based_tick_option_computation_callback() {
    let option = field_payload(
        Incoming::TickOptionComputation,
        &[
            "1", "13", "7", "0.25", "0.5", "1.25", "0.1", "0.2", "0.3", "0.4", "175.5",
        ],
    );
    assert_eq!(
        decoder::decode_payload(false, &option).unwrap(),
        Event::TickOptionComputation {
            req_id: 1,
            tick_type: 13,
            tick_attrib: 7,
            implied_vol: 0.25,
            delta: 0.5,
            opt_price: 1.25,
            pv_dividend: 0.1,
            gamma: 0.2,
            vega: 0.3,
            theta: 0.4,
            und_price: 175.5,
        }
    );
}

#[test]
fn decoder_reads_field_based_tick_by_tick_callbacks() {
    let last = field_payload(
        Incoming::TickByTick,
        &["1", "1", "1700000000", "175.5", "10", "3", "NASDAQ", "@"],
    );
    match decoder::decode_payload(false, &last).unwrap() {
        Event::TickByTick {
            req_id,
            tick_type,
            tick: Some(TickByTickPayload::Last(tick)),
        } => {
            assert_eq!(req_id, 1);
            assert_eq!(tick_type, 1);
            assert!(tick.tick_attrib_last.past_limit);
            assert!(tick.tick_attrib_last.unreported);
            assert_eq!(tick.exchange, "NASDAQ");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let bid_ask = field_payload(
        Incoming::TickByTick,
        &["2", "3", "1700000001", "175.4", "175.6", "20", "30", "3"],
    );
    match decoder::decode_payload(false, &bid_ask).unwrap() {
        Event::TickByTick {
            tick: Some(TickByTickPayload::BidAsk(tick)),
            ..
        } => {
            assert!(tick.tick_attrib_bid_ask.bid_past_low);
            assert!(tick.tick_attrib_bid_ask.ask_past_high);
            assert_eq!(tick.size_ask.to_string(), "30");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let midpoint = field_payload(Incoming::TickByTick, &["3", "4", "1700000002", "175.5"]);
    match decoder::decode_payload(false, &midpoint).unwrap() {
        Event::TickByTick {
            tick: Some(TickByTickPayload::MidPoint(tick)),
            ..
        } => {
            assert_eq!(tick.time, 1_700_000_002);
            assert_eq!(tick.price, 175.5);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_protobuf_order_status_event() {
    let status = protobuf::OrderStatus {
        order_id: Some(1001),
        status: Some("Filled".to_owned()),
        filled: Some("10".to_owned()),
        remaining: Some("0".to_owned()),
        avg_fill_price: Some(123.45),
        perm_id: Some(99),
        parent_id: Some(0),
        last_fill_price: Some(123.45),
        client_id: Some(7),
        why_held: Some(String::new()),
        mkt_cap_price: Some(0.0),
    };
    let payload = status.encode_to_vec();
    let event = decoder::decode_protobuf_event(Incoming::OrderStatus as i32, &payload).unwrap();

    match event {
        Event::OrderStatus {
            order_id,
            status,
            avg_fill_price,
            ..
        } => {
            assert_eq!(order_id, 1001);
            assert_eq!(status, "Filled");
            assert_eq!(avg_fill_price, 123.45);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_protobuf_account_summary_end_event() {
    let payload = protobuf::AccountSummaryEnd { req_id: Some(77) }.encode_to_vec();
    let event =
        decoder::decode_protobuf_event(Incoming::AccountSummaryEnd as i32, &payload).unwrap();

    assert_eq!(event, Event::AccountSummaryEnd { req_id: 77 });
}

#[test]
fn decoder_reads_protobuf_historical_data_bars_event() {
    let payload = protobuf::HistoricalData {
        req_id: Some(12),
        historical_data_bars: vec![protobuf::HistoricalDataBar {
            date: Some("20260708 12:00:00".to_owned()),
            open: Some(10.0),
            high: Some(11.0),
            low: Some(9.5),
            close: Some(10.5),
            volume: Some("1000".to_owned()),
            wap: Some("10.25".to_owned()),
            bar_count: Some(8),
        }],
    }
    .encode_to_vec();

    let event = decoder::decode_protobuf_event(Incoming::HistoricalData as i32, &payload).unwrap();

    match event {
        Event::HistoricalDataBars { req_id, bars } => {
            assert_eq!(req_id, 12);
            assert_eq!(bars.len(), 1);
            assert_eq!(bars[0].date, "20260708 12:00:00");
            assert_eq!(bars[0].close, 10.5);
            assert_eq!(bars[0].bar_count, 8);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_protobuf_position_and_account_events() {
    let position = protobuf::Position {
        account: Some("DU123".to_owned()),
        contract: Some(protobuf::Contract {
            con_id: Some(265598),
            symbol: Some("AAPL".to_owned()),
            sec_type: Some("STK".to_owned()),
            exchange: Some("SMART".to_owned()),
            currency: Some("USD".to_owned()),
            ..protobuf::Contract::default()
        }),
        position: Some("25".to_owned()),
        avg_cost: Some(150.25),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::PositionData as i32, &position.encode_to_vec())
            .unwrap();

    match event {
        Event::Position {
            account,
            contract,
            avg_cost,
            ..
        } => {
            assert_eq!(account, "DU123");
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(avg_cost, 150.25);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let account = protobuf::AccountValue {
        key: Some("NetLiquidation".to_owned()),
        value: Some("100000".to_owned()),
        currency: Some("USD".to_owned()),
        account_name: Some("DU123".to_owned()),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::AccountValue as i32, &account.encode_to_vec())
            .unwrap();

    assert_eq!(
        event,
        Event::AccountValue {
            key: "NetLiquidation".to_owned(),
            value: "100000".to_owned(),
            currency: "USD".to_owned(),
            account_name: "DU123".to_owned(),
        }
    );
}

#[test]
fn decoder_reads_protobuf_pnl_and_news_events() {
    let pnl = protobuf::PnLSingle {
        req_id: Some(44),
        position: Some("3".to_owned()),
        daily_pn_l: Some(1.5),
        unrealized_pn_l: Some(2.5),
        realized_pn_l: Some(3.5),
        value: Some(4.5),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::PnlSingle as i32, &pnl.encode_to_vec()).unwrap();

    match event {
        Event::PnlSingle {
            req_id,
            daily_pnl,
            value,
            ..
        } => {
            assert_eq!(req_id, 44);
            assert_eq!(daily_pnl, 1.5);
            assert_eq!(value, 4.5);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let news = protobuf::TickNews {
        req_id: Some(55),
        timestamp: Some(1_700_000_000),
        provider_code: Some("BRFG".to_owned()),
        article_id: Some("A1".to_owned()),
        headline: Some("Headline".to_owned()),
        extra_data: Some("extra".to_owned()),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::TickNews as i32, &news.encode_to_vec()).unwrap();

    assert_eq!(
        event,
        Event::TickNews {
            req_id: 55,
            timestamp: 1_700_000_000,
            provider_code: "BRFG".to_owned(),
            article_id: "A1".to_owned(),
            headline: "Headline".to_owned(),
            extra_data: "extra".to_owned(),
        }
    );
}

#[test]
fn decoder_reads_protobuf_open_and_completed_order_events() {
    let contract = Some(protobuf_contract());
    let order = Some(protobuf_order());
    let order_state = Some(protobuf::OrderState {
        status: Some("Submitted".to_owned()),
        commission_and_fees: Some(1.25),
        commission_and_fees_currency: Some("USD".to_owned()),
        init_margin_before_outside_rth: Some(2.5),
        order_allocations: vec![protobuf::OrderAllocation {
            account: Some("DU123".to_owned()),
            position: Some("10".to_owned()),
            position_desired: Some("12".to_owned()),
            position_after: Some("11".to_owned()),
            desired_alloc_qty: Some("2".to_owned()),
            allowed_alloc_qty: Some("1".to_owned()),
            is_monetary: Some(true),
        }],
        ..protobuf::OrderState::default()
    });

    let open_order = protobuf::OpenOrder {
        order_id: Some(77),
        contract: contract.clone(),
        order: order.clone(),
        order_state: order_state.clone(),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::OpenOrder as i32, &open_order.encode_to_vec())
            .unwrap();

    match event {
        Event::OpenOrder {
            order_id,
            contract,
            order,
            order_state,
        } => {
            assert_eq!(order_id, 77);
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(order.action, "BUY");
            assert_eq!(order.total_quantity.to_string(), "10");
            assert_eq!(order.active_start_time, "20260709 09:30:00 US/Eastern");
            assert_eq!(order.fa_group, "group");
            assert_eq!(order.origin, Origin::Firm);
            assert_eq!(order.delta_neutral_order_type, "MKT");
            assert_eq!(order.scale_init_level_size, 100);
            assert_eq!(order.hedge_type, "D");
            assert!(order.what_if);
            assert_eq!(order.reference_contract_id, 265598);
            assert_eq!(order.ext_operator, "operator");
            assert_eq!(order.cash_qty, 1000.0);
            assert_eq!(order.route_marketable_to_bbo, 1);
            assert_eq!(order.parent_perm_id, 123456);
            assert_eq!(order.hedge_max_size, 10);
            assert_eq!(order_state.status, "Submitted");
            assert_eq!(order_state.commission_and_fees, 1.25);
            assert_eq!(order_state.init_margin_before_outside_rth, 2.5);
            assert_eq!(order_state.order_allocations.len(), 1);
            assert_eq!(order_state.order_allocations[0].position.to_string(), "10");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let completed_order = protobuf::CompletedOrder {
        contract,
        order,
        order_state,
    };
    let event = decoder::decode_protobuf_event(
        Incoming::CompletedOrder as i32,
        &completed_order.encode_to_vec(),
    )
    .unwrap();

    match event {
        Event::CompletedOrder {
            contract,
            order,
            order_state,
        } => {
            assert_eq!(contract.sec_type, "STK");
            assert_eq!(order.limit_price, 175.5);
            assert_eq!(order_state.commission_and_fees_currency, "USD");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_protobuf_contract_and_execution_events() {
    let contract_data = protobuf::ContractData {
        req_id: Some(88),
        contract: Some(protobuf_contract()),
        contract_details: Some(protobuf::ContractDetails {
            market_name: Some("NASDAQ".to_owned()),
            min_tick: Some("0.01".to_owned()),
            order_types: Some("LMT,MKT".to_owned()),
            valid_exchanges: Some("SMART,NASDAQ".to_owned()),
            long_name: Some("Apple Inc".to_owned()),
            ev_rule: Some("ev-rule".to_owned()),
            ev_multiplier: Some(2.5),
            sec_id_list: [("ISIN".to_owned(), "US0378331005".to_owned())]
                .into_iter()
                .collect(),
            agg_group: Some(7),
            under_symbol: Some("AAPL".to_owned()),
            under_sec_type: Some("STK".to_owned()),
            market_rule_ids: Some("26".to_owned()),
            ..protobuf::ContractDetails::default()
        }),
    };
    let event = decoder::decode_protobuf_event(
        Incoming::ContractData as i32,
        &contract_data.encode_to_vec(),
    )
    .unwrap();

    match event {
        Event::ContractDetails { req_id, details } => {
            assert_eq!(req_id, 88);
            assert_eq!(details.contract.symbol, "AAPL");
            assert_eq!(details.market_name, "NASDAQ");
            assert_eq!(details.min_tick, 0.01);
            assert_eq!(details.ev_rule, "ev-rule");
            assert_eq!(details.ev_multiplier, 2.5);
            assert_eq!(details.sec_id_list[0].tag, "ISIN");
            assert_eq!(details.aggregate_group, 7);
            assert_eq!(details.under_symbol, "AAPL");
            assert_eq!(details.under_sec_type, "STK");
            assert_eq!(details.market_rule_ids, "26");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let execution = protobuf::ExecutionDetails {
        req_id: Some(89),
        contract: Some(protobuf_contract()),
        execution: Some(protobuf::Execution {
            order_id: Some(77),
            exec_id: Some("E1".to_owned()),
            time: Some("20260709 12:00:00".to_owned()),
            acct_number: Some("DU123".to_owned()),
            exchange: Some("NASDAQ".to_owned()),
            side: Some("BOT".to_owned()),
            shares: Some("10".to_owned()),
            price: Some(175.5),
            cum_qty: Some("10".to_owned()),
            avg_price: Some(175.5),
            ..protobuf::Execution::default()
        }),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::ExecutionData as i32, &execution.encode_to_vec())
            .unwrap();

    match event {
        Event::ExecutionDetails {
            req_id,
            contract,
            execution,
        } => {
            assert_eq!(req_id, 89);
            assert_eq!(contract.symbol, "AAPL");
            assert_eq!(execution.exec_id, "E1");
            assert_eq!(execution.shares.to_string(), "10");
            assert_eq!(execution.avg_price, 175.5);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

fn protobuf_contract() -> protobuf::Contract {
    protobuf::Contract {
        con_id: Some(265598),
        symbol: Some("AAPL".to_owned()),
        sec_type: Some("STK".to_owned()),
        exchange: Some("SMART".to_owned()),
        primary_exch: Some("NASDAQ".to_owned()),
        currency: Some("USD".to_owned()),
        ..protobuf::Contract::default()
    }
}

fn protobuf_order() -> protobuf::Order {
    protobuf::Order {
        order_id: Some(77),
        client_id: Some(1002),
        perm_id: Some(123456),
        action: Some("BUY".to_owned()),
        total_quantity: Some("10".to_owned()),
        order_type: Some("LMT".to_owned()),
        lmt_price: Some(175.5),
        tif: Some("DAY".to_owned()),
        active_start_time: Some("20260709 09:30:00 US/Eastern".to_owned()),
        active_stop_time: Some("20260709 16:00:00 US/Eastern".to_owned()),
        account: Some("DU123".to_owned()),
        settling_firm: Some("IB".to_owned()),
        clearing_account: Some("DU123".to_owned()),
        clearing_intent: Some("IB".to_owned()),
        fa_group: Some("group".to_owned()),
        fa_method: Some("PctChange".to_owned()),
        fa_percentage: Some("100".to_owned()),
        open_close: Some("O".to_owned()),
        origin: Some(1),
        short_sale_slot: Some(2),
        designated_location: Some("loc".to_owned()),
        exempt_code: Some(3),
        volatility: Some(0.2),
        volatility_type: Some(2),
        delta_neutral_order_type: Some("MKT".to_owned()),
        delta_neutral_aux_price: Some(1.25),
        delta_neutral_con_id: Some(265598),
        continuous_update: Some(true),
        reference_price_type: Some(1),
        scale_init_level_size: Some(100),
        scale_subs_level_size: Some(50),
        scale_price_increment: Some(0.01),
        hedge_type: Some("D".to_owned()),
        hedge_param: Some("delta=0.5".to_owned()),
        algo_id: Some("algo-1".to_owned()),
        what_if: Some(true),
        not_held: Some(true),
        solicited: Some(true),
        reference_contract_id: Some(265598),
        adjusted_order_type: Some("STP".to_owned()),
        ext_operator: Some("operator".to_owned()),
        cash_qty: Some(1000.0),
        mifid2_decision_maker: Some("maker".to_owned()),
        is_oms_container: Some(true),
        route_marketable_to_bbo: Some(1),
        parent_perm_id: Some(123456),
        use_price_mgmt_algo: Some(1),
        min_trade_qty: Some(5),
        compete_against_best_offset: Some(0.05),
        customer_account: Some("cust".to_owned()),
        professional_customer: Some(true),
        include_overnight: Some(true),
        manual_order_indicator: Some(1),
        submitter: Some("submitter".to_owned()),
        post_only: Some(true),
        allow_pre_open: Some(true),
        ignore_open_auction: Some(true),
        seek_price_improvement: Some(1),
        what_if_type: Some(2),
        hedge_max_size: Some(10),
        transmit: Some(true),
        ..protobuf::Order::default()
    }
}

#[test]
fn decoder_reads_remaining_protobuf_market_metadata_events() {
    let commission = protobuf::CommissionAndFeesReport {
        exec_id: Some("E1".to_owned()),
        commission_and_fees: Some(1.25),
        currency: Some("USD".to_owned()),
        realized_pnl: Some(2.5),
        ..protobuf::CommissionAndFeesReport::default()
    };
    let event = decoder::decode_protobuf_event(
        Incoming::CommissionAndFeesReport as i32,
        &commission.encode_to_vec(),
    )
    .unwrap();
    match event {
        Event::CommissionAndFeesReport { report } => {
            assert_eq!(report.exec_id, "E1");
            assert_eq!(report.commission_and_fees, 1.25);
            assert_eq!(report.realized_pnl, 2.5);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let smart = protobuf::SmartComponents {
        req_id: Some(12),
        smart_components: vec![protobuf::SmartComponent {
            bit_number: Some(1),
            exchange: Some("ISLAND".to_owned()),
            exchange_letter: Some("Q".to_owned()),
        }],
    };
    let event =
        decoder::decode_protobuf_event(Incoming::SmartComponents as i32, &smart.encode_to_vec())
            .unwrap();
    match event {
        Event::SmartComponents { req_id, components } => {
            assert_eq!(req_id, 12);
            assert_eq!(components[0].exchange, "ISLAND");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let symbols = protobuf::SymbolSamples {
        req_id: Some(13),
        contract_descriptions: vec![protobuf::ContractDescription {
            contract: Some(protobuf_contract()),
            derivative_sec_types: vec!["OPT".to_owned(), "FUT".to_owned()],
        }],
    };
    let event =
        decoder::decode_protobuf_event(Incoming::SymbolSamples as i32, &symbols.encode_to_vec())
            .unwrap();
    match event {
        Event::SymbolSamples {
            req_id,
            descriptions,
        } => {
            assert_eq!(req_id, 13);
            assert_eq!(descriptions[0].contract.symbol, "AAPL");
            assert_eq!(descriptions[0].derivative_sec_types, ["OPT", "FUT"]);
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn decoder_reads_remaining_protobuf_historical_tick_events() {
    let ticks = protobuf::HistoricalTicks {
        req_id: Some(21),
        historical_ticks: vec![protobuf::HistoricalTick {
            time: Some(1_700_000_000),
            price: Some(175.5),
            size: Some("10".to_owned()),
        }],
        is_done: Some(true),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::HistoricalTicks as i32, &ticks.encode_to_vec())
            .unwrap();
    match event {
        Event::HistoricalTicks {
            req_id,
            ticks,
            done,
        } => {
            assert_eq!(req_id, 21);
            assert!(done);
            assert_eq!(ticks[0].size.to_string(), "10");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let bid_ask = protobuf::HistoricalTicksBidAsk {
        req_id: Some(22),
        historical_ticks_bid_ask: vec![protobuf::HistoricalTickBidAsk {
            time: Some(1_700_000_001),
            tick_attrib_bid_ask: Some(protobuf::TickAttribBidAsk {
                bid_past_low: Some(true),
                ask_past_high: Some(false),
            }),
            price_bid: Some(175.4),
            price_ask: Some(175.6),
            size_bid: Some("20".to_owned()),
            size_ask: Some("30".to_owned()),
        }],
        is_done: Some(false),
    };
    let event = decoder::decode_protobuf_event(
        Incoming::HistoricalTicksBidAsk as i32,
        &bid_ask.encode_to_vec(),
    )
    .unwrap();
    match event {
        Event::HistoricalTicksBidAsk { ticks, done, .. } => {
            assert!(!done);
            assert!(ticks[0].tick_attrib_bid_ask.bid_past_low);
            assert_eq!(ticks[0].size_ask.to_string(), "30");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let last_tick = protobuf::HistoricalTickLast {
        time: Some(1_700_000_002),
        tick_attrib_last: Some(protobuf::TickAttribLast {
            past_limit: Some(true),
            unreported: Some(false),
        }),
        price: Some(175.7),
        size: Some("40".to_owned()),
        exchange: Some("NASDAQ".to_owned()),
        special_conditions: Some("@".to_owned()),
    };
    let last = protobuf::HistoricalTicksLast {
        req_id: Some(23),
        historical_ticks_last: vec![last_tick.clone()],
        is_done: Some(true),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::HistoricalTicksLast as i32, &last.encode_to_vec())
            .unwrap();
    match event {
        Event::HistoricalTicksLast { ticks, done, .. } => {
            assert!(done);
            assert_eq!(ticks[0].exchange, "NASDAQ");
            assert!(ticks[0].tick_attrib_last.past_limit);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let tick_by_tick = protobuf::TickByTickData {
        req_id: Some(24),
        tick_type: Some(1),
        tick: Some(protobuf::tick_by_tick_data::Tick::HistoricalTickLast(
            last_tick,
        )),
    };
    let event =
        decoder::decode_protobuf_event(Incoming::TickByTick as i32, &tick_by_tick.encode_to_vec())
            .unwrap();
    match event {
        Event::TickByTick {
            req_id,
            tick: Some(TickByTickPayload::Last(tick)),
            ..
        } => {
            assert_eq!(req_id, 24);
            assert_eq!(tick.price, 175.7);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let schedule = protobuf::HistoricalSchedule {
        req_id: Some(25),
        start_date_time: Some("20260709 09:30:00".to_owned()),
        end_date_time: Some("20260709 16:00:00".to_owned()),
        time_zone: Some("US/Eastern".to_owned()),
        historical_sessions: vec![protobuf::HistoricalSession {
            start_date_time: Some("20260709 09:30:00".to_owned()),
            end_date_time: Some("20260709 16:00:00".to_owned()),
            ref_date: Some("20260709".to_owned()),
        }],
    };
    let event = decoder::decode_protobuf_event(
        Incoming::HistoricalSchedule as i32,
        &schedule.encode_to_vec(),
    )
    .unwrap();
    match event {
        Event::HistoricalSchedule {
            req_id,
            time_zone,
            sessions,
            ..
        } => {
            assert_eq!(req_id, 25);
            assert_eq!(time_zone, "US/Eastern");
            assert_eq!(sessions[0].ref_date, "20260709");
        }
        other => panic!("unexpected event: {other:?}"),
    }
}
