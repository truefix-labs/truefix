use prost::Message;
use truefix_twsapi_client::comm;
use truefix_twsapi_client::message::Outgoing;
use truefix_twsapi_client::protobuf;

#[test]
fn generated_start_api_request_serializes_and_frames() {
    let request = protobuf::StartApiRequest {
        client_id: Some(7),
        optional_capabilities: Some("caps".to_owned()),
    };

    let payload = request.encode_to_vec();
    let frame = comm::make_msg_proto(Outgoing::StartApi.protobuf_id(), &payload);

    assert_eq!(
        &frame[4..8],
        &Outgoing::StartApi.protobuf_id().to_be_bytes()
    );
    let decoded = protobuf::StartApiRequest::decode(&frame[8..]).unwrap();
    assert_eq!(decoded.client_id, Some(7));
    assert_eq!(decoded.optional_capabilities.as_deref(), Some("caps"));
}

#[test]
fn generated_market_data_request_has_contract_and_options() {
    let mut request = protobuf::MarketDataRequest {
        req_id: Some(42),
        contract: Some(protobuf::Contract {
            con_id: Some(265598),
            symbol: Some("AAPL".to_owned()),
            sec_type: Some("STK".to_owned()),
            exchange: Some("SMART".to_owned()),
            currency: Some("USD".to_owned()),
            ..protobuf::Contract::default()
        }),
        generic_tick_list: Some("233".to_owned()),
        snapshot: Some(false),
        regulatory_snapshot: Some(false),
        market_data_options: Default::default(),
    };
    request
        .market_data_options
        .insert("key".to_owned(), "value".to_owned());

    let bytes = request.encode_to_vec();
    let decoded = protobuf::MarketDataRequest::decode(bytes.as_slice()).unwrap();

    assert_eq!(decoded.req_id, Some(42));
    assert_eq!(
        decoded
            .contract
            .as_ref()
            .and_then(|contract| contract.symbol.as_deref()),
        Some("AAPL")
    );
    assert_eq!(
        decoded.market_data_options.get("key").map(String::as_str),
        Some("value")
    );
}
