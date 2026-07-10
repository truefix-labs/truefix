# Design Document: truefix-futu-client

## Overview

`truefix-futu-client` 是对富途 OpenD 私有 TCP+Protobuf 协议的薄封装 crate。它采用 **actor/channel** 架构：I/O 收发与协议逻辑集中在一个 tokio task（`ConnectionActor`）中，通过 channel 与调用方解耦，使多个请求可以真正并发，推送事件以 `broadcast` 流而非轮询方式分发。

本 crate 直接暴露 OpenD 原始 proto 结构，不做跨券商抽象。上层 `truefix-gateway::adapters::opend` 通过组合本 crate 实现统一 `TradingGateway` 接口，本 crate 对 gateway 无依赖。

对应 roadmap §3.9 阶段 3s：crate 独立可用，OpenD 模拟环境下单与行情验证通过。

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                   crate 公开 API (Clone + Send)               │
│                                                              │
│  FutuClient (Arc<ActorHandle>)                               │
│    ├── fn quote(&self) -> QuoteClient   行情方法命名空间       │
│    └── fn trade(&self) -> TradeClient   交易方法命名空间       │
└──────────────────────────┬───────────────────────────────────┘
                           │ Command (oneshot reply)
                           ▼
┌──────────────────────────────────────────────────────────────┐
│              ConnectionActor  (单 tokio task)                  │
│                                                              │
│  loop {                                                      │
│    select! {                                                 │
│      cmd  = cmd_rx.recv()          => dispatch_command(cmd)  │
│      frame = transport.recv_frame() => dispatch_frame(frame) │
│      _    = keepalive.tick()       => send_keepalive()       │
│    }                                                         │
│  }                                                           │
│                                                              │
│  pending: HashMap<u32, oneshot::Sender<RawFrame>>            │
│  push_tx: broadcast::Sender<Push>                            │
│  serial:  u32                                                │
└──────────────────────────┬───────────────────────────────────┘
                           │ TcpStream
                           ▼
                        OpenD TCP
```

## Sequence Diagrams

### connect & 握手

```mermaid
sequenceDiagram
    participant App
    participant FutuClient
    participant Actor
    participant OpenD

    App->>FutuClient: FutuClient::connect(config)
    FutuClient->>OpenD: TCP connect
    FutuClient->>OpenD: InitConnect Request (proto_id=1001, serial_no=1, plain)
    OpenD-->>FutuClient: InitConnect Response (connID, connAESKey, keepAliveInterval)
    FutuClient->>Actor: tokio::spawn(ConnectionActor)
    FutuClient-->>App: Ok(FutuClient)
```

### 并发请求（actor 路由）

```mermaid
sequenceDiagram
    participant A as Caller A
    participant B as Caller B
    participant Client as FutuClient
    participant Actor

    A->>Client: place_order(...)
    Client->>Actor: Command::Request{sn=2, reply=tx_a}
    B->>Client: get_funds(...)
    Client->>Actor: Command::Request{sn=3, reply=tx_b}
    Actor-->>OpenD: send frame sn=2
    Actor-->>OpenD: send frame sn=3
    OpenD-->>Actor: response sn=3
    Actor->>B: tx_b.send(Ok(frame))
    OpenD-->>Actor: response sn=2
    Actor->>A: tx_a.send(Ok(frame))
```

### 推送分发（broadcast）

```mermaid
sequenceDiagram
    participant OpenD
    participant Actor
    participant Sub1 as Subscriber 1
    participant Sub2 as Subscriber 2

    OpenD-->>Actor: push frame proto_id=2208
    Actor->>Actor: decode → Push::UpdateOrder(...)
    Actor->>Sub1: push_tx.send(push.clone())
    Actor->>Sub2: push_tx.send(push.clone())
```

---

## Components and Interfaces

### `codec/frame.rs` — 44 字节帧头（纯函数）


```rust
pub const HEADER_FLAG: [u8; 2] = *b"FT";
pub const FRAME_HEADER_LEN: usize = 44;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub proto_id:  u32,
    pub proto_fmt: u8,     // 0=Protobuf, 1=JSON
    pub proto_ver: u8,     // 当前为 0
    pub serial_no: u32,
    pub body_len:  u32,    // 加密后长度
    pub body_sha1: [u8; 20], // 明文 SHA1
}

pub fn encode_frame(header: &FrameHeader, ciphertext: &[u8]) -> [u8; 44];
pub fn decode_header(buf: &[u8; 44])       -> FutuResult<FrameHeader>;
pub fn body_sha1(plaintext: &[u8])         -> [u8; 20];
pub fn verify_sha1(header: &FrameHeader, plaintext: &[u8]) -> bool;
```

职责：编码/解码帧头，计算/校验 SHA1。不涉及 I/O、加密。

---

### `codec/crypto.rs` — 可选加密（纯函数）

```rust
#[derive(Debug, Clone)]
pub enum EncAlgo {
    None,
    FtAesEcb { key: [u8; 16] },
    AesCbc   { key: [u8; 16], iv: [u8; 16] },
}

impl EncAlgo {
    pub fn encrypt(&self, plaintext: &[u8])  -> FutuResult<Vec<u8>>;
    pub fn decrypt(&self, ciphertext: &[u8]) -> FutuResult<Vec<u8>>;
}
```

**FTAES-ECB 规则：** 零填充到 16 字节对齐后加密，追加 16 字节尾块记录原始末尾长度（非标准 padding）。  
**AES-CBC：** 标准 PKCS#7 padding，key/iv 来自 InitConnect S2C。

---

### `transport.rs` — 帧级 TCP 读写

```rust
pub struct FramedTransport {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    enc:    EncAlgo,
}

impl FramedTransport {
    pub async fn connect(host: &str, port: u16) -> FutuResult<Self>;

    /// 发一帧：encrypt(body) → sha1(plaintext) → encode_frame → write_all
    pub async fn send(&mut self, proto_id: u32, serial_no: u32, body: &[u8])
        -> FutuResult<()>;

    /// 收一帧：read_exact(44) → decode_header → read_exact(body_len)
    ///         → decrypt → verify_sha1
    pub async fn recv(&mut self) -> FutuResult<(FrameHeader, Vec<u8>)>;

    /// InitConnect 握手完成后切换加密模式
    pub fn set_enc(&mut self, enc: EncAlgo);

    /// 拆分为独立读写半端（actor 内部使用）
    pub fn split(self) -> (FrameReader, FrameWriter);
}
```

`FrameReader` / `FrameWriter` 对应 `OwnedReadHalf` / `OwnedWriteHalf`，让 actor 可以并发读取帧与写入帧。

---

### `actor.rs` — ConnectionActor

```rust
pub(crate) enum Command {
    Request {
        proto_id: u32,
        body:     Bytes,
        reply:    oneshot::Sender<FutuResult<(FrameHeader, Bytes)>>,
    },
    Shutdown,
}

pub(crate) struct ConnectionActor {
    reader:     FrameReader,
    writer:     FrameWriter,
    cmd_rx:     mpsc::Receiver<Command>,
    pending:    HashMap<u32, oneshot::Sender<FutuResult<(FrameHeader, Bytes)>>>,
    push_tx:    broadcast::Sender<Push>,
    serial:     u32,
    keepalive:  tokio::time::Interval,
}

impl ConnectionActor {
    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => self.on_command(cmd).await,
                Ok(frame) = self.reader.recv() => self.on_frame(frame).await,
                _ = self.keepalive.tick()       => self.on_keepalive().await,
            }
        }
    }
}
```

**`on_command`**：分配 `serial_no`，调用 `writer.send()`，将 `reply` 存入 `pending`。  
**`on_frame`**：若 `is_push(proto_id)` 则 `push_tx.send(decode_push(frame))`；否则按 `serial_no` 从 `pending` 取出 sender 并发送响应。  
**`on_keepalive`**：发送 `KeepAlive` 请求（不等待响应，KeepAlive 响应也走 `on_frame` 正常路径丢弃）。

---

### `handle.rs` — ActorHandle

```rust
#[derive(Clone)]
pub(crate) struct ActorHandle {
    cmd_tx:  mpsc::Sender<Command>,
    push_tx: broadcast::Sender<Push>,
}

impl ActorHandle {
    /// 发送请求，返回对应响应的 raw body
    pub async fn request(&self, proto_id: u32, body: Bytes)
        -> FutuResult<(FrameHeader, Bytes)>;

    /// 订阅推送流
    pub fn subscribe(&self) -> broadcast::Receiver<Push>;
}
```

`ActorHandle` 是 `Clone`，通过 `Arc` 共享给 `QuoteClient` 和 `TradeClient`。

---

### `client.rs` — FutuClient（公开入口）

```rust
#[derive(Clone)]
pub struct FutuClient {
    inner: Arc<ActorHandle>,
}

impl FutuClient {
    /// 建立连接、执行 InitConnect 握手、启动 ConnectionActor
    pub async fn connect(config: FutuClientConfig) -> FutuResult<Self>;

    /// 订阅所有推送事件
    pub fn subscribe_push(&self) -> broadcast::Receiver<Push>;

    /// 行情方法命名空间
    pub fn quote(&self) -> QuoteClient;

    /// 交易方法命名空间
    pub fn trade(&self) -> TradeClient;
}

#[derive(Debug, Clone)]
pub struct FutuClientConfig {
    pub host:               String,      // 默认 "127.0.0.1"
    pub port:               u16,         // 默认 11111
    pub client_id:          String,
    pub recv_notify:        bool,
    pub request_timeout_ms: u64,         // 默认 10_000
}
```

**connect 流程：**
1. `FramedTransport::connect`
2. 握手：直接用 transport 发/收 InitConnect（握手前无 actor）
3. `transport.set_enc(negotiated_enc)`
4. `transport.split()` → `(reader, writer)`
5. 创建 `mpsc::channel`、`broadcast::channel`
6. `tokio::spawn(ConnectionActor { reader, writer, ... }.run())`
7. 返回 `FutuClient { inner: Arc::new(ActorHandle { cmd_tx, push_tx }) }`


---

### `quote.rs` — QuoteClient

```rust
#[derive(Clone)]
pub struct QuoteClient {
    handle: Arc<ActorHandle>,
}

impl QuoteClient {
    pub async fn subscribe(&self, req: qot_sub::C2S)
        -> FutuResult<qot_sub::S2C>;

    pub async fn get_basic_qot(&self, securities: Vec<qot_common::Security>)
        -> FutuResult<Vec<qot_common::BasicQot>>;

    pub async fn get_kl(&self, req: qot_get_kl::C2S)
        -> FutuResult<qot_get_kl::S2C>;

    pub async fn get_order_book(&self, security: qot_common::Security, num: i32)
        -> FutuResult<qot_get_order_book::S2C>;

    pub async fn get_ticker(&self, req: qot_get_ticker::C2S)
        -> FutuResult<qot_get_ticker::S2C>;

    pub async fn get_static_info(&self, req: qot_get_static_info::C2S)
        -> FutuResult<qot_get_static_info::S2C>;

    pub async fn get_security_snapshot(&self, securities: Vec<qot_common::Security>)
        -> FutuResult<Vec<qot_get_security_snapshot::Snapshot>>;
}
```

注意：`&self`（非 `&mut self`），`QuoteClient` 是 `Clone`，可以安全地在多个任务中并发调用。

---

### `trade.rs` — TradeClient

```rust
#[derive(Clone)]
pub struct TradeClient {
    handle: Arc<ActorHandle>,
}

impl TradeClient {
    pub async fn get_acc_list(&self, user_id: u64, trd_category: i32)
        -> FutuResult<Vec<trd_common::TrdAcc>>;

    pub async fn unlock_trade(&self, req: UnlockTradeRequest)
        -> FutuResult<()>;

    pub async fn sub_acc_push(&self, acc_id_list: Vec<u64>)
        -> FutuResult<()>;

    pub async fn get_funds(&self, header: trd_common::TrdHeader)
        -> FutuResult<trd_common::Funds>;

    pub async fn get_position_list(&self, req: PositionListRequest)
        -> FutuResult<Vec<trd_common::Position>>;

    pub async fn get_order_list(&self, header: trd_common::TrdHeader,
                                 filter: trd_common::TrdFilterConditions)
        -> FutuResult<Vec<trd_common::Order>>;

    pub async fn place_order(&self, req: PlaceOrderRequest)
        -> FutuResult<u64>;   // orderID

    pub async fn modify_order(&self, req: ModifyOrderRequest)
        -> FutuResult<u64>;   // orderID

    pub async fn get_order_fill_list(&self, header: trd_common::TrdHeader,
                                      filter: trd_common::TrdFilterConditions)
        -> FutuResult<Vec<trd_common::OrderFill>>;

    pub async fn get_history_order_list(&self, header: trd_common::TrdHeader,
                                         filter: trd_common::TrdFilterConditions)
        -> FutuResult<Vec<trd_common::Order>>;
}
```

复杂请求使用 builder：

```rust
// 取代 8 个位置参数
let req = PlaceOrderRequest::new(header, TrdSide::Buy, "HK.00700")
    .order_type(OrderType::Normal)
    .qty(1000.0)
    .price(350.0)
    .time_in_force(TimeInForce::Day);

let order_id = client.trade().place_order(req).await?;
```

---

### `push.rs` — Push 枚举

```rust
#[derive(Debug, Clone)]
pub enum Push {
    Notify(notify::S2C),                               // 1003
    UpdateOrder(trd_update_order::S2C),                // 2208
    UpdateOrderFill(trd_update_order_fill::S2C),       // 2218
    UpdateBasicQot(qot_update_basic_qot::S2C),         // 3005
    UpdateKl(qot_update_kl::S2C),                      // 3007
    UpdateRt(qot_update_rt::S2C),                      // 3009
    UpdateTicker(qot_update_ticker::S2C),               // 3011
    UpdateOrderBook(qot_update_order_book::S2C),        // 3013
    UpdateBroker(qot_update_broker::S2C),              // 3015
    UpdatePriceReminder(qot_update_price_reminder::S2C), // 3019
    Unknown { proto_id: u32, body: Bytes },
}
```

`Push: Clone` 是 `broadcast` channel 的要求。

---

## Data Flow: 请求并发模型

```
Caller A ──┐                              ┌── Caller A 解除等待
           │  Command{sn=2, reply=tx_a}   │
           ├──────────────────────────────┤
           │                     Actor   │
Caller B ──┤  Command{sn=3, reply=tx_b}   │
           │                              │
           └──────────────────────────────┘── Caller B 解除等待

一个 Actor task 独占 TcpStream，
多个调用方通过 oneshot 通道真正并发，
无任何 Mutex 争用。
```

---

## Error Handling

```rust
pub type FutuResult<T> = Result<T, FutuError>;

#[derive(Debug, thiserror::Error)]
pub enum FutuError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame magic mismatch: got {0:?}")]
    BadMagic([u8; 2]),
    #[error("frame body SHA1 mismatch")]
    Sha1Mismatch,
    #[error("encryption error: {0}")]
    Crypto(String),
    #[error("protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),
    #[error("OpenD error: retType={ret_type}, msg={ret_msg:?}")]
    OpenDError { ret_type: i32, ret_msg: Option<String> },
    #[error("request timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("serial number overflow")]
    SerialOverflow,
    #[error("actor shut down")]
    ActorGone,
}
```

`ActorGone`：actor task panic 或主动 shutdown 时，`cmd_tx.send()` 返回 `SendError`，统一转换为此错误。

---

## Testing Strategy

**codec/ 层（纯函数）**：单元测试 + proptest，无 I/O，不需要 tokio runtime。

**transport.rs**：使用 `tokio::io::duplex()` 构造内存管道，测试帧读写与加密切换。

**actor.rs**：使用 `tokio::io::duplex()` 模拟 OpenD，注入预设响应帧序列，验证并发请求路由与推送分发。

**integration（tests/mock_opend.rs）**：`tokio::net::TcpListener` 实现最小化 mock OpenD，覆盖 InitConnect 握手、并发下单、推送夹杂响应、Actor 正常关闭。

---

## Dependencies

| crate | 用途 |
|-------|------|
| `tokio` (workspace) | 异步 TCP、timer、mpsc、broadcast、oneshot |
| `bytes` | `Bytes` 零拷贝共享 buffer |
| `prost` (workspace) | Protobuf 序列化 |
| `prost-build` (workspace) | build.rs 编译 .proto |
| `thiserror` (workspace) | FutuError derive |
| `tracing` (workspace) | 结构化日志 |
| `aes = "0.8"` | AES-ECB/CBC 加密 |
| `cbc = "0.1"` | AES-CBC mode |
| `sha1 = "0.10"` | 帧体 SHA1 |

---

## Data Models

### Push 枚举（见 `src/push.rs`）

```rust
#[derive(Debug, Clone)]
pub enum Push {
    Notify(notify::S2C),
    UpdateOrder(trd_update_order::S2C),
    UpdateOrderFill(trd_update_order_fill::S2C),
    UpdateBasicQot(qot_update_basic_qot::S2C),
    UpdateKl(qot_update_kl::S2C),
    UpdateRt(qot_update_rt::S2C),
    UpdateTicker(qot_update_ticker::S2C),
    UpdateOrderBook(qot_update_order_book::S2C),
    UpdateBroker(qot_update_broker::S2C),
    UpdatePriceReminder(qot_update_price_reminder::S2C),
    Unknown { proto_id: u32, body: Bytes },
}
```

`Push: Clone` 是 `broadcast::Sender<Push>` 的要求。所有 prost 生成的 S2C 结构体均通过 `#[derive(Clone)]` 满足此约束。

### PlaceOrderRequest builder（见 `src/trade.rs`）

```rust
pub struct PlaceOrderRequest {
    header:       trd_common::TrdHeader,
    trd_side:     i32,
    code:         String,
    qty:          f64,
    order_type:   i32,
    price:        Option<f64>,
    adjust_price: bool,
    time_in_force: i32,
}

impl PlaceOrderRequest {
    pub fn new(header: trd_common::TrdHeader, side: TrdSide, code: impl Into<String>) -> Self;
    pub fn order_type(mut self, v: OrderType)    -> Self;
    pub fn qty(mut self, v: f64)                  -> Self;
    pub fn price(mut self, v: f64)                -> Self;
    pub fn time_in_force(mut self, v: TimeInForce) -> Self;
}
```

类似地，`ModifyOrderRequest` 和 `PositionListRequest` 也使用 builder pattern。

### FutuClientConfig

```rust
#[derive(Debug, Clone)]
pub struct FutuClientConfig {
    pub host:               String,   // 默认 "127.0.0.1"
    pub port:               u16,      // 默认 11111
    pub client_id:          String,   // 默认 "truefix-futu-client"
    pub recv_notify:        bool,     // 默认 false
    pub request_timeout_ms: u64,      // 默认 10_000
    pub push_channel_cap:   usize,    // broadcast 容量，默认 256
}
```

---

## Correctness Properties

### Property 1: 帧编解码往返
任意 proto_id、serial_no、body → encode → decode 还原原值，SHA1 校验通过。

**Validates: Requirements 1.1, 1.3, 1.4, 1.5, 1.8**

### Property 2: 加密往返
任意 plaintext、合法 key/iv → decrypt(encrypt(x)) == x，适用三种模式。

**Validates: Requirements 4.1, 4.6**

### Property 3: 帧体篡改必被检测
任意单字节翻转后 verify_sha1 返回 false。

**Validates: Requirements 1.6, 1.7**

### Property 4: BadMagic 检测
bytes 0–1 不等于 b"FT" 时 decode_header 返回 FutuError::BadMagic。

**Validates: Requirements 1.2**

### Property 5: FTAES-ECB 密文长度
密文长度 = ceil(len/16)*16 + 16，解密还原原始长度。

**Validates: Requirements 4.2, 4.3**

### Property 6: 非法密文长度返回错误
非 16 倍数的密文输入返回 FutuError::Crypto。

**Validates: Requirements 4.5**

### Property 7: is_push 一致性
is_push(id) == (id ∈ ALL_PUSH_IDS)，对全部 u32 成立。

**Validates: Requirements 6.1**

### Property 8: serial_no 严格单调
actor 发出的连续 N 帧 serial_no 严格递增，步长为 1。

**Validates: Requirements 2.4**

### Property 9: 并发请求不交叉
N 个并发请求各自得到匹配 serial_no 的响应；中间插入的推送帧进入 push broadcast，不干扰任何请求。

**Validates: Requirements 5.1, 5.2, 5.3, 5.6, 6.1**

### Property 10: Proto 消息序列化往返
prost-build 生成的任意 Request 序列化再反序列化结构相等。

**Validates: Requirements 9.2, 9.3**
