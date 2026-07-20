# TrueFix 后续发展路线

> **路线提案（非当前能力清单）**：本文自早期阶段持续追加，下面的“当前”、目录草图和阶段状态
> 可能保留当时语境。以 [文档索引](README.md) 的当前项目表和实际 workspace crate 为准。
> 截至 2026-07-20，FAST/SBE、Futu、IB TWS、OKX、IG 已有独立 crate；统一 gateway、Tauri
> 客户端、新闻 provider、AI agent 和插件系统尚未在 workspace 中实现。

> 本文档描述 TrueFix 在完成 QuickFIX/J 功能对等（spec `001-fix-engine-parity`）之后的后续发展方向。
> 这些方向将 TrueFix 从一个 FIX 协议引擎扩展为一个**多协议、多券商、AI 驱动的交易基础设施**。

---

## 路线总览

| Phase | 主题 | 核心交付 |
|-------|------|---------|
| **Phase 1**（当前） | QuickFIX/J 功能对等 | S0–S9 AT 套件通过、FIX SOH 编解码 |
| **Phase 2** | 二进制协议扩展 | FAST/SBE 二进制编解码、低延迟市场数据通道 |
| **Phase 3** | 多券商 API 网关 | IB TWS / OpenD / Binance 适配器、独立 Client SDK、统一 Instruments、IB→FIX 协议桥接、STEP（国内交易所）适配器 |
| **Phase 4** | AI 交易 Agent | LLM 驱动交易决策、风控护栏、订单路由、回测/实盘切换 |
| **Phase 5** | 插件化架构 | 基于 WASM（Extism / Wasmtime Component Model）的策略与适配器插件系统 |

```
Phase 1 (当前)  ──▶  Phase 2  ──▶  Phase 3  ──▶  Phase 4  ──▶  Phase 5
QuickFIX/J 对等       二进制协议扩展   多券商 API 网关   AI 交易 Agent    插件化架构
```

---

## Phase 2: 二进制协议扩展 — FAST / SBE

### 2.1 背景与动机

FIX 协议的 SOH 文本编码（`8=FIX.4.4\x019=185\x01...`）在高吞吐场景下存在显著开销：每条消息需要
解析 ASCII 标签号、将数值字段序列化为十进制字符串、计算 BodyLength 与 CheckSum。对于期权做市、
交易所行情组播等微秒级延迟场景，二进制编码是刚需。

FIX Trading Community 定义了多种二进制编码标准：

| 编码 | 全称 | 特点 |
|------|------|------|
| **FAST** | FIX Adapted for STreaming | 模板驱动，presence map + delta 编码，适用于行情组播，带宽压缩比可达 10:1 |
| **SBE** | Simple Binary Encoding | 固定偏移、零拷贝编解码，适用于订单录入，延迟可预测；规范现已升格为 ISO/IEC 25390:2025 |
| **GPB** | Google Protocol Buffers | 第三方可选编解码，部分券商采用 |

参考文章: [FIX STEP FAST Binary 协议介绍](https://zhuanlan.zhihu.com/p/597926705)

### 2.2 FAST 协议核心技术

FAST 编码的核心机制：

- **模板 (Template)**: XML 定义的 message 编码规则，声明每个字段的操作类型（copy / increment / delta / default）
- **Presence Map**: 位图，指示哪些字段与前序消息不同（需传输），哪些可从前序状态推导（省略传输）
- **Stop-bit 编码**: 每字节最高位为 continuation bit，0 表示该字段结束，实现变长整数编码
- **PMap 继承**: 会话级别状态——同一模板的连续消息共享字段状态上下文
- **Nullable 字段**: 允许字段值为 null（presence map 位为 0 且无 operator）

```
FAST 消息结构:
┌──────────┬──────────────────────────────────────┐
│ PMap     │ Field data (stop-bit encoded)          │
│ (变长)   │ (按模板声明的顺序和操作类型编码)        │
└──────────┴──────────────────────────────────────┘
```

### 2.3 SBE 协议核心技术

SBE 编码的核心机制：

- **Message Schema**: XML 定义的消息结构，声明字段类型、偏移、字节序
- **固定偏移**: 每个字段在消息中的位置在编译期确定，支持零拷贝读写
- **Flyweight 模式**: 直接操作底层 buffer，无中间对象分配
- **Group 头**: 重复组通过 block length + num groups 声明，支持高效遍历
- **VarData**: 可变长度字段通过 length + data 两段表示

```
SBE 消息结构:
┌──────────────┬──────────────┬──────────┬──────────────────┐
│ Header       │ Fixed fields │ Group    │ Var data fields  │
│ (blockLen,   │ (固定偏移,   │ (block   │ (length-prefixed)│
│  templateId, │  按 schema    │  len +   │                  │
│  schemaId)  │  顺序排列)    │  count)  │                  │
└──────────────┴──────────────┴──────────┴──────────────────┘
```

### 2.4 实现计划

#### 新增 crate: `truefix-binary`

```
crates/truefix-binary/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 公共 trait: BinaryCodec, BinaryMessage
│   ├── fast/
│   │   ├── mod.rs          # FAST 引擎: 编解码器, 上下文状态
│   │   ├── template.rs     # 模板模型 + XML 解析
│   │   ├── presence_map.rs # PMap 读写
│   │   ├── encoder.rs      # FAST 编码器 (字段操作: copy/increment/delta/default)
│   │   ├── decoder.rs      # FAST 解码器
│   │   └── context.rs      # 会话级编码上下文 (前值缓存)
│   ├── sbe/
│   │   ├── mod.rs          # SBE 引擎
│   │   ├── schema.rs       # Schema 模型 + XML 解析
│   │   ├── encoder.rs      # SBE 编码器 (零拷贝, 固定偏移)
│   │   ├── decoder.rs       # SBE 解码器 (零拷贝读取)
│   │   └── flyweight.rs    # Flyweight buffer 抽象
│   └── ir.rs               # 中间表示: FAST/SBE ⇄ truefix-core::Message 互转
└── tests/
    ├── fast_roundtrip.rs   # FAST 模板驱动编解码往返
    ├── fast_delta.rs       # delta 编码: 连续行情消息压缩比验证
    ├── sbe_roundtrip.rs    # SBE 零拷贝编解码往返
    └── ir_conversion.rs    # 二进制 ⇄ SOH FIX 互转
```

#### 关键设计

1. **统一中间表示**: `truefix-binary` 的 FAST/SBE 编解码器产出的中间表示 (IR) 可与 `truefix-core::Message` 互转，使得会话层、存储层、Application 回调无需感知底层编码方式。

2. **传输层协议协商**: 扩展 `truefix-transport` 支持 `Protocol` 配置项（`SOH` / `FAST` / `SBE`），在 Logon 阶段协商编码方式（部分交易所支持 FIXT 1.1 的 encoding 类型协商）。

3. **模板/Schema 管理**: FAST 模板和 SBE schema 纳入 `truefix-dict` 的规范化字典体系，作为 dual-track 的第三条轨道（与 SOH 字典共享字段定义，独立声明编码规则）。

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 2a | FAST 模板解析 + 编解码器 | 解析标准 FAST 模板 XML；编码/解码往返与公开 FAST 参考实现（如 OpenFAST 的测试向量，或 FIX Trading Community FAST 规范自带的示例编码）字节一致 |
| 2b | SBE schema 解析 + 编解码器 | 解析标准 SBE schema XML；零拷贝编解码；与 Real Logic `simple-binary-encoding`（SBE/ISO-IEC 25390:2025 的事实参考实现）的示例 schema 编解码结果一致，延迟可测量 |
| 2c | IR 互转 + 传输层集成 | FAST/SBE ⇄ SOH FIX 无损互转；`truefix-transport` 支持协议选择 |

> **勘误**：本文档早前版本在此处将 STEP 描述为"建立在 FAST 之上的会话层，用于接入 CME/EUREX 行情组播"，
> 这一说法不准确。经核实原始标准文本（JR/T 0022—2020《证券交易数据交换协议》及其引用的 JR/T
> 0182—2020《轻量级实时STEP消息传输协议》LFIXT，以及上交所/深交所各自发布的 STEP 接口规格书），
> STEP 的报文本质上是 **SOH 分隔的 FIX 方言**（`BeginString=STEP.x.yz`，消息头/尾结构、校验和算法、
> 重复组机制均与标准 FIX 字节级同构，会话机制直接引用"FIX 标准会话机制"或其轻量化变体 LFIXT），
> 与 FAST/SBE 这类二进制编解码没有任何技术交集，因此已整体移除并移到 **3.12 STEP 适配器** 一节。

---

## Phase 3: 多券商 API 网关

### 3.1 背景与动机

不同券商和交易所使用不同的 API 协议，传统 FIX 引擎仅覆盖标准 FIX 协议。实际交易中，用户需要
对接 IB（盈透）、富途/moomoo、Binance、各交易所原生协议等多种接口。TrueFix 的目标是提供
一个**统一的交易网关抽象层**，屏蔽底层协议差异。

### 3.2 基础能力

> 以下能力是所有适配器的共同依赖。必须在编写任何适配器之前先落地，否则各适配器会各自
> 重新发明轮子，导致行为不一致且难以维护。

#### 3.2.1 统一数据模型

所有适配器共享的跨协议领域模型，定义在 `truefix-gateway` 顶层：

| 模型 | 说明 | 关键字段 |
|------|------|---------|
| `UnifiedOrder` | 跨协议下单表示 | symbol / side / order_type / quantity / price / time_in_force / client_order_id |
| `OrderAck` | 下单回执 | broker_order_id / client_order_id / status / timestamp |
| `OrderStatus` | 订单状态查询结果 | status / filled_qty / avg_price / remaining_qty / rejects |
| `OrderModification` | 改单请求 | order_id / new_quantity / new_price |
| `ExecutionReport` | 成交回报 | order_id / exec_id / side / last_qty / last_price / commission / status |
| `MarketData` | 统一行情 (enum) | `Quote`(买卖价) / `Trade`(成交) / `DepthBook`(深度) / `Ticker` |
| `Position` | 统一持仓 | symbol / direction / quantity / avg_price / unrealized_pnl（跨币种持仓需按 3.11.7 做 FX 转换后才可与账户货币加总） |
| `AccountInfo` | 统一账户 | balance / available / margin / currency（结算货币，可能与持仓 `Instrument.currency` 不同） |
| `Instrument` | 合约元数据（完整分类体系与分层字段见 3.11） | unified_symbol / security_type / tick_regime / lot_size / min_qty / trading_sessions |

#### 3.2.2 Symbol / Instrument 映射

各券商合约代码格式完全不同，需要统一的映射层：

| 券商 | 格式 | 示例 |
|------|------|------|
| IB | conId (int) + exchange + currency | `265598` (AAPL on NASDAQ) |
| 富途 | market + "." + code | `US.AAPL`, `HK.00700` |
| Binance | symbol string | `BTCUSDT` |
| Alpaca | symbol string | `AAPL` |
| FIX 标准 | Symbol(55) + SecurityID(48) + IDSource(22) | 依赖券商字典 |

需要：
- 统一 `Instrument` 类型作为所有适配器的标准输入输出
- 每个适配器实现 `SymbolMapper` trait: `to_broker(Instrument) → BrokerSymbol` / `from_broker(BrokerSymbol) → Instrument`
- **合约元数据查询**：最小交易单位、价格档位、乘数、交易时间——不同市场规则差异大（港股每手股数逐票不同、A 股 T+1、美股 T+0、加密 24/7、韩股分档 tick）
- 合约缓存：避免每次下单都查询

> 这里只是最小闭环所需的接口约定；完整的多资产（股票/加密/期权/期货/窝轮/牛熊证/杠杆产品）
> 分类模型、市场特定规则、公司行为处理见 **3.11 统一 Instruments 服务**。

#### 3.2.3 连接生命周期管理

各券商连接模式差异大，需要统一的状态机：

```
Disconnected ──▶ Connecting ──▶ Authenticating ──▶ Connected ──▶ Ready
      ▲                                                    │
      └──────────────── Reconnecting ◀─────────────────────┘
```

- **断线重连 + 指数退避**：IB TWS 断开、OpenD 崩溃、Binance FIX session 断开都需自动恢复
- **心跳 / keepalive**：IB 有自己的 heartbeat；Binance FIX 用 FIX Heartbeat(35=0)；OpenD 有心跳包；REST 适配器用应用层 ping
- **订阅恢复**：重连后自动恢复之前的行情订阅和订单回调订阅
- **优雅关闭**：确保关闭时清理服务端订阅状态、发送 Logout / disconnect 消息

#### 3.2.4 Order ID 管理与持久化

| 券商 | orderId 方案 |
|------|-------------|
| IB | `orderId` (int)，单调递增，**必须持久化**，跨重启恢复 |
| Binance | `clientOrderId` (字符串) + `orderId` (交易所分配) |
| 富途 | `orderId` (交易所分配) |
| Alpaca | `client_order_id` (字符串，UUID) |

需要：
- 统一的 `ClientOrderId` 类型
- 每适配器的 orderId 生成 + 持久化策略（IB orderId 回退会冲突，必须落盘）
- `client_order_id → broker_order_id` 映射表（用于撤单/查询时映射）
- 跨重启恢复

#### 3.2.5 限流控制 (Rate Limiting)

每个券商都有严格的频率限制，不处理会导致封禁：

| 券商 | 限制类型 |
|------|---------|
| Binance | Weight-based（每分钟权重上限），不同 API 权重不同 |
| IB | 消息/秒（`reqMktData` 最多 100 个并发订阅等） |
| 富途 | 每 API 独立限制（如 `PlaceOrder` 5次/秒） |
| Alpaca | 200 requests/min (free tier) |

需要统一的 `RateLimiter` trait，每个适配器配置自己的令牌桶参数。超限时排队等待，而非直接失败。

#### 3.2.6 统一错误模型

各券商错误码完全不同（IB 的 error code 200 系列 vs Binance 的 `-1003` vs 富途的数字错误码），需要映射：

```rust
pub enum GatewayError {
    /// 可重试：网络超时、临时不可用
    Retryable { source: Box<dyn Error + Send + Sync>, retry_after: Option<Duration> },
    /// 不可重试：认证失败、参数错误、合约不存在
    NonRetryable { code: String, message: String },
    /// 被限流
    RateLimited { retry_after: Duration },
    /// 订单被拒绝
    OrderRejected { reason: String, order_id: String },
    /// 连接断开
    Disconnected { reason: String },
}
```

#### 3.2.7 凭证与配置管理

```toml
[gateway.binance]
api_key = "${BINANCE_API_KEY}"
api_secret = "${BINANCE_API_SECRET}"
testnet = true

[gateway.ib]
host = "127.0.0.1"
port = 7497
client_id = 1

[gateway.futu]
host = "127.0.0.1"
port = 11111

[gateway.alpaca]
api_key = "${ALPACA_API_KEY}"
api_secret = "${ALPACA_API_SECRET}"
paper = true
```

- 环境变量注入（`${VAR}` 语法，复用 `truefix-config` 已有能力）
- API Key / Secret 安全存储（不落盘明文，支持 keyring / env / vault）
- 每适配器独立的配置 struct，但共享解析与验证逻辑

#### 3.2.8 异步事件分发

`TradingGateway` 的 callback 需要安全的事件分发机制：

- **事件总线**：行情更新、成交回报、订单状态变更统一分发
- **mpsc channel**：避免回调中阻塞连接线程
- **背压控制**：慢消费者不能拖垮整个网关（复用 `InChanCapacity` 模式）
- **多订阅者**：同一行情可以被多个消费者订阅（fan-out）

#### 3.2.9 时间处理

各券商时间戳格式完全不统一：

| 券商 | 时间戳格式 |
|------|-----------|
| IB | Unix epoch (秒, int) |
| Binance | epoch 毫秒 (int64) |
| 富途 | 字符串 `"2026-07-04 10:00:00"` |
| Alpaca | RFC 3339 字符串 |
| FIX | `YYYYMMDD-HH:MM:SS.sss` |

需要统一的 `MarketTimestamp` 类型 + 各适配器 `from_broker()` / `to_broker()` 转换。内部统一使用 UTC。

#### 3.2.10 测试基础设施

- **Mock TradingGateway**：实现 `TradingGateway` trait 的 mock，用于单元测试和 Phase 4 回测
- **Broker Simulator**：轻量级模拟服务器，用于集成测试（不需要真实 TWS / OpenD / Binance testnet）
- **录制 / 回放**：录制真实券商交互，离线回放验证
- **共享测试断言**：统一的订单/成交/行情断言 helper，所有适配器测试复用

#### 3.2.11 公司行为处理 (Corporate Actions)

股票拆合股、分红、供股、窝轮/牛熊证到期或收回，都会让"合约元数据"以外的运行时状态失效——
持仓数量、成本价、乃至合约本身是否还能交易。这不是某一个适配器的问题，而是所有持有股票/衍生品
仓位的适配器共同需要的能力，因此列为基础能力，具体数据结构由 3.11.5 的 `CorporateAction` 定义：

- **持仓自动调整**：`ex_date` 当天按 `Split`/`Consolidation` 的 `ratio` 调整 `Position.quantity` 与 `avg_price`
- **分红入账**：`CashDividend` 触发 `AccountInfo.balance` 的现金入账事件
- **合约失效处理**：`Delisting` / `WarrantExpiry` / `CbbcMandatoryCall` 触发持仓强制平仓或标记为待处理
- **回测复权**：4.6 回测引擎在历史数据回放时，必须应用拆合股复权，否则收益率会因价格跳变而失真
- **事件来源**：优先使用 3.11 `truefix-instruments` 的 `corporate_actions()` 查询结果；部分券商（如 IB）也会在持仓查询中直接返回调整后数量，需要去重避免重复调整

#### 3.2.12 行情数据授权与费用 (Market Data Entitlements)

多数交易所的实时行情不是免费的，尤其是港股 / 美股的 Level 2 十档深度行情、期权行情 (OPRA)。
未订阅时通常只能拿到 15–20 分钟延迟数据。这不是"钱"本身的问题——是**数据本身可能是错的假象**：
如果系统把延迟行情当实时行情喂给策略或 LLM Agent，产生的交易决策会基于过期价格，属于隐蔽的
系统性风险，必须在数据模型层面显式区分。

| 券商 / 数据源 | 行情类型 | 授权要求 | 典型费用量级 |
|---|---|---|---|
| IB | 美股 Level 1 (NBBO) | 需在账户管理里订阅交易所数据包，区分"专业 / 非专业投资者"身份 | 非专业免费或每月数美元；专业投资者更高 |
| IB | 美股期权行情 (OPRA) | 需订阅 OPRA 包 | 非专业约每月 $1.5 起，专业投资者显著更高，部分按连接数计费 |
| 富途 OpenD | 港股 Level 2 十档 | 需综合账户资质 + 付费订阅 | 每月约 HK$100+ |
| 老nn虎 / 长桥 | 港 / 美股 Level 2 | 同上，需付费订阅 | 视套餐 |
| Binance | 加密现货 / 合约行情 | 免费，无需订阅 | 免费（仍受 API 限流约束，见 3.2.5） |

**设计要点**：

- `MarketData` 模型增加 `is_delayed: bool` 与 `entitlement_level` 字段，行情消费方（策略、风控、Agent）
  必须能区分数据是否延迟，而不是默认信任
- 凭证配置（3.2.7）中每个适配器声明**已订阅的行情等级**：

```rust
pub struct MarketDataEntitlement {
    pub exchange: String,
    pub level: DataLevel,              // Level1 / Level2 / Opra / DelayedOnly
    pub subscriber_status: SubscriberStatus, // Professional / NonProfessional
}
```

- 连接时校验：若请求订阅的 symbol 超出已声明的授权范围，按配置决定是**降级为延迟数据**还是
  **直接报错拒绝**，默认应选择后者（宁可报错，也不要让未授权数据悄悄流入系统）
- TrueFix 本身不处理计费与合规声明（那是用户和券商之间的合同关系），但必须让开发者在配置阶段
  就清楚看到"这个数据源要花钱"、"这个数据是延迟的"，避免生产环境因为忽略这一层而产生意外账单
  或基于过期数据做出交易决策



```
统一数据模型 (3.2.1) ──┐
Symbol 映射 (3.2.2)   ──┤
错误模型 (3.2.6)       ──┼──▶  TradingGateway trait (3.6)
时间处理 (3.2.9)       ──┤         │
事件分发 (3.2.8)       ──┘         │
                              ▼
连接管理 (3.2.3) ──┐         各适配器实现 (3.3–3.7)
Order ID (3.2.4) ──┤         ├── FIX (标准券商)
限流 (3.2.5)     ──┤         ├── Binance FIX
凭证管理 (3.2.7) ──┘         ├── IB TWS
                              ├── Futu OpenD
测试设施 (3.2.10) ─────────── ├── Alpaca
公司行为处理 (3.2.11) ─────── ├── ...（依赖 3.11 truefix-instruments 提供事件源）
行情授权声明 (3.2.12) ─────── └── 所有需订阅实时行情的适配器
```

### 3.3 协议适配器架构

```
                    ┌─────────────────────────────────────┐
                    │         Unified Trading API          │
                    │  (trait: place_order / cancel_order  │
                    │   / query_position / subscribe_md)   │
                    └──────────────┬──────────────────────┘
                                   │
              ┌────────┬───────────┼───────────┬────────────┐
              │        │           │           │            │
     ┌────────▼──┐ ┌───▼────┐ ┌───▼────┐ ┌────▼─────┐ ┌───▼──────┐
     │ FIX 适配器│ │ IB TWS │ │ OpenD  │ │ Binance  │ │ 交易所   │
     │ (已有)    │ │ 适配器  │ │ 适配器 │ │ FIX 适配 │ │ 原生协议 │
     └───────────┘ └────────┘ └────────┘ └──────────┘ └──────────┘
```

#### 新增 crate: `truefix-gateway`

```
crates/truefix-gateway/
├── Cargo.toml
├── src/
│   ├── lib.rs              # UnifiedTradingApi trait + Gateway 抽象
│   ├── order.rs            # 统一订单模型 (UnifiedOrder): 跨协议订单表示
│   ├── execution.rs        # 统一回报模型 (ExecutionReport)
│   ├── market_data.rs      # 统一行情模型 (Quote / Trade / DepthBook)
│   ├── position.rs         # 统一持仓模型
│   ├── account.rs           # 统一账户模型
│   └── adapters/
│       ├── mod.rs           # BrokerAdapter trait
│       ├── fix.rs           # FIX 协议适配器 (包装 truefix-session, 标准 FIX 券商通用)
│       ├── ib_tws.rs        # Interactive Brokers TWS API 适配器
│       ├── opend.rs         # 富途 OpenD API 适配器
│       └── binance_fix.rs   # Binance FIX API 适配器
└── tests/
    ├── fix_adapter.rs       # FIX 适配器: 标准 FIX 券商下单/撤单/行情
    ├── ib_tws_adapter.rs    # IB TWS: 下单/撤单/账户查询
    ├── opend_adapter.rs     # OpenD: 下单/撤单/港股行情
    └── binance_fix_adapter.rs # Binance: 现货下单/撤单/深度行情
```

### 3.4 IB TWS API 适配器

Interactive Brokers 使用**私有 TCP 协议**（非 FIX），通过 TWS（Trader Workstation）桌面端或
IB Gateway 作为本地代理转发。

#### 协议特点

- **传输**: TCP + 自定义二进制帧（消息长度前缀 + 消息体）
- **消息格式**: 每条消息由 message ID (int) + 按版本号排列的字段序列组成
- **认证**: API key + TWS/IBG 本地握手（`startApi` / `connect`）
- **限制**: 单连接并发请求受限；部分消息有频率限制；需要本地运行 TWS 或 IB Gateway

#### 核心消息映射

| IB API 消息 | 统一 API 方法 | 说明 |
|-------------|--------------|------|
| `reqMktData` | `subscribe_market_data(symbol)` | 订阅实时行情（Level 1 / Level 2） |
| `reqMktDepth` | `subscribe_order_book(symbol, depth)` | 订阅深度行情 |
| `placeOrder` | `place_order(UnifiedOrder)` | 下单（支持股票/期权/期货/外汇） |
| `cancelOrder` | `cancelOrder(order_id)` | 撤单 |
| `reqOpenOrders` | `query_open_orders()` | 查询当日未成交订单 |
| `reqAccountUpdates` | `subscribe_account()` | 订阅账户余额与持仓变化 |
| `reqHistoricalData` | `query_historical_bar(...)` | 查询历史 K 线 |
| `reqPositions` | `query_positions()` | 查询所有持仓 |
| `reqContractDetails` | `query_instrument(symbol)` | 查询合约信息 |

#### 实现要点

- 需实现 IB 私有协议的编解码（消息帧解析、字段序列化/反序列化）
- 管理 orderId 单调递增序列（需持久化，跨重启恢复）
- 处理 TWS/IBG 连接断开与重连
- IB Gateway 无头模式优先（无需 GUI）
- 支持多账户切换（Financial Advisor 模式）

### 3.5 富途 OpenD API 适配器

富途/moomoo OpenAPI 通过 **OpenD** 本地网关代理，使用 **Protobuf (GPB)** 协议通信。

#### 协议特点

- **传输**: TCP + Protobuf 序列化
- **消息格式**: `[4字节长度][2字节protocol_type][2字节proto_id][protobuf_body]`
- **认证**: OpenD 本地连接 + 富途账号授权（在 OpenD 启动时登录）
- **限制**: OpenD 需本地运行；部分接口有频率限制；港股/A股/美股行情权限需开通

#### 核心消息映射

| OpenD 接口 (proto_id) | 统一 API 方法 | 说明 |
|----------------------|--------------|------|
| 3004 `PlaceOrder` | `place_order(UnifiedOrder)` | 下单（港股/美股/A股） |
| 3005 `ModifyOrder` | `modify_order(...)` | 改单 |
| 3006 `CancelOrder` | `cancelOrder(order_id)` | 撤单 |
| 3007 `QueryOrderList` | `query_orders(...)` | 查询订单 |
| 3008 `QueryDealList` | `query_deals(...)` | 查询成交 |
| 3009 `QueryHistory` | `query_historical_bar(...)` | 查询历史 K 线 |
| 3102 `StockQuoteSubscription` | `subscribe_market_data(symbol)` | 订阅实时行情 |
| 3103 `OrderBookSubscription` | `subscribe_order_book(symbol)` | 订阅深度行情 |
| 3201 `QueryAccount` | `query_account()` | 查询账户资金 |
| 3202 `QueryPosition` | `query_positions()` | 查询持仓 |

#### 实现要点

- 使用 `prost` crate 生成 Protobuf 消息类型（从富途提供的 `.proto` 文件）
- 管理 OpenD 连接生命周期（连接/断线重连/心跳）
- 证券代码映射（统一 symbol → 富途 market + code 格式）
- 港股/美股/A股交易规则差异处理（最小交易单位、T+0/T+1、价格档位）

### 3.6 Binance FIX API 适配器

Binance 于 2024 年推出 FIX API，使用 **FIX 4.4** 协议，支持现货交易和行情订阅。

#### 协议特点

- **传输**: TCP + TLS（需 stunnel 或原生 TLS），标准 FIX 4.4 SOH 编码
- **认证**: Logon 消息携带 API Key（`Username`）+ API Secret（`Password`），Binance 返回 session token
- **端点**:
  - 生产: `fix-oe.binance.vision:9000`（订单录入）、`fix-md.binance.vision:9000`（行情）
  - 测试: `fix-oe.testnet.binance.vision:9000`（测试网）
- **限制**: 每连接需指定 session type（OrderEntry / DropCopy / MarketData）
- **特殊**: Binance 的 FIX 字典有自定义扩展（如 `9921` ExchangeOrderID）

#### 核心消息映射

| Binance FIX 消息 | 标准 FIX MsgType | 统一 API 方法 | 说明 |
|-----------------|-----------------|--------------|------|
| NewOrderSingle | `D` | `place_order(UnifiedOrder)` | 下单（限价/市价/IOC/FOK） |
| OrderCancelRequest | `F` | `cancelOrder(...)` | 撤单 |
| OrderStatusRequest | `H` | `query_order(...)` | 查询订单状态 |
| MarketDataRequest | `V` | `subscribe_market_data(...)` | 订阅行情（深度/Ticker） |
| ExecutionReport | `8` | (回调) ExecutionReport | 成交回报 |
| OrderCancelReject | `9` | (回调) | 撤单拒绝 |
| BusinessMessageReject | `j` | (回调) | 业务拒绝 |

#### 实现要点

- 基于 TrueFix 的 FIX 引擎（`truefix-session` + `truefix-transport`），配置 Binance 自定义字典
- Logon 认证流程: 发送 `Username=API_Key`, `Password=API_Secret`，Binance 返回 `788` (NextExpectedMsgSeqNum) + session 确认
- Binance 要求使用 TLS，需完成 Phase 1 S8（rustls TLS 支持）后方可对接
- 行情订阅: `MarketDataRequest` 支持 `SubscriptionRequestType=1` (Snapshot + Updates)，通过 `MarketDataIncrementalRefresh` 接收增量更新
- 自定义字段注册: 将 Binance 扩展字段加入 runtime DataDictionary

#### 3.6.1 FIX 消息示例 (`examples/binance-fix/`)

为便于验证协议实现、并作为集成测试的黄金样例，新增示例目录，收录 Binance FIX API 的典型报文
（下方 SOH 分隔符 `\x01` 以 `|` 表示，便于阅读；字段值均为脱敏示例）：

**Logon (35=A)**
```
8=FIX.4.4|9=112|35=A|34=1|49=CLIENT_ID|52=20260101-00:00:00.000|56=SPOT|98=0|108=30|141=Y|553=API_KEY|25035=2|10=000|
```

**NewOrderSingle (35=D) — 限价买单**
```
8=FIX.4.4|9=178|35=D|34=2|49=CLIENT_ID|52=20260101-00:00:01.000|56=SPOT|11=cOid001|55=BTCUSDT|54=1|60=20260101-00:00:01.000|38=0.001|40=2|44=65000.00|59=1|10=000|
```

**ExecutionReport (35=8) — 成交回报**
```
8=FIX.4.4|9=210|35=8|34=3|49=SPOT|52=20260101-00:00:01.500|56=CLIENT_ID|37=broker-order-1|11=cOid001|17=exec-1|150=F|39=2|55=BTCUSDT|54=1|38=0.001|44=65000.00|32=0.001|31=65000.00|151=0|14=0.001|6=65000.00|10=000|
```

**MarketDataRequest (35=V) — 订阅深度行情**
```
8=FIX.4.4|9=140|35=V|34=4|49=CLIENT_ID|52=20260101-00:00:02.000|56=MARKETDATA|262=mdReq001|263=1|264=5|265=0|267=2|269=0|269=1|146=1|55=BTCUSDT|10=000|
```

用途：
- 作为 `truefix-binary` / `truefix-gateway` / `truefix-binance-client` 的集成测试黄金样例（byte-level fixture）
- 作为文档，帮助新贡献者快速理解 Binance FIX 字典与标准 FIX 4.4 的差异（如 `56=SPOT`/`56=MARKETDATA` session target、`25035` 等自定义字段）
- 覆盖 Order Entry、Market Data、Drop Copy 三类 session 的典型报文（后续按 session 类型扩充）

### 3.7 统一交易 API 设计

```rust
/// 统一交易接口 — 屏蔽底层券商协议差异
#[async_trait]
pub trait TradingGateway: Send + Sync {
    /// 下单
    async fn place_order(&self, order: &UnifiedOrder) -> Result<OrderAck, GatewayError>;

    /// 撤单
    async fn cancel_order(&self, order_id: &str) -> Result<(), GatewayError>;

    /// 改单
    async fn modify_order(&self, order_id: &str, modifications: &OrderModification)
        -> Result<OrderAck, GatewayError>;

    /// 查询订单状态
    async fn query_order(&self, order_id: &str) -> Result<OrderStatus, GatewayError>;

    /// 查询未成交订单
    async fn query_open_orders(&self) -> Result<Vec<OrderStatus>, GatewayError>;

    /// 查询持仓
    async fn query_positions(&self) -> Result<Vec<Position>, GatewayError>;

    /// 查询账户资金
    async fn query_account(&self) -> Result<AccountInfo, GatewayError>;

    /// 订阅实时行情
    async fn subscribe_market_data(
        &self,
        symbols: &[&str],
        callback: Box<dyn Fn(MarketData) + Send + Sync>,
    ) -> Result<SubscriptionId, GatewayError>;

    /// 退订行情
    async fn unsubscribe_market_data(&self, sub_id: SubscriptionId) -> Result<(), GatewayError>;

    /// 订阅成交回报
    async fn subscribe_executions(
        &self,
        callback: Box<dyn Fn(ExecutionReport) + Send + Sync>,
    ) -> Result<SubscriptionId, GatewayError>;
}
```

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 3a | 基础能力: 统一数据模型 + Symbol 映射 + 错误模型 + 时间处理 + 事件分发 + Mock TradingGateway | 所有 trait/struct 定义完成；Mock 实现通过单元测试 |
| 3b | 基础能力: 连接管理 + Order ID 持久化 + 限流 + 凭证配置(含行情授权声明 3.2.12) + Broker Simulator | 连接状态机通过测试；限流令牌桶验证；配置解析正确；未授权行情请求按配置正确降级或拒绝 |
| 3c | FIX 适配器 (标准券商) | 任意标准 FIX 4.4 券商可下单/撤单/收行情 |
| 3d | Binance FIX 适配器 | Testnet 下单/撤单/行情订阅全通过 |
| 3e | IB TWS 适配器 | IB Gateway 无头模式下单/撤单/账户查询 |
| 3f | OpenD 适配器 | OpenD 下单/撤单/港股行情订阅 |

### 3.8 券商适配器 TODO 列表

> 以下券商均已确认提供测试/模拟环境。按 ROI 分批接入，每批可并行开发。

#### Tier 1 — 已在 roadmap 中，价值最高

| 券商 | 测试环境 | 协议 | 市场覆盖 | 阶段 |
|------|---------|------|---------|------|
| Binance | Testnet | FIX 4.4 | 加密现货/合约 | 3d |
| IB TWS | Paper Trading 账户 | 私有 TCP | 全球股/期权/期货/外汇 | 3e |
| 富途 OpenD | 模拟环境 | TCP + Protobuf | 港/美/A 股 | 3f |

#### Tier 2 — 测试完善 + 增量成本极低

| 券商 | 测试环境 | 协议 | 市场覆盖 | 阶段 | 说明 |
|------|---------|------|---------|------|------|
| Coinbase | Sandbox | FIX 4.4 / REST + WS | 加密现货 | 3g | 复用 FIX 适配器框架，与 Binance FIX 同构，成本极低 |
| Alpaca | Paper Trading API | REST + WebSocket | 美股 | 3h | API-first 设计，免费 paper key，接入最简单 |

#### Tier 3 — 测试完善 + 扩展覆盖面

| 券商 | 测试环境 | 协议 | 市场覆盖 | 阶段 | 说明 |
|------|---------|------|---------|------|------|
| 老虎证券 | 模拟盘 | REST + WebSocket | 港/美/澳/新加坡 | 3i | Futu 直接替代，API 风格接近 |
| 长桥证券 | Testnet | REST + WebSocket | 港/美/新/澳 | 3j | 有 Rust SDK，文档完善 |
| OKX | Demo Trading | REST + WebSocket | 加密现货/合约/期权 | 3k | 加密交易所补充 |

#### Tier 4 — 特定需求时接入

| 券商 | 测试环境 | 协议 | 市场覆盖 | 阶段 | 说明 |
|------|---------|------|---------|------|------|
| Saxo Bank | Sandbox | REST + WebSocket | 全球多资产 (外汇/股票/债券/商品/衍生品) | 3l | 欧洲市场，多资产路由 |
| TradeStation | Sandbox | REST + WebSocket | 美股/期货/期权 | 3m | 期权/期货 specialist |
| Charles Schwab | Sandbox | REST + Streamer | 美股全品种 | 3n | 2024 开放 API，含原 TD Ameritrade |
| IG Markets | Demo Account | REST + WebSocket | CFD/差价合约/全球指数 | 3o | CFD 为主 |
| Bybit | Testnet | REST + WebSocket | 加密衍生品 | 3p | 衍生品 specialist |

#### Tier 5 — A 股 (需券商配合，合规限制)

| 平台 | 测试环境 | 接入方式 | 说明 |
|------|---------|---------|------|
| QMT (miniQMT) | 部分券商提供模拟 | Python SDK + IPC | 国金/银河等支持，需通过券商席位 |
| 掘金量化 (MyQuant) | 模拟环境 | REST + Python | 研究+交易一体化，有回测 |

#### 已排除 (无公开测试环境)

~~WeBull~~、~~盈立证券~~、~~DEGIRO~~、~~Kraken~~、~~日本/韩国本土券商~~

> **与 3.11 的矛盾需注意**：3.11 的 Instruments 设计已把日股 / 韩股纳入统一模型（为未来做准备），
> 但当前 Tier 列表里没有专门的日本 / 韩国券商适配器，交易接入暂时是空的：
> - **日股**：可通过 **3.4 IB TWS 适配器**间接覆盖（IB 支持东京证券交易所直连），因此日股的元数据 +
>   交易在 IB 适配器完成后即可用，不需要单独排期
> - **韩股**：外资交易需注册投资者识别码 (IRC)，主流互联网券商 (IB / 富途 / 老虎) 均无面向零售的
>   KRX 直连通道，暂无可用测试环境。韩股的 `Instrument` 元数据（分档 tick 等）可以先在 3.11 落地，
>   但交易接入留空，待找到合规可行的本土券商 API（如韩国投资证券 OpenAPI）再排期，暂列入 Tier 4/5 观察

### 3.9 独立 Client SDK：`truefix-binance-client` / `truefix-twsapi-client` / `truefix-futu-client`

3.3–3.6 的适配器（`adapters/binance_fix.rs` 等）实现的是 `TradingGateway` trait，服务于
`truefix-gateway` 内部的统一抽象。但很多场景（写脚本验证行情、notebook 交叉验证、CLI 工具、
被其他项目复用）不需要整个网关抽象层，只需要一个**薄、直接、贴近原始协议**的 client。因此规划
三个独立 crate，适配器通过组合这些 client 来实现，而不是各自重复编解码逻辑。

#### `truefix-binance-client`

- 封装 Binance **FIX** 会话（复用 `truefix-session`）与 Binance **REST/WebSocket** API 两种接入方式
- 提供贴近原始语义的方法：`new_order()` / `cancel_order()` / `query_order()` / `subscribe_depth()` / `subscribe_trade()`
- 不做跨券商抽象转换，返回值直接是 Binance 原始字段结构（`serde` 反序列化），由上层决定是否转换为 `UnifiedOrder`
- `truefix-gateway::adapters::binance_fix` 内部通过组合本 client 实现

#### `truefix-twsapi-client`

- 封装 IB TWS/Gateway 私有协议的连接、认证、消息编解码
- 提供 `connect()` / `place_order()` / `req_mkt_data()` / `req_positions()` / `req_historical_data()` 等贴近官方 API 命名的方法
- 管理 IB 特有的 orderId 单调序列与 `nextValidId` 握手
- `truefix-gateway::adapters::ib_tws` 与 3.10 的 IB→FIX 桥接均组合本 client 实现

#### `truefix-futu-client`

- 封装 OpenD 的 Protobuf 帧协议（`prost` 生成的消息类型）
- 提供 `place_order()` / `subscribe_quote()` / `query_position()` 等方法，返回富途原始 proto 结构
- `truefix-gateway::adapters::opend` 内部通过组合本 client 实现

#### 设计原则

- **Client 层 = 协议正确性**：贴近官方文档，字段名/语义尽量与官方一致，便于对照调试
- **Gateway/Adapter 层 = 跨协议一致性**：统一模型、统一错误、统一 symbol
- 两层分离带来两个好处：(a) 只想用 Binance 原生能力的用户可以只依赖 `truefix-binance-client`，
  不必引入整个网关；(b) 网关适配器的实现和测试更聚焦——协议编解码正确性在 client 层验证，
  跨协议一致性在 adapter 层验证

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 3q | `truefix-binance-client` | 独立可用；FIX + REST/WS 双通道下单/行情验证通过 Testnet |
| 3r | `truefix-twsapi-client` | 独立可用；IB Gateway 无头模式下单/行情验证通过 Paper Trading |
| 3s | `truefix-futu-client` | 独立可用；OpenD 模拟环境下单/行情验证通过 |

### 3.10 FIX 协议桥接：IB 行情与交易 → FIX Server (`truefix-ib-fix-bridge`)

#### 背景与动机

3.4 的 IB TWS 适配器让 TrueFix（作为 FIX **client** 一侧的开发者）可以通过统一 API 访问 IB。
另一个常见需求是反方向的桥接：**把 IB 的行情和交易能力，以标准 FIX Server 的形式暴露出去**，
这样任何标准 FIX buy-side 系统（QuickFIX/J、其他厂商 EMS/OMS）都可以像连接普通 FIX 券商一样
连接 TrueFix，而 TrueFix 在背后把 FIX 请求转发给 IB TWS/Gateway，并把 IB 的行情/回报转换回
标准 FIX 报文回传。

典型场景：已有系统基于 FIX 协议构建（EMS/OMS 只认 FIX），但目标经纪商是 IB（无 FIX API，
只有私有 TCP 协议）；或需要给多个下游系统提供统一的 FIX 接入点，而底层实际连接的是 IB。

#### 架构

```
FIX Buy-side 客户端              truefix-ib-fix-bridge                 IB TWS / Gateway
(QuickFIX/J 等)                                                        (私有协议)
      │                                  │                                   │
      │  Logon(A)/NewOrderSingle(D) ────▶│                                   │
      │                                  │── ib-client::place_order() ─────▶│
      │                                  │                                   │
      │◀────── ExecutionReport(8) ───────│◀── execDetails / orderStatus ─────│
      │                                  │                                   │
      │  MarketDataRequest(V) ───────────▶│── req_mkt_data() ────────────────▶│
      │◀ MarketDataSnapshot/Incremental ─│◀── tickPrice / tickSize ─────────│
```

#### 实现要点

- **FIX Server 端**：复用 `truefix-session` + `truefix-transport`，以 Acceptor（而非 Initiator）角色运行，接受下游 FIX 客户端的 Logon
- **协议转换层**：`NewOrderSingle(D)` → `truefix-twsapi-client::place_order()`；IB 的 `execDetails`/`orderStatus`
  回调 → 标准 `ExecutionReport(8)`；IB 的 `tickPrice`/`tickSize` → `MarketDataSnapshotFullRefresh(W)` / `MarketDataIncrementalRefresh(X)`
- **会话隔离**：下游 FIX client 的 session（`SenderCompID`/`TargetCompID`）与到 IB 的单一底层连接解耦，支持多个下游 session 共享同一个 IB 连接（多路复用），需处理鉴权与额度隔离
- **Symbol 映射复用**：直接复用 3.11 统一 Instruments 的 `SymbolMapper`，FIX `Symbol(55)` ⇄ IB `conId`
- **限流传导**：下游 FIX 请求速率需按 3.2.5 的 IB 限流规则整形，避免下游突发请求打爆 IB 连接
- **模式可复用**：该桥接模式（`XxxClient` → FIX Server）设计为通用模式，未来可复制到 Futu（`truefix-futu-fix-bridge`）等其他无 FIX 原生支持的券商

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 3t | `truefix-ib-fix-bridge` 核心转换层 | NewOrderSingle → IB 下单 → ExecutionReport 全链路打通 |
| 3u | 行情桥接 | MarketDataRequest → IB tick 数据 → 标准 FIX 行情报文 |
| 3v | 多下游 session 复用单一 IB 连接 | 2+ 下游 FIX client 同时接入，互不干扰 |

### 3.11 统一 Instruments 服务 (`truefix-instruments`)

3.2.2 定义了 `Instrument` 模型与 `SymbolMapper` trait 作为跨适配器的基础能力。随着接入券商增多
（Tier 1–5 共 15+ 家），symbol 映射、合约元数据、交易日历若继续分散在各适配器中维护会迅速失控，
因此将其从"基础能力"升级为独立 crate，提供统一的**合约主数据服务**，并覆盖 TrueFix 需要交易的
全部资产类别：**加密货币（现货/永续/交割）、港股/美股/日股/韩股/新加坡股票、期权、期货、
窝轮/涡轮、牛熊证(CBBC)、杠杆及反向产品**等。

#### 设计难点

不同资产类别、不同市场的合约规则差异极大，一个扁平的 `Instrument` struct（3.2.1 中 `security_type`
最初只是个宽泛分类）无法承载：期权需要 **到期日 + 行权价 + Call/Put** 三元组才能唯一定位；期货需要
合约月份与展期规则；港股窝轮/牛熊证有发行商、换股比率、收回价，与股票是完全不同的字段集合；
加密货币永续合约有资金费率但股票没有。因此 `truefix-instruments` 采用**分层模型**：通用字段放在
`Instrument` 顶层，资产类型特有字段放进 `SecurityType` 枚举的各个 variant。

#### 3.11.1 资产类型分类体系

| 大类 | `SecurityType` variant | 覆盖市场 / 来源 |
|---|---|---|
| 股票 | `Equity` | 美股 / 港股 / 日股 / 韩股 / 新加坡股 / A股 |
| ETF / 杠杆反向产品 | `Etf` / `LeveragedInverse` | 各市场 ETF；2x/3x 杠杆及反向产品 (L&I Products) |
| 加密货币现货 | `CryptoSpot` | Binance / OKX / Coinbase 等 |
| 加密货币衍生品 | `CryptoPerpetual` / `CryptoFuture` | 永续合约（有资金费率）/ 交割合约 |
| 期权 | `Option` | 股票期权 / 指数期权 / 期货期权 |
| 期货 | `Future` | 商品 / 指数 / 利率期货 |
| 窝轮 / 涡轮（衍生权证） | `Warrant` | 港股 / 新加坡市场主流衍生品 |
| 牛熊证 (CBBC) | `Cbbc` | 港股特有，Callable Bull/Bear Contract |
| 外汇 / 差价合约 | `Forex` / `Cfd` | Saxo / IG 等 Tier 4 券商 |
| 债券 | `Bond` | 长期规划，暂不实现，预留 variant |

#### 3.11.2 核心数据模型

```rust
pub struct Instrument {
    pub id: InstrumentId,                  // 内部全局唯一主键
    pub unified_symbol: UnifiedSymbol,     // exchange (MIC code) + local_code + security_type 标签
    pub isin: Option<String>,              // 跨市场规范标识，能拿到就填，作为跨券商映射的兜底锚点
    pub name: String,
    pub currency: Currency,                // 计价货币（可能与账户结算货币不同，见 3.11.7）
    pub tick_regime: TickRegime,           // 单一 tick 或分档 tick（韩股）
    pub lot_size: Decimal,                 // 每手数量（港股逐票不同，必须按 instrument 存，不能按市场存）
    pub min_qty: Decimal,
    pub status: InstrumentStatus,          // Active / Suspended / Delisted / Expired / Called(牛熊证收回)
    pub details: SecurityType,             // 资产类型特有字段
}

pub enum TickRegime {
    Fixed(Decimal),
    Tiered(Vec<(PriceRange, Decimal)>),    // 分档 tick，如韩股 호가단위 7 档
}

pub enum SecurityType {
    Equity { board_lot: Decimal, is_odd_lot_eligible: bool, sedol: Option<String>, par_value: Option<Decimal> },
    Etf { tracking_index: String },
    LeveragedInverse { underlying_index: String, leverage_ratio: Decimal, rebalance_freq: RebalanceFrequency },
    CryptoSpot { base_asset: String, quote_asset: String, step_size: Decimal },
    CryptoPerpetual { base_asset: String, quote_asset: String, funding_interval_hours: u8, max_leverage: u32 },
    CryptoFuture { base_asset: String, quote_asset: String, delivery_date: NaiveDate, max_leverage: u32 },
    Option {
        underlying: UnifiedSymbol, strike: Decimal, expiry_date: NaiveDate,
        right: OptionRight,                // Call / Put
        exercise_style: ExerciseStyle,     // American / European
        multiplier: Decimal, settlement: SettlementType, // Cash / Physical
    },
    Future { underlying: String, contract_month: String, expiry_date: NaiveDate, multiplier: Decimal, settlement: SettlementType },
    Warrant {                              // 窝轮 / 涡轮
        underlying: UnifiedSymbol, issuer: String, right: OptionRight,
        strike: Decimal, expiry_date: NaiveDate, conversion_ratio: Decimal,
    },
    Cbbc {                                 // 牛熊证
        underlying: UnifiedSymbol, issuer: String, category: CbbcCategory, // N(无残值) / R(有残值)
        call_price: Decimal, strike: Decimal, expiry_date: NaiveDate,
        conversion_ratio: Decimal, called_at: Option<DateTime<Utc>>,
    },
}
```

#### 3.11.3 市场特定规则一览

| 市场 / 资产 | 关键差异点 |
|---|---|
| 美股 | Tick 固定 `$0.01`（价格 ≥ $1）/ `$0.0001`（< $1）；2024 年起 T+1 结算；独立盘前 / 盘后 session |
| 港股 | 每手股数 (board lot) 逐票不同，非全市场统一值；窝轮 / 牛熊证日增 / 日到期量极大 |
| 日股 | 単元株 (unit share) 多数 100 股但非全部统一；2023 年东交所改版后 tick 按价格分档 |
| 韩股 | 호가단위（报价单位）按价格分 7 档；涨跌停 ±30%；外资交易需注册投资者识别码 (IRC) |
| 新加坡股 | Board lot 多数 100 股，部分蓝筹为 1 股（2015 年新交所改革后） |
| 加密货币 | 无涨跌停 / board lot，但有 `tick_size` + `step_size`（最小下单量精度）；永续合约有资金费率，同一 base symbol 在不同交易所合约乘数可能不同 |
| 期权 | 唯一定位需 到期日 + 行权价 + Call/Put 三元组；美式 / 欧式行权方式影响提前行权逻辑；结算方式（现金 / 实物）决定到期处理 |
| 期货 | 合约月份代码（如 `ESH6` = 2026 年 3 月）；展期 (roll) 规则；回测需要连续合约拼接（back-adjusted / ratio-adjusted） |
| 窝轮 / 涡轮 | 发行商 (issuer) + 换股比率 (conversion ratio) + 到期日；到期作废，需按日增量刷新（港股常年有数千只在市） |
| 牛熊证 (CBBC) | 收回价 (call price) 触发日内强制收回 (knock-out)，`status` 需近实时更新，不能只靠日频同步 |

#### 3.11.4 交易日历：多 Session 支持

单一 `trading_hours` 返回值无法表达盘前 / 盘后、午间休市（港/日/韩/新股共有）、期货夜盘，因此改为
返回一天内的多个 session：

```rust
pub enum SessionType { PreMarket, Continuous, LunchBreak, PostMarket, NightSession, AuctionOpen, AuctionClose }

pub struct TradingSession { pub session_type: SessionType, pub start: NaiveTime, pub end: NaiveTime }
```

#### 3.11.5 合约生命周期 / 公司行为 (Corporate Actions)

股票拆合股、分红、供股会改变持仓数量与成本；窝轮 / 牛熊证到期或被收回会使合约直接失效；期货临近
到期需要展期提醒。这些事件必须主动推送给持仓管理与回测模块，而不只是"合约元数据更新"：

```rust
pub enum CorporateAction {
    Split { ratio: Decimal, ex_date: NaiveDate },               // 拆股
    Consolidation { ratio: Decimal, ex_date: NaiveDate },       // 合股
    CashDividend { amount: Decimal, currency: Currency, ex_date: NaiveDate },
    RightsIssue { ratio: Decimal, subscription_price: Decimal, ex_date: NaiveDate },
    SymbolChange { new_symbol: UnifiedSymbol, effective_date: NaiveDate },
    Delisting { effective_date: NaiveDate },
    WarrantExpiry { expiry_date: NaiveDate },
    CbbcMandatoryCall { call_date: DateTime<Utc>, residual_value: Option<Decimal> }, // 牛熊证收回
}
```

> 联动 3.2：建议新增 **3.2.11 公司行为处理** 作为基础能力——持仓管理 (`Position`) 需在 `ex_date`
> 按 `ratio` 自动调整数量与成本价，账户模块需处理现金分红入账，回测引擎（4.6）需在历史数据回放时
> 应用拆合股复权，否则回测结果会因未复权而失真。

#### 3.11.6 按资产类别的数据同步策略

不同资产类别的合约数量级和变化频率差异极大，"一刀切"的定时全量同步不现实：

| 资产类别 | 规模量级 | 变化频率 | 同步策略 |
|---|---|---|---|
| 股票 / ETF | 数千~数万 | 低（日频） | 每日全量同步 + 公司行为增量推送 |
| 期货 | 数百 | 中（合约按月到期） | 每日同步 + 展期日历提前预警 |
| 期权 | 数十万（组合爆炸） | 高 | 不预加载，按需查询（underlying 触发时拉取对应 chain），短 TTL 缓存 |
| 窝轮 / 涡轮 / 牛熊证 | 数千~数万，日增日减 | 极高 | 独立高频同步管道（建议 ≤1 小时一次）；`status` 字段近实时更新（收回事件） |
| 加密货币 | 数百~数千 | 中（新币上线频繁） | 每日同步 + 交易所 symbol 变更 webhook（如支持） |

#### 3.11.7 多币种计价、结算与汇率服务

同一账户可能同时持有 HKD 计价的港股窝轮、USD 计价的美股、JPY 计价的日股、KRW 计价的韩股与加密
货币（USDT/USD 计价）。`Instrument.currency`（计价货币）与账户的结算货币（`AccountInfo.currency`）
经常不一致，因此 `Position` 的浮动盈亏（`unrealized_pnl`）计算需要一层 FX 转换。

**汇率来源需要按用途分优先级，不能只接一个源就当作通用汇率使用**：

| 用途 | 推荐来源 | 原因 |
|------|---------|------|
| 下单 / 实际结算 | 券商原生汇率（如 IB 的 IdealPro 外汇报价） | 必须与实际成交/入账一致，第三方估算值可能和券商实际结算价有点差，用于下单会导致资金计算错误 |
| 实时持仓 PnL 展示 | 券商原生汇率（已连接）优先；未连接对应券商时降级为第三方 FX API | 容忍几秒~几分钟延迟，用 3.2.8 事件分发定时刷新缓存汇率即可 |
| 离线 / 冷启动展示 | 自维护的定时快照汇率表 | 兜底用途，精度较低，仅用于粗略展示，不能用于任何下单或结算路径 |
| 回测复权到统一货币 | 历史汇率时间序列，与历史行情数据同批次存储 | 保证回测可复现；用当前实时汇率去折算历史仓位会引入未来函数 (look-ahead bias) |

```rust
#[async_trait]
pub trait FxRateProvider: Send + Sync {
    /// 查询即时汇率 (base → quote)
    async fn get_rate(&self, base: Currency, quote: Currency) -> Result<Decimal, FxError>;

    /// 查询历史汇率快照，用于回测复权到统一货币
    async fn get_historical_rate(&self, base: Currency, quote: Currency, date: NaiveDate) -> Result<Decimal, FxError>;
}
```

> **关键约束**：下单 / 结算路径**禁止**使用第三方估算汇率兜底——如果对应券商未连接、拿不到原生
> 汇率，应该直接拒绝下单而不是用估算值静默执行，否则实际扣款金额可能和系统展示的不一致。

#### 核心接口（更新）

```rust
#[async_trait]
pub trait InstrumentService: Send + Sync {
    async fn get_instrument(&self, symbol: &UnifiedSymbol) -> Result<Instrument, InstrumentError>;

    /// 期权链查询：给定标的 + 到期日范围，返回全部行权价 / Call/Put 组合
    async fn get_option_chain(&self, underlying: &UnifiedSymbol, expiry_range: (NaiveDate, NaiveDate))
        -> Result<Vec<Instrument>, InstrumentError>;

    fn map_to_broker(&self, symbol: &UnifiedSymbol, broker: BrokerId) -> Result<BrokerSymbol, InstrumentError>;
    fn map_from_broker(&self, symbol: &BrokerSymbol, broker: BrokerId) -> Result<UnifiedSymbol, InstrumentError>;

    fn trading_sessions(&self, exchange: &str, date: NaiveDate) -> Vec<TradingSession>;
    fn is_trading_day(&self, exchange: &str, date: NaiveDate) -> bool;

    /// 公司行为 / 合约生命周期事件（拆合股 / 分红 / 窝轮到期 / 牛熊证收回等）
    async fn corporate_actions(&self, symbol: &UnifiedSymbol, since: NaiveDate)
        -> Result<Vec<CorporateAction>, InstrumentError>;
}
```

#### 阶段目标（更新）

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 3w | `truefix-instruments` 核心模型（分层 `SecurityType`）+ 内存实现 | 覆盖 Equity / Crypto / Option / Future / Warrant / CBBC 全部 variant；单元测试覆盖跨券商映射 |
| 3x | 交易日历（多 session） | 美股盘前盘后、港/日/韩/新股午间休市、期货夜盘均正确返回 |
| 3y | 券商同步器（按资产类别分级） | 股票日频、期权按需、窝轮/牛熊证高频（≤1h）三种同步策略均验证通过 |
| 3z | 期权链 + 期货连续合约 | `get_option_chain` 返回完整链；期货连续合约拼接可用于回测 |
| 3z1 | 窝轮 / 牛熊证生命周期 | 到期 / 收回事件正确产生 `CorporateAction`；`status` 近实时更新 |
| 3z2 | 公司行为管理（联动 3.2.11） | 拆合股 / 分红事件正确调整持仓数量与成本价 |
| 3z3 | `FxRateProvider` 汇率服务 | 已连接券商的原生汇率优先生效；未连接时降级第三方源并明确标注；历史汇率与回测行情同批次可用 |

### 3.12 STEP 适配器（国内交易所数据交换协议）

#### 背景与协议定位

**STEP** 即中国金融行业标准 **JR/T 0022—2020《证券交易数据交换协议》**（代替 JR/T 0022—2014，
Securities Trading Exchange Protocol），由中国证监会发布，上交所/深交所/中证信息等参与起草，规定
证券交易所交易系统与市场参与者系统之间的数据交换协议。经核实标准原文（§5 会话机制、§6.2.5 域界定）：
STEP 的会话机制直接引用"FIX 标准会话机制"（该标准附录 B/C），消息格式是**与标准 FIX 字节级同构的
SOH 分隔 tag=value 协议**——消息以 `8=STEP.x.yz<SOH>` 开始、`10=nnn<SOH>` 结束，消息头前三个域固定
为 8/9/35，消息尾最后一个域固定为 10，校验和算法（从 tag 8 的 `8` 累加到 10 之前的 SOH，对 256 取模）
与 FIX 完全相同，重复组机制同构。域号 1–10000 由全国金融标准化技术委员会统一分配，10000 以上由连接
双方自行约定（即自定义域）。

因此 STEP **不是二进制协议**，与 Phase 2 的 FAST/SBE 没有技术交集；架构上它更接近"换了一个
`BeginString` 前缀、拥有国标字段字典的 FIX 方言"，实现路径应归入本 Phase 的适配器模式（复用
`truefix-session`/`truefix-transport` 现有的 SOH 编解码器 + 会话状态机，新增字典与会话 profile），
而不是新写编解码器。

#### 会话层：两种可选 profile

| Profile | 依据 | 特点 |
|---------|------|------|
| 标准 FIX 会话机制 | JR/T 0022—2020 附录 B/C | 等同于现有 `truefix-session` 已实现的 FIXT 会话逻辑（含真正的会话层重传/gap-fill），可直接复用 |
| **LFIXT**（轻量级实时 STEP 消息传输协议） | JR/T 0182—2020，单独标准 | 单一全双工 TCP 连接；仅在建会话（Logon）阶段用 `ResetSeqNumFlag`/`NextExpectedMsgSeqNum` 做序号同步；**没有真正的会话层重传**——序号缺口只能通过应用层自行恢复；不主动发送 TestRequest/ResendRequest；自定义标签 `1408 DefaultCstmApplVerID`（建议格式 `STEP版本号_市场代码_协议版本号`）、`1409 SessionStatus`（退登状态码，取值集合与标准 FIX 的 Logout 场景码不同，需单独在字典中声明） |

实际交易所网关采用哪种 profile 需逐家确认；已核实上交所"交易网关 STEP 接口规格说明书"与深交所
"STEP 交易数据接口规范"均在正文声明会话层兼容 LFIXT。

#### 交易所落地情况（已核实的公开资料）

| 交易所 | 文档 | 覆盖范围 | 备注 |
|--------|------|---------|------|
| 上交所 (SSE) | 《交易网关 STEP 接口规格说明书》 | 按业务条线分别成册，已获取的一份仅覆盖"互联网交易平台固收迁移"（债券/协议回购/询价报价/基金通等），100+ 页 | 全市场需要的业务条线数量未知，需逐条线确认是否有对应文档 |
| 深交所 (SZSE) | 《STEP 交易数据接口规范》 | 覆盖面更广的全市场接口规范（含期权集中竞价、借券还券、行权、备兑锁定解锁等），117 页 | 相对 SSE 更接近"单一入口"，如需优先接入一家，深交所文档更完整 |

两份交易所文档都是自 JR/T 0022 派生的**业务报文字典**（>10000 的自定义域 + 交易所私有消息类型），
体量远大于 STEP 会话层本身，是本节实现工作量的主要来源，而不是协议机制部分。

#### 实现要点

1. **`BeginString` 扩展识别**：`truefix-session`/`truefix-transport` 现有的 `BeginString` 校验逻辑
   需要接受 `STEP.x.yz` 作为合法协议标识，与 `FIX.x.y`/`FIXT.1.1` 并列，而不是报协议不匹配错误。
2. **`truefix-dict` 新增 STEP 命名空间**：国标域（1–10000）作为共享基础字典，各交易所的自定义域
   （10000+）与业务消息类型按交易所、按业务条线分别导入，不要求一次性覆盖全部条线。
3. **`truefix-session` 增加 LFIXT 会话 profile**：作为标准 FIXT 会话逻辑之外的一个可选、更简单的
   变体（无真正会话层重传，登录阶段做序号同步即可），本质是对现有会话状态机的裁剪而非新增状态机。
4. **按交易所 + 业务条线分批接入，不做"一次性全市场"承诺**：SSE 一个业务条线的接口文档就有 100+ 页，
   贸然承诺"接入 STEP"这种笼统目标不可执行，应先选定一家交易所的一个业务条线（建议 SZSE 的标准竞价
   交易，文档覆盖面广且相对通用）作为最小闭环。

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 3aa | STEP 字典接入 | 解析 JR/T 0022—2020 国标字段字典 + 深交所标准竞价交易业务报文子集，纳入 `truefix-dict` |
| 3ab | `BeginString`/会话 profile 扩展 | `truefix-session`/`truefix-transport` 识别 `STEP.x.yz`；实现 LFIXT 会话 profile（单 TCP、登录序号同步、无会话层重传），单元测试覆盖正常/异常登录场景（对照 JR/T 0182 附录 C 的场景用例） |
| 3ac | 单一交易所最小闭环 | 深交所（或已确认可用测试环境的交易所）标准竞价交易：登录/心跳/下单/回报全链路验证通过 |
| 3ad | 扩展业务条线 / 第二家交易所 | 按实际需求扩展上交所及其他业务条线（债券回购/询价报价/基金通等），每条线独立验收 |

---

## Phase 4: AI 交易 Agent

### 4.1 背景与动机

随着 LLM (大语言模型) 的成熟，AI 驱动的交易决策从传统量化策略扩展到自然语言推理、多模态
信号融合、以及基于市场新闻/公告/社交媒体的实时情绪分析。TrueFix 的定位是提供**可靠的执行层**，
让 AI Agent 专注于策略逻辑而非协议细节。

### 4.2 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                      AI Trading Agent                            │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐ │
│  │ LLM 决策 │  │ 量化策略 │  │ 信号聚合 │  │   风控护栏       │ │
│  │ 引擎     │  │ 引擎     │  │ 引擎     │  │ (Pre-trade Risk) │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────────┬─────────┘ │
│       └──────────────┴──────────────┘                 │           │
│                      │ TradingDecision                 │           │
│                      ▼                                 ▼           │
│              ┌──────────────────────────────────────────┐         │
│              │         Order Router                      │         │
│              │  (Smart Order Routing, 最佳执行)          │         │
│              └──────────────────┬───────────────────────┘         │
└─────────────────────────────────┼────────────────────────────────┘
                                  │
                    ┌─────────────▼──────────────┐
                    │    truefix-gateway          │
                    │  (统一交易 API, Phase 3)    │
                    └────────────────────────────┘
```

#### 新增 crate: `truefix-agent`

```
crates/truefix-agent/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Agent trait + AgentContext
│   ├── decision.rs         # TradingDecision 模型 (方向/数量/价格/时限/理由)
│   ├── strategy.rs         # Strategy trait (量化 + LLM 统一接口)
│   ├── llm/
│   │   ├── mod.rs          # LLM 调用 trait (支持 OpenAI / Anthropic / 本地模型)
│   │   ├── prompt.rs       # Prompt 模板构建 (市场上下文 + 历史决策 + 指令)
│   │   ├── parser.rs       # LLM 输出解析 → TradingDecision
│   │   └── memory.rs       # 决策记忆 (短期 + 长期上下文)
│   ├── signal/
│   │   ├── mod.rs          # Signal trait (新闻 / 行情指标 / 链上数据 / 社交情绪)
│   │   ├── news.rs         # 新闻情绪信号源
│   │   ├── technical.rs     # 技术指标信号源 (MA / RSI / MACD / 波动率)
│   │   └── social.rs        # 社交媒体情绪信号源
│   ├── risk/
│   │   ├── mod.rs          # RiskGuard trait
│   │   ├── pre_trade.rs     # 盘前风控 (最大持仓 / 单笔限额 / 日亏损限额)
│   │   ├── exposure.rs      # 敞口监控 (净头寸 / 希腊字母)
│   │   └── circuit_breaker.rs # 熔断机制 (异常波动自动停机)
│   ├── router/
│   │   ├── mod.rs          # OrderRouter trait
│   │   ├── smart.rs         # 智能路由 (多券商最优执行)
│   │   └── split.rs         # 拆单算法 (TWAP / VWAP / Iceberg)
│   └── backtest/
│       ├── mod.rs          # 回测引擎
│       ├── replay.rs       # 历史行情回放
│       └── report.rs       # 回测报告 (Sharpe / MaxDrawdown / WinRate)
└── tests/
    ├── llm_decision.rs     # LLM 决策流程: 市场 context → prompt → 解析 → TradingDecision
    ├── risk_guard.rs       # 风控拦截: 超限额订单被拒绝
    ├── smart_router.rs     # 智能路由: 多券商择优
    └── backtest.rs         # 回测: 历史数据 → 策略 → 报告
```

### 4.3 核心抽象

```rust
/// 交易决策
pub struct TradingDecision {
    pub action: Action,              // Buy / Sell / Close / Hold
    pub symbol: String,
    pub quantity: Decimal,
    pub order_type: OrderType,       // Market / Limit / Stop
    pub price: Option<Decimal>,      // 限价单价格
    pub time_in_force: TimeInForce,  // Day / GTC / IOC / FOK
    pub reason: String,              // 决策理由 (LLM 自然语言 / 策略 ID)
    pub confidence: f64,             // 置信度 0.0-1.0
    pub source: DecisionSource,      // LLM / QuantSignal / Manual
}

/// AI Agent trait
#[async_trait]
pub trait TradingAgent: Send + Sync {
    /// 接收市场数据更新
    async fn on_market_data(&self, data: MarketData) -> Result<()>;

    /// 接收成交回报
    async fn on_execution(&self, report: ExecutionReport) -> Result<()>;

    /// 生成交易决策 (由内部定时器或信号触发)
    async fn decide(&self, context: &AgentContext) -> Result<Vec<TradingDecision>>;

    /// 执行决策 (经过风控 + 路由)
    async fn execute(&self, decisions: Vec<TradingDecision>) -> Result<Vec<OrderAck>>;
}

/// 风控护栏
#[async_trait]
pub trait RiskGuard: Send + Sync {
    /// 盘前检查 — 返回 Ok 通过, Err 拒绝
    async fn check(&self, decision: &TradingDecision, positions: &[Position]) -> Result<(), RiskError>;

    /// 实时敞口监控
    async fn monitor(&self, positions: &[Position]) -> Result<ExposureReport, RiskError>;

    /// 熔断检查
    async fn circuit_break(&self, market_data: &MarketData) -> bool;
}
```

### 4.4 LLM 集成设计

#### Prompt 构建策略

```
[System]
You are a trading agent. Analyze market conditions and make trading decisions.
Always respond in JSON format matching TradingDecision schema.
Never recommend actions that violate the risk constraints provided.

[Context]
- Current positions: {positions}
- Account balance: {balance}
- Risk limits: max_position={max}, max_daily_loss={loss_limit}
- Current market data:
  {symbol}: price={price}, volume={vol}, change={chg}%
  Technical indicators: RSI={rsi}, MA20={ma20}, MACD={macd}
- Recent news sentiment: {news_sentiment}
- Recent decisions: {recent_decisions}

[Instruction]
Based on the above context, decide whether to trade. If trading, specify action, quantity, and price.
```

#### LLM 输出解析

- 强制 JSON schema 验证（使用 `serde_json` 反序列化为 `TradingDecision`）
- 置信度阈值过滤（低于阈值的决策转为 Hold）
- 多轮对话记忆（保留最近 N 条决策上下文）
- Fallback: LLM 调用失败时降级为纯量化策略

#### 模型支持

| 模型 | 接入方式 | 场景 |
|------|---------|------|
| OpenAI GPT-4o | HTTP API (`reqwest`) | 通用推理 |
| Anthropic Claude | HTTP API | 长上下文分析 |
| 本地模型 (Ollama / llama.cpp) | HTTP API | 低延迟、隐私敏感 |

### 4.5 智能订单路由

当对接多个券商时，Order Router 负责:

1. **最优价格路由**: 比较多券商报价，选择最优成交价
2. **流动性拆分**: 大单拆分为多笔小单，分发给不同券商
3. **成本优化**: 考虑手续费、滑点、汇率
4. **冗余路由**: 主券商故障时自动切换备用券商

#### 拆单算法

| 算法 | 说明 |
|------|------|
| TWAP | 将大单均匀分配到时间段内执行 |
| VWAP | 按历史成交量分布加权执行 |
| Iceberg | 只暴露小部分数量，隐藏真实意图 |
| POV | 按市场成交占比参与 |

### 4.6 回测引擎

- **数据回放**: 从历史行情文件/数据库回放 tick 级数据
- **策略隔离**: 同一 Agent 代码可运行于回测模式或实盘模式（通过 `AgentMode` 区分）
- **性能报告**: Sharpe ratio, Max drawdown, Win rate, Turnover, Slippage analysis
- **参数扫描**: 批量运行不同参数组合，输出最优配置

### 4.7 期权定价与风险指标引擎 (`truefix-pricing`)

#### 背景与动机

4.3 的 `RiskGuard::monitor` 提到"敞口监控（净头寸 / 希腊字母）"，但没有定义希腊字母 (Greeks) 从哪来。
只要账户持有期权仓位，Delta/Gamma/Vega/Theta 就是风控无法绕开的输入——而这些值有两个来源：券商
原生推送（如 IB 的 `modelGreeks`），或者自己用定价模型计算。两者精度、时效、覆盖范围都不同，
不能假设"总能拿到"。

#### 两种 Greeks 来源

| 来源 | 优点 | 局限 |
|------|------|------|
| **券商原生推送**（如 IB `modelGreeks`） | 与券商自身风控口径一致，无需自己建模；免去隐含波动率反解的数值误差 | 并非所有券商 / 所有期权都推送（如部分交易所只给最新价，不给 IV/Greeks）；不同券商的定价模型假设可能不一致 |
| **自算**（Black-Scholes-Merton + 隐含波动率反解） | 覆盖所有场景，不依赖券商是否推送 | 欧式期权用 BS 解析解，美式期权需二叉树近似；波动率反解本身是数值方法，存在收敛失败/精度问题；未建模股息、提前行权等因素会带来系统性偏差 |

**策略**：优先使用券商推送的 Greeks；缺失或不支持时降级到自算；返回结构必须显式标注来源
（`GreeksSource::Broker` / `Calculated`），避免风控模块把两种精度不同的数据当作同一等级使用。

#### 新增 crate: `truefix-pricing`

```
crates/truefix-pricing/
├── Cargo.toml
├── src/
│   ├── lib.rs              # PricingEngine trait
│   ├── black_scholes.rs    # BS 欧式定价 + Greeks 解析解
│   ├── binomial.rs         # 二叉树 (CRR)，用于美式期权近似
│   ├── implied_vol.rs      # 隐含波动率反解 (Newton-Raphson / 二分法)
│   └── greeks.rs           # 组合层面 Greeks 聚合
└── tests/
    ├── bs_pricing.rs        # BS 定价结果与已知解析解对比
    └── portfolio_greeks.rs  # 多腿期权组合 Greeks 汇总正确性
```

```rust
pub trait PricingEngine: Send + Sync {
    fn implied_volatility(&self, option: &OptionDetails, spot: Decimal, market_price: Decimal, risk_free_rate: Decimal)
        -> Result<Decimal, PricingError>;

    fn greeks(&self, option: &OptionDetails, spot: Decimal, vol: Decimal, risk_free_rate: Decimal) -> Greeks;
}

pub struct Greeks {
    pub delta: Decimal, pub gamma: Decimal, pub vega: Decimal, pub theta: Decimal, pub rho: Decimal,
    pub source: GreeksSource, // Broker | Calculated
}
```

#### 组合层面风险

单腿期权的 Greeks 需要按持仓数量与合约乘数汇总到组合层面（`portfolio_delta` / `portfolio_gamma` ...），
才能接入 4.3 `RiskGuard::monitor` 的敞口检测（如"组合 Delta 超过限额时预警或自动对冲"）。

> **长期规划（暂不实现）**：完整波动率曲面（不同行权价 / 到期日的 IV 网格）是期权做市或更复杂策略
> 的需求，超出当前 AI Agent 场景的必要范围，先标注为已知缺口，不纳入近期阶段目标。

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 4a | `truefix-agent` 核心 trait + TradingDecision 模型 | Agent trait 定义完成；风控 trait 定义完成 |
| 4b | LLM 决策引擎 | 给定市场 context，LLM 产出可执行的 TradingDecision (JSON 验证通过) |
| 4c | 风控护栏 | 超限额订单被拦截；熔断机制在异常波动时自动停机 |
| 4d | 智能路由 | 多券商择优执行；TWAP/VWAP 拆单 |
| 4e | 回测引擎 | 历史 tick 数据回放 → 策略执行 → 性能报告 |
| 4f | 端到端 Demo | LLM Agent → 风控 → 路由 → Binance Testnet 下单 → 成交回报 → 闭环 |
| 4g | `truefix-pricing` 核心：BS 定价 + 隐含波动率反解 | 与已知期权定价参考实现（如 py_vollib）结果误差 < 0.1% |
| 4h | 券商原生 Greeks 优先 + 自算 fallback | IB `modelGreeks` 优先使用；不支持的资产自动降级自算，来源标注正确 |
| 4i | 组合层面 Greeks 聚合接入 RiskGuard | 多腿期权组合净 Delta/Gamma 正确汇总，敞口超限触发预警 |

---

## Phase 5: 插件化架构 — WASM 插件系统

### 5.1 背景与动机

Phase 3/4 的适配器、策略、信号源都是编译进主二进制的 Rust 代码，这带来两个问题：

1. **扩展成本高**：新增一个券商适配器或一个策略需要修改 TrueFix 源码、重新编译、重新发布
2. **信任边界模糊**：第三方贡献的适配器/策略与核心引擎运行在同一进程、同一权限下，缺乏隔离

WebAssembly 插件系统可以解决这两个问题：插件以 `.wasm` 模块形式独立分发，运行时动态加载，
在沙箱中执行，通过明确定义的接口（capability-based）与宿主通信。

### 5.2 技术选型：Extism vs Wasmtime Component Model

| 维度 | Extism | Wasmtime + WIT + Component Model |
|------|--------|-----------------------------------|
| 定位 | 面向应用开发者的插件框架（更高层封装） | WebAssembly 官方 Component Model 的参考实现（更底层、更标准） |
| 接口定义 | PDK（Plugin Development Kit），多语言 SDK | WIT (WebAssembly Interface Types)，接口即标准 |
| 语言支持 | Rust / Go / JS / Python / C 等（各语言 PDK） | 任何可编译到 wasm32-wasip2 / Component 的语言 |
| 宿主集成复杂度 | 低（`extism` crate 提供高层 API：`Plugin::new().call()`） | 中（需生成 WIT bindings，手动处理 resource/interface） |
| 生态成熟度 | 插件生态成熟，已广泛用于生产（Dylibso 维护） | Component Model 仍在标准化演进中，工具链变动较快 |
| 适用场景 | 快速接入第三方策略插件、轻量沙箱扩展 | 需要严格类型化跨语言接口、长期与 WASM 标准演进对齐 |

初步倾向：**优先评估 Extism**，因为它落地更快、多语言 SDK 更成熟，贴合第三方策略插件的场景
（策略作者可能是 Python/Go 开发者，不熟悉 Rust）。Wasmtime Component Model 作为长期演进方向保留
观察，若未来需要更严格的接口标准化（例如跨厂商互操作）再迁移。此决策需要一个独立的 spike 阶段
验证两者在 TrueFix 场景下的实际开发体验与性能开销。

### 5.3 插件化范围

| 插件类型 | 说明 | 宿主接口 |
|---------|------|---------|
| **策略插件** | Phase 4 的 `Strategy` / `TradingAgent` 以 WASM 模块形式提供 | `on_market_data` / `decide` 通过宿主 host function 调用 |
| **信号源插件** | 自定义新闻/情绪/链上数据信号，无需修改核心代码 | `Signal` trait 的 WASM 版本 |
| **适配器插件（长期）** | 社区贡献的小众券商适配器，独立分发，不需合并进主仓库 | `BrokerAdapter` trait 的 WASM 版本（受限于 WASM 无法直接做 TCP socket，需要 host-provided I/O capability） |
| **字段转换/校验插件** | 自定义 FIX 字段校验规则、自定义 symbol 映射逻辑 | 轻量 pure-function 接口，最适合 WASM 沙箱（无 I/O 需求） |

#### 新增 crate: `truefix-plugin`

```
crates/truefix-plugin/
├── Cargo.toml
├── src/
│   ├── lib.rs              # PluginHost trait，插件生命周期管理
│   ├── manifest.rs         # 插件清单 (plugin.toml: 权限声明/入口点/版本)
│   ├── host_functions.rs   # 暴露给插件的宿主函数 (日志/行情查询/下单请求，均需权限校验)
│   ├── sandbox.rs          # 资源限制 (内存上限/CPU 超时/fuel 计量)
│   └── loader.rs           # .wasm 模块加载与热更新
└── tests/
    ├── strategy_plugin.rs  # 策略插件: 加载 → 调用 decide() → 校验输出
    └── permission_deny.rs  # 权限校验: 插件越权调用被拒绝
```

#### 关键设计

1. **能力受限（Capability-based Security）**：插件默认无网络、无文件系统访问；所需能力（如"查询指定
   symbol 的最新行情"）通过宿主显式注入的 host function 提供，插件清单中声明所需权限
2. **资源限制**：内存上限、执行超时（fuel-based 计量，防止死循环插件拖垮引擎）
3. **版本兼容**：插件接口通过 WIT/PDK schema 版本化，宿主向后兼容旧版本插件
4. **策略插件优先落地**：Phase 4 的策略引擎是插件化的第一个落地场景（价值最高、I/O 需求最少，适合 WASM 沙箱）

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 5a | 技术选型 Spike：Extism vs Wasmtime Component Model | 两种方案分别实现同一个玩具策略插件，对比开发体验/性能/生态，形成决策记录 |
| 5b | `truefix-plugin` 核心：加载 + 沙箱 + host functions | 加载策略插件并调用成功；越权调用被正确拒绝；超时插件被正确终止 |
| 5c | 策略插件化 | Phase 4 的一个内置策略改造为 WASM 插件，行为与原生实现一致 |
| 5d | 插件市场雏形 | 插件清单规范 + 本地插件目录扫描加载（暂不做远程分发） |

---

## 依赖关系

```
Phase 1 (001-fix-engine-parity)
  │
  ├─ S8 TLS 支持 ──────────────┐
  ├─ S9 AT 套件通过 ───────────┤
  │                            │
  ▼                            │
Phase 2: FAST/SBE              │ (Phase 2 依赖 S1 编解码层)
  │                            │
  ▼                            │
Phase 3: 多券商网关 ◀──────────┘ (Binance FIX 适配器依赖 TLS)
  │
  ├── 3a 基础能力: 统一数据模型 + Symbol 映射 + 错误模型 + 时间 + 事件分发 + Mock
  ├── 3b 基础能力: 连接管理 + Order ID + 限流 + 凭证配置 + Simulator
  ├── 3w–3y 统一 Instruments 服务 (可与 3a/3b 并行；3d/3e/3f 及 3t–3v 均依赖其 SymbolMapper)
  ├── 3z–3z2 期权链/期货连续合约/窝轮牛熊证生命周期/公司行为 (依赖 3w–3y，可延后于 Tier 1 适配器)
  │     └── 3.2.11 公司行为处理 (基础能力) 依赖 3z2 提供的 CorporateAction 事件源
  ├── 3q–3s 独立 Client SDK: truefix-binance-client / truefix-twsapi-client / truefix-futu-client
  │     └── Tier 1 适配器组合对应 client 实现，而非重复编解码逻辑
  ├── FIX 适配器 (依赖 Phase 1 完成)
  ├── Tier 1:
  │     ├── Binance FIX 适配器 (依赖 Phase 1 S8 TLS + 3q truefix-binance-client)
  │     ├── IB TWS 适配器 (独立，仅需 TCP；依赖 3r truefix-twsapi-client)
  │     └── OpenD 适配器 (独立，仅需 TCP + Protobuf；依赖 3s truefix-futu-client)
  ├── 3t–3v IB→FIX 桥接 (truefix-ib-fix-bridge)
  │     └── 依赖 3r truefix-twsapi-client + 3w–3y 统一 Instruments
  ├── Tier 2:
  │     ├── Coinbase FIX 适配器 (复用 FIX 框架)
  │     └── Alpaca REST+WS 适配器 (独立，仅需 HTTP+WS)
  ├── Tier 3:
  │     ├── 老虎证券 REST+WS 适配器
  │     ├── 长桥证券 REST+WS 适配器
  │     └── OKX REST+WS 适配器
  ├── 3aa–3ad STEP 适配器 (国内交易所，独立于 Tier 1-4；复用 truefix-session 的 SOH 编解码器，
  │     不依赖 FAST/SBE/Phase 2)
  └── Tier 4+: 按需扩展
  │
  ▼
Phase 4: AI 交易 Agent
  │
  ├── 依赖 Phase 3 的统一交易 API
  ├── 4g–4i truefix-pricing (期权 Greeks) 可独立提前开发 (纯计算，无需连接券商)
  │     └── 4i 组合 Greeks 聚合依赖 4a TradingDecision/Position 模型
  └── 回测可独立于 Phase 3 开发 (使用模拟数据)
  │
  ▼
Phase 5: 插件化架构
  │
  ├── 5a 技术选型 Spike 可独立提前进行，不阻塞 Phase 3/4
  └── 5c 策略插件化依赖 Phase 4 的 Strategy / TradingAgent trait
```

## 技术决策记录

| ID | 决策 | 理由 |
|----|------|------|
| R-FUTURE-01 | FAST/SBE 作为独立 crate `truefix-binary` 而非扩展 `truefix-core` | 二进制编解码与 SOH 编解码的关注点不同；独立 crate 可按需引入 |
| R-FUTURE-02 | 统一 API 使用 async trait 而非同步 | 与 TrueFix 的 async-first 设计一致 (Constitution I) |
| R-FUTURE-03 | LLM 集成通过 HTTP API 而非进程内推理 | 避免引入重型 Python 依赖；支持远程模型；保持 Rust 纯净 |
| R-FUTURE-04 | IB TWS 和 OpenD 作为适配器而非 FIX 扩展 | 它们使用私有协议，不应污染 FIX 引擎核心 |
| R-FUTURE-05 | Binance 优先于其他交易所对接 | Binance FIX API 相对标准，且已有 testnet 可用于验证 |
| R-FUTURE-06 | Coinbase FIX 适配器复用 Binance FIX 框架 | Coinbase 同为 FIX 4.4 + TLS，与 Binance FIX 适配器 90% 代码共享，增量成本极低 |
| R-FUTURE-07 | Alpaca 作为首个 REST+WS 适配器 | API-first 设计理念与 TrueFix 的 async-first 一致，免费 paper trading key 降低接入门槛 |
| R-FUTURE-08 | 券商分 Tier 优先级接入 | Tier 1 已在 roadmap 核心；Tier 2 增量成本极低优先跟进；Tier 3+ 按需扩展，避免过早投入 |
| R-FUTURE-09 | 基础能力 (3.2) 先于适配器开发 | 统一数据模型/错误模型/连接管理/限流等是所有适配器的共同依赖；先落地框架层可避免各适配器各自重新发明轮子 |
| R-FUTURE-10 | Mock TradingGateway + Broker Simulator 作为测试基础 | Mock 实现 trait 用于单元测试和 Phase 4 回测；Simulator 用于集成测试，不依赖真实券商测试环境 |
| R-FUTURE-11 | 事件分发使用 mpsc channel 而非直接回调 | 避免回调阻塞连接线程；复用 `InChanCapacity` 背压模式；支持多订阅者 fan-out |
| R-FUTURE-12 | Client SDK (`truefix-*-client`) 与 Gateway Adapter 分层 | 协议正确性与跨协议一致性关注点分离；用户可仅依赖单一券商 client 而不引入整个网关；adapter 组合 client 避免编解码逻辑重复 |
| R-FUTURE-13 | IB→FIX 桥接复用 Acceptor 角色的 `truefix-session`，而非新写 FIX server 引擎 | 复用已验证的会话/心跳/序号逻辑；桥接模式（`XxxClient` → FIX Server）可复制到其他无 FIX 原生支持的券商 |
| R-FUTURE-14 | 统一 Instruments 从"基础能力" (3.2.2) 升级为独立 crate `truefix-instruments` | 券商数量增长 (15+) 后 symbol 映射与合约元数据需要集中治理，避免各适配器各自维护、口径不一致 |
| R-FUTURE-15 | 插件系统优先评估 Extism，Wasmtime Component Model 作为长期观察方向 | Extism 落地更快、多语言 SDK 成熟，贴合第三方策略插件场景；Component Model 标准仍在演进，工具链变动快 |
| R-FUTURE-16 | 策略插件是 WASM 插件化的第一个落地场景，而非适配器插件 | I/O 需求最少，最适合沙箱；价值最高——社区可贡献策略而无需接触 Rust/核心代码库；适配器插件因需 TCP I/O，留待 capability 机制成熟后再评估 |
| R-FUTURE-17 | `Instrument` 采用分层模型（顶层通用字段 + `SecurityType` 枚举承载资产特有字段），而非单一扁平 struct | 期权/期货/窝轮/牛熊证/加密永续合约的字段集合互不相同，扁平 struct 会导致大量字段对某些资产类型永远为空；分层模型让每种资产类型的约束在类型系统层面就是完整的 |
| R-FUTURE-18 | 窝轮/牛熊证等高换手合约走独立高频同步管道，不与股票共用日频同步 | 数量级和变化频率相差两个数量级（股票日频 vs 窝轮/牛熊证需 ≤1h 甚至近实时的收回事件），共用同一同步策略要么拖慢股票同步、要么让窝轮数据过期 |
| R-FUTURE-19 | 期权链不预加载，改为按需查询 + 短 TTL 缓存 | 到期日 × 行权价 × Call/Put 的组合数量级达数十万，全量预加载对内存和同步带宽都不现实；实际使用中也只关心用户持仓/关注标的的链 |
| R-FUTURE-20 | 公司行为处理 (3.2.11) 列为基础能力而非仅 Instruments 内部细节 | 持仓调整、分红入账、回测复权都不是 Instruments 一个模块能独立完成的，需要 Position/Account/回测引擎共同响应 `CorporateAction` 事件，因此提升为跨模块的基础能力 |
| R-FUTURE-21 | Greeks 优先用券商原生推送，自算仅作 fallback，且必须标注来源 | 自算依赖隐含波动率反解与定价模型假设，精度不如券商自身口径；风控如果混用两种精度不同的数据而不加区分，会产生难以排查的风险计算误差 |
| R-FUTURE-22 | 下单 / 结算路径禁止使用第三方估算汇率兜底，未连接对应券商时直接拒绝而非静默使用估算值 | 第三方汇率与券商实际结算价可能存在点差，静默使用会导致系统展示金额与实际扣款不一致；宁可拒绝下单，也不能让用户在不知情的情况下按错误汇率成交 |
| R-FUTURE-23 | 行情数据默认按"未授权即拒绝"处理，而非静默降级为延迟数据 | 延迟行情若不被消费方（策略/风控/LLM Agent）感知，等同于让交易决策基于过期价格做出，是隐蔽的系统性风险；显式拒绝能倒逼开发者在配置阶段就处理授权问题 |
| R-FUTURE-24 | STEP 从 Phase 2 移至 Phase 3，独立为 3.12 节，定位为"国内交易所适配器"而非二进制协议 | 本文档早前版本误将 STEP 描述为"FAST 之上的会话层，用于接入 CME/EUREX"。经核实标准原文（JR/T 0022—2020《证券交易数据交换协议》、JR/T 0182—2020《轻量级实时STEP消息传输协议》LFIXT，以及上交所/深交所各自的 STEP 接口规格书）：STEP 的消息格式与标准 FIX 字节级同构（SOH 分隔 tag=value，`BeginString=STEP.x.yz`，校验和/重复组机制相同），会话机制引用"FIX 标准会话机制"或其轻量化变体 LFIXT，与 FAST/SBE 这类二进制编解码无技术交集；架构上应作为 Phase 3 的适配器（复用 `truefix-session`/`truefix-dict`），而非 Phase 2 的编解码器 |




Binance https://github.com/binance/binance-connector-rust
IB TWS /Users/jiayin/workspace/dev/dev/rust/truefix/crates/truefix-twsapi-client
富途 OpenD https://github.com/loadstarCN/nautilus-futu 不完全 https://github.com/tensorchen/futu-rs
长桥证券 https://github.com/longbridge/openapi/tree/main/rust
老虎证券 https://github.com/tigerfintech/openapi-rust-sdk
OKX /Users/jiayin/workspace/dev/dev/rust/truefix/crates/truefix-okx-client https://github.com/fairwic/okx_rs
Bybit https://github.com/bybit-exchange/bybit-rust-api


TradeStation https://github.com/antonio-hickey/tradestation-rs
Alpaca  https://github.com/jonkarrer/alpaca_api_client
IG Markets https://github.com/joaquinbejar/ig-client
Charles Schwab https://github.com/bvelasquez/schwab-api-cli


https://github.com/nautechsystems/nautilus_trader/tree/develop
