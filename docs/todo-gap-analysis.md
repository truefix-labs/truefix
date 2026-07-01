# TrueFix 与 QuickFIX/J + QuickFIX/Go 功能差异 TODO

> 基于 2026-07-01 全量代码审查，对照 `specs/001` + `specs/002` 规格文档。
> 规格基线: Appendix A (~150 配置键) + Appendix B (73 server AT 场景 + 7 特殊类别套件)。

## 当前状态总览

| 维度 | 数据 |
|------|------|
| Crates | 9 个 (core/dict/session/store/log/transport/config/at/truefix) |
| 配置键 | ~151 个注册 (Impl/Rec/Unsup 三态) — SC-004 ✅ |
| AT 场景 | **56 种 / 81 实例** (2 版本: FIX.4.2 + FIX.4.4) |
| 规格覆盖 | **~22/73** server 场景 + 5/7 特殊套件部分覆盖 |
| Typed codegen | ✅ message struct + field enum + group + `crack_<version>` dispatcher |
| `.cfg`→引擎启动 | ✅ `Engine::start` + `builder.rs` + `resolve()` |
| TLS 从配置构建 | ✅ `rustls-pemfile` + `TlsSpec` + mTLS (`WebPkiClientVerifier`) |
| Socket 选项 | ✅ keepalive/linger/buffers/reuse/traffic/oob via `socket2` |
| 多端点 Failover | ✅ `connect_initiator_reconnecting_multi` + `SocketConnectHost1` |
| Metrics 导出 | ✅ `metrics_export.rs` (gauges/counters, SessionID labelled) |
| Schedule reset | ✅ `schedule_reset.rs` + StartDay/EndDay 周窗口 |
| 反向路由 | ✅ `reverse_route()` + AT 场景 |
| RejectGarbledMessage | ✅ `on_garbled()` → 35=3 |
| CheckCompID | ✅ `identity_problem` 强制执行 |
| Typed callback | ✅ `Reject`/`DoNotSend`/`BusinessReject` |
| Group 解析 | ✅ `decode_with_groups` + `GroupSpec` |
| Benchmarks | ✅ 编解码吞吐 (codec.rs) — SC-008 部分 ✅ |
| API 文档 | ✅ `#![deny(missing_docs)]` facade — SC-005 ✅ |
| No-panic 审计 | ✅ 文档化 — SC-005 ✅ |
| Parity matrix | ✅ 文档化 — CHK033/CHK036 ✅ |
| 示例 | 4 个 (executor/banzai/ordermatch/multi_acceptor) + smoke 测试 |
| MIGRATION.md | ✅ typed callback breaking change 文档 |
| SQL 多数据库 | ✅ PostgreSQL/MySQL/SQLite (`Pool` enum + per-backend SQL) — `sql.rs` (store+log) |
| CachedFileStore 缓存+fsync | ✅ `BodyLog` 磁盘索引 + 有界缓存淘汰 + `FileStoreSync` |
| 日志配置开关 | ✅ heartbeat 过滤/毫秒时间戳/可见性开关 + `SessionPrefixLog` |

---

## P0 — 发布阻塞项

### TODO-01: AT 场景覆盖率 (~22/73 = 30%)

**当前**: 56 种场景 / 81 实例, 覆盖 FIX.4.2 + FIX.4.4 两个版本。
**目标**: 73 个 server 场景 + 7 个特殊类别套件, 覆盖全部目标版本 (FR-M3)。

**已覆盖的规格 server 场景** (精确匹配 Appendix B):

- [x] `1a_ValidLogonWithCorrectMsgSeqNum` (×2 版本)
- [x] `1a_ValidLogonMsgSeqNumTooHigh` (×2 版本)
- [x] `2b_MsgSeqNumTooHigh` (×2 版本)
- [x] `2c_MsgSeqNumTooLow` (×2 版本)
- [x] `4a_NoDataSentDuringHeartBtInt` → `0_IdleHeartbeatEmitted` (×2 版本)
- [x] `4b_ReceivedTestRequest` (×2 版本)
- [x] `6_SendTestRequest` → `4_TestRequestOnSilence` (×2 版本)
- [x] `13b_UnsolicitedLogoutMessage` (×2 版本)
- [x] `14a_BadField` → `14a_InvalidTagNumber` (×2 版本)
- [x] `14b_RequiredFieldMissing` (×2 版本)
- [x] `14c_TagNotDefinedForMsgType` (×2 版本)
- [x] `14d_TagSpecifiedWithoutValue` (×2 版本)
- [x] `14e_IncorrectEnumValue` (×2 版本)
- [x] `14f_IncorrectDataFormat` (×2 版本)
- [x] `14h_RepeatedTag` (FIX.4.4)
- [x] `14i_RepeatingGroupCountNotEqual` (FIX.4.4)
- [x] `14j_OutOfOrderRepeatingGroupMembers` (FIX.4.4)
- [x] `21_RepeatingGroupSpecifierWithValueOfZero` (FIX.4.4)
- [x] `QFJ934_MissingDelimiterNestedRepeatingGroup` (FIX.4.4)
- [x] `2r_UnregisteredMsgType` (FIX.4.4)
- [x] `QFJ650_MissingMsgSeqNum` → `2_MissingMsgSeqNum` (×2 版本)
- [x] `7_ReceiveRejectMessage` → `3a_ReceivedRejectConsumed` (×2 版本)
- [x] `ReverseRoute` (×2 版本)
- [x] `ReverseRouteWithEmptyRoutingTags` (×2 版本)

**额外覆盖的 TrueFix 专属场景** (测试规格相关行为, 非精确匹配 Appendix B):

- [x] `1c_LogonAdoptsHeartBtInt` (×2) / `1d_LogonResponseResetFlag` (×2)
- [x] `2f_ResendRequestGapFill` (×2) / `2f3_ResendRequestBoundedEnd` (×2)
- [x] `2_ResendRequestNotDuplicated` (×2) / `2_ResendRequestBeginZeroIgnored` (×2)
- [x] `2x_OutOfOrderQueuedThenDrained` (×2) / `2h_ResendRequestNothingToResend` (×2)
- [x] `2g_SequenceResetReset` (×2) / `2d_SequenceResetGapFillAdvances` (×2) / `2e_SequenceResetGapFillBackwardIgnored` (×2)
- [x] `0a_ReceivedHeartbeatConsumed` (×2) / `2m_PossDupMsgSeqNumTooLow` (×2)
- [x] `14_valid_NewOrderSingleAccepted` (FIX.4.2 + FIX.4.4)
- [x] `app_NewOrderSingleExecuted` / `app_OrdersOutboundSequenced` / `app_MessageResentAsPossDup` / `app_MixedResendGapFillThenPossDup` (FIX.4.4)
- [x] `5_AcceptorInitiatedLogout` (FIX.4.4)

**特殊类别套件覆盖** (仅 FIX.4.4):

- [x] `special_NextExpectedMsgSeqNum` — 部分 (1/4 场景)
- [x] `special_LastMsgSeqNumProcessed` — 部分 (1/1 场景, 缺其他版本)
- [x] `special_ResendRequestChunkSize` — 部分 (1/2 场景)
- [x] `special_GarbledMessageDropped` (garbled drop)
- [x] `special_RejectGarbledMessage` (Reject 35=3)
- [ ] `validateChecksum` — 未覆盖
- [ ] `timestamps` — 未覆盖
- [ ] `resynch` — 未覆盖

**未覆盖的规格 server 场景** (~51 个):

- [ ] `1b_DuplicateIdentity`
- [ ] `1c_InvalidSenderCompID` / `1c_InvalidTargetCompID`
- [ ] `1d_InvalidLogonBadSendingTime` / `1d_InvalidLogonLengthInvalid` / `1d_InvalidLogonNoDefaultApplVerID` / `1d_InvalidLogonWrongBeginString`
- [ ] `1e_NotLogonMessage`
- [ ] `2a_MsgSeqNumCorrect`
- [ ] `2d_GarbledMessage` / `3b_InvalidChecksum` / `3c_GarbledMessage`
- [ ] `2e_PossDupAlreadyReceived` / `2e_PossDupNotReceived`
- [ ] `2f_PossDupOrigSendingTimeTooHigh` / `2g_PossDupNoOrigSendingTime`
- [ ] `2i_BeginStringValueUnexpected` / `2k_CompIDDoesNotMatchProfile`
- [ ] `2m_BodyLengthValueNotCorrect` / `2o_SendingTimeValueOutOfRange` / `2q_MsgTypeNotValid` / `2t_FirstThreeFieldsOutOfOrder`
- [ ] `8_AdminAndApplicationMessages` / `8_AdminAndApplicationMessages-FIX50SP2` / `8_OnlyAdminMessages` / `8_OnlyApplicationMessages`
- [ ] `10_MsgSeqNumEqual` / `10_MsgSeqNumGreater` / `10_MsgSeqNumLess`
- [ ] `11a_NewSeqNoGreater` / `11b_NewSeqNoEqual` / `11c_NewSeqNoLess`
- [ ] `14g_HeaderBodyTrailerFieldsOutOfOrder`
- [ ] `15_HeaderAndBodyFieldsOrderedDifferently`
- [ ] `19a_PossResendMessageThatHasAlreadyBeenSent` / `19b_PossResendMessageThatHasNotBeenSent`
- [ ] `20_SimultaneousResendRequest`
- [ ] `AlreadyLoggedOn` / `bugfix_QFJ634_ResendRequestAndSequenceReset` / `LogonUnknownDefaultApplVerID`
- [ ] `MinQty40` / `MinQty41` / `MinQty42` / `MinQty43` / `MinQty44` / `MinQty50`
- [ ] `QFJ648_NegativeHeartBtInt` / `RejectResentMessage` / `SessionReset`

**未覆盖的目标版本**: 当前仅 FIX 4.2/4.4。需扩展到 fix40/41/43/50/fixLatest (FR-M3)。

**文件**: `crates/truefix-at/src/scenarios.rs`, `crates/truefix-at/src/runner.rs`

---

### TODO-02: Session 内 MessageStore 集成

**当前**: Transport 层已持久化 seq numbers (best-effort after each dispatch), `seed_sequences()` / `seed_sent_messages()` 从 store 恢复。但 `Session` 内的 resend 仍使用内存 `BTreeMap<u64, Message>`, 跨重启的 message resend 无法工作。
**规格**: FR-G2, SC-006 — "persisted state is sufficient to satisfy a ResendRequest for messages sent before a process restart"。

**需要**:

- [ ] `Session::with_store(config, store: Arc<dyn MessageStore>)` — 注入持久化存储
- [ ] `send_stored()` 调用 `store.save(seq, &msg.encode())`
- [ ] `build_resend()` 调用 `store.get(begin, end)` 而非内存 BTreeMap
- [ ] `reset()` 调用 `store.reset()`

**文件**: `crates/truefix-session/src/state.rs`

---

## P1 — 功能完整性差距

### TODO-03: `ValidateFieldsOutOfOrder` 验证

**当前**: `ValidationOptions` 有 9 个开关 (含 group 验证), 但**顶层字段顺序验证** (`ValidateFieldsOutOfOrder`) 未实现。解码器保留原始 wire 顺序, 不拒绝乱序字段。
**规格**: FR-C3 — `ValidateFieldsOutOfOrder` 是 Appendix A 验证开关之一。
**QF/J / QF/Go**: 默认 Y, 验证 header/body/trailer 字段顺序。

- [ ] `ValidationOptions` 增加 `validate_fields_out_of_order: bool`
- [ ] `validate()` 检查字段顺序是否符合字典定义的 header/body/trailer 顺序
- [ ] AT 场景 `14g_HeaderBodyTrailerFieldsOutOfOrder` / `15_HeaderAndBodyFieldsOrderedDifferently` / `2t_FirstThreeFieldsOutOfOrder`

**文件**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-04: 剩余 Session 配置开关 (Recognized 未实现)

- [ ] `SendRedundantResendRequests`
- [ ] `ClosedResendInterval` / `UseClosedResendInterval`
- [ ] `ResetOnError`
- [ ] `DisconnectOnError`
- [ ] `DisableHeartBeatCheck`
- [ ] `RejectMessageOnUnhandledException`
- [ ] `LogonTag` — Logon 携带自定义 tag=value 对
- [ ] `MaxScheduledWriteRequests`
- [ ] `ContinueInitializationOnError`
- [ ] `LogMessageWhenSessionNotFound`
- [ ] `RefreshOnLogon` — 字段存在但 builder 未读取
- [ ] `ForceResendWhenCorruptedStore` — 检测有, 强制重发行为未完整

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`, `crates/truefix-config/src/builder.rs`

---

### TODO-05: 组件 (Components) 模型

**当前**: 字典仅有 `field` / `message` / `group` 指令, 无 `component` 指令。无 `Component` 类型。
**规格**: FR-C1 — "System MUST parse dictionary fields, components, groups, and messages"。
**QF/J / QF/Go**: 组件是可重用的字段/group 集合, 在多个消息中引用。

- [ ] normalized `.fixdict` 增加 `component` 指令
- [ ] `DataDictionary` 增加 `ComponentDef` 类型
- [ ] 消息定义引用组件
- [ ] 解码时展开组件为字段
- [ ] 验证组件内字段

**文件**: `crates/truefix-dict/src/parser.rs`, `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-06: 自定义字典运行时加载

**当前**: `parse(&str)` 是公开的, 但无 `load_from_file(path)` 辅助函数。仅 bundled `load_fixNN()` 可用。
**规格**: FR-C5 — "System MUST support User-Defined Fields (UDF) and loading custom / extended dictionaries"。

- [ ] `DataDictionary::load_from_file(path: &Path) -> Result<DataDictionary, ParseError>`
- [ ] `DataDictionary::extend(other: &DataDictionary)` — 合并扩展字典
- [ ] 集成测试: 从自定义文件加载字典并验证

**文件**: `crates/truefix-dict/src/lib.rs`

---

## P2 — 传输层补全

(TODO-07 SQL 多数据库支持、TODO-08 CachedFileStore 缓存优化+FileStoreSync、TODO-09 日志配置开关 已于
002/US12 解决并从本清单移除；见上方状态总览表与 `docs/parity-matrix.md` 的 "US12" 章节。)

## P3 — 日志/Benchmark 补全

### TODO-07: Session round-trip latency benchmark

**当前**: 仅 codec throughput benchmark。
**规格**: SC-008 — "reproducible benchmarks for codec throughput and session round-trip latency"。

- [ ] `benches/session.rs` — 会话往返延迟 (criterion 或自定义 harness)

**文件**: `benches/session.rs` (新建)

---

## QuickFIX/Go 独有功能 (可选, 超出 QF/J 对等范围)

- [ ] `ResetSeqTime` — 连接中定时序列号重置
- [ ] `InChanCapacity` — 入站消息有界缓冲
- [ ] `ConnectionValidator` + `NewListenerCallback` — acceptor 自定义 hook
- [ ] TCP PROXY protocol (HAProxy/ELB) — `UseTCPProxy`
- [ ] 内联 PEM bytes 配置 — `SocketPrivateKeyBytes`/`CertificateBytes`/`CABytes`
- [ ] MongoDB 存储/日志
- [ ] `DynamicQualifier` — 动态会话限定符
- [ ] `HeartBtIntOverride` — 覆盖对端 HeartBtInt
- [ ] `generate-fix` CLI — 独立代码生成命令行工具

## QuickFIX/J 独有功能 (无 Rust 等价物)

- [ ] `ApplicationFunctionalAdapter` — Lambda 监听器, 多消费者 FIFO, 类型安全
- [ ] `ApplicationExtended` 接口 — `canLogon` Predicate + `onBeforeSessionReset`
- [ ] JMX MBean 远程管理 — `JmxExporter` + 远程协议 (Monitor 是能力等价但无远程协议)
- [ ] 线程模型选择 — 单线程 vs ThreadPerSession (tokio async 是能力等价)
- [ ] 队列背压 / 水位线 — watermark-based flow control
- [ ] OSGi Bundle — `maven-bundle-plugin` (Rust 无 OSGi)
- [ ] `@Handler` 注解 MessageCracker — 反射类型安全分派 (有 codegen `crack_<version>` 替代)
- [ ] `RejectLogon` 异常 — SessionStatus + logoutBeforeDisconnect (`Reject` 近似但缺 SessionStatus)
- [ ] `FieldNotFound` 异常 — 带字段号命名异常 (`RejectReason` 枚举近似)
- [ ] `dictgenerator` CLI — FPL repository → 字典 XML
- [ ] SLF4J 日志门面 — `SLF4JLogFactory` (tracing 替代)
- [ ] FIX Latest — `quickfixj-messages-fixlatest` 模块
- [ ] SleepycatStore — Berkeley DB JE (Unsup)
