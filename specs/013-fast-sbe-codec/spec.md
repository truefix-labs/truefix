# Feature Specification: FAST/SBE Binary Protocol Codec (`truefix-binary`)

**Feature Branch**: `013-fast-sbe-codec`

**Created**: 2026-07-08

**Status**: Draft

**Input**: User description: "实现phase 2 的内容"

## Clarifications

### Session 2026-07-08

- Q: `docs/roadmap.md` 2a/2b 的验收标准要求编解码结果与公开参考实现（OpenFAST 测试向量 / FIX
  Trading Community FAST 规范示例编码；Real Logic `simple-binary-encoding` 的示例 schema）逐字节
  一致。这些参考文件的引入方式，直接决定了本 feature 能否达成其自身写下的验收标准。 → A: 采用外部
  公开参考实现的官方示例文件（OpenFAST 测试向量、Real Logic SBE 示例 schema）作为测试 fixture，
  前提是先确认其许可证与 Apache-2.0 OR MIT 兼容（Constitution 原则 III）；若某个参考文件许可证不
  兼容，则该项验收标准改为对照标准自带的示例编码（FAST/SBE 规范文档本身内嵌的例子），不得跳过验证。
- Q: `docs/roadmap.md` 2.4 第 2 条要求"在 Logon 阶段协商编码方式"。这既可以理解为运行时动态协商
  （对端在 Logon 消息中声明支持的编码，双方运行时选定），也可以理解为运营方在配置阶段静态指定
  （`Protocol = SOH | FAST | SBE` 写死在 session 配置里，不做运行时协商）。两者对 `truefix-session`
  会话状态机的改动量级差异很大。 → A: 本 feature 仅实现**静态配置**（session 配置项 `Protocol`
  决定该 session 全程使用的编码；连接双方须提前双边约定，不一致则 Logon 失败并给出明确的类型化
  错误）；运行时动态协商是否需要留给后续 feature 视实际需求决定，不在本 feature 范围内。
- Q: 2c 的 IR ⇄ SOH FIX 无损互转，是否要求覆盖全部已支持的 FIX 版本/消息字典（含自定义/vendor
  字典），还是聚焦一个有代表性的子集？ → A: 聚焦代表性子集——覆盖 FIX44 的 `NewOrderSingle`(D)、
  `ExecutionReport`(8)、一个含重复组的行情消息（如 `MarketDataSnapshotFullRefresh`(W)）三类消息，
  含至少一个嵌套重复组场景；全量字典覆盖留给后续迭代按需扩展，不在本 feature 验收范围内。
- Q: 一个 session 是否只能对应一份 FAST 模板 / 一份 SBE schema，还是可以加载一组模板/schema、
  按消息内联携带的模板/schema 标识符选择对应的编解码规则？ → A: 一个 session 可加载一组 FAST
  模板与一组 SBE schema/消息模板；编解码时按消息内联携带的标识符（FAST 的 template ID、SBE 的
  `templateId`/`schemaId`）从已加载集合中选择对应的编解码规则——这与 FAST/SBE 两个协议本身"一份
  文件声明多个模板/消息、按 ID 寻址"的设计一致；标识符在已加载集合中找不到匹配时 MUST 返回具名
  类型化错误，而不是套用任意一个已加载模板/schema 强行解码。
- Q: Constitution 原则 I（生产就绪优先）要求会话状态、连接健康度等 MUST 可通过结构化可观测性
  （如 `tracing`）暴露。本 feature 新增的二进制编解码路径是否也要遵循同一要求，为解码失败与
  Encoding Context 重置产生结构化 tracing 事件/指标？ → A: 是——编解码错误与 Encoding Context
  重置 MUST 产生结构化 tracing 事件/可查询指标，与既有会话可观测性要求保持一致地延伸到二进制
  编解码路径，而不是只满足于返回类型化错误（FR-009）却让上层无从观测。
- Q: 本 feature 是否需要为 FAST/SBE 编解码设定量化的吞吐/延迟 Success Criterion（阻塞性验收
  标准），还是沿用项目现有惯例（`cargo bench` 是观测/回归工具，非 CI 阻塞门槛）？ → A: 不设
  量化性能目标，沿用现有惯例——Success Criteria 保持功能性（字节级往返一致、SBE 零拷贝、解码
  失败零 panic），性能基准留给实现阶段做非阻塞的观测/回归对比，不作为本 feature 的验收范围。

## User Scenarios & Testing *(mandatory)*

TrueFix 是一个 FIX 协议引擎库。本 feature 的"用户"是把 TrueFix 用于高吞吐、低延迟场景（做市、
行情组播、订单录入）的集成工程师——他们今天只能使用 SOH 文本编码（`truefix-core`/`truefix-session`/
`truefix-transport`），本 feature 让他们可以选择 FAST（模板驱动、适合行情组播的压缩编码）或 SBE
（固定偏移、零拷贝，适合订单录入的低延迟编码）作为 wire 编码，同时复用现有的 `truefix-core::Message`
语义、`truefix-dict` 字典体系与 `truefix-session` 会话逻辑，不需要为二进制编码重新实现一套独立的
消息模型或会话状态机。

参见 `docs/roadmap.md` Phase 2（FAST/SBE 二进制协议扩展）§2.1–2.4：协议动机、FAST/SBE 核心技术、
新增 crate `truefix-binary` 的模块划分与阶段目标（2a/2b/2c）。

### User Story 1 - FAST 模板驱动编解码（行情组播场景） (Priority: P1)

集成工程师需要对接一个用 FAST 编码发布行情的交易所或数据源。工程师提供该数据源发布的 FAST 模板
（XML，声明每个字段的 copy/increment/delta/default 操作类型），需要引擎据此模板把接收到的二进制
行情流解码为可读的字段值，并能反向把行情消息编码为符合该模板的二进制流。

**Why this priority**: FAST 是本 feature 动机（`docs/roadmap.md` §2.1：交易所行情组播的微秒级延迟
场景）中价值最高、最具体的独立能力——只要模板解析与编解码器正确，就能独立于 SBE、独立于会话层集成
交付价值（工程师可以先用它做离线的行情流解码验证）。

**Independent Test**: 提供一份标准 FAST 模板 XML（声明若干字段的 copy/delta/increment 操作）和一段
按该模板编码的二进制样例流，验证解码结果的字段值正确；再验证把解码结果重新编码后与原始字节一致
（round-trip）。全程不依赖 SBE 代码路径或 `truefix-session`/`truefix-transport` 集成。

**Acceptance Scenarios**:

1. **Given** 一份声明了 copy/increment/delta/default 四种操作类型字段的 FAST 模板 XML，**When** 解析
   该模板，**Then** 引擎产出可查询各字段操作类型、数据类型的模板模型，不丢失任何字段声明。
2. **Given** 已解析的模板与一条按该模板编码的二进制消息（presence map + stop-bit 变长字段），
   **When** 解码该消息，**Then** 每个字段的值被正确还原，包括通过 delta/copy 操作从会话上下文
   推导出的、消息本身未显式携带的字段值。
3. **Given** 同一模板下的连续多条消息（同一会话上下文），**When** 后续消息的某字段与前一条消息相同
   （copy 操作）或按固定增量变化（increment 操作），**Then** presence map 正确指示该字段被省略传输，
   解码端仍能从上下文正确推导其值。
4. **Given** 一条包含 nullable 字段（presence map 位为 0 且无 operator）的消息，**When** 解码，
   **Then** 该字段被正确识别为 null，而非被误解码为某个默认值或导致解码失败。
5. **Given** 已解码的一组字段值，**When** 使用同一模板重新编码，**Then** 产出的二进制字节序列与
   Clarifications Q1 确定的参考编码（OpenFAST 测试向量/FAST 规范示例编码，视许可证核查结果而定）
   逐字节一致。
6. **Given** 一段被截断或包含非法 stop-bit 序列的二进制流，**When** 尝试解码，**Then** 引擎返回
   具名类型化错误（不 panic、不 unwrap），并指出解码失败的具体位置或原因类别。
7. **Given** 一个 session 已加载一组 FAST 模板（同一份或多份 XML 声明的多个模板，各自有唯一
   模板标识符），**When** 收到的消息内联携带的模板标识符匹配其中之一，**Then** 引擎使用该模板
   解码该消息；**When** 该标识符不在已加载集合中，**Then** 返回具名类型化错误，而不是套用任意
   已加载模板强行解码。

---

### User Story 2 - SBE Schema 驱动零拷贝编解码（订单录入场景） (Priority: P2)

集成工程师需要对接一个用 SBE 编码进行订单录入或低延迟行情分发的交易所。工程师提供该交易所发布的
SBE message schema（XML，声明字段类型、固定偏移、重复组的 block length/count），需要引擎据此
schema 以零拷贝方式编解码消息——不为每条消息分配中间对象，直接在底层 buffer 上读写。

**Why this priority**: SBE 覆盖本 feature 动机中的另一半场景（订单录入的可预测低延迟），且其"零拷贝
+ 固定偏移"的正确性可以独立于 FAST 验证；置于 P2 是因为它复用 User Story 1 建立的 IR/crate 结构
（`truefix-binary` 的模块划分），但不依赖 FAST 的具体编解码逻辑。

**Independent Test**: 提供一份标准 SBE schema XML（含固定字段、一个重复组、一个变长字段 VarData）
和一段按该 schema 编码的二进制样例消息，验证以 flyweight 方式读取的字段值正确，且读取过程不产生
除输入 buffer 本身以外的额外堆分配（可通过基准测试/内存剖析观察）；反向验证编码结果与参考编码一致。

**Acceptance Scenarios**:

1. **Given** 一份声明固定字段、一个重复组（block length + num groups）与一个 VarData 字段的 SBE
   schema XML，**When** 解析该 schema，**Then** 引擎产出可查询各字段类型、偏移、字节序的 schema
   模型。
2. **Given** 已解析的 schema 与一条按该 schema 编码的二进制消息，**When** 以 flyweight 方式解码，
   **Then** 每个固定偏移字段、重复组的每一项、VarData 字段的值均被正确读出，且读取操作不复制或
   重新分配消息 payload 本身。
3. **Given** 一条包含嵌套重复组或多个 VarData 字段的消息，**When** 解码，**Then** 所有层级的重复
   组与变长字段都被正确解析，顺序与 schema 声明一致。
4. **Given** 已解码的字段值集合，**When** 使用同一 schema 重新编码，**Then** 产出的二进制字节
   序列与 Clarifications Q1 确定的参考编码（Real Logic SBE 示例 schema/SBE 规范示例编码，视
   许可证核查结果而定）一致。
5. **Given** 一条声明的 `blockLength`/`numGroups`/`VarData length` 与实际二进制内容不一致的消息，
   **When** 尝试解码，**Then** 引擎返回具名类型化错误，而不是读取越界或返回错误数据。
6. **Given** 一个 session 已加载一组 SBE schema/消息模板，**When** 收到的消息 `templateId`/
   `schemaId` 匹配其中之一，**Then** 引擎使用该 schema 解码该消息；**When** 该标识符不在已加载
   集合中，**Then** 返回具名类型化错误，而不是套用任意已加载 schema 强行解码。

---

### User Story 3 - IR 互转与传输层编码选择 (Priority: P3)

集成工程师希望在不改变上层 Application 回调、会话逻辑、存储层代码的前提下，让某个 session 使用
FAST 或 SBE 而不是默认的 SOH 文本编码——即编码方式对上层是透明的。工程师在 session 配置中声明
`Protocol = FAST`（或 `SBE`），FAST/SBE 编解码器产出的中间表示（IR）可以和 `truefix-core::Message`
相互转换，使已有的 Application/会话/存储代码无需感知底层是哪种编码。

**Why this priority**: 这一项把 User Story 1/2 的独立编解码器接入到 TrueFix 现有引擎中，是"可用"
与"仅是两个独立编解码器"的分水岭；列为 P3 是因为 User Story 1/2 已经能独立交付价值（工程师可以
先用编解码器做离线验证），而集成工作依赖二者都已完成。

**Independent Test**: 配置一个 session 的 `Protocol = FAST`（或 `SBE`），验证该 session 建立后，
通过既有的 Application 回调收到的 `Message` 与直接用 SOH 编码时的语义等价（同样的字段、同样的
类型），即使底层实际收发的是二进制流；同时验证 `Protocol` 不匹配时 Logon 按配置失败并给出明确
错误，而不是静默按错误编码解析。

**Acceptance Scenarios**:

1. **Given** 一个 session 配置 `Protocol = FAST` 并关联一份 FAST 模板，**When** 收到一条编码后的
   二进制消息，**Then** Application 回调收到的 `Message` 与该消息对应的 SOH 版本在字段层面语义
   等价。
2. **Given** 同上，但 `Protocol = SBE`，**When** 发送一条消息，**Then** 实际写入连接的字节流是
   按 SBE schema 编码的二进制数据，而不是 SOH 文本。
3. **Given** 两端 session 配置的 `Protocol` 不一致（如一端 FAST、另一端 SOH），**When** 尝试建立
   连接并 Logon，**Then** Logon 失败并返回具名类型化错误，明确指出协议不匹配，而不是把二进制流
   错误解析为文本或反之。
4. **Given** Clarifications Q3 确定的代表性消息子集（`NewOrderSingle`(D)、`ExecutionReport`(8)、
   含重复组的行情消息），**When** 逐一通过 IR 与 `truefix-core::Message` 互转，**Then** 互转前后
   消息的字段集合与重复组结构完全一致，无信息丢失。
5. **Given** 一条消息经 IR 互转后，其中包含 FAST/SBE 侧不支持的、`truefix-core::Message` 独有的
   构造（如某些 SOH 专属字段编排方式），**When** 互转，**Then** 引擎明确报告该构造不受支持，
   而不是静默丢弃或产出错误数据。

---

### Edge Cases

- FAST 会话上下文（前值缓存，用于 copy/delta 推导）在连接中断重连后如何处理？（本 feature 默认
  上下文随 TCP 连接生命周期重置，不做跨重连持久化——如未来需要行情组播场景下的跨重连连续性，
  留给后续 feature；本 feature 要求重置行为产生 FR-013 规定的结构化 tracing 事件，而不是静默
  重置或静默产生错误解码。）
- 一个 FAST 模板或 SBE schema 引用了本 feature 未实现的字段类型/操作符时会发生什么？（应在模板/
  schema 解析阶段报告明确的类型化错误，而不是在实际编解码时才失败或产出错误数据。）
- 同一 session 的编码方式（`Protocol`）在会话建立后是否允许中途切换？（不允许——`Protocol` 是
  session 级别的静态配置，中途切换不在本 feature 范围内，见 Clarifications Q2。）
- 一条 FAST 或 SBE 消息内联携带的模板/schema 标识符（FAST template ID、SBE `templateId`/
  `schemaId`）在该 session 已加载的模板/schema 集合中找不到匹配时会发生什么？（应返回具名类型化
  错误，明确指出未知的 template/schema 标识，而不是尝试用任意一个已加载的模板/schema 强行解码；
  见 Clarifications。）
- FAST 的 stop-bit 变长整数编码在字段值超出可表示范围时如何处理？（应在编码阶段报错拒绝，而不是
  产出静默截断或溢出的二进制数据。）
- Acceptor 与 Initiator 两侧对同一份模板/schema 的编解码行为是否对称？（必须对称——两侧都需要能
  编码与解码同一模板/schema，见 Constitution 原则 VI。）

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 解析符合 FIX Trading Community FAST 规范的模板 XML，产出声明每个字段
  操作类型（copy / increment / delta / default）与数据类型的模板模型。
- **FR-002**: 系统 MUST 依据已解析的 FAST 模板，对二进制消息进行编码与解码，正确实现 presence map
  的位图语义与 stop-bit 变长整数编码。
- **FR-003**: 系统 MUST 支持 FAST 的会话级前值缓存（PMap 继承）：同一模板连续消息间，被省略传输
  的字段值可从上下文正确推导；MUST 支持 nullable 字段（presence map 位为 0 且无 operator）。
- **FR-004**: 系统 MUST 解析符合 FIX Trading Community SBE / ISO-IEC 25390:2025 规范的 message
  schema XML，产出声明字段类型、固定偏移、字节序的 schema 模型。
- **FR-005**: 系统 MUST 依据已解析的 SBE schema，以零拷贝（flyweight）方式编码与解码消息的固定
  偏移字段、重复组（block length + num groups）与变长字段（VarData）。
- **FR-006**: 系统 MUST 提供一个中间表示（IR），使 FAST/SBE 编解码器的输出可与 `truefix-core::Message`
  相互转换，且该转换对 Application 回调、会话层、存储层透明。
- **FR-007**: `truefix-transport`/`truefix-session` MUST 支持按 session 配置静态选择 wire 编码
  （`Protocol = SOH | FAST | SBE`）；两端 `Protocol` 不一致时，Logon MUST 失败并返回具名类型化
  错误（不得静默按错误编码解析对端数据）。
- **FR-008**: FAST 模板与 SBE schema MUST 纳入 `truefix-dict` 的规范化字典体系，作为与既有 SOH
  字典并存的独立轨道（Constitution 原则 IV：dual-track 扩展到二进制编码）。
- **FR-009**: 任何格式错误、截断、越界或声明与实际内容不一致的二进制输入，编解码路径 MUST 返回
  具名类型化错误，MUST NOT panic、unwrap、expect 或产生越界读取（Constitution 原则 I）。
- **FR-010**: FAST 与 SBE 的编码、解码能力 MUST 同时在 acceptor 与 initiator 两种会话角色下实现
  并测试，不得将其中一侧作为附属路径（Constitution 原则 VI）。
- **FR-011**: 系统 MUST NOT 要求 Application 层代码为了使用 FAST/SBE 而改写其消息处理逻辑——
  通过 FR-006 的 IR 转换，Application 收到的仍是 `truefix-core::Message`。
- **FR-012**: 一个 session MUST 能够加载一组 FAST 模板 / 一组 SBE schema（消息模板），而非仅限
  一份；编解码时 MUST 依据消息内联携带的模板/schema 标识符（FAST template ID、SBE `templateId`/
  `schemaId`）从已加载集合中选择对应的编解码规则。标识符在已加载集合中找不到匹配时 MUST 返回
  具名类型化错误（呼应 FR-009），MUST NOT 套用任意一个已加载模板/schema 强行解码。
- **FR-013**: 二进制编解码路径的解码失败（FR-009）、未知模板/schema 标识符（FR-012）与
  Encoding Context 重置（见 Edge Cases）MUST 产生结构化可观测信号（如 `tracing` 事件/可查询
  指标），延伸 Constitution 原则 I 对既有会话状态、连接健康度可观测性的要求，不得让上层仅凭
  返回的类型化错误而对这些事件失去可观测性。

### Key Entities *(include if feature involves data)*

- **FAST Template Set**：一个 session 加载的一组 FAST 模板，每个模板有唯一的模板标识符、若干
  字段的操作类型（copy/increment/delta/default）、数据类型与嵌套分组结构；一份 XML 可声明多个
  模板。
- **Presence Map (PMap)**：随 FAST 消息传输的位图，指示每个（或每组）字段是否在本消息中显式携带；
  按 FR-012 选定的具体模板解释。
- **Encoding Context**：会话级别的前值缓存，供 FAST 的 copy/delta/increment 操作在编解码时推导
  被省略的字段值；按模板标识符分别维护上下文，生命周期与底层连接绑定（见 Edge Cases）。
- **SBE Schema Set**：一个 session 加载的一组 SBE schema/消息模板，每个消息模板有唯一的
  `templateId`（及可选 `schemaId`），包含字段的固定偏移、类型、字节序，以及重复组、变长字段
  （VarData）的声明；一份 schema XML 可声明多个消息模板。
- **Intermediate Representation (IR)**：FAST/SBE 编解码结果与 `truefix-core::Message` 之间的
  中间形式，是本 feature 与既有引擎（会话层/存储层/Application 回调）解耦的关键抽象。
- **Protocol（session 配置项）**：声明某个 session 使用的 wire 编码（`SOH`/`FAST`/`SBE`），静态
  配置，见 Clarifications Q2。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 对任意合法的 FAST 模板与匹配的二进制消息样例，解码后重新编码的结果与
  Clarifications Q1 确定的参考编码来源逐字节一致，零字节差异。
- **SC-002**: 对任意合法的 SBE schema 与匹配的二进制消息样例，解码后重新编码的结果与
  Clarifications Q1 确定的参考编码来源逐字节一致，零字节差异。
- **SC-003**: SBE 消息的字段读取操作在基准测试中不产生消息 payload 之外的额外堆分配（零拷贝
  特性可通过内存剖析或分配计数验证）。
- **SC-004**: 使用 Clarifications Q3 确定的代表性消息子集，100% 的消息经 IR 与
  `truefix-core::Message` 互转后，字段集合与重复组结构与互转前完全一致。
- **SC-005**: 提供给编解码路径的格式错误、截断或越界二进制输入，100% 产生具名类型化错误，
  零 panic。
- **SC-006**: 一个配置 `Protocol = FAST`（或 `SBE`）的 session，其 Application 回调收到的消息
  与同一逻辑消息在 `Protocol = SOH` 下的字段语义 100% 一致；两端 `Protocol` 不一致时，100% 的
  Logon 尝试按 FR-007 明确失败，而不是产生错误解析或连接挂起。
- **SC-007**: 100% 的解码失败、未知模板/schema 标识符与 Encoding Context 重置事件产生可查询的
  结构化 tracing 事件/指标，运维方无需读取源码即可通过既有可观测性通道发现并区分这些事件。

## Assumptions

- Google Protocol Buffers（roadmap §2.1 表格中提及的第三种二进制编码）不在本 feature 范围内——
  roadmap 的阶段目标（2a/2b/2c）从未为 GPB 定义交付物或验收标准，本 feature 只覆盖 FAST 与 SBE。
- 本 feature 新增独立 crate `truefix-binary`，不修改 `truefix-core::Message` 的既有 SOH 编解码
  行为；SOH 路径的既有测试套件（含 AT 接受性测试）行为不变。
- STEP（国内交易所数据交换协议）不在本 feature 范围内——已确认其为 SOH tag=value 的 FIX 方言、
  与 FAST/SBE 无技术交集，归入 `docs/roadmap.md` Phase 3 §3.12 单独跟进。
- 低延迟市场数据的物理传输层（如交易所行情组播使用的 UDP 组播/PGM）不在本 feature 范围内；本
  feature 仅覆盖 FAST/SBE 的编解码与既有 TCP 传输上的协议选择，不新增组播传输能力。
- FAST 模板 / SBE schema 的解析器基于 `quick-xml`（`truefix-dict` 已有依赖），复用现有的
  feature-gated 解析工具模式（参考 `truefix-dict` 的 `dict-tooling` feature 与 `orchestra.rs`/
  `vendor_xml.rs` 的既有做法），而非引入新的 XML 解析依赖。
- 本 feature 不设量化性能/吞吐验收目标；`cargo bench` 风格的基准测试作为非阻塞的观测/回归工具
  留给实现阶段，与项目现有的 `truefix-core`/`truefix-session` 基准测试惯例一致（见 Clarifications）。
