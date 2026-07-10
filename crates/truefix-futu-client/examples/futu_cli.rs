//! Interactive example CLI for `truefix-futu-client`.
//!
//! Run:
//!   cargo run -p truefix-futu-client --example futu_cli

use std::env;
use std::io::{self, Write};
use std::str::FromStr;

use thiserror::Error;
use truefix_futu_client::pb;
use truefix_futu_client::quote::{
    GetBasicQotRequest, GetKlRequest, GetOrderBookRequest, GetTickerRequest, SubscribeRequest,
};
use truefix_futu_client::trade::{
    CancelAllOrderRequest, GetAccListRequest, GetFundsRequest, GetHistoryOrderListRequest,
    GetOrderFillListRequest, GetOrderListRequest, GetPositionListRequest, PlaceOrderRequest,
    TradeHeader,
};
use truefix_futu_client::{FutuClient, FutuClientConfig, Push};

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Futu(#[from] truefix_futu_client::FutuError),
}

#[derive(Debug, Clone)]
struct CliState {
    quote_market: i32,
    trade_sec_market: i32,
    user_id: u64,
    trd_category: Option<i32>,
    qot_security_firm: Option<i32>,
    trade_header: Option<TradeHeader>,
    last_accounts: Vec<pb::trd_common::TrdAcc>,
}

#[derive(Debug)]
enum Command {
    Help,
    Quote {
        symbol: String,
        market: Option<i32>,
    },
    Subscribe {
        symbol: String,
        market: Option<i32>,
        sub_types: Vec<i32>,
    },
    Watch {
        symbol: String,
        market: Option<i32>,
        sub_types: Vec<i32>,
    },
    Unsubscribe {
        symbol: String,
        market: Option<i32>,
        sub_types: Vec<i32>,
    },
    UnsubscribeAll,
    SubInfo {
        all_conn: bool,
    },
    Kline {
        symbol: String,
        market: Option<i32>,
        kl_type: i32,
        rehab_type: i32,
        req_num: i32,
    },
    Ticker {
        symbol: String,
        market: Option<i32>,
        max_ret_num: i32,
    },
    OrderBook {
        symbol: String,
        market: Option<i32>,
        num: i32,
        order_book_type: Option<i32>,
    },
    Accounts,
    UseAccount {
        index: usize,
    },
    Funds,
    Positions,
    Orders,
    Fills,
    HistoryOrders,
    HistoryFills,
    PlaceOrder {
        symbol: String,
        side: i32,
        qty: f64,
        price: Option<f64>,
        sec_market: Option<i32>,
    },
    CancelAll,
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let _ = tracing_subscriber::fmt().try_init();

    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut client = connect_client().await?;
    let mut state = CliState::from_env();

    spawn_push_printer(&client);
    print_banner(&state);

    if args.is_empty() {
        run_repl(&mut client, &mut state).await?;
    } else {
        let command = parse_command(&args)?;
        run_command(&mut client, &mut state, command).await?;
    }

    let _ = client.disconnect().await;
    Ok(())
}

async fn connect_client() -> Result<FutuClient, CliError> {
    let config = FutuClientConfig {
        host: env_string("FUTU_HOST", "127.0.0.1"),
        port: env_u16("FUTU_PORT", 11111)?,
        client_id: env_string("FUTU_CLIENT_ID", "1"),
        client_ver: env_i32("FUTU_CLIENT_VER", 300)?,
        recv_notify: true,
        request_timeout_ms: env_u64("FUTU_REQUEST_TIMEOUT_MS", 10_000)?,
        packet_enc_algo: env_i32(
            "FUTU_PACKET_ENC_ALGO",
            pb::common::PacketEncAlgo::None as i32,
        )?,
        init_rsa_key_path: env_optional_string("FUTU_INIT_RSA_KEY"),
        auto_reconnect: env_bool("FUTU_AUTO_RECONNECT", true),
        reconnect_interval_ms: env_u64("FUTU_RECONNECT_INTERVAL_MS", 6_000)?,
        security_firm: None,
    };
    println!(
        "connecting to {}:{} as {}",
        config.host, config.port, config.client_id
    );
    Ok(FutuClient::connect(config).await?)
}

fn print_banner(state: &CliState) {
    println!("connected");
    println!("quote market: {}", state.quote_market);
    println!("trade sec market: {}", state.trade_sec_market);
    println!("type `help` for commands");
}

fn spawn_push_printer(client: &FutuClient) {
    let mut rx = client.subscribe_push();
    tokio::spawn(async move {
        while let Ok(push) = rx.recv().await {
            print_push(&push);
        }
    });
}

async fn run_repl(client: &mut FutuClient, state: &mut CliState) -> Result<(), CliError> {
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        print!("futu> ");
        io::stdout().flush()?;
        line.clear();

        if stdin.read_line(&mut line)? == 0 {
            println!();
            break;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, "exit" | "quit" | "q") {
            break;
        }

        match parse_command(&split_args(input)) {
            Ok(Command::Help) => print_help(),
            Ok(command) => {
                if let Err(err) = run_command(client, state, command).await {
                    eprintln!("{err}");
                }
            }
            Err(err) => eprintln!("{err}"),
        }
    }

    Ok(())
}

async fn run_command(
    client: &mut FutuClient,
    state: &mut CliState,
    command: Command,
) -> Result<(), CliError> {
    let quote = client.quote();
    let trade = client.trade();

    match command {
        Command::Help => print_help(),
        Command::Quote { symbol, market } => {
            let resp = quote
                .get_basic_qot(GetBasicQotRequest {
                    security_list: vec![security(market.unwrap_or(state.quote_market), &symbol)],
                    header: state.quote_header(),
                })
                .await?;
            print_basic_qot(&resp);
        }
        Command::Subscribe {
            symbol,
            market,
            sub_types,
        } => {
            let sub_types = if sub_types.is_empty() {
                vec![pb::qot_common::SubType::Basic as i32]
            } else {
                sub_types
            };
            quote
                .subscribe(SubscribeRequest {
                    security_list: vec![security(market.unwrap_or(state.quote_market), &symbol)],
                    sub_type_list: sub_types.clone(),
                    is_sub_or_un_sub: true,
                    is_reg_or_un_reg_push: Some(true),
                    reg_push_rehab_type_list: vec![],
                    is_first_push: Some(true),
                    is_unsub_all: None,
                    is_sub_order_book_detail: None,
                    extended_time: None,
                    session: None,
                    header: state.quote_header(),
                })
                .await?;
            println!("subscribed: {} {:?}", symbol, normalize_sub_types(&sub_types));
            println!("waiting for push updates in background");
        }
        Command::Watch {
            symbol,
            market,
            sub_types,
        } => {
            let sub_types = if sub_types.is_empty() {
                vec![pb::qot_common::SubType::Basic as i32]
            } else {
                sub_types
            };
            quote
                .subscribe(SubscribeRequest {
                    security_list: vec![security(market.unwrap_or(state.quote_market), &symbol)],
                    sub_type_list: sub_types.clone(),
                    is_sub_or_un_sub: true,
                    is_reg_or_un_reg_push: Some(true),
                    reg_push_rehab_type_list: vec![],
                    is_first_push: Some(true),
                    is_unsub_all: None,
                    is_sub_order_book_detail: None,
                    extended_time: None,
                    session: None,
                    header: state.quote_header(),
                })
                .await?;
            println!("watching {} {:?}", symbol, normalize_sub_types(&sub_types));
            println!("type `unsub {symbol}` or `unsub-all` to stop");
        }
        Command::Unsubscribe {
            symbol,
            market,
            sub_types,
        } => {
            quote
                .unsubscribe(SubscribeRequest {
                    security_list: vec![security(market.unwrap_or(state.quote_market), &symbol)],
                    sub_type_list: if sub_types.is_empty() {
                        vec![pb::qot_common::SubType::Basic as i32]
                    } else {
                        sub_types
                    },
                    is_sub_or_un_sub: true,
                    is_reg_or_un_reg_push: None,
                    reg_push_rehab_type_list: vec![],
                    is_first_push: None,
                    is_unsub_all: None,
                    is_sub_order_book_detail: None,
                    extended_time: None,
                    session: None,
                    header: state.quote_header(),
                })
                .await?;
            println!("unsubscribed");
        }
        Command::UnsubscribeAll => {
            quote
                .unsubscribe_all(SubscribeRequest {
                    security_list: vec![],
                    sub_type_list: vec![],
                    is_sub_or_un_sub: true,
                    is_reg_or_un_reg_push: None,
                    reg_push_rehab_type_list: vec![],
                    is_first_push: None,
                    is_unsub_all: Some(true),
                    is_sub_order_book_detail: None,
                    extended_time: None,
                    session: None,
                    header: state.quote_header(),
                })
                .await?;
            println!("unsubscribed all");
        }
        Command::SubInfo { all_conn } => {
            let resp = quote
                .query_subscription(pb::qot_get_sub_info::Request {
                    c2s: pb::qot_get_sub_info::C2s {
                        is_req_all_conn: Some(all_conn),
                        header: state.quote_header(),
                    },
                })
                .await?;
            print_sub_info(&resp);
        }
        Command::Kline {
            symbol,
            market,
            kl_type,
            rehab_type,
            req_num,
        } => {
            let market = market.unwrap_or(state.quote_market);
            let security = security(market, &symbol);
            quote
                .subscribe(SubscribeRequest {
                    security_list: vec![security.clone()],
                    sub_type_list: vec![kl_type_to_sub_type(kl_type)],
                    is_sub_or_un_sub: true,
                    is_reg_or_un_reg_push: Some(true),
                    reg_push_rehab_type_list: vec![],
                    is_first_push: Some(false),
                    is_unsub_all: None,
                    is_sub_order_book_detail: None,
                    extended_time: None,
                    session: None,
                    header: state.quote_header(),
                })
                .await?;
            let resp = quote
                .get_kl(GetKlRequest {
                    rehab_type,
                    kl_type,
                    security: Some(security),
                    req_num,
                    header: state.quote_header(),
                })
                .await?;
            print_kl(&resp);
        }
        Command::Ticker {
            symbol,
            market,
            max_ret_num,
        } => {
            let resp = quote
                .get_ticker(GetTickerRequest {
                    security: Some(security(market.unwrap_or(state.quote_market), &symbol)),
                    max_ret_num,
                    header: state.quote_header(),
                })
                .await?;
            print_ticker(&resp);
        }
        Command::OrderBook {
            symbol,
            market,
            num,
            order_book_type,
        } => {
            let resp = quote
                .get_order_book(GetOrderBookRequest {
                    security: Some(security(market.unwrap_or(state.quote_market), &symbol)),
                    num,
                    order_book_type,
                    header: state.quote_header(),
                })
                .await?;
            print_order_book(&resp);
        }
        Command::Accounts => {
            let resp = trade
                .get_acc_list(GetAccListRequest {
                    user_id: state.user_id,
                    trd_category: state.trd_category,
                    need_general_sec_account: Some(true),
                })
                .await?;
            state.last_accounts = resp.acc_list.clone();
            if state.trade_header.is_none() {
                state.trade_header = state
                    .last_accounts
                    .first()
                    .map(|acc| trade_header_from_acc(acc, state.trade_sec_market));
            }
            print_accounts(&state.last_accounts);
        }
        Command::UseAccount { index } => {
            let acc = state
                .last_accounts
                .get(index.saturating_sub(1))
                .ok_or_else(|| CliError::Usage(format!("no account at index {index}")))?;
            state.trade_header = Some(trade_header_from_acc(acc, state.trade_sec_market));
            println!(
                "selected account: acc_id={} trd_env={} trd_market={}",
                acc.acc_id,
                acc.trd_env,
                acc.trd_market_auth_list
                    .first()
                    .copied()
                    .unwrap_or(state.trade_sec_market)
            );
        }
        Command::Funds => {
            let header = state.trade_header_or_err()?;
            let resp = trade
                .get_funds(GetFundsRequest {
                    header: Some(header.into_proto()),
                    refresh_cache: Some(true),
                    currency: None,
                    asset_category: None,
                })
                .await?;
            println!("{resp:#?}");
        }
        Command::Positions => {
            let header = state.trade_header_or_err()?;
            let resp = trade
                .get_position_list(GetPositionListRequest {
                    header: Some(header.into_proto()),
                    filter_conditions: None,
                    filter_pl_ratio_min: None,
                    filter_pl_ratio_max: None,
                    refresh_cache: Some(true),
                    asset_category: None,
                    currency: None,
                    option_strategy_view: None,
                })
                .await?;
            println!("{resp:#?}");
        }
        Command::Orders => {
            let header = state.trade_header_or_err()?;
            let resp = trade
                .get_order_list(GetOrderListRequest {
                    header: Some(header.into_proto()),
                    filter_conditions: None,
                    filter_status_list: vec![],
                    refresh_cache: Some(true),
                })
                .await?;
            println!("{resp:#?}");
        }
        Command::Fills => {
            let header = state.trade_header_or_err()?;
            let resp = trade
                .get_order_fill_list(GetOrderFillListRequest {
                    header: Some(header.into_proto()),
                    filter_conditions: None,
                    refresh_cache: Some(true),
                })
                .await?;
            println!("{resp:#?}");
        }
        Command::HistoryOrders => {
            let header = state.trade_header_or_err()?;
            let resp = trade
                .get_history_order_list(GetHistoryOrderListRequest {
                    header: Some(header.into_proto()),
                    filter_conditions: None,
                    filter_status_list: vec![],
                })
                .await?;
            println!("{resp:#?}");
        }
        Command::HistoryFills => {
            let header = state.trade_header_or_err()?;
            let resp = trade
                .get_history_order_fill_list(pb::trd_get_history_order_fill_list::Request {
                    c2s: pb::trd_get_history_order_fill_list::C2s {
                        header: header.into_proto(),
                        filter_conditions: Default::default(),
                    },
                })
                .await?;
            println!("{resp:#?}");
        }
        Command::PlaceOrder {
            symbol,
            side,
            qty,
            price,
            sec_market,
        } => {
            let header = state.trade_header_or_err()?;
            let order_type = if price.is_some() {
                pb::trd_common::OrderType::Normal as i32
            } else {
                pb::trd_common::OrderType::Market as i32
            };
            let order_id = trade
                .place_order(PlaceOrderRequest {
                    header: header.into_proto(),
                    trd_side: side,
                    order_type,
                    code: symbol,
                    qty,
                    price,
                    adjust_price: None,
                    adjust_side_and_limit: None,
                    sec_market: Some(sec_market.unwrap_or(state.trade_sec_market)),
                    remark: Some("truefix-futu-cli".to_owned()),
                    time_in_force: None,
                    fill_outside_rth: None,
                    aux_price: None,
                    trail_type: None,
                    trail_value: None,
                    trail_spread: None,
                    session: None,
                    position_id: None,
                    expire_time: None,
                })
                .await?;
            println!("order_id={order_id}");
        }
        Command::CancelAll => {
            let header = state.trade_header_or_err()?;
            let proto_header = header.clone().into_proto();
            trade
                .cancel_all_order(CancelAllOrderRequest {
                    header: Some(proto_header),
                    trd_market: Some(header.trd_market),
                })
                .await?;
            println!("cancel all submitted");
        }
    }

    Ok(())
}

fn parse_command(args: &[String]) -> Result<Command, CliError> {
    if args.is_empty() {
        return Ok(Command::Help);
    }

    let cmd = args[0].to_ascii_lowercase();
    let rest = &args[1..];
    match cmd.as_str() {
        "help" | "h" | "?" => Ok(Command::Help),
        "quote" | "q" => Ok(Command::Quote {
            symbol: required_arg(rest, 0, "symbol")?.to_owned(),
            market: optional_market(rest.get(1)),
        }),
        "sub" | "subscribe" => {
            let (symbol, market, sub_types) = parse_subscription_target(rest)?;
            Ok(Command::Subscribe {
                symbol,
                market,
                sub_types,
            })
        }
        "watch" => {
            let (symbol, market, sub_types) = parse_subscription_target(rest)?;
            Ok(Command::Watch {
                symbol,
                market,
                sub_types,
            })
        }
        "unsub" | "unsubscribe" => {
            let (symbol, market, sub_types) = parse_subscription_target(rest)?;
            Ok(Command::Unsubscribe {
                symbol,
                market,
                sub_types,
            })
        }
        "unsub-all" | "unsubscribe-all" => Ok(Command::UnsubscribeAll),
        "sub-info" | "subs" => Ok(Command::SubInfo {
            all_conn: !matches!(rest.first().map(String::as_str), Some("current" | "own")),
        }),
        "kline" | "kl" => Ok(Command::Kline {
            symbol: required_arg(rest, 0, "symbol")?.to_owned(),
            market: optional_market(rest.get(1)),
            kl_type: optional_kl_type(rest.get(2))
                .unwrap_or(pb::qot_common::KlType::KlType1min as i32),
            rehab_type: optional_rehab_type(rest.get(3))
                .unwrap_or(pb::qot_common::RehabType::None as i32),
            req_num: optional_i32(rest.get(4)).unwrap_or(100),
        }),
        "ticker" => Ok(Command::Ticker {
            symbol: required_arg(rest, 0, "symbol")?.to_owned(),
            market: optional_market(rest.get(1)),
            max_ret_num: optional_i32(rest.get(2)).unwrap_or(20),
        }),
        "book" | "orderbook" => Ok(Command::OrderBook {
            symbol: required_arg(rest, 0, "symbol")?.to_owned(),
            market: optional_market(rest.get(1)),
            num: optional_i32(rest.get(2)).unwrap_or(10),
            order_book_type: optional_i32(rest.get(3)),
        }),
        "accounts" | "accs" => Ok(Command::Accounts),
        "use-account" | "use" => Ok(Command::UseAccount {
            index: required_arg(rest, 0, "index")?
                .parse::<usize>()
                .map_err(|_| CliError::Usage("index must be a positive integer".to_owned()))?,
        }),
        "funds" => Ok(Command::Funds),
        "positions" | "pos" => Ok(Command::Positions),
        "orders" => Ok(Command::Orders),
        "fills" | "deals" => Ok(Command::Fills),
        "history-orders" => Ok(Command::HistoryOrders),
        "history-fills" => Ok(Command::HistoryFills),
        "place-order" | "order" => Ok(Command::PlaceOrder {
            symbol: required_arg(rest, 0, "symbol")?.to_owned(),
            side: parse_side(required_arg(rest, 1, "side")?)?,
            qty: required_arg(rest, 2, "qty")?
                .parse::<f64>()
                .map_err(|_| CliError::Usage("qty must be a number".to_owned()))?,
            price: optional_f64(rest.get(3)),
            sec_market: optional_sec_market(rest.get(4)),
        }),
        "cancel-all" => Ok(Command::CancelAll),
        other => Err(CliError::Usage(format!("unknown command: {other}"))),
    }
}

fn print_help() {
    println!(
        "commands:\n\
         help\n\
         quote <symbol> [market]\n\
         sub <symbol> [market] [basic|ticker|book|rt|all...]\n\
         watch <symbol> [market] [basic|ticker|book|rt|all...]\n\
         unsub <symbol> [market] [basic|ticker|book|rt...]\n\
         unsub-all\n\
         sub-info [all|current]\n\
         kline <symbol> [market] [kl_type] [rehab_type] [req_num]\n\
         ticker <symbol> [market] [max_ret_num]\n\
         book <symbol> [market] [num] [order_book_type]\n\
         accounts\n\
         use-account <index>\n\
         funds\n\
         positions\n\
         orders\n\
         fills\n\
         history-orders\n\
         history-fills\n\
         place-order <symbol> <buy|sell|sellshort|buyback> <qty> [price] [sec_market]\n\
         cancel-all\n\
         exit\n\n\
         examples:\n\
         watch AAPL us basic\n\
         watch AAPL ticker\n\
         watch HK.00700 hk book\n\
         sub AAPL ticker\n\
         unsub AAPL\n\n\
         env:\n\
         FUTU_HOST, FUTU_PORT, FUTU_CLIENT_ID, FUTU_REQUEST_TIMEOUT_MS,\n\
         FUTU_MARKET, FUTU_TRD_SEC_MARKET, FUTU_USER_ID, FUTU_TRD_CATEGORY,\n\
         FUTU_TRD_ENV, FUTU_ACC_ID, FUTU_SECURITY_FIRM"
    );
}

fn split_args(input: &str) -> Vec<String> {
    input.split_whitespace().map(str::to_owned).collect()
}

fn required_arg<'a>(args: &'a [String], idx: usize, name: &str) -> Result<&'a str, CliError> {
    args.get(idx)
        .map(String::as_str)
        .ok_or_else(|| CliError::Usage(format!("missing {name}")))
}

fn optional_i32(arg: Option<&String>) -> Option<i32> {
    arg.and_then(|value| value.parse::<i32>().ok())
}

fn optional_f64(arg: Option<&String>) -> Option<f64> {
    arg.and_then(|value| value.parse::<f64>().ok())
}

fn optional_market(arg: Option<&String>) -> Option<i32> {
    arg.and_then(|value| parse_qot_market(value).or_else(|| value.parse::<i32>().ok()))
}

fn optional_sec_market(arg: Option<&String>) -> Option<i32> {
    arg.and_then(|value| parse_trd_sec_market(value).or_else(|| value.parse::<i32>().ok()))
}

fn optional_kl_type(arg: Option<&String>) -> Option<i32> {
    arg.and_then(|value| parse_kl_type(value).or_else(|| value.parse::<i32>().ok()))
}

fn optional_rehab_type(arg: Option<&String>) -> Option<i32> {
    arg.and_then(|value| parse_rehab_type(value).or_else(|| value.parse::<i32>().ok()))
}

fn parse_subscription_target(args: &[String]) -> Result<(String, Option<i32>, Vec<i32>), CliError> {
    let symbol = required_arg(args, 0, "symbol")?.to_owned();
    let market = optional_market(args.get(1));
    let sub_type_start = if market.is_some() { 2 } else { 1 };
    let sub_types = parse_sub_types(&args[sub_type_start..])?;
    Ok((symbol, market, sub_types))
}

fn parse_sub_types(values: &[String]) -> Result<Vec<i32>, CliError> {
    if values.is_empty() {
        return Ok(vec![pb::qot_common::SubType::Basic as i32]);
    }

    let mut out = Vec::new();
    for value in values {
        match value.to_ascii_lowercase().as_str() {
            "basic" => out.push(pb::qot_common::SubType::Basic as i32),
            "ticker" => out.push(pb::qot_common::SubType::Ticker as i32),
            "book" | "orderbook" => out.push(pb::qot_common::SubType::OrderBook as i32),
            "rt" => out.push(pb::qot_common::SubType::Rt as i32),
            "kl1m" | "1m" | "1min" => out.push(pb::qot_common::SubType::Kl1min as i32),
            "kl3m" | "3m" | "3min" => out.push(pb::qot_common::SubType::Kl3min as i32),
            "kl5m" | "5m" | "5min" => out.push(pb::qot_common::SubType::Kl5min as i32),
            "kl10m" | "10m" | "10min" => out.push(pb::qot_common::SubType::Kl10min as i32),
            "kl15m" | "15m" | "15min" => out.push(pb::qot_common::SubType::Kl15min as i32),
            "kl30m" | "30m" | "30min" => out.push(pb::qot_common::SubType::Kl30min as i32),
            "kl60m" | "60m" | "60min" => out.push(pb::qot_common::SubType::Kl60min as i32),
            "kl120m" | "120m" | "120min" => out.push(pb::qot_common::SubType::Kl120min as i32),
            "kl180m" | "180m" | "180min" => out.push(pb::qot_common::SubType::Kl180min as i32),
            "kl240m" | "240m" | "240min" => out.push(pb::qot_common::SubType::Kl240min as i32),
            "klday" | "day" => out.push(pb::qot_common::SubType::KlDay as i32),
            "klweek" | "week" => out.push(pb::qot_common::SubType::KlWeek as i32),
            "klmonth" | "month" => out.push(pb::qot_common::SubType::KlMonth as i32),
            "klquarter" | "quarter" => {
                out.push(pb::qot_common::SubType::KlQurater as i32)
            }
            "klyear" | "year" => out.push(pb::qot_common::SubType::KlYear as i32),
            "all" => {
                out.push(pb::qot_common::SubType::Basic as i32);
                out.push(pb::qot_common::SubType::Ticker as i32);
                out.push(pb::qot_common::SubType::OrderBook as i32);
                out.push(pb::qot_common::SubType::Rt as i32);
            }
            other => return Err(CliError::Usage(format!("unknown sub type: {other}"))),
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

fn parse_side(value: &str) -> Result<i32, CliError> {
    match value.to_ascii_lowercase().as_str() {
        "buy" => Ok(pb::trd_common::TrdSide::Buy as i32),
        "sell" => Ok(pb::trd_common::TrdSide::Sell as i32),
        "sellshort" | "short" => Ok(pb::trd_common::TrdSide::SellShort as i32),
        "buyback" | "cover" => Ok(pb::trd_common::TrdSide::BuyBack as i32),
        other => Err(CliError::Usage(format!("unknown side: {other}"))),
    }
}

fn parse_qot_market(value: &str) -> Option<i32> {
    match value.to_ascii_lowercase().as_str() {
        "hk" => Some(pb::qot_common::QotMarket::HkSecurity as i32),
        "us" => Some(pb::qot_common::QotMarket::UsSecurity as i32),
        "cnsh" | "sh" => Some(pb::qot_common::QotMarket::CnshSecurity as i32),
        "cnsz" | "sz" => Some(pb::qot_common::QotMarket::CnszSecurity as i32),
        "sg" => Some(pb::qot_common::QotMarket::SgSecurity as i32),
        "jp" => Some(pb::qot_common::QotMarket::JpSecurity as i32),
        "au" => Some(pb::qot_common::QotMarket::AuSecurity as i32),
        "my" => Some(pb::qot_common::QotMarket::MySecurity as i32),
        "ca" => Some(pb::qot_common::QotMarket::CaSecurity as i32),
        "fx" => Some(pb::qot_common::QotMarket::FxSecurity as i32),
        "cc" | "crypto" => Some(pb::qot_common::QotMarket::CcSecurity as i32),
        _ => None,
    }
}

fn parse_trd_sec_market(value: &str) -> Option<i32> {
    match value.to_ascii_lowercase().as_str() {
        "hk" => Some(pb::trd_common::TrdSecMarket::Hk as i32),
        "us" => Some(pb::trd_common::TrdSecMarket::Us as i32),
        "cnsh" | "sh" => Some(pb::trd_common::TrdSecMarket::CnSh as i32),
        "cnsz" | "sz" => Some(pb::trd_common::TrdSecMarket::CnSz as i32),
        "sg" => Some(pb::trd_common::TrdSecMarket::Sg as i32),
        "jp" => Some(pb::trd_common::TrdSecMarket::Jp as i32),
        "au" => Some(pb::trd_common::TrdSecMarket::Au as i32),
        "my" => Some(pb::trd_common::TrdSecMarket::My as i32),
        "ca" => Some(pb::trd_common::TrdSecMarket::Ca as i32),
        "fx" => Some(pb::trd_common::TrdSecMarket::Fx as i32),
        "cc" | "crypto" => Some(pb::trd_common::TrdSecMarket::Cc as i32),
        _ => None,
    }
}

fn parse_kl_type(value: &str) -> Option<i32> {
    match value.to_ascii_lowercase().as_str() {
        "1m" | "1min" => Some(pb::qot_common::KlType::KlType1min as i32),
        "3m" | "3min" => Some(pb::qot_common::KlType::KlType3min as i32),
        "5m" | "5min" => Some(pb::qot_common::KlType::KlType5min as i32),
        "10m" | "10min" => Some(pb::qot_common::KlType::KlType10min as i32),
        "15m" | "15min" => Some(pb::qot_common::KlType::KlType15min as i32),
        "30m" | "30min" => Some(pb::qot_common::KlType::KlType30min as i32),
        "60m" | "60min" => Some(pb::qot_common::KlType::KlType60min as i32),
        "120m" | "120min" => Some(pb::qot_common::KlType::KlType120min as i32),
        "180m" | "180min" => Some(pb::qot_common::KlType::KlType180min as i32),
        "240m" | "240min" => Some(pb::qot_common::KlType::KlType240min as i32),
        "day" => Some(pb::qot_common::KlType::Day as i32),
        "week" => Some(pb::qot_common::KlType::Week as i32),
        "month" => Some(pb::qot_common::KlType::Month as i32),
        "quarter" => Some(pb::qot_common::KlType::Quarter as i32),
        "year" => Some(pb::qot_common::KlType::Year as i32),
        _ => None,
    }
}

fn kl_type_to_sub_type(kl_type: i32) -> i32 {
    match kl_type {
        x if x == pb::qot_common::KlType::Day as i32 => pb::qot_common::SubType::KlDay as i32,
        x if x == pb::qot_common::KlType::Week as i32 => pb::qot_common::SubType::KlWeek as i32,
        x if x == pb::qot_common::KlType::Month as i32 => {
            pb::qot_common::SubType::KlMonth as i32
        }
        x if x == pb::qot_common::KlType::Quarter as i32 => {
            pb::qot_common::SubType::KlQurater as i32
        }
        x if x == pb::qot_common::KlType::Year as i32 => pb::qot_common::SubType::KlYear as i32,
        x if x == pb::qot_common::KlType::KlType1min as i32 => {
            pb::qot_common::SubType::Kl1min as i32
        }
        x if x == pb::qot_common::KlType::KlType3min as i32 => {
            pb::qot_common::SubType::Kl3min as i32
        }
        x if x == pb::qot_common::KlType::KlType5min as i32 => {
            pb::qot_common::SubType::Kl5min as i32
        }
        x if x == pb::qot_common::KlType::KlType10min as i32 => {
            pb::qot_common::SubType::Kl10min as i32
        }
        x if x == pb::qot_common::KlType::KlType15min as i32 => {
            pb::qot_common::SubType::Kl15min as i32
        }
        x if x == pb::qot_common::KlType::KlType30min as i32 => {
            pb::qot_common::SubType::Kl30min as i32
        }
        x if x == pb::qot_common::KlType::KlType60min as i32 => {
            pb::qot_common::SubType::Kl60min as i32
        }
        x if x == pb::qot_common::KlType::KlType120min as i32 => {
            pb::qot_common::SubType::Kl120min as i32
        }
        x if x == pb::qot_common::KlType::KlType180min as i32 => {
            pb::qot_common::SubType::Kl180min as i32
        }
        x if x == pb::qot_common::KlType::KlType240min as i32 => {
            pb::qot_common::SubType::Kl240min as i32
        }
        _ => pb::qot_common::SubType::KlDay as i32,
    }
}

fn parse_rehab_type(value: &str) -> Option<i32> {
    match value.to_ascii_lowercase().as_str() {
        "none" => Some(pb::qot_common::RehabType::None as i32),
        "forward" | "qfq" => Some(pb::qot_common::RehabType::Forward as i32),
        "backward" | "hfq" => Some(pb::qot_common::RehabType::Backward as i32),
        _ => None,
    }
}

fn security(market: i32, code: &str) -> pb::qot_common::Security {
    pb::qot_common::Security {
        market,
        code: code.to_owned(),
    }
}

fn normalize_sub_types(sub_types: &[i32]) -> Vec<&'static str> {
    sub_types.iter().map(|value| sub_type_name(*value)).collect()
}

fn sub_type_name(sub_type: i32) -> &'static str {
    match sub_type {
        x if x == pb::qot_common::SubType::Basic as i32 => "basic",
        x if x == pb::qot_common::SubType::Ticker as i32 => "ticker",
        x if x == pb::qot_common::SubType::OrderBook as i32 => "book",
        x if x == pb::qot_common::SubType::Rt as i32 => "rt",
        x if x == pb::qot_common::SubType::Kl1min as i32 => "kl1m",
        x if x == pb::qot_common::SubType::Kl3min as i32 => "kl3m",
        x if x == pb::qot_common::SubType::Kl5min as i32 => "kl5m",
        x if x == pb::qot_common::SubType::Kl10min as i32 => "kl10m",
        x if x == pb::qot_common::SubType::Kl15min as i32 => "kl15m",
        x if x == pb::qot_common::SubType::Kl30min as i32 => "kl30m",
        x if x == pb::qot_common::SubType::Kl60min as i32 => "kl60m",
        x if x == pb::qot_common::SubType::Kl120min as i32 => "kl120m",
        x if x == pb::qot_common::SubType::Kl180min as i32 => "kl180m",
        x if x == pb::qot_common::SubType::Kl240min as i32 => "kl240m",
        _ => "unknown",
    }
}

fn print_push(push: &Push) {
    match push {
        Push::Notify(value) => println!("\n[notify] {value:#?}"),
        Push::UpdateOrder(value) => println!("\n[order] {value:#?}"),
        Push::UpdateOrderFill(value) => println!("\n[fill] {value:#?}"),
        Push::UpdateBasicQot(value) => print_basic_qot_push(value),
        Push::UpdateKl(value) => println!("\n[kline-push] {value:#?}"),
        Push::UpdateRt(value) => print_rt_push(value),
        Push::UpdateTicker(value) => print_ticker_push(value),
        Push::UpdateOrderBook(value) => print_order_book_push(value),
        Push::UpdateBroker(value) => println!("\n[broker] {value:#?}"),
        Push::UpdatePriceReminder(value) => println!("\n[price-reminder] {value:#?}"),
        Push::UpdateOptionEvent(value) => println!("\n[option-event] {value:#?}"),
        Push::PushIndicatorCalc(value) => println!("\n[indicator] {value:#?}"),
        Push::Unknown { proto_id, body } => {
            println!("\n[push-unknown] proto_id={proto_id} body_len={}", body.len())
        }
    }
    print!("futu> ");
    let _ = io::stdout().flush();
}

fn print_basic_qot_push(resp: &pb::qot_update_basic_qot::S2c) {
    for qot in &resp.basic_qot_list {
        let symbol = format_security(&qot.security);
        let name = qot.name.as_deref().unwrap_or("-");
        let change = qot.cur_price - qot.last_close_price;
        let change_rate = if qot.last_close_price.abs() > f64::EPSILON {
            (change / qot.last_close_price) * 100.0
        } else {
            0.0
        };
        println!(
            "\n[basic] {} {} last={:.4} chg={:+.4} ({:+.2}%) open={:.4} high={:.4} low={:.4} vol={} turnover={:.2} at={}",
            symbol,
            name,
            qot.cur_price,
            change,
            change_rate,
            qot.open_price,
            qot.high_price,
            qot.low_price,
            qot.volume,
            qot.turnover,
            qot.update_time
        );
    }
}

fn print_rt_push(resp: &pb::qot_update_rt::S2c) {
    let symbol = format_security(&resp.security);
    let name = resp.name.as_deref().unwrap_or("-");
    if let Some(last) = resp.rt_list.last() {
        println!(
            "\n[rt] {} {} time={} price={} avg={} vol={} turnover={}",
            symbol,
            name,
            last.time,
            fmt_opt_f64(last.price),
            fmt_opt_f64(last.avg_price),
            fmt_opt_i64(last.volume),
            fmt_opt_f64(last.turnover)
        );
    } else {
        println!("\n[rt] {} {} empty", symbol, name);
    }
}

fn print_ticker_push(resp: &pb::qot_update_ticker::S2c) {
    let symbol = format_security(&resp.security);
    let name = resp.name.as_deref().unwrap_or("-");
    for ticker in &resp.ticker_list {
        println!(
            "\n[ticker] {} {} time={} dir={} price={:.4} vol={} turnover={:.2} seq={}",
            symbol,
            name,
            ticker.time,
            ticker_direction_name(ticker.dir),
            ticker.price,
            ticker.volume,
            ticker.turnover,
            ticker.sequence
        );
    }
}

fn print_order_book_push(resp: &pb::qot_update_order_book::S2c) {
    let symbol = format_security(&resp.security);
    let name = resp.name.as_deref().unwrap_or("-");
    let best_ask = resp.order_book_ask_list.first();
    let best_bid = resp.order_book_bid_list.first();
    println!(
        "\n[book] {} {} bid={}x{} ask={}x{}",
        symbol,
        name,
        best_bid.map(|x| format!("{:.4}", x.price)).unwrap_or_else(|| "-".to_owned()),
        best_bid.map(fmt_order_book_volume).unwrap_or_else(|| "-".to_owned()),
        best_ask.map(|x| format!("{:.4}", x.price)).unwrap_or_else(|| "-".to_owned()),
        best_ask.map(fmt_order_book_volume).unwrap_or_else(|| "-".to_owned()),
    );
}

fn format_security(security: &pb::qot_common::Security) -> String {
    format!("{}.{}", market_name(security.market), security.code)
}

fn market_name(market: i32) -> &'static str {
    match market {
        x if x == pb::qot_common::QotMarket::HkSecurity as i32 => "HK",
        x if x == pb::qot_common::QotMarket::UsSecurity as i32 => "US",
        x if x == pb::qot_common::QotMarket::CnshSecurity as i32 => "SH",
        x if x == pb::qot_common::QotMarket::CnszSecurity as i32 => "SZ",
        x if x == pb::qot_common::QotMarket::SgSecurity as i32 => "SG",
        x if x == pb::qot_common::QotMarket::JpSecurity as i32 => "JP",
        x if x == pb::qot_common::QotMarket::AuSecurity as i32 => "AU",
        x if x == pb::qot_common::QotMarket::MySecurity as i32 => "MY",
        x if x == pb::qot_common::QotMarket::CaSecurity as i32 => "CA",
        x if x == pb::qot_common::QotMarket::FxSecurity as i32 => "FX",
        x if x == pb::qot_common::QotMarket::CcSecurity as i32 => "CC",
        _ => "MKT",
    }
}

fn ticker_direction_name(direction: i32) -> &'static str {
    match direction {
        x if x == pb::qot_common::TickerDirection::Bid as i32 => "bid",
        x if x == pb::qot_common::TickerDirection::Ask as i32 => "ask",
        x if x == pb::qot_common::TickerDirection::Neutral as i32 => "neutral",
        _ => "unknown",
    }
}

fn fmt_order_book_volume(level: &pb::qot_common::OrderBook) -> String {
    level
        .hp_volume
        .map(|value| format!("{value}"))
        .unwrap_or_else(|| level.volume.to_string())
}

fn fmt_opt_f64(value: Option<f64>) -> String {
    value
        .map(|inner| format!("{inner:.4}"))
        .unwrap_or_else(|| "-".to_owned())
}

fn fmt_opt_i64(value: Option<i64>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn print_basic_qot(resp: &pb::qot_get_basic_qot::S2c) {
    for qot in &resp.basic_qot_list {
        println!("{qot:#?}");
    }
}

fn print_kl(resp: &pb::qot_get_kl::S2c) {
    println!("security: {:?}", resp.security);
    for kl in &resp.kl_list {
        println!("{kl:#?}");
    }
}

fn print_ticker(resp: &pb::qot_get_ticker::S2c) {
    println!("security: {:?}", resp.security);
    println!("name: {:?}", resp.name);
    for ticker in &resp.ticker_list {
        println!("{ticker:#?}");
    }
}

fn print_order_book(resp: &pb::qot_get_order_book::S2c) {
    println!("security: {:?}", resp.security);
    println!("name: {:?}", resp.name);
    println!("asks:");
    for item in &resp.order_book_ask_list {
        println!("  {item:#?}");
    }
    println!("bids:");
    for item in &resp.order_book_bid_list {
        println!("  {item:#?}");
    }
}

fn print_accounts(accounts: &[pb::trd_common::TrdAcc]) {
    for (idx, acc) in accounts.iter().enumerate() {
        println!(
            "{}: acc_id={} trd_env={} auth={:?} firm={:?} role={:?} status={:?}",
            idx + 1,
            acc.acc_id,
            acc.trd_env,
            acc.trd_market_auth_list,
            acc.security_firm,
            acc.acc_role,
            acc.acc_status
        );
    }
}

fn print_sub_info(resp: &pb::qot_get_sub_info::S2c) {
    println!(
        "subscriptions total_used={} remain={} option_used={:?} option_remain={:?}",
        resp.total_used_quota,
        resp.remain_quota,
        resp.option_used_quota,
        resp.option_remain_quota
    );
    for (idx, conn) in resp.conn_sub_info_list.iter().enumerate() {
        println!(
            "conn {} own={} used={} option_used={:?} security_firm={:?}",
            idx + 1,
            conn.is_own_conn_data,
            conn.used_quota,
            conn.option_used_quota,
            conn.security_firm
        );
        for sub in &conn.sub_info_list {
            let subtype = sub_type_name(sub.sub_type);
            let symbols = sub
                .security_list
                .iter()
                .map(format_security)
                .collect::<Vec<_>>()
                .join(", ");
            println!("  {subtype}: {symbols}");
        }
    }
}

fn trade_header_from_acc(acc: &pb::trd_common::TrdAcc, default_market: i32) -> TradeHeader {
    TradeHeader {
        trd_env: acc.trd_env,
        acc_id: acc.acc_id,
        trd_market: acc
            .trd_market_auth_list
            .first()
            .copied()
            .unwrap_or(default_market),
        jp_acc_type: acc.jp_acc_type.first().copied(),
    }
}

impl CliState {
    fn from_env() -> Self {
        let trade_header = env_u64_opt("FUTU_ACC_ID").map(|acc_id| TradeHeader {
            trd_env: env_i32("FUTU_TRD_ENV", pb::trd_common::TrdEnv::Simulate as i32).unwrap_or(0),
            acc_id,
            trd_market: env_i32("FUTU_TRD_MARKET", pb::trd_common::TrdMarket::Us as i32)
                .unwrap_or(pb::trd_common::TrdMarket::Us as i32),
            jp_acc_type: env_i32_opt("FUTU_JP_ACC_TYPE"),
        });

        Self {
            quote_market: env_market("FUTU_MARKET", pb::qot_common::QotMarket::UsSecurity as i32),
            trade_sec_market: env_sec_market(
                "FUTU_TRD_SEC_MARKET",
                pb::trd_common::TrdSecMarket::Us as i32,
            ),
            user_id: env_u64("FUTU_USER_ID", 0).unwrap_or(0),
            trd_category: env_i32_opt("FUTU_TRD_CATEGORY"),
            qot_security_firm: env_i32_opt("FUTU_SECURITY_FIRM"),
            trade_header,
            last_accounts: Vec::new(),
        }
    }

    fn quote_header(&self) -> Option<pb::qot_common::QotHeader> {
        Some(pb::qot_common::QotHeader {
            security_firm: self.qot_security_firm,
        })
        .filter(|header| header.security_firm.is_some())
    }

    fn trade_header_or_err(&self) -> Result<TradeHeader, CliError> {
        self.trade_header
            .clone()
            .ok_or_else(|| CliError::Usage("trade header is not configured; run `accounts` or set FUTU_ACC_ID/FUTU_TRD_ENV/FUTU_TRD_MARKET`".to_owned()))
    }
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn env_optional_string(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"),
        Err(_) => default,
    }
}

fn env_u16(name: &str, default: u16) -> Result<u16, CliError> {
    match env::var(name) {
        Ok(value) => {
            u16::from_str(&value).map_err(|_| CliError::Usage(format!("{name} must be a u16")))
        }
        Err(_) => Ok(default),
    }
}

fn env_u64(name: &str, default: u64) -> Result<u64, CliError> {
    match env::var(name) {
        Ok(value) => {
            u64::from_str(&value).map_err(|_| CliError::Usage(format!("{name} must be a u64")))
        }
        Err(_) => Ok(default),
    }
}

fn env_u64_opt(name: &str) -> Option<u64> {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
}

fn env_i32(name: &str, default: i32) -> Result<i32, CliError> {
    match env::var(name) {
        Ok(value) => {
            i32::from_str(&value).map_err(|_| CliError::Usage(format!("{name} must be an i32")))
        }
        Err(_) => Ok(default),
    }
}

fn env_i32_opt(name: &str) -> Option<i32> {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
}

fn env_market(name: &str, default: i32) -> i32 {
    env::var(name)
        .ok()
        .and_then(|value| parse_qot_market(&value).or_else(|| value.parse::<i32>().ok()))
        .unwrap_or(default)
}

fn env_sec_market(name: &str, default: i32) -> i32 {
    env::var(name)
        .ok()
        .and_then(|value| parse_trd_sec_market(&value).or_else(|| value.parse::<i32>().ok()))
        .unwrap_or(default)
}
