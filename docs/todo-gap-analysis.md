# TrueFix 与 QuickFIX/J + QuickFIX/Go 功能差异 TODO

> 基于 2026-06-30 全量代码审查，对照 `specs/001-fix-engine-parity/` 规格文档。
> 规格基线: Appendix A (~150 配置键) + Appendix B (73 server AT 场景 + 7 特殊类别套件)。

## 当前状态总览

| 维度 | 数据 |
|------|------|
| Crates | 9 个 (core/dict/session/store/log/transport/config/at/truefix) |
| 配置键 | ~151 个注册 (Impl/Rec/Unsup 三态) — SC-004 ✅ |
| AT 场景 | **45 种 / 66 实例** (2 版本: FIX.4.2 + FIX.4.4) |
| 规格覆盖 | **15/73** server 场景 + 4/7 特殊套件部分覆盖 |
| Benchmarks | ✅ 编解码吞吐 (codec.rs) — SC-008 ✅ |
| API 文档 | ✅ `#![deny(missing_docs)]` facade — SC-005 ✅ |
| No-panic 审计 | ✅ 文档化 — SC-005 ✅ |
| Parity matrix | ✅ 文档化 — CHK033/CHK036 ✅ |
| 示例 | 4 个 (executor/banzai/ordermatch/multi_acceptor) + smoke 测试 |

### tasks.md 任务状态

| TaskID | 描述 | tasks.md | 实际代码 |
|--------|------|----------|---------|
| T059 | SQL log via sqlx | ❌ 未勾选 | ✅ **已实现** (`crates/truefix-log/src/sql.rs`) |
| T070 | scheduled-reset 语义 | ❌ 未勾选 | ⚠️ **部分** (`run_scheduled_initiator` 存在, 缺 session 层 `schedule_reset.rs`) |
| T085 | 73 个 server AT 场景 | ❌ 未勾选 | ⚠️ 15/73 已覆盖, 30 个额外场景 |
| T086 | 特殊类别 AT 套件 | ❌ 未勾选 | ⚠️ 4/7 部分覆盖 (各 1 场景, 仅 FIX.4.4) |

---

## P0 — 发布阻塞项

### TODO-01: AT 场景覆盖率 (15/73 = 21%)

**当前**: 45 种场景 / 66 实例, 覆盖 FIX.4.2 + FIX.4.4 两个版本。
**目标**: 73 个 server 场景 + 7 个特殊类别套件, 覆盖全部目标版本 (FR-M3)。

**已覆盖的 15 个规格 server 场景** (精确匹配 Appendix B):

- [x] `1a_ValidLogonWithCorrectMsgSeqNum` (×2 版本)
- [x] `1a_ValidLogonMsgSeqNumTooHigh` (×2 版本)
- [x] `2b_MsgSeqNumTooHigh` (×2 版本)
- [x] `2c_MsgSeqNumTooLow` (×2 版本)
- [x] `4b_ReceivedTestRequest` (×2 版本)
- [x] `13b_UnsolicitedLogoutMessage` (×2 版本)
- [x] `14a_BadField` → `14a_InvalidTagNumber` (×2 版本)
- [x] `14b_RequiredFieldMissing` (×2 版本)
- [x] `14c_TagNotDefinedForMsgType` (×2 版本)
- [x] `14d_TagSpecifiedWithoutValue` (×2 版本)
- [x] `14e_IncorrectEnumValue` (×2 版本)
- [x] `14f_IncorrectDataFormat` (×2 版本)
- [x] `2r_UnregisteredMsgType` (FIX.4.4)
- [x] `QFJ650_MissingMsgSeqNum` → `2_MissingMsgSeqNum` (×2 版本)
- [x] `7_ReceiveRejectMessage` → `3a_ReceivedRejectConsumed` (×2 版本)

**额外覆盖的 30 个 TrueFix 专属场景** (测试规格相关行为, 非精确匹配 Appendix B):

- [x] `1c_LogonAdoptsHeartBtInt` — Acceptor 采纳对端 HeartBtInt (×2)
- [x] `1d_LogonResponseResetFlag` — Logon 响应中 ResetSeqNumFlag (×2)
- [x] `2f_ResendRequestGapFill` — ResendRequest 回复含 GapFill (×2)
- [x] `2f3_ResendRequestBoundedEnd` — ResendRequest 有界 end (×2)
- [x] `2_ResendRequestNotDuplicated` — ResendRequest 不重复 (×2)
- [x] `2_ResendRequestBeginZeroIgnored` — begin=0 被忽略 (×2)
- [x] `2x_OutOfOrderQueuedThenDrained` — 乱序排队后排水 (×2)
- [x] `2g_SequenceResetReset` — SequenceReset-Reset 模式 (×2)
- [x] `2d_SequenceResetGapFillAdvances` — GapFill 前进 (×2)
- [x] `2e_SequenceResetGapFillBackwardIgnored` — GapFill 后退被忽略 (×2)
- [x] `2h_ResendRequestNothingToResend` — 无消息可重发 (×2)
- [x] `0a_ReceivedHeartbeatConsumed` — Heartbeat 被消费 (×2)
- [x] `2m_PossDupMsgSeqNumTooLow` — PossDup 低序列号 (×2)
- [x] `14_valid_NewOrderSingleAccepted` — 有效订单被接受 (FIX.4.2 + FIX.4.4)
- [x] `app_NewOrderSingleExecuted` — 应用层订单执行 (FIX.4.4)
- [x] `app_OrdersOutboundSequenced` — 出站消息有序 (FIX.4.4)
- [x] `app_MessageResentAsPossDup` — 消息作为 PossDup 重发 (FIX.4.4)
- [x] `app_MixedResendGapFillThenPossDup` — 混合重发 GapFill+PossDup (FIX.4.4)

**特殊类别套件覆盖** (各 1 场景, 仅 FIX.4.4):

- [x] `special_NextExpectedMsgSeqNum` — 部分 (1/4 场景)
- [x] `special_LastMsgSeqNumProcessed` — 部分 (1/1 场景, 缺其他版本)
- [x] `special_ResendRequestChunkSize` — 部分 (1/2 场景)
- [x] `special_GarbledMessageDropped` — 部分 (测试丢弃, 非 Reject)
- [ ] `validateChecksum` — 未覆盖
- [ ] `timestamps` — 未覆盖
- [ ] `resynch` — 未覆盖

**未覆盖的 58 个规格 server 场景**:

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
- [ ] `2t_FirstThreeFieldsOutOfOrder`
- [ ] `3b_InvalidChecksum`
- [ ] `3c_GarbledMessage`
- [ ] `4a_NoDataSentDuringHeartBtInt`
- [ ] `6_SendTestRequest`
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
- [ ] `QFJ934_MissingDelimiterNestedRepeatingGroup`
- [ ] `RejectResentMessage`
- [ ] `ReverseRoute`
- [ ] `ReverseRouteWithEmptyRoutingTags`
- [ ] `SessionReset`

**未覆盖的目标版本**: 当前仅 FIX 4.2/4.4。需扩展到 fix40/41/43/50/fixLatest (FR-M3)。

**文件**: `crates/truefix-at/src/scenarios.rs`, `crates/truefix-at/src/runner.rs`

---

### TODO-02: SessionSettings → SessionConfig 映射

**当前**: `truefix-config` 解析 `.cfg` 文件为 `BTreeMap<String, String>`, ~151 个配置键已注册分类, 但**没有代码将解析结果转换为 `SessionConfig` / `StoreConfig` / `LogConfig` / `Services`**。
**规格**: FR-I1/I3, contracts/config-keys.md — 需要 `.cfg` → 完整引擎配置 → 启动。

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

**当前**: Transport 层已持久化 seq numbers (best-effort after each dispatch), `seed_sequences()` 从 store 恢复序列号。但 `Session` 内的 resend 仍使用内存 `BTreeMap<u64, Message>`, 跨重启的 message resend 无法工作。
**规格**: FR-G2, SC-006 — "persisted state is sufficient to satisfy a ResendRequest for messages sent before a process restart"。

**需要**:

- [ ] `Session::with_store(config, store: Arc<dyn MessageStore>)` — 注入持久化存储
- [ ] `send_stored()` 调用 `store.save(seq, &msg.encode())`
- [ ] `build_resend()` 调用 `store.get(begin, end)` 而非内存 BTreeMap
- [ ] `reset()` 调用 `store.reset()`
- [ ] `PersistMessages=N` 时跳过 save

**文件**: `crates/truefix-session/src/state.rs`

---

### TODO-04: Codegen 类型化消息

**当前**: `build.rs` 仅生成 MsgType 字符串常量 (如 `LOGON="A"`) + FNV-1a hash (dual-track invariant)。
**规格**: FR-C2, contracts/dictionary.md — "generates strongly-typed per-version message/field structs"。
**QF/J**: 生成完整 Java 类 (`NewOrderSingle` 带 typed accessor); QF/Go: Go struct + `generate-fix` CLI。

**需要**:

- [ ] 从 normalized `.fixdict` 生成 per-version typed message struct
- [ ] 每个字段生成 typed accessor
- [ ] 生成字段枚举 (如 `Side::Buy("1")`, `Side::Sell("2")`)
- [ ] 生成 Group 的 typed struct
- [ ] 生成 Component 的 typed struct
- [ ] MessageFactory 基于 codegen 类型创建 Message
- [ ] MessageCracker 基于 codegen 类型分发到 typed handler (当前 `cracker.rs` 是空壳 trait)

**文件**: `crates/truefix-dict/build.rs`, `crates/truefix-dict/src/codegen/` (新建)

---

### TODO-05: 字典驱动的 Group 解析

**当前**: `codec::decode` 将 wire bytes 解析为扁平字段; `Group` 仅支持构建/编码, 不支持从 flat wire 解析。`validate.rs` 仅遍历扁平 `fields()`。
**规格**: FR-B1/B9, FR-C3, data-model.md — "dictionary-driven group parsing", `ValidateUnorderedGroupFields`, `FirstFieldInGroupIsDelimiter`。
**QF/J / QF/Go**: 解码时使用 DataDictionary 识别 repeating group。

**需要**:

- [ ] `decode_with_dict(bytes, &DataDictionary) -> Result<Message, DecodeError>`
- [ ] 识别 group count tag → 解析 N 个 group entries
- [ ] 嵌套 group 递归解析
- [ ] 验证 group delimiter (`FirstFieldInGroupIsDelimiter`)
- [ ] Component 展开为 fields
- [ ] AT 场景 `14i` / `14j` / `21` / `QFJ934`

**文件**: `crates/truefix-core/src/codec/decode.rs`, `crates/truefix-dict/src/`

---

### TODO-06: Application 回调返回类型化错误

**当前**: `Application` trait 的 `from_admin` 返回 `Result<(), String>`, `to_app` 返回 `Result<(), String>`, `from_app` 返回 `Result<(), String>`。
**规格**: contracts/application-api.md — `fromAdmin -> Result<(), Reject>`, `toApp -> Result<(), DoNotSend>`, `fromApp -> Result<(), BusinessReject>`。

**需要**:

- [ ] 定义 `Reject` 错误类型 (session-level reject reason + ref tag)
- [ ] 定义 `DoNotSend` 错误类型 (中止发送/重发)
- [ ] 定义 `BusinessReject` 错误类型 (business-level reject reason + ref tag)
- [ ] `from_admin` 返回 `Result<(), Reject>` (等价 RejectLogon)
- [ ] `to_app` 返回 `Result<(), DoNotSend>`
- [ ] `from_app` 返回 `Result<(), BusinessReject>`

**文件**: `crates/truefix/src/application.rs` (或 `crates/truefix-core/src/error.rs`)

---

## P1 — 功能完整性差距

### TODO-07: RejectGarbledMessage

**当前**: 传输层遇到 garbled 帧时直接丢弃。AT 有 `special_GarbledMessageDropped` 场景验证丢弃行为。
**规格**: FR-C4, FR-B8 — "RejectGarbledMessage handling" + "two rejection layers"。
**QF/J / QF/Go**: 当 `RejectGarbledMessage=Y` 时, 生成 session-level Reject (35=3)。

- [ ] `Session` 增加 `reject_garbled: bool` 配置 (已注册为 Recognized)
- [ ] Transport 层 garbled 帧时通知 Session
- [ ] Session 生成 Reject (35=3, RefSeqNum=0, SessionRejectReason=0)
- [ ] AT 场景 `2d_GarbledMessage` / `3c_GarbledMessage` / `QFJ950-RejectGarbledMessages`

**文件**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-session/src/state.rs`

---

### TODO-08: 剩余 Session 配置开关

已注册为 Recognized 但未实现行为:

- [ ] `SendRedundantResendRequests`
- [ ] `ClosedResendInterval` / `UseClosedResendInterval`
- [ ] `ResetOnError`
- [ ] `DisconnectOnError`
- [ ] `ForceResendWhenCorruptedStore` — 检测有 (`was_corrupted()`), 强制重发行为未完整
- [ ] `PersistMessages` (N 时不持久化消息)
- [ ] `HeartBeatTimeoutMultiplier` (当前固定 2×hb+2)
- [ ] `DisableHeartBeatCheck`
- [ ] `TestRequestDelayMultiplier` (当前固定 hb+1)
- [ ] `LogonTag`
- [ ] `MaxScheduledWriteRequests`
- [ ] `ContinueInitializationOnError`
- [ ] `LogMessageWhenSessionNotFound`
- [ ] `TimeStampPrecision` (SECONDS/MILLIS/MICROS/NANOS) — 当前仅毫秒
- [ ] `ResetSeqTime` — 连接中定时序列号重置 (QF/Go 独有)

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-09: 字典验证开关补全

**当前**: `ValidationOptions` 有 5 个开关 (`validate_fields_have_values`, `validate_user_defined_fields`, `allow_unknown_msg_fields`, `check_required_fields`, `check_field_types`)。
**规格**: FR-C3 — 需要完整的验证开关集。

- [ ] `CheckCompID` — 验证 SenderCompID/TargetCompID 匹配 (已注册 Recognized)
- [ ] `ValidateFieldsOutOfOrder` — 验证字段顺序 (已注册 Recognized)
- [ ] `ValidateUnorderedGroupFields` — 验证 group 内字段顺序 (已注册 Recognized, 依赖 TODO-05)
- [ ] `ValidateIncomingMessage` (已注册 Recognized)
- [ ] `ValidateChecksum` (已注册 Recognized)
- [ ] `RejectMessageOnUnhandledException` (已注册 Recognized)
- [ ] `FirstFieldInGroupIsDelimiter` (已注册 Recognized, 依赖 TODO-05)
- [ ] `RejectInvalidMessage` (已注册 Recognized)
- [ ] 自定义/扩展字典加载 (`DataDictionary::load_from_file(path)`) — FR-C5
- [ ] 组件 (Components) 支持 — 模型和验证

**文件**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-10: 反向路由 (Reverse Routing)

**当前**: 未实现。
**规格**: Appendix B — `ReverseRoute` / `ReverseRouteWithEmptyRoutingTags` 场景。
**QF/J**: header 中的 `OnBehalfOfCompID`/`DeliverToCompID` 等路由字段可被反转。

- [ ] Message header 反转路由字段
- [ ] AT 场景 `ReverseRoute` / `ReverseRouteWithEmptyRoutingTags`

**文件**: `crates/truefix-core/src/message.rs`, `crates/truefix-session/src/state.rs`

---

### TODO-11: Scheduled-Reset 语义 (T070)

**当前**: `run_scheduled_initiator` 存在 — 连接仅在 `Schedule::is_in_session()` 为 true 时, 进入窗口时 reset store, 离开时 logout。但缺少 session 层的 `schedule_reset.rs` 模块。
**规格**: FR-E3 — "scheduled reset = disconnect → reset sequence numbers → clear stored messages → reconnect"。
**tasks.md**: T070 未勾选。

- [ ] 调度边界检测: 当会话超出调度窗口时自动执行 disconnect→reset seq→clear store
- [ ] 当回到调度窗口时自动重连 (initiator)
- [ ] `NonStopSession=Y` 时跳过调度重置
- [ ] 将逻辑从 transport 层下沉到 session 层

**文件**: `crates/truefix-session/src/schedule_reset.rs` (新建)

---

## P2 — 传输层补全

### TODO-12: 完整 Socket 选项

**当前**: 仅 `TcpNoDelay`。以下已注册为 Recognized:
`SocketKeepAlive`, `SocketReuseAddress`, `SocketLinger`, `SocketOobInline`, `SocketReceiveBufferSize`, `SocketSendBufferSize`, `SocketTrafficClass`, `SocketSynchronousWrites`, `SocketSynchronousWriteTimeout`。

- [ ] 扩展 `SocketOptions` struct
- [ ] 在 `apply()` 中逐一应用
- [ ] 从配置键映射

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-13: mTLS (客户端证书认证)

**当前**: TLS 配置使用 `with_no_client_auth()`。
**规格**: FR-F6 — "TLS including client-auth/SNI keys"。
`NeedClientAuth` 已注册为 Implemented 但实际未使用。

- [ ] `ServerConfig` 使用 `with_client_cert_verifier(roots)`
- [ ] `NeedClientAuth=Y` → 验证客户端证书
- [ ] 集成测试: mTLS 双向证书认证

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-14: 多端点 Failover

**当前**: Initiator 仅连接单个端点, `connect_initiator_reconnecting` 在同一地址上重连。
**QF/J / QF/Go**: `SocketConnectHost<N>` / `SocketConnectPort<N>` 编号备选端点, 轮询切换。

- [ ] `SessionConfig` 增加 `connect_endpoints: Vec<SocketAddr>`
- [ ] `connect_initiator_reconnecting` 在端点间轮换
- [ ] 配置键解析 `SocketConnectHost1`, `SocketConnectPort1`, etc.

**文件**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-15: Schedule StartDay/EndDay

**当前**: `Schedule` 支持 daily (StartTime/EndTime) + weekdays + UTC offset, 不支持 `StartDay`/`EndDay`。
**规格**: FR-E1 — "StartDay/EndDay weekly windows"。
已注册为 Recognized。

- [ ] `Schedule` 增加 `start_day: Option<Weekday>` / `end_day: Option<Weekday>`
- [ ] `is_in_session()` 处理跨日周窗口
- [ ] 配置键映射

**文件**: `crates/truefix-session/src/schedule.rs`

---

### TODO-16: TLS 配置从 SSL 键构建

**当前**: Transport 接受预构建的 `Arc<rustls::ClientConfig/ServerConfig>`。配置键 (`EnabledProtocols`, `CipherSuites`, `KeyStoreType`, `SocketKeyStore`, etc.) 标记为 Implemented, 但 transport 不解析这些键。
**规格**: FR-F6, Appendix A SSL/TLS group。

- [ ] 从 PEM 文件/字节 加载 key/cert (rustls-pemfile)
- [ ] `SocketMinimumTLSVersion` 配置 (SSL30/TLS10/TLS11/TLS12)
- [ ] `SocketInsecureSkipVerify` 配置
- [ `SocketServerName` (SNI) 配置
- [ ] `NeedClientAuth` 配置 → mTLS (与 TODO-13 联动)
- [ ] 内联 PEM bytes 配置 (`SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes`) — QF/Go 独有

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-17: SQL 多数据库支持

**当前**: SQL Store + SQL Log 仅支持 SQLite (sqlx)。
**规格**: FR-G1/H1 — "JDBC-equivalent" (research R7: "supports Postgres/MySQL/SQLite")。
**QF/Go**: PostgreSQL/Oracle/MySQL/MSSQL/SQLite; QF/J: JDBC 任意驱动。

- [ ] 扩展 sqlx 支持 PostgreSQL / MySQL
- [ ] 自定义 SQL 表名 (`JdbcStoreMessagesTableName` / `JdbcStoreSessionsTableName`) — 已注册 Recognized
- [ ] SQL 连接池配置 (`JdbcMaxConnectionLifeTime`, `JdbcMinIdleConnection`, etc.) — 已注册 Recognized

**文件**: `crates/truefix-store/src/sql.rs`, `crates/truefix-log/src/sql.rs`

---

### TODO-18: CachedFileStore 缓存优化

**当前**: `CachedFileStore` 直接委托 `FileStore`, 无独立缓存逻辑。
`FileStoreMaxCachedMsgs` / `FileStoreSync` 已注册为 Recognized。
**QF/J / QF/Go**: CachedFileStore 内存缓存 + FileStoreSync fsync 开关。

- [ ] `CachedFileStore` 内部维护内存缓存
- [ ] `save()` 先写缓存, 达到 `max_cached_msgs` 时 flush
- [ ] `get()` 先查缓存, 未命中再查文件
- [ ] `FileStoreSync` fsync 开关 (当前 FileStore 使用同步 `fs::write`, 无 fsync toggle)

**文件**: `crates/truefix-store/src/file.rs`

---

## P3 — 日志/监控补全

### TODO-19: 日志配置开关

已注册为 Recognized 但未实现行为:
`FileLogHeartbeats`, `FileIncludeMilliseconds`, `FileIncludeTimeStampForMessages`, `ScreenLogShowEvents/HeartBeats/Incoming/Outgoing`, `ScreenIncludeMilliseconds`, `SLF4JLogPrependSessionID`, `SLF4JLogHeartbeats`。

- [ ] 各 Log 实现增加对应配置字段
- [ ] Heartbeat 过滤 (N 时不记录 35=0/1)
- [ ] 毫秒时间戳包含开关

**文件**: `crates/truefix-log/src/*.rs`

---

### TODO-20: Metrics 仪表盘

**当前**: `Monitor` 提供 status/force_logout/reset/send_app, 但无 metrics 导出。
**规格**: FR-L1 — "structured, queryable/observable signals"; research R9 — "metrics facade for session state, sequence numbers, connection health gauges/counters"。
`metrics` crate 已在 workspace 依赖中但未使用。

- [ ] 导出: session_state (gauge), next_sender_seq (gauge), next_target_seq (gauge), messages_sent (counter), messages_received (counter), reconnect_count (counter)
- [ ] Prometheus exporter (可选)
- [ ] FR-L1 完整达标

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-21: Session round-trip latency benchmark

**当前**: 仅 codec throughput benchmark。
**规格**: SC-008 — "reproducible benchmarks for codec throughput and session round-trip latency"。

- [ ] `benches/session.rs` — 会话往返延迟 (criterion 或自定义 harness)

**文件**: `benches/session.rs` (新建)

---

## 已完成项 (对照旧版 TODO)

| 旧 TODO | 描述 | 状态 |
|---------|------|------|
| TODO-07 (旧) | EnableLastMsgSeqNumProcessed (369) | ✅ 已实现 — `SessionConfig.enable_last_msg_seq_num_processed` + 出站 stamping |
| TODO-15 (旧) | SQL 日志后端 (T059) | ✅ 已实现 — `crates/truefix-log/src/sql.rs` (SQLite, 后台写入) |
| — | FileStore 损坏恢复 | ✅ 已实现 — 截断/损坏记录自动恢复 good prefix + `was_corrupted()` |
| — | 字典验证集成到 transport | ✅ 已实现 — AT runner 可用 DataDictionary 验证 |
| — | AT 场景扩展 | ✅ 从 7→45 种 (新增 resend/gapfill/sequence-reset/out-of-order/possdup/app-execution/special) |
| — | RefreshOnLogon | ✅ `SessionConfig.refresh_on_logon` 字段存在且被使用 |

---

## QuickFIX/Go 独有功能 (可选, 超出 QF/J 对等范围)

- [ ] `ResetSeqTime` — 连接中定时序列号重置 (已列入 TODO-08)
- [ ] `InChanCapacity` — 入站消息有界缓冲
- [ ] `ConnectionValidator` + `NewListenerCallback` — acceptor 自定义 hook
- [ ] TCP PROXY protocol (HAProxy/ELB) — `UseTCPProxy`
- [ ] 内联 PEM bytes 配置 — `SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes` (已列入 TODO-16)
- [ ] MongoDB 存储/日志
- [ ] `DynamicQualifier` — 动态会话限定符
- [ ] `HeartBtIntOverride` — 覆盖对端 HeartBtInt (已列入 TODO-08)
- [ ] `SocketMinimumTLSVersion` — 最低 TLS 版本 (已列入 TODO-16)
- [ ] `generate-fix` CLI — 独立代码生成命令行工具 (已列入 TODO-04)
