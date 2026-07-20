# truefix-ig-client

`truefix-ig-client` 是 TrueFix 的原生、类型化 IG REST Trading API 客户端。它支持 IG
Demo 与 Live 环境、v2 CST/XST session 认证以及可选的 v3 OAuth 认证。

Demo 是默认环境。Live 环境必须显式确认风险；凭证由调用方的 secret provider 注入，crate
不会从环境变量或 `.env` 文件读取凭证。

## 安装

```toml
[dependencies]
truefix-ig-client = "0.1.4"
tokio = { version = "1", features = ["full"] }
```

## 快速开始：Demo + v2

v2 是默认认证模式。登录后，IG 返回 `CST` 与 `X-SECURITY-TOKEN`，客户端会在后续 REST
请求中自动携带它们。

```rust,no_run
use truefix_ig_client::{ClientConfig, Credentials, IgClient};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let credentials = Credentials::new("identifier", "password", "api-key")?;
let client = IgClient::new(ClientConfig::demo(Some(credentials)))?;

let session = client.login().await?;
println!("client ID: {}", session.client_id);

let positions = client.positions().await?;
println!("open positions: {}", positions.positions.len());

client.logout().await?;
# Ok(()) }
```

## 可选认证：v3 OAuth

选择 v3 时必须指定活动账户 ID。登录后客户端保存 OAuth access/refresh token；每个 REST
请求会自动发送 `Authorization: Bearer …` 与 `IG-ACCOUNT-ID`，并在 access token 距离过期
少于 10 秒时通过 `POST /session/refresh-token` 刷新。

```rust,no_run
use truefix_ig_client::{ClientConfig, Credentials, IgClient};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let credentials = Credentials::new("identifier", "password", "api-key")?;
let config = ClientConfig::demo(Some(credentials))
    .with_v3_authentication("ABC123")?;
let client = IgClient::new(config)?;

client.login().await?;
let accounts = client.accounts().await?;
# Ok(()) }
```

| 模式 | 登录请求 | 后续 REST 请求 | 适用情形 |
| --- | --- | --- | --- |
| v2（默认） | `Version: 2` | `CST`、`X-SECURITY-TOKEN` | 简单的 session 认证 |
| v3（可选） | `Version: 3`、账户 ID | Bearer token、`IG-ACCOUNT-ID` | 需要 OAuth token 生命周期管理 |

IG 的 streaming 服务仍需要 CST/XST；本 crate 当前仅提供 REST 客户端。`login()` 返回的
`lightstreamer_endpoint` 供应用自行创建 streaming 连接，不能硬编码。

## 已支持的 REST 操作

| Rust API | IG endpoint | API version |
| --- | --- | --- |
| `login` / `login_v2` | `POST /session` | 2 |
| `login` / `login_v3` | `POST /session` | 3 |
| `logout` | `DELETE /session` | 1 |
| `accounts` | `GET /accounts` | 1 |
| `positions` | `GET /positions` | 2 |
| `market` | `GET /markets/{epic}` | 3 |
| `search_markets` | `GET /markets?searchTerm=…` | 1 |
| `historical_prices` | `GET /prices/{epic}` | 3 |
| `create_position` | `POST /positions/otc` | 2 |

路径与 query 参数会被编码；例如 epic 中的 `/` 不会被误认为 URL path 分隔符。

## 查询市场与历史价格

```rust,no_run
use truefix_ig_client::{types::HistoricalPricesQuery, ClientConfig, Credentials, IgClient};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
# let credentials = Credentials::new("identifier", "password", "api-key")?;
# let client = IgClient::new(ClientConfig::demo(Some(credentials)))?;
# client.login().await?;
let markets = client.search_markets("EURUSD").await?;
let prices = client
    .historical_prices(
        "CS.D.EURUSD.MINI.IP",
        HistoricalPricesQuery::new("HOUR").max(100),
    )
    .await?;
# Ok(()) }
```

## 创建仓位

写操作不会自动重试，避免网络中断后的重复下单。应用应保存并用 IG 返回的
`deal_reference` 进行后续确认和对账。

```rust,no_run
use truefix_ig_client::{
    types::{CreatePositionRequest, Direction, OrderType},
    ClientConfig, Credentials, IgClient,
};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
# let credentials = Credentials::new("identifier", "password", "api-key")?;
# let client = IgClient::new(ClientConfig::demo(Some(credentials)))?;
# client.login().await?;
let acknowledgement = client
    .create_position(&CreatePositionRequest {
        currency_code: "USD".to_owned(),
        direction: Direction::Buy,
        epic: "CS.D.EURUSD.MINI.IP".to_owned(),
        expiry: "DFB".to_owned(),
        force_open: false,
        guaranteed_stop: false,
        order_type: OrderType::Market,
        size: 1.0,
        level: None,
        limit_level: None,
        stop_level: None,
    })
    .await?;
println!("deal reference: {}", acknowledgement.deal_reference);
# Ok(()) }
```

## Live 环境

构造 Live client 时需要 `LiveTradingConfirmation`，使生产交易意图在调用点可见：

```rust,no_run
use truefix_ig_client::{ClientConfig, Credentials, IgClient, LiveTradingConfirmation};

# fn example() -> Result<(), Box<dyn std::error::Error>> {
let credentials = Credentials::new("identifier", "password", "api-key")?;
let config = ClientConfig::live(
    credentials,
    LiveTradingConfirmation::acknowledge_risk(),
);
let _client = IgClient::new(config)?;
# Ok(()) }
```

测试或支持的区域路由可使用 `Environment::Custom { rest_base }`。`ClientConfig` 还提供
`timeout`（默认 15 秒）及可选 `proxy` 字段。

## 错误处理与安全性

所有方法返回 `IgResult<T>`。`IgError` 会区分配置、缺少凭证、未认证 session、IG API 拒绝、
网络传输与响应解析错误。IG 返回非 2xx 时，错误会保留 HTTP status、IG `errorCode` 和响应信息。

- `Credentials` 的 `Debug` 输出始终为 `Credentials(REDACTED)`。
- API key、密码、session token 与 OAuth token 不会出现在日志或错误文本中。
- Client 构造不发起网络请求；只有 `login` 和业务 API 会访问 IG。
- 遇到 token 被服务端撤销或刷新失败时，调用方应处理错误并重新登录。

## 验证

```bash
cargo test -p truefix-ig-client
cargo clippy -p truefix-ig-client --all-targets -- -D warnings
```

协议与账户配置请参阅 [IG Labs](https://labs.ig.com/) 的官方文档。
