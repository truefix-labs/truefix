# TrueFix 与 QuickFIX/J + QuickFIX/Go 功能差异 TODO

> 基于 2026-06-30 全量代码审查。
> Git: `1b89828` (Polish phase 已提交)，工作区干净。
> **118 个测试全部通过**。**97/101 任务完成，4 个任务剩余**。

## 当前状态总览

| 维度 | 数据 |
|------|------|
| Git | `1b89828` — S0–S9 + Polish 全部已提交 |
| 测试 | **118 个通过** (29 个测试文件) |
| 配置键 | **151 个注册** (54 Implemented / 80 Recognized / 17 Unsupported) — SC-004 ✅ |
| AT 场景 | **7/73** (10%) — runner 完整，场景逐步移植中 |
| Benchmarks | ✅ 编解码吞吐 (codec.rs) — SC-008 ✅ |
| API 文档 | ✅ `#![deny(missing_docs)]` facade — SC-005 ✅ |
| No-panic 审计 | ✅ 文档化 — SC-005 ✅ |
| Parity matrix | ✅ 文档化 — CHK033/CHK036 ✅ |
| Acceptance record | ✅ V1–V9 映射 — T098 ✅ |
| 示例 | 4 个 + smoke 测试 |

### tasks.md 剩余 4 个任务

| TaskID | 描述 | 状态 |
|--------|------|------|
| T059 | SQL log via sqlx (incoming/outgoing/event tables) | ❌ 未完成 |
| T070 | scheduled-reset 语义 (disconnect→reset seq→clear store→reconnect) | ❌ 未完成 |
| T085 | 73 个 server AT 场景 (67 个剩余) | ❌ 未完成 |
| T086 | 特殊类别 AT 套件 (7 个) | ❌ 未完成 |

---

## P0 — 发布阻塞项

### TODO-01: AT 场景覆盖率 (7/73 = 10%)

**当前**: 7 个 server 场景已移植 (1a×2, 2b, 2c, 4b, 13b)，在 FIX 4.2/4.4 两个版本运行。
**目标**: 73 个 server 场景 + 7 个特殊类别套件，覆盖全部目标版本 (FR-M3)。
**tasks.md**: T085 (server scenarios) + T086 (special suites)

**已覆盖**:
- [x] `1a_ValidLogonWithCorrectMsgSeqNum`
- [x] `1a_ValidLogonMsgSeqNumTooHigh`
- [x] `2b_MsgSeqNumTooHigh`
- [x] `2c_MsgSeqNumTooLow`
- [x] `4b_ReceivedTestRequest`
- [x] `13b_UnsolicitedLogoutMessage`

**未覆盖的 66 个 server 场景**:

- [ ] `1b_DuplicateIdentity`
- [ ] `1c_InvalidSenderCompID`
- [ ] `1c_InvalidTargetCompID`
- [ ] `1d_InvalidLogonBadSendingTime`
- [ ] `1d_InvalidLogonLengthInvalid`
- [ ] `1d_InvalidLogonNoDefaultApplVerID`
- [ ] `1d_InvalidLogonWrongBeginString`
- [ ] `1e_NotLogonMessage`
- [ ] `2a_MsgSeqNumCorrect`
- [ ] `2d_GarbledMessage`
- [ ] `2e_PossDupAlreadyReceived`
- [ ] `2e_PossDupNotReceived`
- [ ] `2f_PossDupOrigSendingTimeTooHigh`
- [ ] `2g_PossDupNoOrigSendingTime`
- [ ] `2i_BeginStringValueUnexpected`
- [ ] `2k_CompIDDoesNotMatchProfile`
- [ ] `2m_BodyLengthValueNotCorrect`
- [ ] `2o_SendingTimeValueOutOfRange`
- [ ] `2q_MsgTypeNotValid`
- [ ] `2r_UnregisteredMsgType`
- [ ] `2t_FirstThreeFieldsOutOfOrder`
- [ ] `3b_InvalidChecksum`
- [ ] `3c_GarbledMessage`
- [ ] `4a_NoDataSentDuringHeartBtInt`
- [ ] `6_SendTestRequest`
- [ ] `7_ReceiveRejectMessage`
- [ ] `8_AdminAndApplicationMessages`
- [ ] `8_AdminAndApplicationMessages-FIX50SP2`
- [ ] `8_OnlyAdminMessages`
- [ ] `8_OnlyApplicationMessages`
- [ ] `10_MsgSeqNumEqual`
- [ ] `10_MsgSeqNumGreater`
- [ ] `10_MsgSeqNumLess`
- [ ] `11a_NewSeqNoGreater`
- [ ] `11b_NewSeqNoEqual`
- [ ] `11c_NewSeqNoLess`
- [ ] `14a_BadField`
- [ ] `14b_RequiredFieldMissing`
- [ ] `14c_TagNotDefinedForMsgType`
- [ ] `14d_TagSpecifiedWithoutValue`
- [ ] `14e_IncorrectEnumValue`
- [ ] `14f_IncorrectDataFormat`
- [ ] `14g_HeaderBodyTrailerFieldsOutOfOrder`
- [ ] `14h_RepeatedTag`
- [ ] `14i_RepeatingGroupCountNotEqual`
- [ ] `14j_OutOfOrderRepeatingGroupMembers`
- [ ] `15_HeaderAndBodyFieldsOrderedDifferently`
- [ ] `19a_PossResendMessageThatHasAlreadyBeenSent`
- [ ] `19b_PossResendMessageThatHasNotBeenSent`
- [ ] `20_SimultaneousResendRequest`
- [ ] `21_RepeatingGroupSpecifierWithValueOfZero`
- [ ] `AlreadyLoggedOn`
- [ ] `bugfix_QFJ634_ResendRequestAndSequenceReset`
- [ ] `LogonUnknownDefaultApplVerID`
- [ ] `MinQty40` / `MinQty41` / `MinQty42` / `MinQty43` / `MinQty44` / `MinQty50`
- [ ] `QFJ648_NegativeHeartBtInt`
- [ ] `QFJ650_MissingMsgSeqNum`
- [ ] `QFJ934_MissingDelimiterNestedRepeatingGroup`
- [ ] `RejectResentMessage`
- [ ] `ReverseRoute`
- [ ] `ReverseRouteWithEmptyRoutingTags`
- [ ] `SessionReset`

**未覆盖的 7 个特殊类别套件** (T086):

- [ ] nextExpectedMsgSeqNum (fix44/50/fixLatest): 4 场景
- [ ] lastMsgSeqNumProcessed (fix42–50/fixLatest): 1 场景
- [ ] resendRequestChunkSize (fix40–50/fixLatest): 2 场景
- [ ] validateChecksum (fix50/fixLatest): 1 场景
- [ ] rejectGarbledMessages (fix50/fixLatest): 1 场景
- [ ] timestamps (fix44): 2 场景
- [ ] resynch: 1 场景

**未覆盖的目标版本**: 当前仅 FIX 4.2/4.4。需扩展到 fix40/41/43/50/fixLatest。

**文件**: `crates/truefix-at/src/scenarios.rs`, `crates/truefix-at/src/runner.rs`

---

### TODO-02: SessionSettings → SessionConfig 映射

**当前**: `truefix-config` 解析 `.cfg` 文件为 `BTreeMap<String, String>`，151 个配置键已注册分类，但**没有代码将解析结果转换为 `SessionConfig` / `StoreConfig` / `LogConfig` / `Services`**。
**目标**: `.cfg` 文件 → `SessionSettings` → 完整引擎配置 → 启动。

**需要**:

- [ ] `SessionConfig::from_settings(&BTreeMap<String,String>) -> Result<SessionConfig, ConfigError>`
- [ ] `StoreConfig::from_settings(...) -> Result<StoreConfig, ConfigError>`
- [ ] `LogConfig::from_settings(...) -> Result<LogConfig, ConfigError>`
- [ ] `Services::from_settings(...) -> Result<Services, ConfigError>`
- [ ] `SocketOptions::from_settings(...)` (扩展当前仅 TcpNoDelay 的实现)
- [ ] `rustls::ServerConfig` / `ClientConfig` 从 SSL 键构建
- [ ] `Schedule::from_settings(...)` (StartTime/EndTime/Weekdays/TimeZone/NonStopSession)
- [ ] 从 `ConnectionType` 路由到 initiator vs acceptor 构建逻辑
- [ ] 集成测试: 从 `.cfg` 文件启动完整引擎

**文件**: `crates/truefix-config/src/lib.rs` (新增 `builder.rs` 或 `mapping.rs`)

---

### TODO-03: Session 内 MessageStore 集成

**当前**: Transport 层已持久化 seq numbers，但 `Session` 内的 resend 仍使用内存 `BTreeMap<u64, Message>`。跨重启的 message resend 无法工作。
**目标**: Session 的 `store` 字段从 `BTreeMap` 替换为 `dyn MessageStore`。

**需要**:

- [ ] `Session::with_store(config, store: Arc<dyn MessageStore>)` — 注入持久化存储
- [ ] `send_stored()` 调用 `store.save(seq, &msg.encode())`
- [ ] `build_resend()` 调用 `store.get(begin, end)` 而非内存 BTreeMap
- [ ] `reset()` 调用 `store.reset()`
- [ ] `PersistMessages=N` 时跳过 save
- [ ] `ForceResendWhenCorruptedStore`: store 损坏时的恢复路径

**文件**: `crates/truefix-session/src/state.rs`

---

## P1 — 功能完整性差距

### TODO-04: Codegen 类型化消息

**当前**: `build.rs` 仅生成 MsgType 字符串常量 (如 `LOGON="A"`)。
**QF/J**: 生成完整的 Java 类 (如 `NewOrderSingle` 带每个字段的 typed accessor)。
**QF/Go**: 同上 (Go struct)。

**需要**:

- [ ] 从 normalized `.fixdict` 生成 per-version typed message struct
- [ ] 每个字段生成 typed accessor
- [ ] 生成字段枚举 (如 `Side::Buy("1")`, `Side::Sell("2")`)
- [ ] 生成 Group 的 typed struct
- [ ] 生成 Component 的 typed struct
- [ ] MessageFactory 基于 codegen 类型创建 Message
- [ ] MessageCracker 基于 codegen 类型分发到 typed handler

**文件**: `crates/truefix-dict/build.rs`, `crates/truefix-dict/src/codegen/` (新建)

---

### TODO-05: RejectGarbledMessage

**当前**: 传输层遇到 garbled 帧时直接丢弃。
**QF/J / QF/Go**: 当 `RejectGarbledMessage=Y` 时，生成 session-level Reject (35=3)。

- [ ] `Session` 增加 `reject_garbled: bool` 配置 (已注册为 Recognized)
- [ ] Transport 层 garbled 帧时通知 Session
- [ ] Session 生成 Reject (35=3, RefSeqNum=0, SessionRejectReason=0)
- [ ] AT 场景 `2d_GarbledMessage` / `3c_GarbledMessage` / `QFJ950-RejectGarbledMessages`

**文件**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-session/src/state.rs`

---

### TODO-06: 字典驱动的 Group 解析

**当前**: `codec::decode` 将 wire bytes 解析为扁平字段。
**QF/J / QF/Go**: 解码时使用 DataDictionary 识别 repeating group。

- [ ] `decode_with_dict(bytes, &DataDictionary) -> Result<Message, DecodeError>`
- [ ] 识别 group count tag → 解析 N 个 group entries
- [ ] 嵌套 group 递归解析
- [ ] 验证 group delimiter (FirstFieldInGroupIsDelimiter)
- [ ] Component 展开为 fields
- [ ] AT 场景 `14i` / `14j` / `21` / `QFJ934`

**文件**: `crates/truefix-core/src/codec/decode.rs`, `crates/truefix-dict/src/`

---

### TODO-07: EnableLastMsgSeqNumProcessed (369)

**当前**: 未实现 (已注册为 Recognized)。
**QF/J / QF/Go**: 当 `EnableLastMsgSeqNumProcessed=Y` 时，在出站消息 header 中设置 tag 369。

- [ ] `SessionConfig` 增加 `enable_last_msg_seq_num_processed: bool`
- [ ] Session 追踪 `last_processed_in_seq`
- [ ] 出站消息 header 自动设置 tag 369
- [ ] AT 场景 `LastProcessedMsgSeqNum`

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-08: 剩余 Session 配置开关 (已注册 Recognized 但未实现行为)

- [ ] `SendRedundantResendRequests`
- [ ] `ClosedResendInterval` / `UseClosedResendInterval`
- [ ] `ResetOnError`
- [ ] `DisconnectOnError`
- [ ] `ForceResendWhenCorruptedStore` (检测有, 恢复路径未完整)
- [ ] `PersistMessages` (N 时不持久化消息)
- [ ] `RefreshOnLogon` (字段存在, 未执行)
- [ ] `HeartBeatTimeoutMultiplier` (当前固定 2×hb+2)
- [ ] `DisableHeartBeatCheck`
- [ ] `TestRequestDelayMultiplier` (当前固定 hb+1)
- [ ] `LogonTag`
- [ ] `MaxScheduledWriteRequests`
- [ ] `ContinueInitializationOnError`
- [ ] `LogMessageWhenSessionNotFound`
- [ ] `TimeStampPrecision` (SECONDS/MILLIS/MICROS/NANOS)

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-09: 字典验证开关补全

**当前**: `ValidationOptions` 有 5 个开关。QF/J 有更多。

- [ ] `CheckCompID` — 验证 SenderCompID/TargetCompID 匹配 (已注册 Recognized)
- [ ] `ValidateFieldsOutOfOrder` — 验证字段顺序 (已注册 Recognized)
- [ ] `ValidateUnorderedGroupFields` — 验证 group 内字段顺序 (已注册 Recognized)
- [ ] `RejectMessageOnUnhandledException` (已注册 Recognized)
- [ ] `FirstFieldInGroupIsDelimiter` (已注册 Recognized)
- [ ] 自定义/扩展字典加载 (`DataDictionary::load_from_file(path)`)
- [ ] 组件 (Components) 支持 — 模型和验证

**文件**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-10: 反向路由 (Reverse Routing)

**当前**: 未实现。
**QF/J**: header 中的 `OnBehalfOfCompID`/`DeliverToCompID` 等路由字段可被反转。

- [ ] Message header 反转路由字段
- [ ] AT 场景 `ReverseRoute` / `ReverseRouteWithEmptyRoutingTags`

**文件**: `crates/truefix-core/src/message.rs`, `crates/truefix-session/src/state.rs`

---

## P2 — 传输层补全

### TODO-11: 完整 Socket 选项

**当前**: 仅 `TcpNoDelay`。以下已注册为 Recognized:
`SocketKeepAlive`, `SocketReuseAddress`, `SocketLinger`, `SocketOobInline`, `SocketReceiveBufferSize`, `SocketSendBufferSize`, `SocketTrafficClass`, `SocketSynchronousWrites`, `SocketSynchronousWriteTimeout`。

- [ ] 扩展 `SocketOptions` struct
- [ ] 在 `apply()` 中逐一应用
- [ ] 从配置键映射

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-12: mTLS (客户端证书认证)

**当前**: TLS 配置使用 `with_no_client_auth()`。
**NeedClientAuth** 已注册为 Implemented 但实际未使用。

- [ ] `ServerConfig` 使用 `with_client_cert_verifier(roots)`
- [ ] `NeedClientAuth=Y` → 验证客户端证书
- [ ] 集成测试: mTLS 双向证书认证

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-13: 多端点 Failover

**当前**: Initiator 仅连接单个端点。
**QF/J / QF/Go**: `SocketConnectHost<N>` / `SocketConnectPort<N>` 编号备选端点。

- [ ] `SessionConfig` 增加 `connect_endpoints: Vec<SocketAddr>`
- [ ] `connect_initiator_reconnecting` 在端点间轮换
- [ ] 配置键解析 `SocketConnectHost1`, `SocketConnectPort1`, etc.

**文件**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-14: Schedule StartDay/EndDay

**当前**: `Schedule` 支持 daily + weekdays + UTC offset，不支持 `StartDay`/`EndDay`。
已注册为 Recognized。

- [ ] `Schedule` 增加 `start_day: Option<Weekday>` / `end_day: Option<Weekday>`
- [ ] `is_in_session()` 处理跨日周窗口
- [ ] 配置键映射

**文件**: `crates/truefix-session/src/schedule.rs`

---

### TODO-15: SQL 日志后端 (T059)

**当前**: `truefix-log` 无 SQL 后端。
**tasks.md**: T059 未完成。

- [ ] `SqlLog` 实现 `Log` trait (sqlx)
- [ ] 表: `incoming(id, session_id, timestamp, msg)`, `outgoing(...)`, `event(...)`
- [ ] `LogConfig::Sql { url }` 变体
- [ ] JDBC 配置键映射 (已注册为 Recognized)

**文件**: `crates/truefix-log/src/sql.rs` (新建)

---

### TODO-16: Scheduled-Reset 语义 (T070)

**当前**: `Schedule::is_in_session()` 已实现，但没有调度驱动的重置循环。
**tasks.md**: T070 未完成。

- [ ] 调度边界检测: 当会话超出调度窗口时自动执行 disconnect→reset seq→clear store
- [ ] 当回到调度窗口时自动重连 (initiator)
- [ ] `NonStopSession=Y` 时跳过调度重置

**文件**: `crates/truefix-session/src/schedule_reset.rs` (新建)

---

### TODO-17: CachedFileStore 缓存优化

**当前**: `CachedFileStore` 直接委托 `FileStore`，无独立缓存逻辑。
`FileStoreMaxCachedMsgs` / `FileStoreSync` 已注册为 Recognized。

- [ ] `CachedFileStore` 内部维护内存缓存
- [ ] `save()` 先写缓存，达到 `max_cached_msgs` 时 flush
- [ ] `get()` 先查缓存，未命中再查文件

**文件**: `crates/truefix-store/src/file.rs`

---

## P3 — 日志/监控补全

### TODO-18: 日志配置开关

已注册为 Recognized 但未实现行为:
`FileLogHeartbeats`, `FileIncludeMilliseconds`, `FileIncludeTimeStampForMessages`, `ScreenLogShowEvents/HeartBeats/Incoming/Outgoing`, `ScreenIncludeMilliseconds`, `SLF4JLogPrependSessionID`, `SLF4JLogHeartbeats`。

- [ ] 各 Log 实现增加对应配置字段
- [ ] Heartbeat 过滤 (N 时不记录 35=0/1)
- [ ] 毫秒时间戳包含开关

**文件**: `crates/truefix-log/src/*.rs`

---

### TODO-19: Metrics 仪表盘

**当前**: `Monitor` 提供 status/force_logout/reset，但无 metrics 导出。
`metrics` crate 已在 workspace 依赖中但未使用。

- [ ] 导出: session_state (gauge), next_sender_seq (gauge), next_target_seq (gauge), messages_sent (counter), messages_received (counter), reconnect_count (counter)
- [ ] Prometheus exporter (可选)
- [ ] FR-L1 完整达标

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-20: Session round-trip latency benchmark

**当前**: 仅 codec throughput benchmark。
**SC-008**: "reproducible benchmarks for codec throughput and session round-trip latency"。

- [ ] `benches/session.rs` — 会话往返延迟 (criterion 或自定义 harness)

**文件**: `benches/session.rs` (新建)

---

## QuickFIX/Go 独有功能 (可选, 超出 QF/J 对等范围)

- [ ] `ResetSeqTime` — 连接中定时序列号重置
- [ ] `InChanCapacity` — 入站消息有界缓冲
- [ ] `IterateMessages` — 流式遍历存储
- [ ] `SaveMessageAndIncrNextSenderMsgSeqNum` — 原子操作
- [ ] `ConnectionValidator` + `NewListenerCallback` — acceptor 自定义 hook
- [ ] TCP PROXY protocol (HAProxy/ELB) — `UseTCPProxy`
- [ ] 内联 PEM bytes 配置 — `SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes`
- [ ] MongoDB 存储/日志
- [ ] `DynamicQualifier` — 动态会话限定符
- [ ] `SocketMinimumTLSVersion` — 最低 TLS 版本
