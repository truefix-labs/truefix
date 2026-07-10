use std::error::Error;

use bytes::Bytes;
use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use truefix_futu_client::codec::frame::{
    FRAME_HEADER_LEN, FrameHeader, body_sha1, decode_header, encode_frame,
};
use truefix_futu_client::error::FutuError;
use truefix_futu_client::pb;
use truefix_futu_client::proto_id;
use truefix_futu_client::push::Push;
use truefix_futu_client::{FutuClient, FutuClientConfig};

const AES_KEY: &str = "0123456789abcdef";

#[tokio::test]
async fn connect_success_and_reject_paths() {
    let server = MockOpenD::spawn(Scenario::ConnectOk).await;
    let client = connect_client(server.port).await;
    let _ = client.disconnect().await;
    server.abort();

    let server = MockOpenD::spawn(Scenario::ConnectReject).await;
    match FutuClient::connect(client_config(server.port)).await {
        Ok(_) => panic!("expected connect to fail"),
        Err(FutuError::OpenDError { ret_type, .. }) => assert_eq!(ret_type, -1),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
    server.abort();
}

#[tokio::test]
async fn place_order_returns_id_and_receives_push() {
    let server = MockOpenD::spawn(Scenario::PlaceOrderWithPush).await;
    let client = connect_client(server.port).await;
    let trade = client.trade();
    let mut push_rx = client.subscribe_push();

    let push_task = tokio::spawn(async move { push_rx.recv().await.unwrap() });
    let order_id = trade
        .place_order(default_place_order_request())
        .await
        .unwrap();
    let push = push_task.await.unwrap();

    assert_eq!(order_id, 12345);
    match push {
        Push::UpdateOrder(update) => {
            assert_eq!(update.order.code, "AAPL");
            assert_eq!(update.order.order_id, 12345);
        }
        other => panic!("unexpected push: {other:?}"),
    }

    let _ = client.disconnect().await;
    server.abort();
}

#[tokio::test]
async fn get_basic_qot_error_surfaces_opend_error() {
    let server = MockOpenD::spawn(Scenario::BasicQotReject).await;
    let client = connect_client(server.port).await;
    let quote = client.quote();

    let err = quote
        .get_basic_qot(truefix_futu_client::quote::GetBasicQotRequest {
            security_list: vec![pb::qot_common::Security {
                market: 1,
                code: "AAPL".to_owned(),
            }],
            header: Some(pb::qot_common::QotHeader {
                security_firm: None,
            }),
        })
        .await
        .unwrap_err();

    match err {
        FutuError::OpenDError { ret_type, .. } => assert_eq!(ret_type, -1),
        other => panic!("unexpected error: {other:?}"),
    }

    let _ = client.disconnect().await;
    server.abort();
}

async fn connect_client(port: u16) -> FutuClient {
    FutuClient::connect(client_config(port)).await.unwrap()
}

fn client_config(port: u16) -> FutuClientConfig {
    FutuClientConfig {
        host: "127.0.0.1".to_owned(),
        port,
        client_id: "1002".to_owned(),
        client_ver: 300,
        recv_notify: true,
        request_timeout_ms: 5_000,
        packet_enc_algo: pb::common::PacketEncAlgo::None as i32,
        init_rsa_key_path: None,
        auto_reconnect: false,
        reconnect_interval_ms: 50,
        security_firm: None,
    }
}

fn default_place_order_request() -> truefix_futu_client::trade::PlaceOrderRequest {
    truefix_futu_client::trade::PlaceOrderRequest {
        header: pb::trd_common::TrdHeader {
            trd_env: 0,
            acc_id: 1,
            trd_market: 2,
            jp_acc_type: None,
        },
        trd_side: pb::trd_common::TrdSide::Buy as i32,
        order_type: 1,
        code: "AAPL".to_owned(),
        qty: 1.0,
        price: Some(100.0),
        adjust_price: None,
        adjust_side_and_limit: None,
        sec_market: Some(pb::trd_common::TrdSecMarket::Us as i32),
        remark: Some("mock".to_owned()),
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

#[derive(Clone, Copy)]
enum Scenario {
    ConnectOk,
    ConnectReject,
    PlaceOrderWithPush,
    BasicQotReject,
}

struct MockOpenD {
    port: u16,
    task: JoinHandle<()>,
}

impl MockOpenD {
    async fn spawn(scenario: Scenario) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            if let Err(err) = run_session(&mut socket, scenario).await {
                panic!("mock opend session failed: {err}");
            }
        });
        Self { port, task }
    }

    fn abort(self) {
        self.task.abort();
    }
}

impl Drop for MockOpenD {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run_session(
    socket: &mut TcpStream,
    scenario: Scenario,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    loop {
        let (header, body) = read_frame(socket).await?;
        match header.proto_id {
            proto_id::INIT_CONNECT => {
                let _req = pb::init_connect::Request::decode(body.as_ref())?;
                match scenario {
                    Scenario::ConnectReject => {
                        let resp = pb::init_connect::Response {
                            ret_type: -1,
                            ret_msg: Some("reject".to_owned()),
                            err_code: Some(1),
                            s2c: None,
                        };
                        write_frame(socket, proto_id::INIT_CONNECT, header.serial_no, &resp)
                            .await?;
                        break;
                    }
                    _ => {
                        let resp = pb::init_connect::Response {
                            ret_type: 0,
                            ret_msg: None,
                            err_code: None,
                            s2c: Some(pb::init_connect::S2c {
                                server_ver: 225,
                                login_user_id: 88,
                                conn_id: 1,
                                conn_aes_key: AES_KEY.to_owned(),
                                keep_alive_interval: 30,
                                aes_cb_civ: None,
                                user_attribution: None,
                            }),
                        };
                        write_frame(socket, proto_id::INIT_CONNECT, header.serial_no, &resp)
                            .await?;
                    }
                }
            }
            proto_id::KEEP_ALIVE => {
                let resp = pb::keep_alive::Response {
                    ret_type: 0,
                    ret_msg: None,
                    err_code: None,
                    s2c: Some(pb::keep_alive::S2c { time: 1 }),
                };
                write_frame(socket, proto_id::KEEP_ALIVE, header.serial_no, &resp).await?;
            }
            proto_id::TRD_PLACE_ORDER => {
                let _req = pb::trd_place_order::Request::decode(body.as_ref())?;
                if matches!(scenario, Scenario::PlaceOrderWithPush) {
                    let push = pb::trd_update_order::S2c {
                        header: pb::trd_common::TrdHeader {
                            trd_env: 0,
                            acc_id: 1,
                            trd_market: 2,
                            jp_acc_type: None,
                        },
                        order: pb::trd_common::Order {
                            trd_side: pb::trd_common::TrdSide::Buy as i32,
                            order_type: 1,
                            order_status: 5,
                            order_id: 12345,
                            order_id_ex: "order-12345".to_owned(),
                            code: "AAPL".to_owned(),
                            name: "Apple".to_owned(),
                            qty: 1.0,
                            price: Some(100.0),
                            create_time: "2026-07-09 12:00:00".to_owned(),
                            update_time: "2026-07-09 12:00:00".to_owned(),
                            fill_qty: None,
                            fill_avg_price: None,
                            last_err_msg: None,
                            sec_market: Some(pb::trd_common::TrdSecMarket::Us as i32),
                            create_timestamp: None,
                            update_timestamp: None,
                            remark: Some("mock".to_owned()),
                            time_in_force: None,
                            fill_outside_rth: None,
                            aux_price: None,
                            trail_type: None,
                            trail_value: None,
                            trail_spread: None,
                            currency: None,
                            trd_market: Some(pb::trd_common::TrdMarket::Us as i32),
                            session: None,
                            jp_acc_type: None,
                            expire_time: None,
                            order_amount: None,
                            strategy_type: None,
                            combo_legs: Vec::new(),
                        },
                    };
                    let push_response = pb::trd_update_order::Response {
                        ret_type: 0,
                        ret_msg: None,
                        err_code: None,
                        s2c: Some(push),
                    };
                    write_frame(socket, proto_id::TRD_UPDATE_ORDER, 0, &push_response).await?;
                }
                let resp = pb::trd_place_order::Response {
                    ret_type: 0,
                    ret_msg: None,
                    err_code: None,
                    s2c: Some(pb::trd_place_order::S2c {
                        header: pb::trd_common::TrdHeader {
                            trd_env: 0,
                            acc_id: 1,
                            trd_market: 2,
                            jp_acc_type: None,
                        },
                        order_id: Some(12345),
                        order_id_ex: None,
                    }),
                };
                write_frame(socket, proto_id::TRD_PLACE_ORDER, header.serial_no, &resp).await?;
                break;
            }
            proto_id::QOT_GET_BASIC_QOT => {
                let _req = pb::qot_get_basic_qot::Request::decode(body.as_ref())?;
                let resp = pb::qot_get_basic_qot::Response {
                    ret_type: -1,
                    ret_msg: Some("nope".to_owned()),
                    err_code: Some(1),
                    s2c: None,
                };
                write_frame(socket, proto_id::QOT_GET_BASIC_QOT, header.serial_no, &resp).await?;
                break;
            }
            _ => break,
        }
    }
    Ok(())
}

async fn read_frame(
    socket: &mut TcpStream,
) -> Result<(FrameHeader, Bytes), Box<dyn Error + Send + Sync>> {
    let mut header_buf = [0u8; FRAME_HEADER_LEN];
    socket.read_exact(&mut header_buf).await?;
    let header = decode_header(&header_buf)?;
    let mut body = vec![0u8; header.body_len as usize];
    socket.read_exact(&mut body).await?;
    Ok((header, Bytes::from(body)))
}

async fn write_frame<T: Message>(
    socket: &mut TcpStream,
    proto_id: u32,
    serial_no: u32,
    msg: &T,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let body = msg.encode_to_vec();
    let header = FrameHeader {
        proto_id,
        proto_fmt: 0,
        proto_ver: 0,
        serial_no,
        body_len: body.len() as u32,
        body_sha1: body_sha1(&body),
    };
    let frame = encode_frame(&header, &body);
    socket.write_all(&frame).await?;
    socket.write_all(&body).await?;
    Ok(())
}
