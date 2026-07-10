和 yahoo_finance / yfinance 类似、可以通过程序获取股票行情和财务数据的接口，主要有这些：
接口	免费额度	数据范围	实时性	适合场景
Twelve Data	800 次/天，约 8 credits/分钟	美股、外汇、加密货币、ETF、指数，部分全球市场	免费版支持部分实时数据和试用 WebSocket	最接近 Yahoo Finance 的综合替代
Finnhub	有免费开发额度	股票、外汇、加密货币、公司基本面、新闻、经济数据	REST + WebSocket，具体交易所权限有区别	实时行情、公司数据、新闻
Alpha Vantage	免费版请求量较低	股票、外汇、加密货币、技术指标、财务数据、宏观数据	免费数据通常有限制或延迟	小型项目、技术指标计算
Massive / 原 Polygon.io	免费版 5 次/分钟，约 2 年历史数据	主要是美股、期权、指数、外汇、加密货币	免费版主要用于历史和开发测试	美股专业行情、K线、公司行动
Marketstack	100 次/月	全球股票、EOD、拆股、分红、交易所信息	免费版以日线为主	低频全球股票数据
Nasdaq Data Link	部分数据集免费	股票、期货、宏观、另类数据、经济数据	取决于数据集	历史研究、量化数据集
Stooq	免费下载	股票、指数、ETF、外汇、债券、加密货币	主要是历史数据
免费历史回测数据





创建一个交易客户端 支持

https://github.com/fairwic/okx_rs
https://github.com/tensorchen/futu-rs
https://github.com/jonkarrer/alpaca_api_client
https://github.com/xemwebe/yahoo_finance_api

https://github.com/bybit-exchange/bybit-rust-api
https://github.com/binance/binance-connector-rust
https://github.com/longbridge/openapi/tree/main/rust
https://github.com/tigerfintech/openapi-rust-sdk
/Users/jiayin/workspace/dev/dev/rust/truefix/crates/truefix-twsapi-client



测试的账号地址等可以读取本地配置文件，可以配置读写有默认值有单独的设置界面。

行情的vender 和交易的vender单独抽象
新闻数据的vender单独抽象
instruments.单独抽象独立通用。

举个例子 ib账号的连接 可以支持
ib 的行情provider
ib 的交易provider
 的新闻provider
有些可能只支持交易 或者只支持行情

有行情展示，交易操作，新闻展示， 使用tauri,标准交易窗口支持k线

