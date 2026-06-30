# TrueFix 与 QuickFIX/J + QuickFIX/Go 功能差异 TODO

> 基于 2026-06-30 全量代码审查（S0–S9 全部已提交 + 配置键注册表未提交）。
> 113 个测试全部通过。以下为与 QuickFIX/J (Java) 和 QuickFIX/Go 的功能差距，按优先级排序。

---

## P0 — 发布阻塞项

### TODO-01: AT 场景覆盖率 (6/73 = 8%)

**当前**: 6 个 server 场景已移植 (1a×2, 2b, 2c, 4b, 13b)，在 FIX 4.2/4.4 两个版本运行。
**目标**: 73 个 server 场景 + 7 个特殊类别套件，覆盖全部目标版本 (FR-M3)。

**未覆盖的 67 个 server 场景**:

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

**未覆盖的 7 个特殊类别套件**:

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

**当前**: `truefix-config` 解析 `.cfg` 文件为 `BTreeMap<String, String>`，151 个配置键已注册分类 (54 Implemented / 80 Recognized / 17 Unsupported)，但**没有代码将解析结果转换为 `SessionConfig` / `StoreConfig` / `LogConfig` / `Services`**。
**目标**: `.cfg` 文件 → `SessionSettings` → `SessionConfig` + `StoreConfig` + `LogConfig` + `Services` + `AcceptorBuilder`/`connect_initiator` 完整链路。

**需要**:

- [ ] `SessionConfig::from_settings(&BTreeMap<String,String>) -> Result<SessionConfig, ConfigError>`
- [ ] `StoreConfig::from_settings(...) -> Result<StoreConfig, ConfigError>`
- [ ] `LogConfig::from_settings(...) -> Result<LogConfig, ConfigError>`
- [ ] `Services::from_settings(...) -> Result<Services, ConfigError>`
- [ ] `SocketOptions::from_settings(...)` (扩展当前仅 TcpNoDelay 的实现)
- [ ] `rustls::ServerConfig` / `ClientConfig` 从 SSL 键构建 (SocketUseSSL, SocketKeyStore, etc.)
- [ ] `Schedule::from_settings(...)` (StartTime/EndTime/Weekdays/TimeZone/NonStopSession)
- [ ] 从 `ConnectionType` 路由到 initiator vs acceptor 构建逻辑
- [ ] 集成测试: 从 `.cfg` 文件启动完整引擎

**文件**: `crates/truefix-config/src/lib.rs` (新增 `builder.rs` 或 `mapping.rs`)

---

### TODO-03: Session 内 MessageStore 集成

**当前**: Transport 层已持久化 seq numbers (`seed_sequences` + `set_next_*_seq` after dispatch)，但 `Session` 内的 resend 仍使用内存 `BTreeMap<u64, Message>`。跨重启的 message resend 无法工作。
**目标**: Session 的 `store` 字段从 `BTreeMap` 替换为 `dyn MessageStore`，`save()` 和 `get()` 走持久化后端。

**需要**:

- [ ] `Session::with_store(config, store: Arc<dyn MessageStore>)` — 注入持久化存储
- [ ] `send_stored()` 调用 `store.save(seq, &msg.encode())`
- [ ] `build_resend()` 调用 `store.get(begin, end)` 而非内存 BTreeMap
- [ ] `reset()` 调用 `store.reset()`
- [ ] `seed_sequences()` 从 store 恢复 (已有, 确认 session 层也用)
- [ ] PersistMessages=N 时跳过 save (当前未实现)
- [ ] ForceResendWhenCorruptedStore: store 损坏时的恢复路径

**文件**: `crates/truefix-session/src/state.rs`

---

## P1 — 功能完整性差距

### TODO-04: Codegen 类型化消息

**当前**: `build.rs` 仅生成 MsgType 字符串常量 (如 `LOGON="A"`)。
**QF/J**: 生成完整的 Java 类 (如 `NewOrderSingle` 带每个字段的 typed accessor)。
**QF/Go**: 同上 (Go struct)。

**需要**:

- [ ] 从 normalized `.fixdict` 生成 per-version typed message struct (如 `fix44::NewOrderSingle`)
- [ ] 每个字段生成 typed accessor (`fn symbol(&self) -> Option<StringField>`)
- [ ] 生成字段枚举 (如 `Side::Buy("1")`, `Side::Sell("2")`)
- [ ] 生成 Group 的 typed struct
- [ ] 生成 Component 的 typed struct
- [ ] MessageFactory 基于 codegen 类型创建 Message
- [ ] MessageCracker 基于 codegen 类型分发到 typed handler

**文件**: `crates/truefix-dict/build.rs`, `crates/truefix-dict/src/codegen/` (新建)

---

### TODO-05: RejectGarbledMessage

**当前**: 传输层遇到 garbled 帧时直接丢弃 (`drain_messages` 中 `Err(_) => buf.clear()`)。
**QF/J / QF/Go**: 当 `RejectGarbledMessage=Y` 时，生成 session-level Reject (35=3) 消息。

**需要**:

- [ ] `Session` 增加 `reject_garbled: bool` 配置
- [ ] Transport 层 garbled 帧时通知 Session (新 Event 或回调)
- [ ] Session 生成 Reject (35=3, RefSeqNum=0, SessionRejectReason=0)
- [ ] AT 场景 `2d_GarbledMessage` / `3c_GarbledMessage` / `QFJ950-RejectGarbledMessages`

**文件**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-session/src/state.rs`

---

### TODO-06: 字典驱动的 Group 解析

**当前**: `codec::decode` 将 wire bytes 解析为扁平字段。Repeating group 的结构信息丢失 (count tag 被当作普通字段)。
**QF/J / QF/Go**: 解码时使用 DataDictionary 识别 repeating group，构建嵌套 Group 结构。

**需要**:

- [ ] `decode_with_dict(bytes, &DataDictionary) -> Result<Message, DecodeError>` — 字典感知解码
- [ ] 识别 group count tag → 解析 N 个 group entries
- [ ] 嵌套 group 递归解析
- [ ] 验证 group delimiter (FirstFieldInGroupIsDelimiter)
- [ ] Component 展开为 fields
- [ ] AT 场景 `14i_RepeatingGroupCountNotEqual` / `14j_OutOfOrderRepeatingGroupMembers` / `21_RepeatingGroupSpecifierWithValueOfZero` / `QFJ934`

**文件**: `crates/truefix-core/src/codec/decode.rs`, `crates/truefix-dict/src/` (新增 dict-aware decoder)

---

### TODO-07: EnableLastMsgSeqNumProcessed (369)

**当前**: 未实现。
**QF/J / QF/Go**: 当 `EnableLastMsgSeqNumProcessed=Y` 时，在每条出站消息的 header 中设置 tag 369 = 最后处理的入站消息 seq。

**需要**:

- [ ] `SessionConfig` 增加 `enable_last_msg_seq_num_processed: bool` (已注册为 Recognized)
- [ ] Session 追踪 `last_processed_in_seq`
- [ ] 出站消息 header 自动设置 tag 369
- [ ] AT 场景 `LastProcessedMsgSeqNum`

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-08: 剩余 Session 配置开关

**当前已注册为 Recognized 但未实现行为**:

- [ ] `SendRedundantResendRequests` — 已发 ResendRequest 后收到重复 ResendRequest 时是否重新发送
- [ ] `ClosedResendInterval` / `UseClosedResendInterval` — 使用固定间隔而非请求整个范围
- [ ] `ResetOnError` — 发生错误时重置序列号
- [ ] `DisconnectOnError` — 发生错误时断开连接
- [ ] `ForceResendWhenCorruptedStore` — store 损坏时强制重发 (检测有, 恢复路径未完整)
- [ ] `PersistMessages` — N 时不持久化消息 (仅维护 seq)
- [ ] `RefreshOnLogon` — Logon 时从 store 恢复状态 (字段存在, 未执行)
- [ ] `HeartBeatTimeoutMultiplier` — 心跳超时乘数 (当前固定 2×hb+2)
- [ ] `DisableHeartBeatCheck` — 禁用心跳检查
- [ ] `TestRequestDelayMultiplier` — TestRequest 延迟乘数 (当前固定 hb+1)
- [ ] `LogonTag` — Logon 中附加自定义 tag
- [ ] `MaxScheduledWriteRequests` — 最大排队写请求数
- [ ] `ContinueInitializationOnError` — 初始化错误时继续
- [ ] `LogMessageWhenSessionNotFound` — 会话未找到时记录日志
- [ ] `TimeStampPrecision` — 时间戳精度 (SECONDS/MILLIS/MICROS/NANOS)

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-09: 字典验证开关补全

**当前**: `ValidationOptions` 有 5 个开关。QF/J 有更多。

- [ ] `CheckCompID` — 验证 SenderCompID/TargetCompID 匹配 (已注册 Recognized)
- [ ] `ValidateFieldsOutOfOrder` — 验证字段顺序 (已注册 Recognized, 验证逻辑未实现)
- [ ] `ValidateUnorderedGroupFields` — 验证 group 内字段顺序 (已注册 Recognized)
- [ ] `RejectMessageOnUnhandledException` — 未处理异常时拒绝 (已注册 Recognized)
- [ ] `FirstFieldInGroupIsDelimiter` — group 首字段为分隔符 (已注册 Recognized)
- [ ] 自定义/扩展字典加载 (`DataDictionary::load_from_file(path)`)
- [ ] 组件 (Components) 支持 — 模型和验证

**文件**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-10: 反向路由 (Reverse Routing)

**当前**: 未实现。
**QF/J**: header 中的 `OnBehalfOfCompID`/`DeliverToCompID` 等路由字段可以被反转回复给原始发送者。

**需要**:

- [ ] Message header 反转路由字段 (OnBehalfOf→DeliverTo, etc.)
- [ ] AT 场景 `ReverseRoute` / `ReverseRouteWithEmptyRoutingTags`

**文件**: `crates/truefix-core/src/message.rs`, `crates/truefix-session/src/state.rs`

---

## P2 — 传输层补全

### TODO-11: 完整 Socket 选项

**当前**: 仅 `TcpNoDelay`。
**QF/J**: `SocketKeepAlive`, `SocketReuseAddress`, `SocketLinger`, `SocketOobInline`, `SocketReceiveBufferSize`, `SocketSendBufferSize`, `SocketTrafficClass`, `SocketSynchronousWrites`, `SocketSynchronousWriteTimeout`。

- [ ] 扩展 `SocketOptions` struct 包含上述全部字段
- [ ] 在 `apply()` 中逐一应用到 `TcpStream`
- [ ] 从配置键映射

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-12: mTLS (客户端证书认证)

**当前**: TLS 配置使用 `with_no_client_auth()`。
**QF/J / QF/Go**: 支持 `NeedClientAuth=Y` 要求客户端提供证书。

- [ ] `ServerConfig` 使用 `with_client_cert_verifier(roots)`
- [ ] `NeedClientAuth` 配置键 → 验证客户端证书
- [ ] 集成测试: mTLS 双向证书认证

**文件**: `crates/truefix-transport/src/lib.rs`

---

### TODO-13: 多端点 Failover

**当前**: Initiator 仅连接单个 `SocketConnectHost:SocketConnectPort`。
**QF/J / QF/Go**: 支持 `SocketConnectHost<N>` / `SocketConnectPort<N>` 编号备选端点，故障时自动切换。

- [ ] `SessionConfig` 增加 `connect_endpoints: Vec<SocketAddr>`
- [ ] `connect_initiator_reconnecting` 在端点间轮换
- [ ] 配置键解析 `SocketConnectHost1`, `SocketConnectPort1`, etc.

**文件**: `crates/truefix-transport/src/lib.rs`, `crates/truefix-session/src/config.rs`

---

### TODO-14: Schedule StartDay/EndDay

**当前**: `Schedule` 支持 daily window + weekdays 过滤 + UTC offset，但不支持 `StartDay`/`EndDay` 周窗口。
**QF/J**: 支持 `StartDay=Mon` / `EndDay=Fri` + times 形成跨日周窗口。

- [ ] `Schedule` 增加 `start_day: Option<Weekday>` / `end_day: Option<Weekday>`
- [ ] `is_in_session()` 处理跨日周窗口逻辑
- [ ] 配置键 `StartDay` / `EndDay` 映射 (已注册为 Recognized)

**文件**: `crates/truefix-session/src/schedule.rs`

---

### TODO-15: SQL 日志后端

**当前**: `truefix-log` 有 Screen/File/Tracing/Composite，无 SQL 后端。
**QF/J**: `JdbcLog` 写入 incoming/outgoing/event 表。
**QF/Go**: `SqlLog` 同上。

- [ ] `SqlLog` 实现 `Log` trait (sqlx)
- [ ] 表: `incoming(id, session_id, timestamp, msg)`, `outgoing(...)`, `event(...)`
- [ ] `LogConfig::Sql { url }` 变体
- [ ] JDBC 配置键映射 (已注册为 Recognized)

**文件**: `crates/truefix-log/src/sql.rs` (新建)

---

### TODO-16: CachedFileStore 缓存优化

**当前**: `CachedFileStore` 直接委托 `FileStore`，无独立缓存逻辑。
**QF/J**: 内存缓存层批量写入，`FileStoreMaxCachedMsgs` 控制缓存上限。

- [ ] `CachedFileStore` 内部维护 `BTreeMap<u64, Vec<u8>>` 内存缓存
- [ ] `save()` 先写缓存，达到 `max_cached_msgs` 时 flush 到文件
- [ ] `get()` 先查缓存，未命中再查文件
- [ ] `FileStoreMaxCachedMsgs` / `FileStoreSync` 配置键

**文件**: `crates/truefix-store/src/file.rs`

---

## P3 — 日志/监控/Polish

### TODO-17: 日志配置开关

**当前**: 所有日志后端无配置开关。
**QF/J**: `FileLogHeartbeats`, `FileIncludeMilliseconds`, `FileIncludeTimeStampForMessages`, `ScreenLogShowEvents/HeartBeats/Incoming/Outgoing`, `ScreenIncludeMilliseconds` 等 (已注册为 Recognized)。

- [ ] 各 Log 实现增加对应配置字段
- [ ] 从配置键映射
- [ ] Heartbeat 过滤 (N 时不记录 35=0/1)
- [ ] 毫秒时间戳包含开关

**文件**: `crates/truefix-log/src/*.rs`

---

### TODO-18: Metrics 仪表盘

**当前**: `Monitor` 提供 status/is_connected/force_logout/reset，但无 metrics 导出。
**QF/J**: JMX 暴露 gauges/counters。
**QF/Go**: 通过 `metrics` facade。

- [ ] 使用 `metrics` crate 导出: session_state (gauge), next_sender_seq (gauge), next_target_seq (gauge), messages_sent (counter), messages_received (counter), reconnect_count (counter)
- [ ] Prometheus exporter (可选, `metrics-exporter-prometheus`)
- [ ] FR-L1 完整达标

**文件**: `crates/truefix-transport/src/lib.rs` (Monitor 扩展)

---

### TODO-19: Benchmarks

**当前**: 无 benchmarks。
**SC-008**: "Reproducible benchmarks exist for codec throughput and session round-trip latency and run in CI for regression visibility."

- [ ] `benches/codec.rs` — 编解码吞吐 (criterion)
- [ ] `benches/session.rs` — 会话往返延迟
- [ ] CI 中运行 benchmarks (visibility-only, 无数值门槛)

**文件**: `benches/` (新建)

---

### TODO-20: 公共 API 文档

**当前**: 无 `#![deny(missing_docs)]`。
**SC**: Constitution Principle I 要求每个公开类型/trait/fn 有 doc 注释。

- [ ] Facade crate `crates/truefix/src/lib.rs` 启用 `#![deny(missing_docs)]`
- [ ] 所有 public crate 检查并补全 doc 注释
- [ ] `cargo doc --workspace` 无警告

**文件**: 所有 `crates/*/src/lib.rs`

---

### TODO-21: Parity Traceability Matrix

**当前**: 未创建。
**CHK036**: "Is a stable ID scheme present so each config key and AT scenario is individually traceable from spec → plan → tasks → implement coverage?"

- [ ] `docs/parity-matrix.md` — Appendix A 每个 key → owning crate/test; Appendix B 每个场景 → fixture/version
- [ ] 每个 key 标注 Implemented / Recognized / Unsupported + 对应测试
- [ ] 每个AT场景标注 已移植/未移植 + 对应版本

**文件**: `docs/parity-matrix.md` (新建)

---

### TODO-22: No-Panic 审计文档

**当前**: clippy lint 已全局生效，但无审计文档。
**SC-005**: "No reachable panic!/unwrap/expect exists on the codec, session state machine, I/O, or timer paths (verified by lint/audit)."

- [ ] `docs/no-panic-audit.md` — 记录审计方法、结果、已验证路径
- [ ] 手动扫描确认无 `panic!`/`unwrap`/`expect`/索引越界 (clippy lint 已覆盖, 需文档化)

**文件**: `docs/no-panic-audit.md` (新建)

---

### TODO-23: Quickstart 端到端验证

**当前**: `specs/001-fix-engine-parity/quickstart.md` 存在但未验证。

- [ ] 按 quickstart.md V1–V9 逐步执行并记录结果
- [ ] 最终 `cargo deny` clean report

**文件**: `specs/001-fix-engine-parity/quickstart.md`

---

## QuickFIX/Go 独有功能 (可选, 超出 QF/J 对等范围)

这些功能 QF/J 可能也没有, 但 QF/Go 有, 可作为 TrueFix 的差异化特性:

- [ ] `ResetSeqTime` — 连接中定时序列号重置 (QF/Go 独有)
- [ ] `InChanCapacity` — 入站消息有界缓冲 (QF/Go 独有)
- [ ] `IterateMessages` — 流式遍历存储 (QF/Go 独有)
- [ ] `SaveMessageAndIncrNextSenderMsgSeqNum` — 原子操作 (QF/Go 独有)
- [ ] `ConnectionValidator` + `NewListenerCallback` — acceptor 自定义 hook (QF/Go 独有)
- [ ] TCP PROXY protocol (HAProxy/ELB) — `UseTCPProxy` (QF/Go 独有)
- [ ] 内联 PEM bytes 配置 — `SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes` (QF/Go 独有)
- [ ] MongoDB 存储/日志 (QF/Go 独有)
- [ ] `DynamicQualifier` — 动态会话限定符 (QF/Go 独有)
- [ ] `SocketMinimumTLSVersion` — 最低 TLS 版本 (QF/Go 独有)
