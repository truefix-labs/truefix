# TrueFix 与 QuickFIX/J + QuickFIX/Go 功能差异 TODO

> 基于 2026-07-01 全量代码审查 (TrueFix crates + thrdpty/quickfixj + thrdpty/quickfix 三方对照)，对照 `specs/001` + `specs/002` 规格文档。

## P0 — 发布阻塞项

### TODO-01: AT 场景覆盖率 (已完成 — 353/353 场景运行通过, 9/9 目标版本已接入)

**2026-07-01 (003 会话) 进展**: `SUITE_VERSIONS` 已扩展到全部 9 个目标版本 (fix40/41/42/43/44/50/50SP1/50SP2 +
fixLatest，US9 落地后完成)；~34 个版本无关场景现跨全部 9 版本运行；新增约 20+ 个具名场景 (见下方勾选)。
US1 收尾 (Phase 12) 新增了 `timestamps_suite()`/`resynch_suite()` 两个特殊套件函数，以及
`crates/truefix-at/tests/coverage.rs` 的套件完整性回归下限测试。详见 `docs/acceptance-record.md`
"003 — QuickFIX/J parity closure"节与 `specs/003-qfj-full-parity-closure/tasks.md` T012–T017/T054–T057
的逐项完成/延后记录。

**特殊类别套件 (三个，spec Acceptance Scenario 2)**:

- [x] `validateChecksum` — 已完成 (US3/T022): `validate_checksum_suite()`，复用
      `garbled_message_dropped`/`garbled_message_rejected`；checksum 校验是无条件的 (设计决策，见
      `ValidationOptions::validate_checksum` 文档)，故坏 checksum 帧总是走 garbled-message 路径。
- [x] `timestamps` — 已完成，代表性覆盖 (US1 收尾/T054): `timestamps_suite()`，复用
      `check_latency_timestamps` (CheckLatency/SendingTime 时效性校验)。**仍有一处已知、披露的
      harness 能力缺口**：对*出站* SendingTime 做精度级格式断言 (如区分毫秒 vs 秒精度，而非固定期望
      值——因为 SendingTime 总是"当前时间")需要 `ExpectMsg` 支持谓词/格式匹配而非仅精确值匹配；这是
      harness 能力缺口，不是协议缺口，未在本次范围内实现。
- [x] `resynch` — 已完成 (US1 收尾/T054): 新增 `resynch_suite()`，复用既有 ResendRequest gap
      recovery/SequenceReset Reset+GapFill(双向)/乱序排队排空/分块 resend 场景，作为独立可发现套件
      (此前这些场景仅隐式跑在 `server_suite()` 内，未被归组为独立套件)；transport 集成测试层
      (`reconnect.rs`/`restart_continuity.rs`) 的覆盖依然保留，两者互补而非替代。

**未覆盖的规格 server 场景**:

- [ ] `1b_DuplicateIdentity` — 延后: 需要并发连接/会话去重基础设施
- [ ] `1c_InvalidSenderCompID` / `1c_InvalidTargetCompID` — 延后 (Logon 时): `start_acceptor` 的动态模板会直接采纳首个 Logon 声明的身份，无法在首连接上制造"不匹配"；已用会话中段等价场景验证同一 `identity_problem` 逻辑 (见下方 `2i`/`2k` 已完成项)，需要 harness 增加非动态固定身份 acceptor 模式才能补上 Logon-时变体
- [x] `1d_InvalidLogonBadSendingTime` — 完成
- [ ] `1d_InvalidLogonLengthInvalid` — 延后: BodyLength 损坏可能破坏 `frame_length` 本身的分帧，需要更安全的损坏辅助函数
- [ ] `1d_InvalidLogonNoDefaultApplVerID` / `LogonUnknownDefaultApplVerID` — 不适用当前架构: 本代码库的 FIX50+ 版本未实现真正的 FIXT.1.1 ApplVerID 协商 (与 FIX4.x 一样用扁平 BeginString)
- [ ] `1d_InvalidLogonWrongBeginString` — 延后 (同 `1c_Invalid*CompID` 的动态模板限制)
- [ ] `1e_NotLogonMessage` — 延后: 需要产品决策 (首条非 Logon 消息是否应显式拒绝)
- [x] `2a_MsgSeqNumCorrect` — 完成
- [ ] `2d_GarbledMessage` / `3b_InvalidChecksum` / `3c_GarbledMessage` — 已在 `garbled_message_dropped`/`garbled_message_rejected` 下等价覆盖，不重复
- [ ] `2e_PossDupAlreadyReceived` — 已在 `poss_dup_too_low` 下等价覆盖
- [x] `2e_PossDupNotReceived` — 完成
- [ ] `2f_PossDupOrigSendingTimeTooHigh` / `2g_PossDupNoOrigSendingTime` — 延后: 底层 `requires_orig_sending_time`/`allow_pos_dup` 开关本身已实现 (见 TODO-09，已完成)，仅 AT 场景尚未编写 —不属于 T054–T057 的既定范围，留作后续场景补齐项
- [x] `2i_BeginStringValueUnexpected` — 完成 (会话中段)
- [x] `2k_CompIDDoesNotMatchProfile` — 完成 (会话中段)
- [ ] `2m_BodyLengthValueNotCorrect` — 延后 (同 `1d_InvalidLogonLengthInvalid` 的分帧风险)
- [x] `2o_SendingTimeValueOutOfRange` — 完成 (新增 `fresh_logon` 辅助函数)
- [ ] `2q_MsgTypeNotValid` — 延后: 与既有 `unregistered_msg_type`("2r") 业务层拒绝场景边界不清晰，无参考定义前不贸然区分
- [x] `2t_FirstThreeFieldsOutOfOrder` — 完成 (US3/T022)
- [x] `8_AdminAndApplicationMessages` / `8_OnlyAdminMessages` / `8_OnlyApplicationMessages` — 完成
- [ ] `8_AdminAndApplicationMessages-FIX50SP2` — 延后: 需要给 `start_acceptor` 接入 FIX50SP2 字典校验器
- [x] `10_MsgSeqNumEqual` / `10_MsgSeqNumLess` — 完成 (`10_MsgSeqNumGreater` 已由既有 `sequence_reset_reset` 覆盖)
- [x] `11b_NewSeqNoEqual` — 完成 (`11a`/`11c` 已由既有 `sequence_reset_gap_fill_advances`/`_backward_ignored` 覆盖)
- [x] `14g_HeaderBodyTrailerFieldsOutOfOrder` / `15_HeaderAndBodyFieldsOrderedDifferently` — 完成 (US3/T022)
- [ ] `19a_PossResendMessageThatHasAlreadyBeenSent` / `19b_PossResendMessageThatHasNotBeenSent` — 延后: 无参考定义，精确语义不确定，为避免断言错误行为暂缓
- [ ] `20_SimultaneousResendRequest` — 延后: 需要并发连接 harness 支持
- [ ] `AlreadyLoggedOn` — 延后: 需要产品决策 (重复 Logon 是否应显式拒绝)
- [ ] `bugfix_QFJ634_ResendRequestAndSequenceReset` — 延后: 无参考定义，精确交错时序不确定
- [ ] `MinQty40` / `MinQty41` / `MinQty42` / `MinQty43` / `MinQty44` / `MinQty50` — 延后: 已用 grep 确认 tag 110 (MinQty) 未出现在任何已捆绑字典子集中，需扩展字典内容而非仅编写场景
- [x] `QFJ648_NegativeHeartBtInt` — 完成
- [x] `RejectResentMessage` — 完成
- [ ] `SessionReset` — 延后: 需要 admin/control-channel 钩子接入 `Step` 模型 (目前只脚本化 wire 层 Send/Expect)

**目标版本**: fix40/41/42/43/44/50/50SP1/50SP2/fixLatest (9/9，已全部接入 `SUITE_VERSIONS`)。

**文件**: `crates/truefix-at/src/scenarios.rs`, `crates/truefix-at/src/runner.rs`

---

### TODO-02: Session 内 MessageStore 集成 (已完成 — 设计有修正，见下)

**2026-07-01 (003 会话) 结论**: 深入阅读实现后发现，"重启后从 store 恢复重发"这部分其实已经正确且已有测试
(`crates/truefix-session/tests/restart_resend.rs`，来自 002)——`run_connection` 在每次新连接 (含崩溃后重连)
时都会调用 `seed_sequences`/`seed_sent_messages` 从 store 全量恢复。真正的缺口是**重置一致性**：
`on_logon` 的 `ResetSeqNumFlag` 内部重置、以及 `enter_disconnected` 的 `ResetOnLogout`/`ResetOnDisconnect`
内部重置，都没有告知持久化 store 一并清空 (只有显式 `Control::Reset` 路径手动配对了)。

- [x] ~~`Session::with_store(config, store: Arc<dyn MessageStore>)`~~ — **设计修正**: `Session` 刻意保持
  sans-IO (无 I/O)，为此新增异步 store 句柄会违反该架构；改为新增 `Action::ResetStore` 声明式信号，
  由已经异步的 transport 层执行 `store.reset().await`，复用既有"Session 声明意图、transport 执行 I/O"模式
- [x] ~~`build_resend()` 调用 `store.get(begin, end)`~~ — 已确认现有 `seed_sequences`/`seed_sent_messages`
  机制已满足此需求 (见上)，无需改动
- [x] `reset()` 调用 `store.reset()` — 通过 `Action::ResetStore` 在 `on_logon`/`enter_disconnected` 的
  内部重置路径补齐，端到端验证见 `crates/truefix-transport/tests/restart_continuity.rs` 的
  `reset_on_logout_clears_the_durable_store`

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-transport/src/lib.rs`

---

## P1 — 功能完整性差距

### TODO-03: `ValidateFieldsOutOfOrder` 验证 (已完成)

**2026-07-01 (003 会话) 结论**: 新增 `truefix_core::Message::fields_out_of_order()` 标志，由
`decode()` 在按 tag 静态归类 header/body/trailer 时一并计算——3 个 `FieldMap` 各自保留段内 wire
顺序，但跨段交叉 (如 body 字段先于 header 段结束出现) 只有在 decode 过程中才可观察，解码完成后
的 `Message` 无法反推。

- [x] `ValidationOptions` 增加 `validate_fields_out_of_order: bool` (默认 `false`，维持现状行为)
- [x] `validate()` 检查字段顺序 (读取 `Message::fields_out_of_order()`)
- [x] AT 场景 `14g` / `15` / `2t` (需要给 `start_acceptor` 增加 `validate_fields_out_of_order`
  `SessionTweaks` 字段；场景通过 `Step::SendRaw` 发送手工乱序的原始字节，因为 `Message::encode()`
  总是重新按 canonical 顺序输出)

**文件**: `crates/truefix-core/src/message.rs`, `crates/truefix-core/src/codec/decode.rs`,
`crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`,
`crates/truefix-at/src/scenarios.rs`, `crates/truefix-at/src/runner.rs`

---

### TODO-04: 剩余 Session 配置开关 (已完成)

**2026-07-02 (003 会话) 结论**: 12 个全部有了明确结论；其中 8 个 `SessionConfig` 级别的开关真正做到了
`.cfg` → engine 全链路接入 (`builder.rs::resolve_one`)，不同于 TODO-09 发现的 "validation" 组缺口。

- [x] `SendRedundantResendRequests` — 已实现 (解除重发抑制)
- [x] `ClosedResendInterval` — 设计决策: 文档化为有意 no-op (单线程 sans-IO session 无并发重发竞争可言)
- [x] `ResetOnError` — 已实现
- [x] `DisconnectOnError` — 已实现
- [x] `DisableHeartBeatCheck` — 已实现
- [x] `RejectMessageOnUnhandledException` — 设计决策: 有意 no-op (Rust 类型化错误架构无"未处理异常"概念)
- [x] `LogonTag` — 已实现 (`LogonTag=<tag>=<value>` 格式)
- [x] `MaxScheduledWriteRequests` — 设计决策: 有意 no-op (session 状态机同步返回 action，无内部写队列可限)
- [x] `ContinueInitializationOnError` — 仍为 `Recognized`: 属于 `truefix::Engine::start` 多会话启动编排，非
  per-connection `Session` 运行时行为，留待后续会话在正确的层实现
- [x] `LogMessageWhenSessionNotFound` — 已实现，但发现它实际是 acceptor/路由层 (`route_and_run`)，不是
  `SessionConfig` 字段，改为 `truefix-transport::Services.log_message_when_session_not_found`
- [x] `RefreshOnLogon` — 已实现 (新增 `Session::refresh_sequences()` 无条件版本 + transport 端 logon 完成时钩子)
- [x] `ForceResendWhenCorruptedStore` — 已实现 (新增 `MessageStore::was_corrupted()` trait 方法 + 连接时强制
  `reset()`)

**附带修复的真实 bug**: `on_tick` 的 `LoggedOn` 心跳超时分支的 `enter_disconnected()` 调用点缩进与 US2 中
修复的另外两处不同，导致当时的 `replace_all` 编辑漏掉了它——该路径此前在超时断连时不会发出
`Action::ResetStore`，已在实现 `DisableHeartBeatCheck` 时一并修复。

**文件**: `crates/truefix-session/src/state.rs`, `crates/truefix-session/src/config.rs`,
`crates/truefix-session/src/admin.rs`, `crates/truefix-transport/src/lib.rs`,
`crates/truefix-store/src/lib.rs`, `crates/truefix-store/src/file.rs`,
`crates/truefix-config/src/builder.rs`, `crates/truefix-config/src/keys.rs`

---

### TODO-05: 组件 (Components) 模型 (已完成 — 但发现 build.rs codegen 侧尚未支持)

**2026-07-02 (003 会话) 结论**: normalized `.fixdict` 运行时侧 (`parser.rs`/`model.rs`) 已完整支持
`component` 指令 (含嵌套 component、嵌套 group、循环检测、未定义引用报错)，在 `DataDictionary` 构建期
完全展开为扁平 tag 列表，`decode.rs`/`validate.rs` 无需任何改动。

- [x] normalized `.fixdict` 增加 `component` 指令
- [x] `DataDictionary` 增加 `ComponentDef` 类型
- [x] 消息定义引用组件 (`component:<Name>` token), 构建期展开, 验证行为与手工内联字典完全一致 (SC-005)

**新发现的缺口 (未修复，已记录)**: `build.rs` 的独立 codegen 解析器 (`parse_dict`，双轨设计的另一半)
完全不认识 `component`/`component:<Name>`——它的成员列表解析是
`filter_map(|s| s.parse::<u32>().ok())`，会**静默丢弃**无法解析成 `u32` 的 `component:Name` token
而不报错。若未来任何已捆绑字典采用 `component`，codegen 会静默生成不完整的强类型结构体，而运行期
字典仍然正确——这是真实的双轨分歧风险。目前是**休眠状态** (无已捆绑字典使用 `component`)，扩展
`build.rs` 支持 component 是比本 US 运行期模型范围更大的任务，留待后续会话，已在
`docs/parity-matrix.md` 中明确记录以免被遗忘。

**文件**: `crates/truefix-dict/src/parser.rs`, `crates/truefix-dict/src/model.rs`

---

### TODO-06: 自定义字典运行时加载 (已完成)

**2026-07-02 (003 会话) 结论**: 两者均已实现，且 `extend()` 用"先全量冲突检测、后应用合并"的两阶段设计
保证冲突时 `self` 完全不受影响 (而非部分合并后中止)。

- [x] `DataDictionary::load_from_file(path: impl AsRef<Path>)` — 新增 `DictLoadError::Io`/`Parse`，
  两者都携带路径
- [x] `DataDictionary::extend(&mut self, other: &DataDictionary)` — 合并扩展字典；相同重定义幂等，
  冲突重定义返回 `DictMergeConflict` 且不修改 `self`；header/trailer 直接取并集 (无"冲突"概念)；
  `hash` 保持不变 (仍标识双轨基准来源，扩展字典有意游离在该不变量之外)

**附带修复**: `load_from_file` 测试最初用纳秒时间戳做临时文件名去重，在并发测试线程间可能撞车，导致
`cargo test --workspace` 下偶发失败 (单独运行时不可见)；已改用原子计数器 (与 `truefix-transport` 测试
已有模式一致)。

**文件**: `crates/truefix-dict/src/lib.rs`, `crates/truefix-dict/src/model.rs`

---

## P1 — 功能完整性差距 (续)

### TODO-08: 字段类型完整性 (已完成 — Data/UtcDateOnly/UtcTimeOnly；Double 按范围决策排除)

**2026-07-02 (003 会话) 结论**: 按 spec 003 Assumptions 中记录的范围决策，`Field::double`/`as_double`
不在本次范围内 (审计本身标注为可选，`rust_decimal` 已覆盖 Price/Qty 场景)，其余 3 种全部实现。

- [x] `Field::bytes(value: &[u8])` + `as_bytes()` — Data 字段类型 (FIX tag 95/96/212/348/352/445)；
  语义化包装 `new`/`value_bytes`，验证含内嵌 SOH 字节的场景
- [x] `Field::utc_date_only(date)` + `as_utc_date_only()` — UtcDateOnly (`YYYYMMDD`)
- [x] `Field::utc_time_only(time)` + `as_utc_time_only()` — UtcTimeOnly (`HH:MM:SS.sss`，与既有
  `utc_timestamp` 的毫秒精度惯例一致)；容忍到皮秒级小数位，截断为纳秒 (与 `as_utc_timestamp` 一致)
- [ ] `Field::double(value: f64)` + `as_double()` — **有意排除** (spec Assumptions 明确的范围决策)

**文件**: `crates/truefix-core/src/field.rs`, `crates/truefix-core/src/error.rs`

---

### TODO-09: 额外验证选项 (已完成)

**2026-07-01 (003 会话) 结论**: 4 个字段全部实现；其中 `validate_checksum` 采用"文档化的强制行为"
设计——TrueFix 解码器已经无条件校验 wire checksum (解码期错误，走既有 `RejectGarbledMessage` 路径)，
新增一个可关闭该校验的开关会构成正确性倒退 (违反宪法 Principle I/II)，因此该字段仅为 QFJ 配置键
对齐而存在，**不会**被用来削弱强制校验。

- [x] `validate_checksum: bool` — 保留字段以对齐 QFJ 配置键，但校验始终强制生效 (设计决策，非缺口)
- [x] `validate_incoming_message: bool` — 总体验证开关, 关闭则跳过所有字典验证 (QFJ `ValidateIncomingMessage`)
- [x] `allow_pos_dup: bool` — PossDup 消息接受策略 (QFJ `AllowPosDup`)
- [x] `requires_orig_sending_time: bool` — PossDup 必须携带 OrigSendingTime (QFJ `RequiresOrigSendingTime`)

**同时发现 (Principle VII)**: 这 5 个字段 (含 TODO-03) 对应的 Appendix A 键此前部分已标记
`Implemented`，但 `ValidationOptions` 根本没有对应字段、`validate()` 也未做任何检查——是一个
先于本次会话就存在的登记不准确。更广泛地看，"validation" 组的**任何**键都没有从 `.cfg` 接入
`Engine::start` 的 `Services.validator` (`builder.rs` 完全没有 dictionary/validator 解析函数)，
这是比本次 5 个字段更大的缺口，记录在 `docs/parity-matrix.md`，留待后续会话。

**文件**: `crates/truefix-dict/src/model.rs`, `crates/truefix-dict/src/validate.rs`

---

### TODO-10: FIX Latest 支持 (已完成)

**当前**: 9 个字典源覆盖 FIX40–FIX50SP2 + FIXT11, 无 FIX Latest。QFJ 有独立 `quickfixj-messages-fixlatest` 模块 + Orchestra XSLT 转换。

- [x] 从 FIX Orchestra 生成 normalized `.fixdict` (FIX Latest) — 新增 `crates/truefix-dict/src/orchestra.rs`
      (feature `dict-tooling`, `quick-xml` 解析一个有代表性的 Orchestra repository schema 子集:
      `<fixr:field>`/`<fixr:component>`/`<fixr:group>`/`<fixr:message>`，`StandardHeader`/
      `StandardTrailer` 特化为 `header`/`trailer` 指令)；`dict-src/orchestra/FIXLATEST.orchestra.xml`
      是自行编写的、体现 Orchestra schema 形状的 fixture(不拷贝任何 FPL 文件内容，Principle III)，
      转换产物即 `dict-src/normalized/FIXLATEST.fixdict`(与 FIX40–44 一致的会话层内联式子集,
      外加一个 `Parties` component 包一个 `NoPartyIDs` group，用于串联 US5/US7 的产出)。
- [x] `DataDictionary::load_fixlatest()` 加载器 — `crates/truefix-dict/src/lib.rs`；`ALL_DICTS` 现有 10 项。
- [x] build.rs codegen 生成 FIX Latest typed structs + `crack_fixlatest` — **附带修复**：`build.rs` 自己
      的极简 dict 解析器此前完全不认识 `component`/`component:<Name>` token(`req:`/`opt:` 列表用
      `.parse::<u32>().ok()` 直接把它们静默过滤掉——一个先前已被记录但未修的“静默丢数据”风险，
      本次因为 FIXLATEST 是第一个真正使用 `component:` 的内置字典而实测触发)。已按运行时
      `parser.rs` 同款的两阶段(先解析 raw component、再展开引用、带环检测)逻辑把 `build.rs` 补齐。
- [x] AT 场景扩展至 `FIX.Latest` — 加入 `SUITE_VERSIONS`(现 9 项)。逻辑与其余版本无关的核心场景
      (logon/时序/resend/管理消息等)自动对新版本生效，无需逐条新增；套件规模 318 → 353 runs，全绿。

**文件**: `crates/truefix-dict/src/orchestra.rs`(新增)、`crates/truefix-dict/src/lib.rs`、
`crates/truefix-dict/build.rs`、`crates/truefix-dict/dict-src/orchestra/FIXLATEST.orchestra.xml`(新增)、
`crates/truefix-dict/dict-src/normalized/FIXLATEST.fixdict`(新增)、`crates/truefix-at/src/scenarios.rs`

---

## P2 — Benchmark & 工具补全

### TODO-07: Session round-trip latency benchmark (已完成 — US11)

- [x] `benches/session.rs` — 会话往返延迟 (message-in → processed → response-out, 代表性
      Heartbeat/TestRequest/NewOrderSingle 混合场景; 观测/回归工具, 不设数值门槛, CI `bench` job
      `continue-on-error: true`)

**文件**: `crates/truefix-session/benches/session.rs` (新建)

---

### TODO-11: 网络增强 (已完成)

**当前**: transport 已有 TLS/mTLS、多端点 failover、socket 选项 (8 项)、IP allow-list。缺少以下 QFJ / QF/Go 网络功能。

- [x] **TCP PROXY protocol** (HAProxy/ELB) — acceptor 从 PROXY header 恢复真实客户端 IP (QF/Go `UseTCPProxy`)。
      新增 `crates/truefix-transport/src/proxy.rs`(`ppp` 解析 v1/v2)；仅当物理对端 IP 在
      `TrustedProxyAddresses` 中才解析/信任该 header (Clarifications 的信任边界)；`Services.
      trusted_proxy_addresses` 同时接入单会话 `Acceptor` 与多会话 `AcceptorBuilder`(后者把解析出的
      IP 送入既有 allow-list 检查，前者没有 allow-list 概念，仅做剥离避免 header 字节被误当 FIX 帧)。
- [x] **SOCKS Proxy** — initiator 透过 SOCKS 代理连接 (QFJ `Proxy*` settings, QF/Go
      `ProxyType/Host/Port`)。SOCKS4(+user ID)/SOCKS5(+用户名密码) 用 `tokio-socks`；HTTP CONNECT
      手写实现(单条请求行+头部块，不值得为此引入完整 HTTP 客户端依赖)。新增
      `connect_initiator_via_proxy`/`connect_initiator_via_proxy_tls`(纯增量 API，不改动既有
      `connect_initiator*` 系列的签名)。
- [x] **内联 PEM bytes** — 替代文件路径 (QF/Go 独有)。**设计偏离**: 本代码库的 `TlsSpec` 早在
      001/002 就把 cert+key 合并成单一 `key_store_path`(而非 QFJ 的 `SocketPrivateKeyFile`/
      `SocketCertificateFile` 分离两个文件)，故新增的内联字段也对应合并为
      `key_store_bytes`/`trust_store_bytes` 两项(通过 `SocketKeyStoreBytes`/`SocketTrustStoreBytes`
      两个新 key)，而非契约草案里 `SocketPrivateKeyBytes`/`SocketCertificateBytes`/`SocketCABytes`
      三项——与既有合并文件的既定设计保持一致，已如实记录而非悄悄改契约。`.cfg` 是逐行
      `key=value` 格式，PEM 块天然多行，故用字面 `\n` 两字符转义表示真实换行(已文档化)。
      `key_store_path` 字段类型从 `PathBuf` 改为 `Option<PathBuf>`(路径与内联 bytes 二选一)——
      技术上是 `TlsSpec` 的又一处破坏性字段变更，与 US10 的 `Reject.session_status` 同类，已披露。
- [x] **CipherSuites 配置** — rustls `SupportedCipherSuites` 可配 (QFJ `CipherSuites`)。构造一个过滤过的
      `rustls::crypto::CryptoProvider`(基于 `aws_lc_rs::default_provider()`，按 `Debug` 格式的套件名
      如 `"TLS13_AES_128_GCM_SHA256"` 做大小写不敏感匹配)，通过 `builder_with_provider` 接入。
- [x] **SocketSynchronousWrites** + `SocketSynchronousWriteTimeout` (QFJ 独有)。用
      `tokio::time::timeout` 包裹 `perform_actions` 里的出站 `write_all`；超时时通过 `Log::on_event`
      记录一条可区分的超时事件(而非泛化 I/O 失败)，随后断开连接——这是本代码库现有
      "Result<bool,()>" 错误处理约定下可行的、可测试的"typed error"落地方式。

**测试**: `crates/truefix-transport/tests/proxy_protocol.rs`(trusted/untrusted 边界，2 项)、
`proxy_client.rs`(SOCKS4/SOCKS5±认证/HTTP CONNECT，4 项，各自手写最小代理服务器并真实转发到一个
真实 FIX acceptor)、`tls_hardening.rs`(内联 PEM bytes、cipher suite 匹配/不匹配，3 项)、
`sync_writes.rs`(用小 socket 缓冲区 + 不再读取的"卡住的对端"逼出超时，1 项)，加上
`crates/truefix-config/tests/network_hardening_mapping.rs`(16 项 `.cfg` → `ResolvedSession` 映射测试)。

**文件**: `crates/truefix-transport/src/proxy.rs`(新增)、`crates/truefix-transport/src/lib.rs`、
`crates/truefix-transport/src/tls_config.rs`、`crates/truefix-config/src/builder.rs`、
`crates/truefix-config/src/keys.rs`、`crates/truefix/src/lib.rs`(facade `Engine::start` 接入)

---

### TODO-12: CLI 字典工具 (已完成)

**当前**: build.rs codegen 在编译时生成 typed structs, 但无独立 CLI 工具。QFJ 有 `dictgenerator` (FPL repo → XML), QF/Go 有 `generate-fix`。

- [x] `truefix-dict` CLI — 从 FIX Orchestra / FPL repository 生成 normalized `.fixdict`
      (`generate-dict --source <orchestra.xml> --out <normalized.fixdict>`)
- [x] `truefix-dict` CLI — 从 `.fixdict` 生成 typed Rust 代码 (脱离 build.rs 使用)
      (`generate-code --dict <normalized.fixdict> --out <generated.rs> [--name <Name>]`)
- [x] `truefix-dict` CLI — 验证字典文件语法 + 打印 hash (`validate --dict <normalized.fixdict>`)

**实现**: 新增 `crates/truefix-dict/src/codegen.rs`(把 `build.rs` 原有的 codegen 逻辑原样搬出来，
两边通过 `#[path = "src/codegen.rs"] mod codegen;` 共享**同一份源文件**——build.rs 天然不能依赖
自己尚未构建完成的库 crate，故用"共享源文件、各自独立编译"而非"库依赖库"来满足
"no parallel implementation"（Constitution Principle IV））。**顺带修复**：这次重构把
codegen 内部原有的 6 处 `panic!`/`unwrap_or_else(|_| panic!(...))` 全部改成 `Result<_,
CodegenError>`——build.rs 自身作为构建脚本继续在顶层 `main()` panic（构建脚本失败即 panic 是其
本就正确、符合惯例的行为，未改动），但这些函数现在也会被 CLI（面向用户的工具）直接调用，
用户提供格式错误的字典文件不应该看到 Rust panic 堆栈，而应该是一条干净的错误信息 + 非零退出码
(Constitution Principle I)。CLI 二进制 (`crates/truefix-dict/src/bin/truefix-dict.rs`) 手写
`--flag value` 参数解析(未新增 `clap` 等依赖，3 个子命令、每个最多 3-4 个 flag，不足以为此新增
依赖并走一次许可证/来源审计)；`[[bin]] required-features = ["dict-tooling"]`，故默认
`cargo build --workspace` 不构建该二进制(与其运行时依赖 `quick-xml` 保持一致的 tooling-only 边界)。

**测试**: `crates/truefix-dict/tests/cli.rs`(8 项，通过 `std::process::Command` 起子进程驱动
真正编译出的二进制——不是直接调用库函数——用以验证 CLI 本身：参数解析、退出码、错误信息，而不只是
它包装的底层逻辑；含"未知子命令"/"缺少必填 flag"两个专门断言"干净报错、不是 panic"的用例)。

**文件**: `crates/truefix-dict/src/codegen.rs`(新增)、`crates/truefix-dict/src/bin/truefix-dict.rs`
(新增)、`crates/truefix-dict/build.rs`(改写为薄驱动)、`crates/truefix-dict/Cargo.toml`

---

### TODO-13: 入站消息背压 — **已完成** (US14)

**当前**: transport 无入站消息有界缓冲。QF/Go 有 `InChanCapacity` (有界 channel), QFJ 有 `MessageQueue` (InMemory / BoundInMemory) + `MaxScheduledWriteRequests`。

- [x] `SessionConfig` 增加 `in_chan_capacity: Option<usize>` — 有界入站 channel, 满时施加背压
- [x] transport 层拆分为 reader 任务 + processor 任务：admin/session 消息走独立无界 channel
      (`admin_tx`)，application 消息走受 `in_chan_capacity` 限制的有界 channel (`app_tx`)。
      `in_chan_capacity` 缺省 (`None`) 时 application channel 的唯一 sender 立即被丢弃，
      `classify_buffered` 因此把所有消息都送入 `admin_tx`，行为与 US14 之前的单 channel 完全一致
      (对应 spec Acceptance Scenario 2 的显式要求)。
- [x] 满载行为: 阻塞投递 (背压) 而非丢弃 — 解码 (`classify_buffered`，从不阻塞) 与投递 (对
      `Sender::reserve()`——已确认 cancel-safe——与继续读取套接字做 `select!`) 完全解耦，
      reader 因此始终能在 application channel 满载时继续把新的 admin 流量送达，不会被卡住。

**文件**: `crates/truefix-session/src/config.rs`, `crates/truefix-transport/src/lib.rs`,
`crates/truefix-config/src/keys.rs` (`InChanCapacity` → `Impl`), `crates/truefix-config/src/builder.rs`
(`.cfg` → `SessionConfig.in_chan_capacity` 映射)

**测试**: `crates/truefix-transport/tests/backpressure.rs` (2 用例：有界 channel 满载不丢消息；
满载期间 admin 流量仍被及时处理)、`crates/truefix-config/tests/session_switches_mapping.rs`
(`in_chan_capacity_*` 3 用例)。详见 `docs/parity-matrix.md` "Feature 003 — US14" 一节。

---

### TODO-14: 额外 SQL 后端 — **部分完成** (US14: MSSQL 已完成；Oracle 按 spec Clarifications 延期)

**当前**: `SqlStore` 通过 sqlx 支持 PostgreSQL / MySQL / SQLite。QFJ (JDBC) 和 QF/Go 均额外支持 MSSQL 和 Oracle。

- [x] `MssqlStore`/`MssqlLog` 支持 MSSQL — 独立的 `mssql` feature，经 `tiberius` (TDS 驱动；sqlx
      无官方 MSSQL 支持) 而非 sqlx 实现，与 `SqlStore`/`SqlLog` 是并列而非同一实现 (二者共享同一
      `MessageStore`/`Log` trait 契约与相同的一致性测试模式，但底层驱动类型 (`tiberius::Client` vs.
      `sqlx::Pool`) 结构上不兼容，见 `docs/parity-matrix.md` "Feature 003 — US14" 一节的详细说明)。
- [ ] Oracle 支持 — **确认延期，非遗漏**。`oracle` crate 本身许可证宽松，但需链接闭源、仅按
      Oracle 自家 OTN License Agreement 分发的 Oracle Instant Client，与本项目 Principle III
      "纯净 Apache-2.0 OR MIT 发行" 的立场冲突。按 spec Clarifications 的明确授权
      ("MAY downgrade Oracle support to documented-interface-only (deferred) if no
      license-compatible mature option exists")，`StoreConfig`/`LogConfig` 不新增 `Oracle`
      分支；需要 Oracle 的使用方按同样方式直接实现 `MessageStore`/`Log` trait。详见
      `docs/parity-matrix.md` "Feature 003 — Dependency & Provenance Audit (T002)" 表格中
      `oracle` 一行的完整法务论证。
- [x] `SqlLog` 同步 → `MssqlLog` 支持 MSSQL (Oracle 同上延期)

**文件**: `crates/truefix-store/src/mssql.rs` (新增)、`crates/truefix-log/src/mssql.rs` (新增)、
`crates/truefix-store/Cargo.toml`/`crates/truefix-log/Cargo.toml` (`mssql` feature)、
`.github/workflows/ci.yml` (`mssql` job，MSSQL service container)、`deny.toml`
(RUSTSEC-2025-0134 的 `mssql`-feature-限定豁免)

**测试**: `crates/truefix-store/tests/mssql_backend.rs`、`crates/truefix-log/tests/mssql_log.rs`，
按 `DATABASE_URL_MSSQL` 门控，与既有 Postgres/MySQL 模式一致。

---

## QuickFIX/Go 独有功能 (可选, 超出 QF/J 对等范围)

> 以下功能 QFJ 不支持, TrueFix 可选择性实现。

- [ ] `ResetSeqTime` / `EnableResetSeqTime` — 连接中定时序列号重置 → 已纳入 TODO-04 评估
- [x] `InChanCapacity` — 入站消息有界缓冲 → **TODO-13** (已完成)
- [ ] `ConnectionValidator` + `NewListenerCallback` — acceptor 自定义认证 hook (mTLS/IP 之外的扩展认证)
- [x] TCP PROXY protocol (HAProxy/ELB) — `UseTCPProxy` → **TODO-11** (已完成)
- [x] 内联 PEM bytes 配置 → **TODO-11** (已完成)
- [x] MongoDB 存储/日志 — NoSQL 后端选项 → **GAP-03** (已完成，004 会话)
- [ ] `DynamicQualifier` — 动态会话限定符 (不预配 CompID 的 acceptor 场景)
- [ ] `HeartBtIntOverride` — 覆盖对端 HeartBtInt (对端配置不合理时强制纠正)
- [x] `generate-fix` CLI → **TODO-12** (已完成)

## QuickFIX/J 独有功能 (无 Rust 等价物)

> 以下功能 QF/Go 不支持。标注是否适合 TrueFix 实现。

- [x] `ApplicationExtended` 接口 — `canLogon` Predicate + `onBeforeSessionReset` → **已完成** (US10)。
      `canLogon` 复用既有 `from_admin(&Message, &SessionId) -> Result<(), Reject>` 回调 (无需新方法，
      仅补充文档说明它就是任意 Logon 拒绝逻辑的执行点)；新增 `Application::on_before_reset(&self,
      &SessionId)`(no-op 默认)。**设计修正**：`Session` 本身是刻意 sans-IO 的、从不持有
      `Application` 句柄 (US2 已确立此边界)，因此该 hook 无法真的"写在 `reset()` 内部"，而是由
      transport 层在三处实际触发 reset 的地方调用：显式 `Monitor::reset()`(`Control::Reset`)、
      `ForceResendWhenCorruptedStore` 内部触发、以及 `Action::ResetStore`(覆盖 logon 期
      `ResetSeqNumFlag`、`ResetOnLogout`/`ResetOnDisconnect` 等其余内部触发路径)。
- [ ] `ApplicationFunctionalAdapter` — Lambda 监听器, 多消费者 FIFO, 类型安全 → Rust 用 `Arc<dyn Application>` + `tokio::sync` 可替代, 优先级低
- [ ] JMX MBean 远程管理 — `JmxExporter` + 远程协议 → 不适合 (Rust 无 JMX; `metrics` facade + `Monitor` 是能力等价)
- [ ] 线程模型选择 — 单线程 vs ThreadPerSession → 不适合 (tokio async 是能力等价)
- [ ] 队列背压 / 水位线 — watermark-based flow control → **TODO-13** (有界 channel 近似)
- [ ] OSGi Bundle — `maven-bundle-plugin` → 不适合 (Rust 无 OSGi)
- [ ] `@Handler` 注解 MessageCracker — 反射类型安全分派 → 不适合 (有 codegen `crack_<version>` 编译时替代)
- [x] `RejectLogon` 异常 — SessionStatus + logoutBeforeDisconnect → **已完成** (US10)。`Reject` 新增
      `session_status: Option<u16>` 字段，`Session::reject_logon` 在其为 `Some` 时把 SessionStatus
      (tag 573) 写入 outbound Logout。**注意**: 这是给 `Reject` (公开结构体、字段全 `pub`) 新增了一个
      字段，对外部用直接结构体字面量构造 `Reject { .. }` 的调用方而言技术上是破坏性变更(需要补一行
      `session_status: None`)——与 003 计划声明的"无破坏性 API 变更"存在这一处例外，如实记录于此；
      本仓库内的两处调用点(测试)均已同步修复。`logoutBeforeDisconnect` 未额外建模——现有
      `reject_logon` 本就是"先发送 Logout 再断开"的顺序，天然满足该语义，无需新增开关。
- [ ] `FieldNotFound` 异常 — 带字段号命名异常 → 不适合 (`RejectReason` 枚举 + typed outcomes 已覆盖)
- [x] `dictgenerator` CLI — FPL repository → 字典 XML → **TODO-12** (已完成)
- [ ] SLF4J 日志门面 — `SLF4JLogFactory` → 不适合 (`tracing` 替代)
- [ ] FIX Latest — `quickfixj-messages-fixlatest` 模块 → **TODO-10**
- [x] SleepycatStore — Berkeley DB JE → 不直接移植 (过时技术)，但用 `redb` 做等价的现代替代 → **GAP-04** (已完成，004 会话：`RedbStore`/`RedbLog`)
- [x] `ValidateFieldsOutOfOrder` → **TODO-03** (已完成)
- [x] `ValidateChecksum` / `ValidateIncomingMessage` / `AllowPosDup` / `RequiresOrigSendingTime` → **TODO-09** (已完成)

---

## 2026-07-02 全量代码对比 — 剩余功能差距 (已完成 — 004 会话)

> 基于 2026-07-02 全量代码审查 (TrueFix crates 实现 vs thrdpty/quickfixj + thrdpty/quickfix)，
> 已过滤掉不适合 Rust 生态的项 (JMX/OSGi/SLF4J/反射/Sleepycat 过时技术等)、已有等价替代的项
> (tracing 替代 SLF4J、codegen 替代反射、Monitor 替代 JMX、有界 channel 替代水位线)、以及已完成的项。
>
> **2026-07-02 (004 会话) 结论**: GAP-01–GAP-06 全部完成，见 `specs/004-engine-wiring-extra-backends/`
> (spec.md 的 US1–US6、tasks.md 的 T005–T031 逐项完成记录)。004 不触碰任何 session-state-machine/
> codec/protocol 行为——既有 353/353 场景 AT 套件保持绿且未修改，是本轮的发布门槛本身 (FR-010)。

### P0 — 发布阻塞项

| # | 差距项 | 对比参照 | 说明 | 交叉引用 |
|---|--------|---------|------|----------|
| GAP-01 | ~~**`.cfg` 字典验证接线**~~ — **已完成 (US2)** | QF/Go + QF/J | `builder.rs::resolve_validator` 新增：`UseDataDictionary=Y` 时从 `DataDictionary`/`AppDataDictionary`/`TransportDataDictionary` 键解析出 bundled 版本号或文件路径，构建 `(DataDictionary, ValidationOptions)` 送入 `Services.validator`；`ResolvedSession` 新增 `validator` 字段。10 项 `.cfg`→映射测试 + 1 项端到端 `Engine::start` 拒绝坏消息测试 | TODO-09 结论中已披露；见 `specs/004.../tasks.md` T010–T013 |
| GAP-02 | ~~**Initiator Failover 未接入 Engine**~~ — **已完成 (US1)** | TrueFix 自身缺口 | 新增 `Engine.failover_initiators: Vec<ReconnectHandle>`；`Engine::start` 在 `failover_addresses` 非空且无 proxy 时路由到既有 `connect_initiator_reconnecting_multi`/新增的 `connect_initiator_reconnecting_multi_tls`；proxy+failover 组合暂不支持，记录 `tracing::warn!` 并回退到既有 proxy 路径 | 见 `specs/004.../tasks.md` T005–T009 |

### P1 — 功能完整性差距

| # | 差距项 | 对比参照 | 说明 | 交叉引用 |
|---|--------|---------|------|----------|
| GAP-03 | ~~**MongoDB 存储/日志**~~ — **已完成 (US6)** | QF/Go | `MongoStore`/`MongoLog` (`mongodb` feature，off-by-default)：`SessionDoc`/`MessageDoc` 集合 + `(session_id, seq)` 复合唯一索引；`StoreConfig::Mongo`/`build_store` 已接线，`LogConfig` 未接线 (与 `SqlLog`/`MssqlLog`/`RedbLog` 一致的既有先例，只有 `Screen`/`File`/`Tracing`/`Composite` 是 `.cfg`-可选的)。测试按 `DATABASE_URL_MONGO` 门控 (CI `mongo` service-container job 提供真实断言，本地无 Docker 环境下干净跳过) | QF/Go 独有功能节已列出；见 `specs/004.../tasks.md` T027–T031 |
| GAP-04 | ~~**`RedbStore`/`RedbLog`**~~ — **已完成 (US5)** | QF/J | [`redb`](https://crates.io/crates/redb) (MIT OR Apache-2.0) 嵌入式事务性 KV 存储，替代 QF/J 已过时的 `SleepycatStore`。`StoreConfig::Redb`/`build_store` 已接线 (`sql`/`mssql`-同类 off-by-default feature)；发现真实 `redb` 设计属性——`Database::create`/`open` 单进程互斥文件锁，两个独立连接无法共享同一文件，新增 `RedbStore::with_session_id` 解决 (克隆廉价的 `Arc<Database>`) | 沿用 `mssql` feature 的独立 off-by-default feature 模式；见 `specs/004.../tasks.md` T022–T026 |
| GAP-05 | ~~**`JdbcURL` / SQL 后端 `.cfg` 自动派发**~~ — **已完成 (US3)** | QF/J | `builder.rs::resolve_store`/`resolve_log` 新增 `JdbcURL` scheme 派发 (checked before `FileStorePath`)，通过新的三层可选 feature 透传模式 (`truefix-store`/`truefix-log` → `truefix-config` → `truefix` facade) 让 `.cfg` 能在不引入 SQL 依赖到默认构建的前提下选中 SQL/MSSQL store/log。log 侧新增独立 `SqlLogSpec` 字段 (非 `LogConfig` 枚举变体) | TODO-09 结论中提及 validation 组类似缺口；见 `specs/004.../tasks.md` T014–T019 |
| GAP-06 | ~~**`ContinueInitializationOnError`**~~ — **已完成 (US4)** | QF/J | 新增 `SessionSettings::resolve_lenient()` (逐会话解析，任一会话*解析期*失败时读取该会话原始 `.cfg` 里的 `ContinueInitializationOnError` 决定跳过还是中止——弥补 `resolve()` 本身"全有全无"的 `.collect()` 屏障)，`Engine::start` 的每会话启动循环同样在*启动期*失败时遵循该开关。`ContinueInitializationOnError` 从 `Recognized` 升级为 `Implemented` | TODO-04 已记录；见 `specs/004.../tasks.md` T020–T021 |

**已从本轮差距列表中移除**：
- ~~GAP-05 Oracle DB 存储~~ — 已是终局结论，非待办：Oracle Instant Client 闭源许可证与 Principle III 冲突，`StoreConfig`/`LogConfig` 不会新增 `Oracle` 分支，详见 TODO-14。
- ~~GAP-08 独立性能/压力测试套件~~ — 工具类投入，不影响协议正确性，优先级明显低于上述 6 项，暂不纳入当前差距追踪。

---

## 2026-07-02 深度代码对比 — 按层新增差距 (GAP-07+)

> 以下基于对三个引擎源码的逐文件深度对比 (session/state、transport、codec/dict、store/log/config 四层)，
> 已过滤不适合 Rust 生态项 (JMX/OSGi/SLF4J/反射/Sleepycat 等)、有等价替代项 (tracing/codegen/Monitor/
> 有界 channel 等)、TrueFix 独有优势项，并去重 GAP-01–GAP-06 已覆盖项及 TODO 中已标记"有意 no-op"的项。
> 每项标注 `file:line` 引用。

### 会话/状态机层

| # | 差距项 | 对比参照 | 说明 | 交叉引用 |
|---|--------|---------|------|----------|
| GAP-07 | **resend 时无 `to_app` 否决权** | QFJ `Session.java:1432` / QFGo `session.go:273` | `build_resend` (`state.rs:600`) 不调用 `to_app`，gap-fill 时应用层无法抑制陈旧订单重发；QFJ `resendApproved` 和 QFGo `session.resend` 都允许 app 拒绝并改发 gap-fill | — |
| GAP-08 | **无 `OrigSendingTime > SendingTime` PossDup 校验** | QFJ `Session.java:2581` / QFGo `in_session.go:381` | 防陈旧重放安全校验；TF 在 `seq < expected` + `PossDupFlag=Y` 时直接静默丢弃 (`state.rs:512`)，不校验 OrigSendingTime 与 SendingTime 的时序 | — |
| GAP-09 | **无 chunked-resend 自动续传** | QFJ `Session.java:1559` / QFGo `resend_state.go:50` | 一块满足后不自动发下一块；TF 仅出站分块 (`resend_request_chunk_size`)，入站端需手动触发后续 | — |
| GAP-10 | **`TimeZone` 仅数字偏移，拒绝 IANA 名** | QFJ `java.util.TimeZone` / QFGo `time.LoadLocation` | `builder.rs:794-812` 只接受 `+08:00` 格式；DST/时区规则变更需手动改偏移。可用 `chrono-tz` 解决 | — |
| GAP-11 | **无 `ResetSeqTime`** — 会话中不断连每日重置 | QFGo `session_state.go:158-180` 独有 | 在配置的每日时刻发送 `Logon(ResetSeqNumFlag=Y)` 重置序列号但保持连接；TF 必须断连+重连才能重置 | QF/Go 独有 line 395 |
| GAP-12 | **`LogonTag` 仅单个** | QFJ `SETTING_LOGON_TAG` (列表) | `config.rs:97` 为 `Option<(u32,String)>`；QFJ 支持多个 Logon tag | TODO-04 已实现单 tag |
| GAP-13 | **Reject `SessionRejectReason` 不按 FIX 版本裁剪** | QFJ `Session.java:1660-1679` | TF 总是 stamp reason code；QFJ 按 FIX 4.2/4.3/4.4 过滤掉该版本不支持的 reason 码 | — |

### 传输/网络层

| # | 差距项 | 对比参照 | 说明 | 交叉引用 |
|---|--------|---------|------|----------|
| GAP-14 | **无递增重连退避数组** | QFJ `AbstractSocketInitiator.java:252` | 仅固定 `reconnect_interval` (`lib.rs:1432`)；QFJ 支持 `int[]` 数组逐步升级，最后一个值粘滞 | — |
| GAP-15 | **无 `SocketLocalHost/Port` 本地绑定** | QFJ `AbstractSocketInitiator.java:196-214` | initiator 无法指定本地出口地址 | `keys.rs:145-146` Recognized |
| GAP-16 | **`SocketConnectTimeout` 未实际生效** | QFJ (default 60s) / QFGo `SocketTimeout` | `keys.rs:144` Recognized 但无代码消费此值 | — |
| GAP-17 | **`AllowedRemoteAddresses` 仅 acceptor 级** | QFJ `Session.java:3112` (per-session) | TF `AcceptorBuilder::allow_remotes` (`lib.rs:1224`) 是全局的；QFJ 支持每会话独立 IP 白名单 | — |
| GAP-18 | **Acceptor 层无多-logon 拒绝 + FIXT `DefaultApplVerID` 设置** | QFJ `AcceptorIoHandler.java:76-102` | TF 不在 acceptor 层拒绝重复 logon、不从 Logon 提取 HeartBtInt、不设置 FIXT DefaultApplVerID | — |
| GAP-19 | **动态会话模板仅 compID 替换** | QFJ `DynamicAcceptorSessionProvider.java:50-184` | TF `dynamic_config` (`lib.rs:1366`) 只替换 SenderCompID/TargetCompID；QFJ 支持 SubID/LocID/LocationID 通配 `*` 模式 → templateID 映射 | — |
| GAP-20 | **无 SubID/LocationID 路由** | QFGo `acceptor.go:284-325` | TF acceptor 按 BeginString/SenderCompID/TargetCompID 路由 (`lib.rs:1331`)；QFGo 还匹配 SubID/LocationID | — |
| GAP-21 | **ScreenLog/TracingLog/CompositeLog 无法从 `.cfg` 选择** — `SqlLog` 部分已完成 | QFJ / QFGo | `builder.rs::resolve_log` 只从 `FileLogPath` 构造 `FileLog`；`ScreenLog*` 键 Recognized 未接线 (`keys.rs:234-238`)；`TracingLog`/`CompositeLog` 仍仅编程式可选。**004 会话 (GAP-05) 已解决 `SqlLog` 一侧**：`JdbcURL` 存在时 `resolve_log` 现在会返回一个新增的 `SqlLogSpec`，`Engine::start` 据此构造 `SqlLog`/`MssqlLog`；`ScreenLog`/`TracingLog`/`CompositeLog` 的 `.cfg` 选择仍未接线，剩余缺口范围收窄 | 类比 GAP-01；GAP-05 已部分覆盖 |

### 编解码/字典层

| # | 差距项 | 对比参照 | 说明 | 交叉引用 |
|---|--------|---------|------|----------|
| GAP-22 | **缺 11 个 `FieldType`** | QFJ `FieldType.java:38-59` | `PRICEOFFSET`/`LOCALMKTDATE`/`DAYOFMONTH`/`UTCDATE`/`TIME`/`CURRENCY`/`EXCHANGE`/`MULTIPLEVALUESTRING`/`MULTIPLESTRINGVALUE`/`MULTIPLECHARVALUE`/`COUNTRY` — TF 全当原始字符串处理 (`model.rs:8-44` 仅 16 变体) | TODO-08 |
| GAP-23 | **无 `__ANY__`/`allowOtherValues` 开放枚举** | QFJ `DataDictionary.java:459-465` | QFJ 允许字段值不在枚举中时标记 `__ANY__` 跳过枚举校验；TF (`model.rs:102-104`) 只支持固定枚举 | — |
| GAP-24 | **无 per-group 子字典** | QFJ `DataDictionary.java:1341-1349` | TF 仅扁平 `members: Vec<u32>` (`model.rs:370`)；QFJ 每组携带独立 `DataDictionary` 实现深度嵌套校验 | — |
| GAP-25 | **Group API 仅 add-only** | QFJ `FieldMap.java:657-706` / QFGo `repeating_group.go:111-122` | 缺 `replace`/`remove`/`get-by-index`；TF 仅 `add_group` (`field_map.rs:64-67`) | — |
| GAP-26 | **核心层无 header/trailer repeating groups** | QFJ `Message.java:250-312` / QFGo `tag.go:51` | TF `decode.rs:69-77` 保持 header/trailer 扁平；如 `NoHops` (tag 504) 无法在核心层解析 | — |
| GAP-27 | **无 per-message 自定义 `fieldOrder`** | QFJ `FieldMap.java:52,116-132` | TF 用 `Vec<Member>` 插入序；QFJ 支持 `int[] fieldOrder` + `FieldOrderComparator` | — |
| GAP-28 | **无版本元数据 (major/minor/SP/EP)** | QFJ `DataDictionary.java:90-96` / QFGo `datadictionary.go:16-17` | TF 仅有 `version` 字符串 (`model.rs:154`)；无法做版本匹配校验 (GAP-32) | — |
| GAP-29 | **无 value→label 名称查找** | QFJ `valueNames` (`DataDictionary.java:107,245-258`) / QFGo `Enum.Description` | TF `FieldDef.values` (`model.rs:98`) 只存原始值，无人类可读描述 | — |
| GAP-30 | **无 Signature (tag 89) 长度特殊处理** | QFJ `Message.java:950-952` | QFJ 把 tag 89 的长度字段映射到 93 (而非默认的 88)；TF `data_field_for_length` (`tags.rs:66-81`) 未覆盖 tag 89 | — |
| GAP-31 | **无 picos 时间精度** | QFJ `UtcTimestampConverter.java:46` | TF 截断到纳秒 (`field.rs:195-208`)；QFJ 支持 picos (LENGTH=30)。实践罕见 | — |
| GAP-32 | **无版本匹配校验 vs BeginString** | QFJ `DataDictionary.java:632-639` | QFJ 校验消息的 BeginString 与字典版本一致；TF/QFGo 都不做 | 依赖 GAP-28 |
| GAP-33 | **字典只发子集** | QFJ / QFGo 均发完整 FIX spec | `FIX44.fixdict:1` 注释 "subset"；影响字段覆盖范围与 GAP-24/GAP-32 等联动 | TODO-05/TODO-10 |
| GAP-34 | **无 `toXML` 诊断输出** | QFJ `Message.java:325-435` | 调试用 XML 格式化消息；TF 无等价物 | 优先级低 |
| GAP-35 | **无 TZTIMEONLY/TZTIMESTAMP/LANGUAGE/XMLDATA 类型识别** | QFGo `validation.go:413-426` | QFGo 独有的 4 种类型；TF 不识别 | — |
| GAP-36 | **无 FIX 离线文件批量解析** | QFJ `FIXMessageDecoder.extractMessages` (`:303-339`) | QFJ 支持 mmap 文件流式提取消息 (日志回放/审计场景)；TF `frame_length` 无状态、无流式 reader | — |

### 存储/日志/配置层

| # | 差距项 | 对比参照 | 说明 | 交叉引用 |
|---|--------|---------|------|----------|
| GAP-37 | **`MessageStore` trait 缺 `incr*`/`refresh`/`getCreationTime`/`close`** | QFJ `MessageStore.java` / QFGo `store.go` | TF `lib.rs:56-75` 缺 `incr*` (引擎自算 `seq+1`)、`refresh` (热备刷新)、`getCreationTime` (GAP-38)、`close` (资源清理) | — |
| GAP-38 | **会话创建时间从不持久化** | QFJ `.session` 文件 / QFGo `.session` 文件 | TF file store 无 `.session` 文件 (`file.rs:226`)，SQL 表无 `creation_time` 列 (`sql.rs:154`)；QFJ/QFGo 均持久化且 reset 时更新 | 依赖 GAP-37 |
| GAP-39 | **`SaveMessageAndIncr` 非事务** | QFGo `sql_store.go:358-391` (单事务) | TF 两条独立 SQL 语句；QFGo 包在单事务内。崩溃窗口内可能 seq 已增但消息未存 | — |
| GAP-40 | **`Log` trait 缺 `clear()`/`onErrorEvent`/`onWarnEvent`** | QFJ `Log.java:30,58,66` | TF `lib.rs:63-65` 仅 `on_incoming`/`on_outgoing`/`on_event`；无严重级别区分、无清理 API | — |
| GAP-41 | **SQL log 表仅 `(id, text)` — 无时间戳/会话归属** | QFJ `(time, <8 id cols>, text)` / QFGo 同 | TF `sql.rs:156-186` 三张表均为 `(id autoincrement, text)`；审计不可追溯哪条会话、何时产生 | — |
| GAP-42 | **后台日志写无界 channel** | — | TF `SqlLog`/`MssqlLog` 用 `mpsc::unbounded_channel` (`sql.rs:244`); 背压下内存无限增长风险。可用 `tokio::sync::mpsc::channel(N)` 替换 | — |
| GAP-43 | **配置 `#` 在值中截断值 (bug)** | QFJ/QFGo 均不在值中截断 | `lib.rs:167-172` 在 `key=value` split 前对整行剥注释，`Password=ab#cd` 变成 `Password=ab` | 应修 bug |
| GAP-44 | **`${var}` 仅从 settings 自身解析，不接 env** | QFJ 从 `System.getProperties()` 解析 | TF `lib.rs:180-219` 只从配置自身插值；QFJ `.cfg` 用 `-D` 系统属性/env 的写法在 TF 不工作 | — |
| GAP-45 | **配置为不变快照 — 无运行时增删会话** | QFJ `SessionSettings.setString`/`removeSection` | TF `SessionSettings` parse 后冻结 (`lib.rs:120-134`)；QFJ 支持运行时增删会话 section (动态 acceptor 模式) | — |

**已从本轮深度对比中剔除的三大类**：
- **有替代**：`SessionStateListener` (用 `SessionStatus`+`Monitor`)、`WatermarkTracker` (双通道背压)、`ConnectionValidator` (`fromAdmin` 拒绝)、`toRawString` (`Field` 存原始字节)、解析时重复 tag 检测 (校验时仍检测)、可插拔 `Validator` (`fromApp` 自定义)、`TracingLog` category (tracing subscriber)、`admin/app messageCategory` (静态 admin 集合)、流式解析器 (transport framing)、零拷贝 `TagValue` (纯性能)、`sendToTarget` registry (`Monitor::send_app`)、`setNextSenderMsgSeqNum` (`seed_sequences`)、`logon()/logout()` API (`Event::StartLogout`)、`EndpointIdentificationAlgorithm` (rustls WebPki 隐式)、`canLogon` (`from_admin` 复用, TODO line 409)、`MaxScheduledWriteRequests` (有意 no-op, TODO-04)、file/SQL 跨引擎格式兼容 (自兼容即可)
- **不适合 Rust**：`VM_PIPE` (JVM 内)、JMX 计数器/操作、`KeyStoreType`/`KeyManagerFactoryAlgorithm`/`TrustManagerFactoryAlgorithm` (JSSE 专属)、`SingleThreadedEventHandlingStrategy` (tokio 调度器替代)、NTLM 代理 (basic auth 替代)、SLF4J (`tracing` 替代)、反射 `MessageCracker` (codegen 替代)、OSGi Bundle (无 OSGi)
- **TrueFix 独有优势**：Sans-IO FSM、类型化回调结果、异步 `Application` trait、`Action::ResetStore`、`on_before_reset`、`SessionStatus` 快照、双通道入站背压、`discard_sent` API、解码器无条件 CheckSum+BodyLength 校验、二进制长度前缀字段处理、原始字节往返保真、两层 typed reject、字典 `extend()` 冲突检测、双轨内容哈希、组件循环检测、`truefix-dict` CLI、rustls inline PEM、TLS 1.3-only、cipher suite 过滤、PROXY 可信门控、HTTP CONNECT+TLS-over-proxy、metrics facade、Live Monitor、编译期 panic-free、显式键姿态注册表

**与既有 GAP-01–GAP-06 的关系**：
- GAP-01 (字典验证 `.cfg` 接线) 和 GAP-05 (SQL store `.cfg` 派发) 属于配置接线层，GAP-21 (log `.cfg` 选择) 是同类问题的 log 侧延伸
- GAP-06 (`ContinueInitializationOnError`) 已覆盖 #15，不重复
- GAP-03 (MongoDB) 已覆盖 #19 的 Mongo 部分，不重复
