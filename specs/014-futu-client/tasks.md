# Implementation Plan: truefix-futu-client

## Overview

按 actor/channel 架构分五层递进实现：
1. **codec 层**（纯函数，无 I/O）：frame + crypto
2. **transport 层**（帧级 TCP 读写）
3. **actor 层**（ConnectionActor 事件循环）
4. **handle/client 层**（公开 API：FutuClient、QuoteClient、TradeClient）
5. **集成测试**（mock OpenD）

每层独立可测，后一层依赖前一层。

---

## Tasks

- [x] 1. 搭建 crate 骨架
  - 创建 `crates/truefix-futu-client/Cargo.toml`：依赖 `prost`、`tokio`、`bytes`、`thiserror`、`tracing`（workspace），以及 `aes = "0.8"`、`cbc = "0.1"`、`sha1 = "0.10"`（固定版本）
  - 将 `truefix-futu-client` 加入 workspace `Cargo.toml` 的 `members` 列表，并在 `[workspace.dependencies]` 注册 `bytes = "1"`
  - 创建 `build.rs`：遍历 `proto/` 下所有 `*.proto`，用 `prost-build` 批量编译，emit `cargo:rerun-if-changed`
  - 创建 `proto/` 软链接指向 `thrdpty/clientapi/FTAPI4Python_10.8.6808/futu/common/pb/`
  - 创建 `src/lib.rs`：模块声明（`codec`, `transport`, `actor`, `handle`, `client`, `quote`, `trade`, `push`, `error`, `proto_id`, `pb`）+ `#![cfg_attr(not(test), deny(clippy::unwrap_used, ...))]`
  - 创建 `src/pb.rs`：`include!(concat!(env!("OUT_DIR"), "/..."))` 引入 prost 输出
  - 验证 `cargo build -p truefix-futu-client` 编译通过
  - _Requirements: 9.1, 10.1, 10.2, 10.3_

- [x] 2. 实现 `src/error.rs` 和 `src/proto_id.rs`
  - [x] 2.1 实现 `src/error.rs`：定义 `FutuError`（Io, BadMagic, Sha1Mismatch, Crypto, ProtobufDecode, OpenDError, Timeout, SerialOverflow, ActorGone）和 `FutuResult<T>`
    - _Requirements: 5.4, 5.5_
  - [x] 2.2 实现 `src/proto_id.rs`：会话/交易/行情三组 `pub const`、`ALL_PUSH_IDS: &[u32]`、`pub fn is_push(proto_id: u32) -> bool`
    - _Requirements: 6.1_
  - [ ]* 2.3 property 测试：`is_push(id) == ALL_PUSH_IDS.contains(&id)` 对任意 u32 成立
    - **Property 7** · _Requirements: 6.1_

- [x] 3. 实现 `src/codec/frame.rs`
  - [x] 3.1 定义 `FrameHeader` struct；实现 `encode_frame(header, ciphertext) -> [u8; 44]`：写 b"FT" 魔数、LE u32 字段、SHA1、8 字节保留
    - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.8_
  - [x] 3.2 实现 `decode_header(buf: &[u8; 44]) -> FutuResult<FrameHeader>`：校验魔数，不匹配返回 `FutuError::BadMagic`
    - _Requirements: 1.2_
  - [x] 3.3 实现 `body_sha1(plaintext: &[u8]) -> [u8; 20]` 和 `verify_sha1(header, plaintext) -> bool`
    - _Requirements: 1.6, 1.7_
  - [ ]* 3.4 property 测试：帧往返（Property 1）、SHA1 篡改检测（Property 3）、BadMagic 检测（Property 4）
    - _Requirements: 1.1–1.8_

- [x] 4. 实现 `src/codec/crypto.rs`
  - [x] 4.1 定义 `EncAlgo` 枚举（None, FtAesEcb, AesCbc）；实现 `None` 的 encrypt/decrypt（恒等）
    - _Requirements: 4.1_
  - [x] 4.2 实现 `FtAesEcb::encrypt`：零填充到 16 字节对齐 → AES-ECB 加密 → 追加 16 字节尾块记录原始末尾长度
    - _Requirements: 4.2_
  - [x] 4.3 实现 `FtAesEcb::decrypt`：读取尾块恢复原始长度 → 解密 → 截断；密文非 16 倍数返回 `FutuError::Crypto`
    - _Requirements: 4.3, 4.5_
  - [x] 4.4 实现 `AesCbc::encrypt` / `decrypt`：PKCS#7 padding，使用 `cbc` crate；密文非 16 倍数返回 `FutuError::Crypto`
    - _Requirements: 4.4, 4.5_
  - [ ]* 4.5 property 测试：加密往返三模式（Property 2）、FTAES-ECB 密文长度公式（Property 5）、非法密文长度错误（Property 6）
    - _Requirements: 4.1–4.6_

- [x] 5. 实现 `src/transport.rs`
  - [x] 5.1 定义 `FramedTransport { reader: OwnedReadHalf, writer: OwnedWriteHalf, enc: EncAlgo }`；实现 `connect(host, port) -> FutuResult<Self>`
    - _Requirements: 2.1_
  - [x] 5.2 实现 `send(&mut self, proto_id, serial_no, plaintext)`：`enc.encrypt(body)` → `body_sha1(plaintext)` → `encode_frame` → `writer.write_all`
    - _Requirements: 1.1, 1.6, 4.6_
  - [x] 5.3 实现 `recv(&mut self) -> FutuResult<(FrameHeader, Bytes)>`：`read_exact(44)` → `decode_header` → `read_exact(body_len)` → `enc.decrypt` → `verify_sha1`，不匹配返回 `Sha1Mismatch`
    - _Requirements: 1.2, 1.7_
  - [x] 5.4 实现 `set_enc(&mut self, enc: EncAlgo)` 和 `split(self) -> (FrameReader, FrameWriter)`
    - _Requirements: 2.6_
  - [~] 5.5 单元测试：使用 `tokio::io::duplex()` 验证 send/recv 往返（含加密切换）
    - _Requirements: 1.1–1.8, 4.6_

- [x] 6. 实现 `src/actor.rs` — ConnectionActor
  - [x] 6.1 定义 `Command` 枚举（`Request { proto_id, body, reply }`, `Shutdown`）和 `ConnectionActor` struct（reader, writer, cmd_rx, pending HashMap, push_tx, serial, keepalive Interval）
    - _Requirements: 5.1, 5.2, 6.2_
  - [x] 6.2 实现 `on_command`：`serial.checked_add(1)` 或返回 `SerialOverflow`；`writer.send()` → 存入 `pending`
    - _Requirements: 5.1, 5.5_
  - [x] 6.3 实现 `on_frame`：`is_push` 分支发 `push_tx.send(decode_push(frame))`；否则按 serial_no 从 `pending` 取出 sender 发送响应
    - _Requirements: 5.2, 5.3, 6.1, 6.2, 6.3_
  - [x] 6.4 实现 `on_keepalive`：构造 `KeepAlive::Request`，调用 `writer.send()`（不等待响应，KeepAlive 响应由 on_frame 自动丢弃）
    - _Requirements: 3.1, 3.2_
  - [x] 6.5 实现 `run(mut self)` 主循环：`tokio::select!` 三路 — cmd_rx, reader.recv, keepalive.tick；reader 返回错误时记录 `tracing::warn` 并 break
    - _Requirements: 3.2, 3.3_
  - [ ]* 6.6 单元测试：使用 `tokio::io::duplex()` 验证并发 N 请求各自得到匹配响应（Property 9）；推送帧不干扰请求（Property 9）
    - _Requirements: 5.1–5.6, 6.1–6.5_

- [x] 7. 实现 `src/handle.rs` 和 `src/client.rs`
  - [x] 7.1 实现 `ActorHandle { cmd_tx, push_tx }`：`request(proto_id, body) -> FutuResult<(FrameHeader, Bytes)>`（发 Command，`tokio::time::timeout` 包裹 oneshot await），`subscribe() -> broadcast::Receiver<Push>`
    - _Requirements: 5.3, 5.4, 6.4_
  - [x] 7.2 实现 `FutuClient::connect(config)`：TCP 连接 → 握手（直接用 transport 发/收 InitConnect）→ `transport.set_enc()` → `transport.split()` → `tokio::spawn(ConnectionActor)` → 返回 `FutuClient { inner: Arc::new(ActorHandle) }`
    - _Requirements: 2.1, 2.2, 2.3, 2.5, 2.6_
  - [x] 7.3 实现 `FutuClient::subscribe_push() -> broadcast::Receiver<Push>`、`quote() -> QuoteClient`、`trade() -> TradeClient`
    - _Requirements: 6.4, 6.5_
  - [ ]* 7.4 property 测试：serial_no 严格单调递增（Property 8）
    - _Requirements: 2.4_

- [x] 8. 实现 `src/quote.rs` — QuoteClient
  - [x] 8.1 实现 `QuoteClient::subscribe`（proto_id 3001）、`get_basic_qot`（3004）、`get_kl`（3006）：构造 Request proto → `prost::encode` → `handle.request()` → `prost::decode` Response → 检查 retType
    - _Requirements: 7.1, 7.2, 7.3, 7.5_
  - [x] 8.2 实现 `get_order_book`（3012）、`get_ticker`（3010）、`get_static_info`（3202）、`get_security_snapshot`（3203）
    - _Requirements: 7.4, 7.5_
  - [ ]* 8.3 property 测试：Qot 系列 proto 消息序列化往返（Property 10）
    - _Requirements: 9.2, 9.3_

- [x] 9. 实现 `src/trade.rs` — TradeClient 及 request builders
  - [x] 9.1 实现 `PlaceOrderRequest` builder（替代 8 个位置参数）、`ModifyOrderRequest` builder、`PositionListRequest` builder
    - _Requirements: 8.1, 8.2, 8.3_
  - [x] 9.2 实现 `get_acc_list`（2001）、`unlock_trade`（2005）、`sub_acc_push`（2008）、`get_funds`（2101）、`get_position_list`（2102）
    - _Requirements: 8.1, 8.4, 8.6_
  - [x] 9.3 实现 `get_order_list`（2201）、`place_order`（2202，返回 orderID）、`modify_order`（2205）、`get_order_fill_list`（2211）、`get_history_order_list`（2221）
    - _Requirements: 8.1, 8.2, 8.5, 8.6_
  - [ ]* 9.4 property 测试：Trd 系列 proto 消息序列化往返（Property 10）
    - _Requirements: 9.2, 9.3_

- [x] 10. Mock OpenD 集成测试
  - [x] 10.1 在 `tests/mock_opend.rs` 中用 `tokio::net::TcpListener` 实现最小化 mock OpenD：接受连接，回复 InitConnect Response（retType=0, connID=1, connAESKey=16 字节, keepAliveInterval=30），回复 KeepAlive
    - _Requirements: 2.1, 2.2, 2.3_
  - [x] 10.2 集成测试：`FutuClient::connect` 对 mock 握手成功；`InitConnect` retType=-1 时返回 `FutuError::OpenDError`
    - _Requirements: 2.2, 2.3_
  - [x] 10.3 集成测试：mock 添加 `Trd_PlaceOrder`（2202）响应（orderID=12345），验证 `place_order` 返回值正确；同时推送一帧 2208，验证 `subscribe_push` 收到该帧且 `place_order` 返回正确
    - _Requirements: 5.1, 5.2, 6.4, 8.1_
  - [x] 10.4 集成测试：mock 添加 `Qot_GetBasicQot`（3004）非零 retType 响应，验证 `get_basic_qot` 返回 `FutuError::OpenDError`
    - _Requirements: 7.2, 7.5_
  - [ ]* 10.5 property 测试（`tokio::io::duplex`）：N 个并发请求各自收到匹配响应，推送帧不污染任何请求（Property 9）
    - _Requirements: 5.1–5.6_

- [x] 11. Final Checkpoint
  - `cargo test -p truefix-futu-client` 全部通过
  - `cargo clippy -p truefix-futu-client -- -D warnings` 无告警
  - `cargo build -p truefix-futu-client --release` 编译通过

---

## Notes

- `*` 标注的子任务为可选 property 测试，可跳过以加速 MVP
- `EncAlgo: Clone` 是 transport.split() 后 reader/writer 各持一份的要求，或改由 actor 统一持有加密状态
- KeepAlive 响应帧的 serial_no 在 pending 中不存在时静默丢弃，actor 不报错
- `broadcast::Sender` 容量建议配置为 256；消费者 lagged 时返回 `RecvError::Lagged`，由调用方决定是否重新订阅
- `bytes = "1"` 需加入 workspace dependencies，`Bytes` 实现 `Clone`，满足 `broadcast` 要求
- proto/` 软链接在 macOS 下 `cargo build` 可以正常识别；若 CI 环境不支持软链接，改为 build.rs 中用绝对路径指定 include 目录

---

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1"] },
    { "wave": 2, "tasks": ["2"] },
    { "wave": 3, "tasks": ["3", "4"] },
    { "wave": 4, "tasks": ["5"] },
    { "wave": 5, "tasks": ["6"] },
    { "wave": 6, "tasks": ["7"] },
    { "wave": 7, "tasks": ["8", "9"] },
    { "wave": 8, "tasks": ["10"] },
    { "wave": 9, "tasks": ["11"] }
  ],
  "dependencies": {
    "2":  ["1"],
    "3":  ["2"],
    "4":  ["2"],
    "5":  ["3", "4"],
    "6":  ["5"],
    "7":  ["6"],
    "8":  ["7"],
    "9":  ["7"],
    "10": ["8", "9"],
    "11": ["10"]
  }
}
```

**并行说明：**
- Wave 3：Tasks 3（codec/frame）和 4（codec/crypto）可并行，互不依赖
- Wave 7：Tasks 8（quote）和 9（trade）可并行，均只依赖 Task 7（handle/client）完成
