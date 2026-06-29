<!--
SYNC IMPACT REPORT
==================
Version change: 1.0.0 → 2.0.0
Bump rationale: MAJOR. Backward-incompatible redefinition of the principle set
  and governance: the delivery model changes from "phased milestones / 增量交付"
  to "one-pass full QuickFIX/J parity"; the former Principle VI (增量交付) is
  REMOVED; two new principles are ADDED (数据字典双轨, 基于清点的完整性); the
  conflict-adjudication order is redefined to 协议正确性 > 生产就绪 > 功能数量;
  the test discipline now MANDATES porting QuickFIX/J Acceptance Tests (AT).

Principles (7):
  - I.   生产就绪优先 (Production-Ready First) ............ unchanged
  - II.  协议正确性优先 (Protocol Correctness First) ...... AMENDED (AT = 最高验收项)
  - III. License 与来源纪律 (License & Provenance) ........ unchanged
  - IV.  数据字典双轨 (Dual-Track Data Dictionary) ........ ADDED
  - V.   测试纪律 (Test Discipline) ...................... AMENDED (mandate AT port)
  - VI.  Acceptor/Initiator 对等 (Parity) ................ unchanged (renumbered V→VI)
  - VII. 基于清点的完整性 (Inventory-Based Completeness) .. ADDED

Removed:
  - 增量交付 (Incremental Delivery via Milestones) — superseded by one-pass
    full-parity goal; milestone phasing demoted to non-binding execution tactic.

Sections:
  - "技术与实现约束" — AMENDED (codegen + runtime DataDictionary toolchain added)
  - "开发工作流与质量门" — AMENDED (AT gate + inventory-based parity gate added)
  - "Governance" — AMENDED (conflict order redefined)

Templates / artifacts reviewed for consistency:
  - .specify/templates/plan-template.md ............. ✅ aligned (generic
      "Constitution Check" gate; no hardcoded principles to update)
  - .specify/templates/spec-template.md ............. ✅ aligned (Requirements /
      Success Criteria accommodate AT-port + inventory evidence)
  - .specify/templates/tasks-template.md ............ ✅ aligned (test-first +
      codegen/runtime/AT task types fit existing phase structure)
  - .specify/templates/commands/*.md ................ ⚠ none present (no files)
  - README.md / docs/quickstart.md .................. ⚠ not present yet; align
      with this constitution when authored

Deferred TODOs: none. RATIFICATION_DATE unchanged (initial adoption 2026-06-29).
-->

# TrueFix Constitution

TrueFix 是一个用 Rust 实现的生产级 FIX(Financial Information eXchange)协议引擎。
目标是**一次性达到 QuickFIX/J 的完整功能对等(full feature parity)**,并在三点上差异化:
**生产就绪**、**acceptor/卖方一等公民**、**以通过 FIX conformance(接受性测试)为验收门槛**。
本宪法定义贯穿所有 `specify` / `plan` / `tasks` / `implement` 阶段的强制原则与治理规则。

## Core Principles

### I. 生产就绪优先 (Production-Ready First)

TrueFix 面向生产部署,而非演示。以下为非协商项:

- 公共 API MUST 稳定且文档完整:每个公开类型、trait、函数 MUST 有 doc 注释;破坏性
  变更 MUST 遵循语义化版本并记录迁移说明。
- 关键路径(编解码、会话状态机、I/O、定时器)MUST NOT 使用 `panic!`、`unwrap()`、
  `expect()`、`unreachable!()` 或会 panic 的索引/算术。所有可恢复错误 MUST 以类型化
  错误(`Result<T, E>`,`E` 为具名错误枚举)返回。
- MUST 提供结构化可观测性:会话状态、入站/出站序列号、连接健康度、重发与心跳事件
  MUST 可通过结构化日志(如 `tracing`)与可查询的会话状态导出。
- **Rationale**: 金融连接的中断、静默错误或不可诊断状态会直接造成交易与合规风险;
  生产就绪是 TrueFix 相对参考实现的核心差异化点。

### II. 协议正确性优先 (Protocol Correctness First) — NON-NEGOTIABLE

会话层语义 MUST 严格符合 FIX 规范;协议正确性是第一性需求,优先于性能、便利性与功能数量。

- 任何涉及协议行为(逻辑握手、序列号管理、Resend/GapFill、心跳/TestRequest、
  登出、消息校验与拒绝等)的决策,MUST 在 `spec` 中引用 FIX spec 条款或参考实现所
  体现的**行为(behavior)**作为依据,并记录依据来源。
- 通过 FIX conformance 接受性测试 MUST 作为涉及会话语义的功能的验收门槛;其中移植自
  QuickFIX/J 的接受性测试(AT,见原则 V)通过 MUST 被视为协议正确性的**最高优先级
  验收项**。
- 当实现便利与规范冲突时,MUST 选择规范一致的行为;偏离 MUST 在 spec 中显式标注并论证。
- **Rationale**: 不符合规范的 FIX 引擎在互联互通中不可用;conformance/AT 是可验证的、
  客观的正确性定义。

### III. License 与来源纪律 (License & Provenance Discipline) — NON-NEGOTIABLE

为使 TrueFix 能干净地以 **Apache-2.0 OR MIT** 双协议发布,来源纪律为绝对约束:

- `thrdpty/quickfix`(Go)与 `thrdpty/quickfixj`(Java)仅供**阅读、理解架构与协议行为**。
- MUST NOT 逐行翻译、移植或复制其源代码、注释或私有数据文件。任何借鉴 MUST 停留在
  "设计思想 / 协议语义"的抽象层面,且实现 MUST 基于 FIX 协议规范本身独立完成。
- 例外:接受性测试(AT)用例**作为黑盒行为契约**移植——MUST 以独立编写的测试驱动/夹具
  复现其 given/when/then 的协议行为,MUST NOT 复制其测试源码或运行器实现(见原则 V)。
- 引入任何第三方依赖 MUST 校验其许可证与 Apache-2.0 OR MIT 发布兼容(禁止 copyleft
  传染至 TrueFix 源码);不兼容者 MUST 拒绝。
- **Rationale**: 任何源码污染都会危及整个项目的可发布性与法律安全;这是一票否决项。

### IV. 数据字典双轨 (Dual-Track Data Dictionary)

强类型与运行期校验**并存**,而非二选一(QuickFIX/J 实际即如此):

- **build 期 codegen** MUST 从 FIX 数据字典生成强类型消息/字段定义,供编译期类型安全使用。
- **runtime DataDictionary** MUST 存在,在运行期对入站/出站消息做字段存在性、类型、
  必填项、组结构与会话/应用层归属校验。
- 两轨 MUST 共享同一份规范化字典数据源(单一事实来源),codegen 产物与 runtime 校验
  MUST NOT 出现语义分歧;字典数据来源 MUST 符合原则 III(源自官方规范或自有规范化字典)。
- **Rationale**: 强类型给开发期安全,runtime 字典给跨版本/自定义字典的灵活校验;二者缺一
  都无法对等 QuickFIX/J 的实际能力。

### V. 测试纪律 (Test Discipline)

测试是正确性的可执行证据,倾向测试先行(test-first)。

- 编解码层与会话状态机 MUST 具备充分的**表驱动(table-driven)单元测试**,覆盖正常
  路径、边界与错误/拒绝路径。
- 关键会话语义 MUST 具备**集成测试**:两个本地进程互连,跑完整握手、心跳/TestRequest、
  序列重置、Resend/GapFill、异常断连重连等场景。
- MUST 移植并跑通 QuickFIX/J 继承自 QuickFIX 的**接受性测试(AT)**用例;AT 以黑盒
  行为契约方式复现(见原则 III 例外),其通过是协议正确性的最高优先级验收项(见原则 II)。
- SHOULD 优先编写测试再实现(red → green → refactor);新协议行为的 PR MUST 附带
  对应测试。
- **Rationale**: 状态机与编解码的回归极难靠人工发现;表驱动 + 进程级集成 + AT 把
  conformance 目标转化为日常可执行的回归网。

### VI. Acceptor/Initiator 对等 (Acceptor/Initiator Parity)

卖方/acceptor 与买方/initiator MUST 被视为同等重要的一等公民。

- 任何会话层功能 MUST 同时为 acceptor 与 initiator 设计、实现与测试;MUST NOT 把
  acceptor 当作 initiator 的附属或二等路径。
- 功能 spec 的验收场景 MUST 同时覆盖 acceptor 侧与 initiator 侧的行为。
- **Rationale**: 一等公民的 acceptor/卖方支持是 TrueFix 的核心差异化点,薄弱的
  acceptor 会直接损害产品定位。

### VII. 基于清点的完整性 (Inventory-Based Completeness)

功能对等的完整性 MUST 以"对真实代码库的清点"为准,而非记忆或主观判断。

- 功能对等的验收依据 MUST 是从 `thrdpty/quickfixj` 抽取的完整功能/配置清单(消息类型、
  会话特性、配置项、扩展点等),清单 MUST 可追溯到具体来源(模块/配置键/特性)。
- 每个对等功能 MUST 在清单中标注实现状态与对应测试;声称"已对等"MUST NOT 在缺少清单
  条目与验证证据时成立。
- 抽取清单属"理解架构与行为"范畴,MUST 遵循原则 III(只记录功能/配置事实,不复制源码)。
- **Rationale**: 一次性达到完整对等的目标只有以客观清单为锚才可验收;记忆驱动会遗漏
  长尾功能,使"对等"沦为口号。

## 技术与实现约束 (Technology & Implementation Constraints)

这些约束将上述原则落实到技术选型与取舍上:

- **语言/工具链**: 实现语言为 Rust(stable toolchain)。仓库 MUST 启用 `cargo fmt`、
  `cargo clippy`(关键路径相关 lint 视为 error),CI MUST 运行 fmt/clippy/test/AT。
- **数据字典工具链**: MUST 提供 build 期 codegen(如 `build.rs` 或独立生成器)与 runtime
  DataDictionary 两套机制,二者共享单一规范化字典数据源(原则 IV)。
- **错误模型**: 公共错误类型 MUST 为具名枚举(可配合 `thiserror` 等),保证可匹配与
  可向前兼容;禁止以 `Box<dyn Error>` 掩盖关键路径的可恢复错误语义。
- **依赖纪律**: 新依赖 MUST 评估许可证兼容性(见原则 III)、维护活跃度与最小化原则;
  优先标准库与成熟生态(如 `tokio`、`tracing`、`bytes`),避免引入会迫使复制上游
  代码或带来许可证风险的库。
- **可观测性技术**: 会话状态、序列号与连接健康度 MUST 通过结构化机制暴露,便于外部
  监控接入,而非仅依赖临时打印。

## 开发工作流与质量门 (Development Workflow & Quality Gates)

- **Specify 阶段**: 涉及协议行为的需求 MUST 引用 FIX 规范条款或参考实现行为作为依据并
  记录来源(原则 II);MUST 标注 acceptor 与 initiator 双侧验收场景(原则 VI);对等类
  需求 MUST 链接到清单条目(原则 VII)。
- **Plan 阶段**: `Constitution Check` 门 MUST 通过后方可进入设计/实现;任何偏离原则的
  设计 MUST 在 `Complexity Tracking` 中论证,无法论证者 MUST 改回合规方案。
- **Tasks 阶段**: 关键路径任务 MUST 包含表驱动单测、进程级集成测试与对应 AT 移植任务
  (原则 V);涉及消息/字段的任务 MUST 同时覆盖 codegen 与 runtime 字典两轨(原则 IV)。
- **Implement / Review**: 每个 PR 的评审 MUST 验证:无关键路径 panic/unwrap/expect、
  错误已类型化、文档完整、测试随附且通过、相关 AT 通过、无参考实现源码复制痕迹、
  acceptor 与 initiator 双侧均已覆盖、对等条目在清单中标注。任一项不满足即 MUST 阻塞合并。

## Governance

本宪法 supersedes(优先于)所有其他实践与惯例。所有 specify/plan/tasks/implement 产物与
PR 评审 MUST 验证其合规性。

- **修订程序 (Amendment)**: 对本宪法的修订 MUST 以变更提案形式提交,说明动机、受影响
  原则与对现有 spec/plan/tasks 的迁移影响,经维护者批准后写入;批准后 MUST 更新版本号、
  `Last Amended` 日期与 Sync Impact Report,并同步检查依赖模板的一致性。
- **版本策略 (Versioning)**: 本宪法采用语义化版本。
  - **MAJOR**: 不向后兼容的治理/原则移除或重定义。
  - **MINOR**: 新增原则/章节或实质性扩展指南。
  - **PATCH**: 措辞澄清、错别字、非语义性细化。
- **冲突裁决顺序 (Conflict Adjudication Order)**: License 与来源纪律(原则 III)是法律
  红线,**不在权衡范围内,永远不可让渡**。在此前提下,当其余原则冲突时 MUST 按以下
  优先级裁决(高者胜出):
  1. **协议正确性(原则 II,含 AT)** — 产品存在的理由;正确性优先于一切便利与工期。
  2. **生产就绪(原则 I)** — 安全性/可诊断性优先于功能扩张。
  3. **功能数量 / 完整性对等(原则 VII)** — 在不牺牲正确性与生产就绪的前提下追求完整。
  原则 IV(数据字典双轨)、V(测试纪律)、VI(对等)为实现上述目标的强制手段,MUST
  被满足而非与上述目标权衡取舍。任何裁决 MUST 在相应 spec/plan 中记录所采用与被让步
  的原则及理由。
- **合规审查 (Compliance Review)**: 所有 PR/评审 MUST 核对原则合规;复杂度与偏离
  MUST 被论证,无法论证者驳回。运行期开发指引以 `CLAUDE.md` 及当前 plan 为准。

**Version**: 2.0.0 | **Ratified**: 2026-06-29 | **Last Amended**: 2026-06-29
