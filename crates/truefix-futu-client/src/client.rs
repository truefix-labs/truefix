use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use bytes::Bytes;
use prost::Message;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc};
use tokio::time::{Instant, MissedTickBehavior, interval_at, sleep, timeout};
use tracing::warn;

use crate::actor::ConnectionActor;
use crate::codec::crypto::EncAlgo;
use crate::codec::frame::verify_sha1;
use crate::codec::rsa::InitRsaCipher;
use crate::error::{FutuError, FutuResult};
use crate::handle::ActorHandle;
use crate::pb;
use crate::proto_id;
use crate::push::Push;
use crate::quote::{QuoteClient, SubscribeRequest};
use crate::rpc::ensure_ok;
use crate::trade::{SubAccPushRequest, TradeClient, UnlockTradeRequest};
use crate::transport::FramedTransport;

#[derive(Debug, Clone)]
pub struct FutuClientConfig {
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub client_ver: i32,
    pub recv_notify: bool,
    pub request_timeout_ms: u64,
    pub packet_enc_algo: i32,
    pub init_rsa_key_path: Option<String>,
    pub auto_reconnect: bool,
    pub reconnect_interval_ms: u64,
    pub security_firm: Option<pb::trd_common::SecurityFirm>,
}

impl Default for FutuClientConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 11111,
            client_id: "1".to_owned(),
            client_ver: 300,
            recv_notify: true,
            request_timeout_ms: 10_000,
            packet_enc_algo: crate::pb::common::PacketEncAlgo::None as i32,
            init_rsa_key_path: None,
            auto_reconnect: true,
            reconnect_interval_ms: 6_000,
            security_firm: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ReplayState {
    quote_subscriptions: Vec<SubscribeRequest>,
    unlock_trade: Option<UnlockTradeRequest>,
    sub_acc_push: Vec<SubAccPushRequest>,
}

pub(crate) struct ClientCore {
    config: FutuClientConfig,
    handle: RwLock<Arc<ActorHandle>>,
    push_tx: broadcast::Sender<Push>,
    conn_id: AtomicU64,
    packet_serial: Arc<AtomicU32>,
    reconnect_lock: Mutex<()>,
    replay_state: Mutex<ReplayState>,
    closed: AtomicBool,
    disconnect_tx: mpsc::UnboundedSender<()>,
}

#[derive(Clone)]
pub struct FutuClient {
    pub(crate) core: Arc<ClientCore>,
}

impl FutuClient {
    pub async fn connect(config: FutuClientConfig) -> FutuResult<Self> {
        let (push_tx, _) = broadcast::channel(256);
        let packet_serial = Arc::new(AtomicU32::new(1));
        let (disconnect_tx, disconnect_rx) = mpsc::unbounded_channel();
        let (handle, conn_id) =
            connect_session(&config, push_tx.clone(), Arc::clone(&packet_serial), disconnect_tx.clone())
                .await?;

        let core = Arc::new(ClientCore {
            config,
            handle: RwLock::new(handle),
            push_tx,
            conn_id: AtomicU64::new(conn_id),
            packet_serial,
            reconnect_lock: Mutex::new(()),
            replay_state: Mutex::new(ReplayState::default()),
            closed: AtomicBool::new(false),
            disconnect_tx,
        });

        if core.config.auto_reconnect {
            tokio::spawn(Arc::clone(&core).reconnect_loop(disconnect_rx));
        }

        Ok(Self { core })
    }

    pub fn subscribe_push(&self) -> broadcast::Receiver<Push> {
        self.core.push_tx.subscribe()
    }

    pub fn quote(&self) -> QuoteClient {
        QuoteClient {
            core: Arc::clone(&self.core),
        }
    }

    pub fn trade(&self) -> TradeClient {
        TradeClient {
            core: Arc::clone(&self.core),
        }
    }

    pub fn get_security_firm(&self) -> Option<pb::trd_common::SecurityFirm> {
        self.core.get_security_firm()
    }

    pub fn on_api_socket_reconnected(&self) -> FutuResult<()> {
        Ok(())
    }

    pub async fn reconnect(&self) -> FutuResult<()> {
        self.core.reconnect().await
    }

    pub async fn disconnect(&self) -> FutuResult<()> {
        self.core.shutdown().await
    }

    pub async fn close(&self) -> FutuResult<()> {
        self.disconnect().await
    }
}

impl ClientCore {
    pub(crate) fn get_security_firm(&self) -> Option<pb::trd_common::SecurityFirm> {
        self.config.security_firm
    }

    pub(crate) fn conn_id(&self) -> u64 {
        self.conn_id.load(Ordering::Relaxed)
    }

    pub(crate) fn next_serial(&self) -> FutuResult<u32> {
        let current = self.packet_serial.fetch_add(1, Ordering::Relaxed);
        current.checked_add(1).ok_or(FutuError::SerialOverflow)
    }

    pub(crate) fn packet_id_for(&self, serial_no: u32) -> pb::common::PacketId {
        pb::common::PacketId {
            conn_id: self.conn_id(),
            serial_no,
        }
    }

    pub(crate) async fn request<Req, Resp>(&self, proto_id: u32, request: &Req) -> FutuResult<Resp>
    where
        Req: Message,
        Resp: Message + Default,
    {
        let body = Bytes::from(request.encode_to_vec());
        let (_header, body) = self.request_bytes(proto_id, None, body).await?;
        Ok(Resp::decode(body.as_ref())?)
    }

    pub(crate) async fn request_with_serial<Req, Resp>(
        &self,
        proto_id: u32,
        serial_no: u32,
        request: &Req,
        retry_on_disconnect: bool,
    ) -> FutuResult<Resp>
    where
        Req: Message,
        Resp: Message + Default,
    {
        let body = Bytes::from(request.encode_to_vec());
        let (_header, body) = if retry_on_disconnect {
            self.request_bytes(proto_id, Some(serial_no), body).await?
        } else {
            self.request_bytes_no_reconnect(proto_id, Some(serial_no), body)
                .await?
        };
        Ok(Resp::decode(body.as_ref())?)
    }

    async fn request_no_retry<Req, Resp>(&self, proto_id: u32, request: &Req) -> FutuResult<Resp>
    where
        Req: Message,
        Resp: Message + Default,
    {
        let body = Bytes::from(request.encode_to_vec());
        let (_header, body) = self
            .request_bytes_no_reconnect(proto_id, None, body)
            .await?;
        Ok(Resp::decode(body.as_ref())?)
    }

    async fn request_bytes(
        &self,
        proto_id: u32,
        serial_no: Option<u32>,
        body: Bytes,
    ) -> FutuResult<(crate::codec::frame::FrameHeader, Bytes)> {
        let handle = self.current_handle().await;
        match handle
            .request_with_serial(proto_id, serial_no, body.clone())
            .await
        {
            Ok(response) => Ok(response),
            Err(FutuError::ActorGone) if !self.closed.load(Ordering::Relaxed) => {
                let reconnect_result = self.reconnect().await;
                reconnect_result?;
                let handle = self.current_handle().await;
                handle.request_with_serial(proto_id, serial_no, body).await
            }
            Err(err) => Err(err),
        }
    }

    async fn request_bytes_no_reconnect(
        &self,
        proto_id: u32,
        serial_no: Option<u32>,
        body: Bytes,
    ) -> FutuResult<(crate::codec::frame::FrameHeader, Bytes)> {
        let handle = self.current_handle().await;
        handle.request_with_serial(proto_id, serial_no, body).await
    }

    pub(crate) async fn remember_subscription(&self, request: &SubscribeRequest) {
        let mut replay = self.replay_state.lock().await;
        if request.is_unsub_all == Some(true) {
            replay.quote_subscriptions.clear();
            return;
        }

        let normalized = normalize_subscription(request);
        if request.is_sub_or_un_sub {
            replay
                .quote_subscriptions
                .retain(|existing| !same_subscription_key(existing, &normalized));
            replay.quote_subscriptions.push(normalized);
        } else {
            replay
                .quote_subscriptions
                .retain(|existing| !same_subscription_key(existing, &normalized));
        }
    }

    pub(crate) async fn remember_unlock_trade(&self, request: &UnlockTradeRequest) {
        let mut replay = self.replay_state.lock().await;
        replay.unlock_trade = request.unlock.then(|| request.clone());
    }

    pub(crate) async fn remember_sub_acc_push(&self, request: &SubAccPushRequest) {
        let mut replay = self.replay_state.lock().await;
        replay.sub_acc_push.retain(|existing| existing != request);
        replay.sub_acc_push.push(request.clone());
    }

    async fn current_handle(&self) -> Arc<ActorHandle> {
        self.handle.read().await.clone()
    }

    pub(crate) async fn shutdown(&self) -> FutuResult<()> {
        self.closed.store(true, Ordering::Relaxed);
        let handle = self.current_handle().await;
        handle.shutdown().await
    }

    pub(crate) async fn reconnect(&self) -> FutuResult<()> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(FutuError::ActorGone);
        }

        let _guard = self.reconnect_lock.lock().await;
        if self.closed.load(Ordering::Relaxed) {
            return Err(FutuError::ActorGone);
        }

        let (handle, conn_id) = connect_session(
            &self.config,
            self.push_tx.clone(),
            Arc::clone(&self.packet_serial),
            self.disconnect_tx.clone(),
        )
        .await?;
        self.conn_id.store(conn_id, Ordering::Relaxed);

        let old_handle = {
            let mut writer = self.handle.write().await;
            std::mem::replace(&mut *writer, handle)
        };
        let _ = old_handle.shutdown().await;

        self.replay_state().await?;
        Ok(())
    }

    async fn replay_state(&self) -> FutuResult<()> {
        let replay = self.replay_state.lock().await.clone();

        for request in replay.quote_subscriptions {
            let req = pb::qot_sub::Request {
                c2s: pb::qot_sub::C2s {
                    security_list: request.security_list,
                    sub_type_list: request.sub_type_list,
                    is_sub_or_un_sub: true,
                    is_reg_or_un_reg_push: request.is_reg_or_un_reg_push,
                    reg_push_rehab_type_list: request.reg_push_rehab_type_list,
                    is_first_push: request.is_first_push,
                    is_unsub_all: None,
                    is_sub_order_book_detail: request.is_sub_order_book_detail,
                    extended_time: request.extended_time,
                    session: request.session,
                    header: request.header,
                },
            };
            let resp: pb::qot_sub::Response = self.request_no_retry(proto_id::QOT_SUB, &req).await?;
            ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        }

        if let Some(request) = replay.unlock_trade {
            let req = pb::trd_unlock_trade::Request {
                c2s: pb::trd_unlock_trade::C2s {
                    unlock: request.unlock,
                    pwd_md5: request.pwd_md5,
                    security_firm: request.security_firm.map(|firm| firm as i32),
                },
            };
            let resp: pb::trd_unlock_trade::Response =
                self.request_no_retry(proto_id::TRD_UNLOCK_TRADE, &req).await?;
            ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        }

        for request in replay.sub_acc_push {
            let req = pb::trd_sub_acc_push::Request {
                c2s: pb::trd_sub_acc_push::C2s {
                    acc_id_list: request.acc_id_list,
                },
            };
            let resp: pb::trd_sub_acc_push::Response =
                self.request_no_retry(proto_id::TRD_SUB_ACC_PUSH, &req).await?;
            ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
        }

        Ok(())
    }

    async fn reconnect_loop(self: Arc<Self>, mut disconnect_rx: mpsc::UnboundedReceiver<()>) {
        while disconnect_rx.recv().await.is_some() {
            if self.closed.load(Ordering::Relaxed) || !self.config.auto_reconnect {
                break;
            }

            loop {
                if self.closed.load(Ordering::Relaxed) || !self.config.auto_reconnect {
                    return;
                }
                match self.reconnect().await {
                    Ok(()) => break,
                    Err(err) => {
                        warn!(%err, "futu reconnect failed");
                        sleep(Duration::from_millis(self.config.reconnect_interval_ms)).await;
                    }
                }
            }
        }
    }
}

fn normalize_subscription(request: &SubscribeRequest) -> SubscribeRequest {
    SubscribeRequest {
        security_list: request.security_list.clone(),
        sub_type_list: request.sub_type_list.clone(),
        is_sub_or_un_sub: true,
        is_reg_or_un_reg_push: request.is_reg_or_un_reg_push,
        reg_push_rehab_type_list: request.reg_push_rehab_type_list.clone(),
        is_first_push: request.is_first_push,
        is_unsub_all: None,
        is_sub_order_book_detail: request.is_sub_order_book_detail,
        extended_time: request.extended_time,
        session: request.session,
        header: request.header,
    }
}

fn same_subscription_key(left: &SubscribeRequest, right: &SubscribeRequest) -> bool {
    left.security_list == right.security_list
        && left.sub_type_list == right.sub_type_list
        && left.reg_push_rehab_type_list == right.reg_push_rehab_type_list
        && left.is_sub_order_book_detail == right.is_sub_order_book_detail
        && left.extended_time == right.extended_time
        && left.session == right.session
        && left.header == right.header
}

async fn connect_session(
    config: &FutuClientConfig,
    push_tx: broadcast::Sender<Push>,
    packet_serial: Arc<AtomicU32>,
    disconnect_tx: mpsc::UnboundedSender<()>,
) -> FutuResult<(Arc<ActorHandle>, u64)> {
    packet_serial.store(1, Ordering::Relaxed);

    let mut transport = FramedTransport::connect(&config.host, config.port).await?;
    let init_rsa = match config.init_rsa_key_path.as_deref() {
        Some(path) => Some(InitRsaCipher::from_pkcs1_pem_file(path)?),
        None => None,
    };
    let init_req = pb::init_connect::Request {
        c2s: pb::init_connect::C2s {
            client_ver: config.client_ver,
            client_id: config.client_id.clone(),
            recv_notify: Some(config.recv_notify),
            packet_enc_algo: Some(config.packet_enc_algo),
            push_proto_fmt: Some(0),
            programming_language: Some("rust".to_owned()),
            ai_type: None,
        },
    };
    let init_body = init_req.encode_to_vec();
    let body = if let Some(init_rsa) = init_rsa.as_ref() {
        let ciphertext = init_rsa.encrypt(&init_body)?;
        transport
            .send_custom(proto_id::INIT_CONNECT, 1, &init_body, &ciphertext)
            .await?;
        let (header, ciphertext) = timeout(
            Duration::from_millis(config.request_timeout_ms),
            transport.recv_raw(),
        )
        .await
        .map_err(|_| FutuError::Timeout {
            timeout_ms: config.request_timeout_ms,
        })??;
        let plaintext = init_rsa.decrypt(ciphertext.as_ref())?;
        if !verify_sha1(&header, &plaintext) {
            return Err(FutuError::Sha1Mismatch);
        }
        plaintext
    } else {
        transport.send(proto_id::INIT_CONNECT, 1, &init_body).await?;
        let (_header, body) = timeout(
            Duration::from_millis(config.request_timeout_ms),
            transport.recv(),
        )
        .await
        .map_err(|_| FutuError::Timeout {
            timeout_ms: config.request_timeout_ms,
        })??;
        body.to_vec()
    };

    let resp = pb::init_connect::Response::decode(body.as_ref())?;
    ensure_ok(resp.ret_type, resp.ret_msg.clone())?;
    let s2c = resp.s2c.ok_or_else(|| FutuError::OpenDError {
        ret_type: resp.ret_type,
        ret_msg: resp.ret_msg.clone(),
    })?;
    let conn_id = s2c.conn_id;
    let enc = decode_enc_algo(
        config.packet_enc_algo,
        s2c.conn_aes_key.as_str(),
        s2c.aes_cb_civ.as_deref(),
    )?;
    let keepalive_secs = std::cmp::max(1, s2c.keep_alive_interval.max(1) as u64 * 4 / 5);
    transport.set_enc(enc);
    let (reader, writer) = transport.split();
    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let mut keepalive = interval_at(
        Instant::now() + Duration::from_secs(keepalive_secs),
        Duration::from_secs(keepalive_secs),
    );
    keepalive.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let actor = ConnectionActor {
        reader,
        writer,
        cmd_rx,
        pending: Default::default(),
        push_tx: push_tx.clone(),
        serial: packet_serial,
        keepalive,
        disconnect_tx,
    };
    tokio::spawn(actor.run());
    Ok((
        Arc::new(ActorHandle {
            cmd_tx,
            request_timeout_ms: config.request_timeout_ms,
        }),
        conn_id,
    ))
}

fn decode_enc_algo(packet_enc_algo: i32, key: &str, iv: Option<&str>) -> FutuResult<EncAlgo> {
    Ok(match packet_enc_algo {
        x if x == crate::pb::common::PacketEncAlgo::None as i32 => EncAlgo::None,
        x if x == crate::pb::common::PacketEncAlgo::AesCbc as i32 => EncAlgo::AesCbc {
            key: fixed_16_bytes(key)?,
            iv: fixed_16_bytes(iv.unwrap_or_default())?,
        },
        _ => EncAlgo::FtAesEcb {
            key: fixed_16_bytes(key)?,
        },
    })
}

fn fixed_16_bytes(value: &str) -> FutuResult<[u8; 16]> {
    let bytes = value.as_bytes();
    if bytes.len() != 16 {
        return Err(FutuError::Crypto("expected 16 byte key/iv".to_owned()));
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(bytes);
    Ok(out)
}
