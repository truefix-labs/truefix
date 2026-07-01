# TrueFix 与 QuickFIX/J + QuickFIX/Go 功能差异 TODO

> 基于 2026-07-01 全量代码审查，对照 `specs/001` + `specs/002` 规格文档。

## P0 — 发布阻塞项

### TODO-01: AT 场景覆盖率 (~22/73 = 30%)

**当前**: 56 种场景 / 81 实例, 仅覆盖 FIX.4.2 + FIX.4.4。
**目标**: 73 个 server 场景 + 7 个特殊类别套件, 覆盖全部目标版本 (FR-M3)。

**未覆盖的特殊类别套件**:

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

**未覆盖的目标版本**: fix40/41/43/50/fixLatest (FR-M3)。

**文件**: `crates/truefix-at/src/scenarios.rs`, `crates/truefix-at/src/runner.rs`

---

### TODO-02: Session 内 MessageStore 集成

**当前**: Transport 层已持久化 seq numbers + sent messages, `seed_sequences()` / `seed_sent_messages()` 从 store 恢复。但 `Session` 内的 resend 仍使用内存 `BTreeMap<u64, Message>`。

- [ ] `Session::with_store(config, store: Arc<dyn MessageStore>)`
- [ ] `build_resend()` 调用 `store.get(begin, end)` 而非内存 BTreeMap
- [ ] `reset()` 调用 `store.reset()`

**文件**: `crates/truefix-session/src/state.rs`

---

## P1 — 功能完整性差距

### TODO-03: `ValidateFieldsOutOfOrder` 验证

**当前**: `ValidationOptions` 有 9 个开关, 但顶层字段顺序验证未实现。

- [ ] `ValidationOptions` 增加 `validate_fields_out_of_order: bool`
- [ ] `validate()` 检查字段顺序
- [ ] AT 场景 `14g` / `15` / `2t`

**文件**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-04: 剩余 Session 配置开关 (Recognized 未实现)

- [ ] `SendRedundantResendRequests`
- [ ] `ClosedResendInterval`
- [ ] `ResetOnError`
- [ ] `DisconnectOnError`
- [ ] `DisableHeartBeatCheck`
- [ ] `RejectMessageOnUnhandledException`
- [ ] `LogonTag`
- [ ] `MaxScheduledWriteRequests`
- [ ] `ContinueInitializationOnError`
- [ ] `LogMessageWhenSessionNotFound`
- [ ] `RefreshOnLogon` — 字段存在但 builder 未读取, state 未执行
- [ ] `ForceResendWhenCorruptedStore` — 检测有, 强制重发行为未完整

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`, `crates/truefix-config/src/builder.rs`

---

### TODO-05: 组件 (Components) 模型

**当前**: 字典仅有 `field` / `message` / `group` 指令, 无 `component`。

- [ ] normalized `.fixdict` 增加 `component` 指令
- [ ] `DataDictionary` 增加 `ComponentDef` 类型
- [ ] 消息定义引用组件, 解码时展开, 验证

**文件**: `crates/truefix-dict/src/parser.rs`, `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-06: 自定义字典运行时加载

**当前**: `parse(&str)` 公开, 但无 `load_from_file(path)`。

- [ ] `DataDictionary::load_from_file(path: &Path)`
- [ ] `DataDictionary::extend(other: &DataDictionary)` — 合并扩展字典

**文件**: `crates/truefix-dict/src/lib.rs`

---

## P2 — Benchmark 补全

### TODO-07: Session round-trip latency benchmark

- [ ] `benches/session.rs` — 会话往返延迟

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
