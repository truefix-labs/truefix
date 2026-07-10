# Requirements Document

## Introduction

`truefix-futu-client` 是对富途 OpenD TCP+Protobuf 私有协议的薄封装 Rust crate，提供帧编解码、可选 AES 加密、连接握手、KeepAlive 保活、行情订阅以及交易下单等功能。本文档将设计中的技术决策转化为可测试的 EARS 格式验收标准，供实现阶段逐项验证。

---

## Glossary

- **FrameClient**：`truefix-futu-client` 中负责帧编解码的模块（`frame.rs`）
- **EncAlgo**：加密算法枚举，包含 `None`、`FtAesEcb`、`AesCbc` 三种模式
- **CryptoEngine**：`crypto.rs` 模块中实现 `EncAlgo::encrypt` / `decrypt` 的组件
- **Session**：`session.rs` 中执行 InitConnect 握手及 KeepAlive 保活的状态机
- **FutuClient**：`client.rs` 中的主入口结构体，组合 `FrameConn` + `Session` + 请求/响应路由
- **OpenD**：富途官方本地网关进程，通过 TCP 监听客户端连接
- **proto_id**：OpenD 协议 ID，用于区分不同消息类型
- **serial_no**：每个请求帧携带的自增序列号，用于响应匹配
- **Push 帧**：由 OpenD 主动下发的推送消息（`proto_id ∈ ALL_PUSH_IDS`），无对应 serial_no 请求
- **SHA1**：帧体完整性校验摘要，存储在 44 字节帧头的 `arrBodySHA1` 字段
- **FTAES-ECB**：富途自定义 AES-ECB 变体，尾块使用 16 字节附加块替代标准 padding
- **AES-CBC**：标准 AES-CBC 模式，使用 PKCS#7 padding
- **keepAliveInterval**：InitConnect 握手 S2C 响应中返回的心跳间隔（秒），客户端每 `interval * 4/5` 秒发送一次 KeepAlive

---

## Requirements

### Requirement 1: 帧头编解码

**User Story:** As a developer, I want the client to correctly encode and decode the 44-byte OpenD wire frame header, so that frames can be reliably exchanged with OpenD.

#### Acceptance Criteria

1. THE FrameClient SHALL produce a frame whose first 2 bytes are `0x46 0x54` ("FT") for every encoded frame.
2. WHEN a 44-byte buffer is decoded, THE FrameClient SHALL return `FutuError::BadMagic` if the first 2 bytes are not `b"FT"`.
3. WHEN a frame is encoded, THE FrameClient SHALL store the `proto_id` as a little-endian `u32` at bytes 2–5 of the frame header.
4. WHEN a frame is encoded, THE FrameClient SHALL store the `serial_no` as a little-endian `u32` at bytes 10–13 of the frame header.
5. WHEN a frame is encoded, THE FrameClient SHALL store the `body_len` as a little-endian `u32` at bytes 14–17 of the frame header.
6. WHEN a frame is encoded, THE FrameClient SHALL compute SHA1 over the plaintext body and store it in bytes 18–37 of the frame header.
7. WHEN `verify_body_sha1` is called with a body that differs from the encoded body, THE FrameClient SHALL return `false`.
8. THE FrameClient SHALL produce exactly `44 + body_len` bytes for every call to `encode_frame`.

---

### Requirement 2: 连接握手（InitConnect）

**User Story:** As a developer, I want the client to perform the InitConnect handshake with OpenD on connection, so that an authenticated session with AES keys is established before any trading or market data requests.

#### Acceptance Criteria

1. WHEN `FutuClient::connect` is called, THE Session SHALL send an `InitConnect` request frame with `proto_id = 1001` and `serial_no = 1` as the first frame on the TCP connection.
2. WHEN the `InitConnect` Response `retType` equals `0` (success), THE Session SHALL extract `connID`, `connAESKey`, `keepAliveInterval`, and optionally `aesCBCiv` from the S2C payload.
3. WHEN the `InitConnect` Response `retType` is non-zero, THE Session SHALL return `FutuError::OpenDError` and close the TCP connection.
4. AFTER a successful `InitConnect` handshake, THE FutuClient SHALL increment `serial_no` by 1 for each subsequent outgoing request frame.
5. WHEN `FutuClient::connect` succeeds, THE Session SHALL start a background KeepAlive task that fires every `keepAliveInterval * 4 / 5` seconds.
6. AFTER a successful `InitConnect` handshake, THE FutuClient SHALL apply the negotiated `EncAlgo` to all subsequent frames (encrypt on send, decrypt on receive).

---

### Requirement 3: KeepAlive 保活

**User Story:** As a developer, I want the client to send periodic KeepAlive heartbeats, so that the OpenD connection is not dropped due to inactivity.

#### Acceptance Criteria

1. WHEN the KeepAlive timer fires, THE Session SHALL send a `KeepAlive` request frame with `proto_id = 1004` and the current UTC Unix timestamp in the `time` field.
2. IF a KeepAlive request results in an I/O error, THEN THE Session SHALL log a `tracing::warn` event and mark the connection state as `Disconnected`.
3. IF the KeepAlive response `retType` is non-zero, THEN THE Session SHALL log a `tracing::warn` event.

---

### Requirement 4: 加密（FTAES-ECB / AES-CBC）

**User Story:** As a developer, I want the client to encrypt and decrypt frames using the algorithm negotiated during InitConnect, so that communication with OpenD can be secured when RSA keys are configured.

#### Acceptance Criteria

1. WHEN `EncAlgo` is `None`, THE CryptoEngine SHALL return the plaintext bytes unchanged for both `encrypt` and `decrypt`.
2. WHEN `EncAlgo` is `FtAesEcb`, THE CryptoEngine SHALL pad the plaintext to a 16-byte block boundary using zero-fill, then append a 16-byte tail block recording the original data length, yielding ciphertext of length `ceil(plaintext.len() / 16) * 16 + 16` bytes.
3. WHEN `EncAlgo` is `FtAesEcb`, THE CryptoEngine SHALL on decryption read the 16-byte tail block to recover the original length and truncate the plaintext accordingly.
4. WHEN `EncAlgo` is `AesCbc`, THE CryptoEngine SHALL use PKCS#7 padding and the `key` and `iv` from `InitConnect` S2C.
5. IF the ciphertext length is not a multiple of 16 during decryption for either `FtAesEcb` or `AesCbc`, THEN THE CryptoEngine SHALL return `FutuError::Crypto`.
6. FOR ALL valid plaintexts and valid keys/ivs, `CryptoEngine::decrypt(CryptoEngine::encrypt(plaintext)) == plaintext` SHALL hold (round-trip property).

---

### Requirement 5: 请求-响应路由（Actor 模型）

**User Story:** As a developer, I want each request to be matched to its corresponding response by serial_no via the ConnectionActor, so that multiple callers can issue concurrent requests and push frames never corrupt any request reply.

#### Acceptance Criteria

1. WHEN a request is dispatched via `ActorHandle::request`, THE ConnectionActor SHALL register a `oneshot::Sender` keyed by `serial_no` before sending the frame to OpenD.
2. WHEN a response frame arrives whose `serial_no` matches a pending entry, THE ConnectionActor SHALL resolve that entry's `oneshot::Sender` with the frame and remove it from the pending map.
3. WHEN a Push frame (proto_id ∈ `ALL_PUSH_IDS`) is received, THE ConnectionActor SHALL send it to the `broadcast::Sender` and NOT resolve any pending request entry.
4. IF no matching response arrives within `request_timeout_ms` milliseconds, THEN `ActorHandle::request` SHALL return `FutuError::Timeout`.
5. WHEN `serial_no` would overflow `u32::MAX`, THE ConnectionActor SHALL return `FutuError::SerialOverflow` to the caller before sending.
6. WHEN N callers issue requests concurrently, EACH caller SHALL receive only the response whose `serial_no` matches its own outgoing request.

---

### Requirement 6: 推送帧识别与分发（broadcast）

**User Story:** As a developer, I want push frames to be broadcast to all subscribers so that order updates and quote pushes are received reactively without polling.

#### Acceptance Criteria

1. THE ConnectionActor SHALL classify a received frame as a push frame if and only if its `proto_id` is in `ALL_PUSH_IDS`: `{1003, 2208, 2218, 3005, 3007, 3009, 3011, 3013, 3015, 3019, 3310, 3261}`.
2. WHEN a push frame is received, THE ConnectionActor SHALL decode it into the appropriate `Push` enum variant and send it via `broadcast::Sender<Push>`.
3. WHEN a push frame has an unrecognized `proto_id`, THE ConnectionActor SHALL send `Push::Unknown { proto_id, body }` without returning an error.
4. WHEN `FutuClient::subscribe_push` is called, THE FutuClient SHALL return a `broadcast::Receiver<Push>` that receives all subsequent push frames.
5. Multiple concurrent subscribers SHALL each receive independent copies of every push frame.

---

### Requirement 7: 行情方法

**User Story:** As a developer, I want typed wrappers for the core OpenD quote APIs, so that I can subscribe to quotes and retrieve market data without manually constructing proto messages.

#### Acceptance Criteria

1. WHEN `FutuClient::qot_sub` is called with a valid `Qot_Sub::C2S`, THE FutuClient SHALL send a request with `proto_id = 3001` and return the deserialized `Qot_Sub::S2C` on success.
2. WHEN `FutuClient::get_basic_qot` is called with a non-empty list of `Security`, THE FutuClient SHALL send a request with `proto_id = 3004` and return the `BasicQot` list from the S2C.
3. WHEN `FutuClient::get_kl` is called, THE FutuClient SHALL send a request with `proto_id = 3006`.
4. WHEN `FutuClient::get_order_book` is called, THE FutuClient SHALL send a request with `proto_id = 3012` and return the buy/sell depth from the S2C.
5. IF an OpenD quote response contains `retType != 0`, THEN THE FutuClient SHALL return `FutuError::OpenDError`.

---

### Requirement 8: 交易方法

**User Story:** As a developer, I want typed wrappers for the core OpenD trading APIs, so that I can place, modify, and query orders without manually constructing proto messages.

#### Acceptance Criteria

1. WHEN `FutuClient::place_order` is called with a valid `TrdHeader` and order parameters, THE FutuClient SHALL send a request with `proto_id = 2202` and return the `orderID` from the S2C on success.
2. WHEN `FutuClient::modify_order` is called, THE FutuClient SHALL send a request with `proto_id = 2205`.
3. WHEN `FutuClient::get_position_list` is called, THE FutuClient SHALL send a request with `proto_id = 2102` and return the `Position` list from the S2C.
4. WHEN `FutuClient::get_funds` is called, THE FutuClient SHALL send a request with `proto_id = 2101` and return the `Funds` struct from the S2C.
5. WHEN `FutuClient::get_order_fill_list` is called, THE FutuClient SHALL send a request with `proto_id = 2211` and return the `OrderFill` list.
6. IF an OpenD trade response contains `retType != 0`, THEN THE FutuClient SHALL return `FutuError::OpenDError`.

---

### Requirement 9: Protobuf 生成与往返

**User Story:** As a developer, I want all proto files to be compiled at build time using prost-build, so that Rust types for all OpenD messages are always in sync with the .proto source.

#### Acceptance Criteria

1. THE build script SHALL compile all `*.proto` files found in the `proto/` directory using `prost-build` and emit `cargo:rerun-if-changed` for each file.
2. FOR ALL generated proto message types that have a `Response` wrapper with `retType` and optional `s2c`, THE FutuClient SHALL be able to serialize a `Request` and deserialize the corresponding `Response` without data loss (round-trip property).
3. THE pretty-printer equivalent: THE FutuClient SHALL serialize any generated `Request` message to bytes, then deserialize those bytes back to a `Request` of the same type, yielding a structurally equal message.

---

### Requirement 10: 代码质量与安全约束

**User Story:** As a maintainer, I want the crate to enforce the project-wide lint policy, so that critical-path code is free of panics, unsafe code, and silent failures.

#### Acceptance Criteria

1. THE truefix-futu-client crate SHALL compile without warnings under `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]`.
2. THE truefix-futu-client crate SHALL contain no `unsafe` code blocks (enforced by `unsafe_code = "forbid"` in workspace `Cargo.toml`).
3. THE truefix-futu-client crate SHALL NOT depend on `truefix-gateway` or any other TrueFix crate except via dev-dependencies.
