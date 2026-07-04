# TrueFix 后续发展路线

> 本文档描述 TrueFix 在完成 QuickFIX/J 功能对等（spec `001-fix-engine-parity`）之后的后续发展方向。
> 这些方向将 TrueFix 从一个 FIX 协议引擎扩展为一个**多协议、多券商、AI 驱动的交易基础设施**。

---

## 路线总览

```
Phase 1 (当前)        Phase 2                    Phase 3                    Phase 4
QuickFIX/J 对等  ──▶  二进制协议扩展        ──▶  多券商 API 网关        ──▶  AI 交易 Agent
─────────────────────────────────────────────────────────────────────────────────────────
S0–S9 AT 套件通过     FAST/SBE 二进制编解码      IB TWS / OpenD 适配          LLM 驱动交易决策
FIX SOH 编解码         STEP 协议支持              Binance FIX API 对接         风控护栏 + 订单路由
                      低延迟市场数据通道          统一抽象层                    回测 + 实盘切换
```

---

## Phase 2: 二进制协议扩展 — FAST / SBE / STEP

### 2.1 背景与动机

FIX 协议的 SOH 文本编码（`8=FIX.4.4\x019=185\x01...`）在高吞吐场景下存在显著开销：每条消息需要
解析 ASCII 标签号、将数值字段序列化为十进制字符串、计算 BodyLength 与 CheckSum。对于期权做市、
交易所行情组播等微秒级延迟场景，二进制编码是刚需。

FIX Trading Community 定义了多种二进制编码标准：

| 编码 | 全称 | 特点 |
|------|------|------|
| **FAST** | FIX Adapted for STreaming | 模板驱动，presence map + delta 编码，适用于行情组播，带宽压缩比可达 10:1 |
| **SBE** | Simple Binary Encoding | 固定偏移、零拷贝编解码，适用于订单录入，延迟可预测 |
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

4. **STEP 协议**: STEP (Straight-T hrough Expression Processing) 在 FAST 之上定义了会话管理语义（相当于 FAST 的传输层）。实现 STEP 后，TrueFix 可直接接入 CME、EUREX 等交易所的行情组播。

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 2a | FAST 模板解析 + 编解码器 | 解析标准 FAST 模板 XML；编码/解码往返与 QuickFIX FAST 参考实现字节一致 |
| 2b | SBE schema 解析 + 编解码器 | 解析标准 SBE schema XML；零拷贝编解码，延迟可测量 |
| 2c | IR 互转 + 传输层集成 | FAST/SBE ⇄ SOH FIX 无损互转；`truefix-transport` 支持协议选择 |
| 2d | STEP 会话层 | 可接入交易所行情组播；STEP 会话恢复与心跳 |

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
| `Position` | 统一持仓 | symbol / direction / quantity / avg_price / unrealized_pnl |
| `AccountInfo` | 统一账户 | balance / available / margin / currency |
| `Instrument` | 合约元数据 | symbol / exchange / security_type / multiplier / tick_size / min_qty / trading_hours |

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
- **合约元数据查询**：最小交易单位、价格档位 (tick_size)、乘数 (multiplier)、交易时间——不同市场规则差异大（港股最小 1000 股，A 股 T+1，美股 T+0，加密 24/7）
- 合约缓存：避免每次下单都查询

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

#### 基础能力与适配器的依赖关系

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
                              └── ...
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
| 3b | 基础能力: 连接管理 + Order ID 持久化 + 限流 + 凭证配置 + Broker Simulator | 连接状态机通过测试；限流令牌桶验证；配置解析正确 |
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

~~WeBull~~、~~盈立证券~~、~~DEGIRO~~、~~Kraken~~、~~日本/韩国券商~~

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

#### 阶段目标

| 阶段 | 交付物 | 验收标准 |
|------|--------|----------|
| 4a | `truefix-agent` 核心 trait + TradingDecision 模型 | Agent trait 定义完成；风控 trait 定义完成 |
| 4b | LLM 决策引擎 | 给定市场 context，LLM 产出可执行的 TradingDecision (JSON 验证通过) |
| 4c | 风控护栏 | 超限额订单被拦截；熔断机制在异常波动时自动停机 |
| 4d | 智能路由 | 多券商择优执行；TWAP/VWAP 拆单 |
| 4e | 回测引擎 | 历史 tick 数据回放 → 策略执行 → 性能报告 |
| 4f | 端到端 Demo | LLM Agent → 风控 → 路由 → Binance Testnet 下单 → 成交回报 → 闭环 |

---

## 依赖关系

```
Phase 1 (001-fix-engine-parity)
  │
  ├─ S8 TLS 支持 ──────────────┐
  ├─ S9 AT 套件通过 ───────────┤
  │                            │
  ▼                            │
Phase 2: FAST/SBE/STEP         │ (Phase 2 依赖 S1 编解码层)
  │                            │
  ▼                            │
Phase 3: 多券商网关 ◀──────────┘ (Binance FIX 适配器依赖 TLS)
  │
  ├── 3a 基础能力: 统一数据模型 + Symbol 映射 + 错误模型 + 时间 + 事件分发 + Mock
  ├── 3b 基础能力: 连接管理 + Order ID + 限流 + 凭证配置 + Simulator
  ├── FIX 适配器 (依赖 Phase 1 完成)
  ├── Tier 1:
  │     ├── Binance FIX 适配器 (依赖 Phase 1 S8 TLS)
  │     ├── IB TWS 适配器 (独立，仅需 TCP)
  │     └── OpenD 适配器 (独立，仅需 TCP + Protobuf)
  ├── Tier 2:
  │     ├── Coinbase FIX 适配器 (复用 FIX 框架)
  │     └── Alpaca REST+WS 适配器 (独立，仅需 HTTP+WS)
  ├── Tier 3:
  │     ├── 老虎证券 REST+WS 适配器
  │     ├── 长桥证券 REST+WS 适配器
  │     └── OKX REST+WS 适配器
  └── Tier 4+: 按需扩展
  │
  ▼
Phase 4: AI 交易 Agent
  │
  ├── 依赖 Phase 3 的统一交易 API
  └── 回测可独立于 Phase 3 开发 (使用模拟数据)
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
