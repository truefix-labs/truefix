use std::env;
use std::io::{self, Write};
use std::str::FromStr;
use std::time::Duration;

use rust_decimal::Decimal;
use thiserror::Error;

use truefix_twsapi_client::client::{ClientConfig, TwsApiClient};
use truefix_twsapi_client::events::Event;
use truefix_twsapi_client::requests::{
    CalculateImpliedVolatilityRequest, CalculateOptionPriceRequest, ContractDetailsRequest,
    ExecutionRequest, ExerciseOptionsRequest, HeadTimestampRequest, HistogramDataRequest,
    HistoricalDataRequest, HistoricalNewsRequest, HistoricalTicksRequest, MarketDataRequest,
    MarketDepthRequest, PlaceOrderRequest, RealTimeBarsRequest, ScannerSubscriptionRequest,
    TickByTickRequest, WshEventDataRequest,
};
use truefix_twsapi_client::types::{
    Contract, ExecutionFilter, Order, ScannerSubscription, TagValue,
};

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tws(#[from] truefix_twsapi_client::error::TwsApiError),
}

enum Command {
    CurrentTime,
    CurrentTimeMillis,
    RequestIds,
    SetServerLogLevel,
    MarketData { symbol: Option<String> },
    MarketDepth,
    PlaceOrder,
    CancelOrder,
    OpenOrders,
    AutoOpenOrders,
    AllOpenOrders,
    GlobalCancel,
    CompletedOrders,
    AccountSummary,
    Positions,
    PositionsMulti,
    AccountUpdatesMulti,
    HistoricalData,
    HistoricalTicks,
    HeadTimestamp,
    HistogramData,
    ContractDetails,
    BondContractDetails,
    Executions,
    Scanner,
    ScannerParameters,
    RealTimeBars,
    TickByTick,
    SmartComponents,
    MarketRule,
    CalculateImpliedVolatility,
    CalculateOptionPrice,
    ExerciseOptions,
    Pnl,
    PnlSingle,
    NewsBulletins,
    NewsArticle,
    HistoricalNews,
    ManagedAccounts,
    Disconnect,
    AccountUpdates,
    MarketDepthExchanges,
    SecDefOptParams,
    SoftDollarTiers,
    FamilyCodes,
    MatchingSymbols,
    QueryDisplayGroups,
    SubscribeToGroupEvents,
    UpdateDisplayGroup,
    UnsubscribeFromGroupEvents,
    RequestFa,
    ReplaceFa,
    WshMetaData,
    WshEventData,
    UserInfo,
    NewsProviders,
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut client = connect().await?;
    wait_until_api_ready(&mut client).await?;

    if args.is_empty() {
        print_help();
        run_interactive(&mut client).await?;
    } else {
        let (command, extra_args) = parse_command(args)?;
        run_command(&mut client, command).await?;
        if !extra_args.is_empty() {
            eprintln!("unused args: {}", extra_args.join(" "));
        }
    }

    let _ = client.disconnect().await;
    Ok(())
}

async fn run_command(client: &mut TwsApiClient, command: Command) -> Result<(), CliError> {
    match command {
        Command::CurrentTime => run_current_time(client).await?,
        Command::CurrentTimeMillis => run_current_time_millis(client).await?,
        Command::RequestIds => run_request_ids(client).await?,
        Command::SetServerLogLevel => run_set_server_log_level(client).await?,
        Command::MarketData { symbol } => run_market_data(client, symbol).await?,
        Command::MarketDepth => run_market_depth(client).await?,
        Command::PlaceOrder => run_place_order(client).await?,
        Command::CancelOrder => run_cancel_order(client).await?,
        Command::OpenOrders => run_open_orders(client).await?,
        Command::AutoOpenOrders => run_auto_open_orders(client).await?,
        Command::AllOpenOrders => run_all_open_orders(client).await?,
        Command::GlobalCancel => run_global_cancel(client).await?,
        Command::CompletedOrders => run_completed_orders(client).await?,
        Command::AccountSummary => run_account_summary(client).await?,
        Command::Positions => run_positions(client).await?,
        Command::PositionsMulti => run_positions_multi(client).await?,
        Command::AccountUpdatesMulti => run_account_updates_multi(client).await?,
        Command::HistoricalData => run_historical_data(client).await?,
        Command::HistoricalTicks => run_historical_ticks(client).await?,
        Command::HeadTimestamp => run_head_timestamp(client).await?,
        Command::HistogramData => run_histogram_data(client).await?,
        Command::ContractDetails => run_contract_details(client).await?,
        Command::BondContractDetails => run_bond_contract_details(client).await?,
        Command::Executions => run_executions(client).await?,
        Command::Scanner => run_scanner(client).await?,
        Command::ScannerParameters => run_scanner_parameters(client).await?,
        Command::RealTimeBars => run_real_time_bars(client).await?,
        Command::TickByTick => run_tick_by_tick(client).await?,
        Command::SmartComponents => run_smart_components(client).await?,
        Command::MarketRule => run_market_rule(client).await?,
        Command::CalculateImpliedVolatility => run_calculate_implied_volatility(client).await?,
        Command::CalculateOptionPrice => run_calculate_option_price(client).await?,
        Command::ExerciseOptions => run_exercise_options(client).await?,
        Command::Pnl => run_pnl(client).await?,
        Command::PnlSingle => run_pnl_single(client).await?,
        Command::NewsBulletins => run_news_bulletins(client).await?,
        Command::NewsArticle => run_news_article(client).await?,
        Command::HistoricalNews => run_historical_news(client).await?,
        Command::ManagedAccounts => run_managed_accounts(client).await?,
        Command::Disconnect => run_disconnect(client).await?,
        Command::AccountUpdates => run_account_updates(client).await?,
        Command::MarketDepthExchanges => run_market_depth_exchanges(client).await?,
        Command::SecDefOptParams => run_sec_def_opt_params(client).await?,
        Command::SoftDollarTiers => run_soft_dollar_tiers(client).await?,
        Command::FamilyCodes => run_family_codes(client).await?,
        Command::MatchingSymbols => run_matching_symbols(client).await?,
        Command::QueryDisplayGroups => run_query_display_groups(client).await?,
        Command::SubscribeToGroupEvents => run_subscribe_to_group_events(client).await?,
        Command::UpdateDisplayGroup => run_update_display_group(client).await?,
        Command::UnsubscribeFromGroupEvents => run_unsubscribe_from_group_events(client).await?,
        Command::RequestFa => run_request_fa(client).await?,
        Command::ReplaceFa => run_replace_fa(client).await?,
        Command::WshMetaData => run_wsh_meta_data(client).await?,
        Command::WshEventData => run_wsh_event_data(client).await?,
        Command::UserInfo => run_user_info(client).await?,
        Command::NewsProviders => run_news_providers(client).await?,
    }
    Ok(())
}

async fn run_interactive(client: &mut TwsApiClient) -> Result<(), CliError> {
    let stdin = io::stdin();
    let mut line = String::new();

    loop {
        print!("twsapi> ");
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
        if matches!(input, "quit" | "exit" | "q") {
            break;
        }
        if matches!(input, "help" | "h" | "?") {
            print_help();
            continue;
        }

        let parts = input
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let (command, extra_args) = match parse_command(parts) {
            Ok(parsed) => parsed,
            Err(err) => {
                eprintln!("{err}");
                continue;
            }
        };
        if let Err(err) = run_command(client, command).await {
            eprintln!("{err}");
        }
        if !extra_args.is_empty() {
            eprintln!("unused args: {}", extra_args.join(" "));
        }
    }

    Ok(())
}

fn parse_command(mut args: Vec<String>) -> Result<(Command, Vec<String>), CliError> {
    if args.is_empty() {
        return Err(CliError::Usage("missing command".to_owned()));
    }

    let command = match args.remove(0).as_str() {
        "time" | "current-time" => Command::CurrentTime,
        "current-time-ms" | "time-ms" => Command::CurrentTimeMillis,
        "request-ids" | "req-ids" => Command::RequestIds,
        "set-server-log-level" => Command::SetServerLogLevel,
        "market-data" => {
            let symbol = args.first().cloned();
            if !args.is_empty() {
                args.remove(0);
            }
            Command::MarketData { symbol }
        }
        "market-depth" => Command::MarketDepth,
        "place-order" => Command::PlaceOrder,
        "cancel-order" => Command::CancelOrder,
        "open-orders" => Command::OpenOrders,
        "auto-open-orders" => Command::AutoOpenOrders,
        "all-open-orders" => Command::AllOpenOrders,
        "global-cancel" => Command::GlobalCancel,
        "completed-orders" => Command::CompletedOrders,
        "account-summary" => Command::AccountSummary,
        "positions" => Command::Positions,
        "positions-multi" => Command::PositionsMulti,
        "account-updates-multi" => Command::AccountUpdatesMulti,
        "historical-data" => Command::HistoricalData,
        "historical-ticks" => Command::HistoricalTicks,
        "head-timestamp" => Command::HeadTimestamp,
        "histogram-data" => Command::HistogramData,
        "contract-details" => Command::ContractDetails,
        "bond-contract-details" => Command::BondContractDetails,
        "executions" => Command::Executions,
        "scanner" => Command::Scanner,
        "scanner-params" => Command::ScannerParameters,
        "real-time-bars" => Command::RealTimeBars,
        "tick-by-tick" => Command::TickByTick,
        "smart-components" => Command::SmartComponents,
        "market-rule" => Command::MarketRule,
        "calc-iv" | "calculate-implied-volatility" => Command::CalculateImpliedVolatility,
        "calc-option-price" | "calculate-option-price" => Command::CalculateOptionPrice,
        "exercise-options" => Command::ExerciseOptions,
        "pnl" => Command::Pnl,
        "pnl-single" => Command::PnlSingle,
        "news-bulletins" => Command::NewsBulletins,
        "news-article" => Command::NewsArticle,
        "historical-news" => Command::HistoricalNews,
        "managed-accounts" => Command::ManagedAccounts,
        "disconnect" => Command::Disconnect,
        "account-updates" => Command::AccountUpdates,
        "market-depth-exchanges" => Command::MarketDepthExchanges,
        "sec-def-opt-params" => Command::SecDefOptParams,
        "soft-dollar-tiers" => Command::SoftDollarTiers,
        "family-codes" => Command::FamilyCodes,
        "matching-symbols" => Command::MatchingSymbols,
        "query-display-groups" => Command::QueryDisplayGroups,
        "subscribe-group-events" => Command::SubscribeToGroupEvents,
        "update-display-group" => Command::UpdateDisplayGroup,
        "unsubscribe-group-events" => Command::UnsubscribeFromGroupEvents,
        "request-fa" => Command::RequestFa,
        "replace-fa" => Command::ReplaceFa,
        "wsh-meta-data" => Command::WshMetaData,
        "wsh-event-data" => Command::WshEventData,
        "user-info" => Command::UserInfo,
        "news-providers" => Command::NewsProviders,
        "help" | "-h" | "--help" => {
            return Err(CliError::Usage("use `help` inside the client".to_owned()));
        }
        other => return Err(CliError::Usage(format!("unknown command: {other}"))),
    };

    Ok((command, args))
}

fn print_help() {
    eprintln!(
        "usage:\n\
         \x20\x20interactive: cargo run -p truefix-twsapi-client --example twsapi\n\
         \x20\x20one-shot:    cargo run -p truefix-twsapi-client --example twsapi -- <command>\n\
         \n\
         commands:\n\
         \x20\x20time | current-time\n\
         \x20\x20current-time-ms | time-ms\n\
         \x20\x20request-ids | req-ids\n\
         \x20\x20set-server-log-level\n\
         \x20\x20market-data\n\
         \x20\x20market-depth\n\
         \x20\x20place-order\n\
         \x20\x20cancel-order\n\
         \x20\x20open-orders\n\
         \x20\x20auto-open-orders\n\
         \x20\x20all-open-orders\n\
         \x20\x20global-cancel\n\
         \x20\x20completed-orders\n\
         \x20\x20account-summary\n\
         \x20\x20account-updates\n\
         \x20\x20account-updates-multi\n\
         \x20\x20positions\n\
         \x20\x20positions-multi\n\
         \x20\x20historical-data\n\
         \x20\x20historical-ticks\n\
         \x20\x20head-timestamp\n\
         \x20\x20histogram-data\n\
         \x20\x20contract-details\n\
         \x20\x20bond-contract-details\n\
         \x20\x20disconnect\n\
         \x20\x20executions\n\
         \x20\x20scanner\n\
         \x20\x20scanner-params\n\
         \x20\x20real-time-bars\n\
         \x20\x20tick-by-tick\n\
         \x20\x20smart-components\n\
         \x20\x20market-rule\n\
         \x20\x20calc-iv | calculate-implied-volatility\n\
         \x20\x20calc-option-price | calculate-option-price\n\
         \x20\x20exercise-options\n\
         \x20\x20pnl\n\
         \x20\x20pnl-single\n\
         \x20\x20news-bulletins\n\
         \x20\x20news-article\n\
         \x20\x20historical-news\n\
         \x20\x20managed-accounts\n\
         \x20\x20market-depth-exchanges\n\
         \x20\x20sec-def-opt-params\n\
         \x20\x20soft-dollar-tiers\n\
         \x20\x20family-codes\n\
         \x20\x20matching-symbols\n\
         \x20\x20query-display-groups\n\
         \x20\x20subscribe-group-events\n\
         \x20\x20update-display-group\n\
         \x20\x20unsubscribe-group-events\n\
         \x20\x20request-fa\n\
         \x20\x20replace-fa\n\
         \x20\x20wsh-meta-data\n\
         \x20\x20wsh-event-data\n\
         \x20\x20user-info\n\
         \x20\x20news-providers\n\
         \n\
         common env:\n\
         \x20\x20TWS_HOST=127.0.0.1 TWS_PORT=7497 TWS_CLIENT_ID=1002\n\
         \x20\x20TWS_SYMBOL=AAPL TWS_SEC_TYPE=STK TWS_EXCHANGE=SMART TWS_CURRENCY=USD\n\
         \x20\x20TWS_REQ_ID=9001 TWS_ACCOUNT=<acct> TWS_MODEL_CODE=<model>\n"
    );
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn env_opt_string(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.is_empty())
}

fn env_parse<T>(key: &str, default: T) -> Result<T, CliError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(value) if !value.is_empty() => value
            .parse::<T>()
            .map_err(|err| CliError::Usage(format!("{key}={value}: {err}"))),
        _ => Ok(default),
    }
}

fn env_bool(key: &str, default: bool) -> Result<bool, CliError> {
    match env::var(key) {
        Ok(value) if !value.is_empty() => match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => Ok(true),
            "0" | "false" | "no" | "n" => Ok(false),
            _ => Err(CliError::Usage(format!(
                "{key} must be one of 1|0|true|false|yes|no|y|n"
            ))),
        },
        _ => Ok(default),
    }
}

fn env_decimal(key: &str) -> Result<Option<Decimal>, CliError> {
    match env::var(key) {
        Ok(value) if !value.is_empty() => value
            .parse::<Decimal>()
            .map(Some)
            .map_err(|err| CliError::Usage(format!("{key}={value}: {err}"))),
        _ => Ok(None),
    }
}

fn env_tag_values(prefix: &str) -> Vec<TagValue> {
    let mut values = Vec::new();
    for idx in 1..=16 {
        let tag_key = format!("{prefix}_{idx}_TAG");
        let value_key = format!("{prefix}_{idx}_VALUE");
        let Some(tag) = env_opt_string(&tag_key) else {
            continue;
        };
        let value = env_opt_string(&value_key).unwrap_or_default();
        values.push(TagValue { tag, value });
    }
    values
}

fn req_id(default: i32) -> Result<i32, CliError> {
    env_parse("TWS_REQ_ID", default)
}

fn contract_from_env() -> Result<Contract, CliError> {
    let mut contract = Contract::default();
    contract.con_id = env_parse("TWS_CONID", 0i32)?;
    contract.symbol = env_string("TWS_SYMBOL", "AAPL");
    contract.sec_type = env_string("TWS_SEC_TYPE", "STK");
    contract.last_trade_date_or_contract_month =
        env_string("TWS_LAST_TRADE_DATE_OR_CONTRACT_MONTH", "");
    contract.strike = env_parse("TWS_STRIKE", contract.strike)?;
    contract.right = env_string("TWS_RIGHT", "");
    contract.multiplier = env_string("TWS_MULTIPLIER", "");
    contract.exchange = env_string("TWS_EXCHANGE", "SMART");
    contract.primary_exchange = env_string("TWS_PRIMARY_EXCHANGE", "");
    contract.currency = env_string("TWS_CURRENCY", "USD");
    contract.local_symbol = env_string("TWS_LOCAL_SYMBOL", "");
    contract.trading_class = env_string("TWS_TRADING_CLASS", "");
    contract.include_expired = env_bool("TWS_INCLUDE_EXPIRED", false)?;
    contract.sec_id_type = env_string("TWS_SEC_ID_TYPE", "");
    contract.sec_id = env_string("TWS_SEC_ID", "");
    contract.combo_legs_description = env_string("TWS_COMBO_LEGS_DESCRIPTION", "");
    Ok(contract)
}

fn parse_order_kind() -> Result<String, CliError> {
    Ok(env_string("TWS_ORDER_KIND", "limit").to_ascii_lowercase())
}

fn side(value: &str) -> Result<String, CliError> {
    match value.to_ascii_uppercase().as_str() {
        "BUY" | "SELL" => Ok(value.to_ascii_uppercase()),
        other => Err(CliError::Usage(format!(
            "TWS_ORDER_SIDE must be BUY or SELL, got {other}"
        ))),
    }
}

fn order_from_env() -> Result<Order, CliError> {
    let mut order = Order::default();
    order.order_id = req_id(5001)?;
    order.action = side(&env_string("TWS_ORDER_SIDE", "BUY"))?;
    order.total_quantity = env_decimal("TWS_ORDER_QTY")?.unwrap_or_else(|| Decimal::ONE);
    order.account = env_string("TWS_ACCOUNT", "");
    order.order_type = env_string("TWS_ORDER_TYPE", "");
    order.tif = env_string("TWS_TIF", "DAY");
    order.good_till_date = env_string("TWS_GOOD_TILL_DATE", "");
    order.transmit = env_bool("TWS_TRANSMIT", true)?;
    order.what_if = env_bool("TWS_WHAT_IF", false)?;
    order.outside_rth = env_bool("TWS_OUTSIDE_RTH", false)?;
    order.hidden = env_bool("TWS_HIDDEN", false)?;
    order.solicited = env_bool("TWS_SOLICITED", false)?;
    order.cash_qty = env_parse("TWS_CASH_QTY", order.cash_qty)?;
    order.order_ref = env_string("TWS_ORDER_REF", "");
    Ok(order)
}

fn order_kind_from_env(order: &mut Order) -> Result<(), CliError> {
    match parse_order_kind()?.as_str() {
        "market" => {
            order.order_type = "MKT".to_owned();
        }
        "limit" => {
            order.order_type = "LMT".to_owned();
            order.limit_price = env_parse("TWS_LIMIT_PRICE", 0.0f64)?;
        }
        "stop" => {
            order.order_type = "STP".to_owned();
            order.trail_stop_price = env_parse("TWS_TRIGGER_PRICE", 0.0f64)?;
        }
        "stoplimit" => {
            order.order_type = "STP LMT".to_owned();
            order.limit_price = env_parse("TWS_LIMIT_PRICE", 0.0f64)?;
            order.trail_stop_price = env_parse("TWS_TRIGGER_PRICE", 0.0f64)?;
        }
        other => {
            return Err(CliError::Usage(format!(
                "TWS_ORDER_KIND must be market|limit|stop|stoplimit, got {other}"
            )));
        }
    }
    Ok(())
}

async fn connect() -> Result<TwsApiClient, CliError> {
    let host = env_string("TWS_HOST", "127.0.0.1");
    let port = env_parse("TWS_PORT", 7497u16)?;
    let client_id = env_parse("TWS_CLIENT_ID", 1002i32)?;
    Ok(TwsApiClient::connect(ClientConfig::new(host, port, client_id)).await?)
}

async fn wait_until_api_ready(client: &mut TwsApiClient) -> Result<(), CliError> {
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        while !client.api_ready() {
            match client.read_event().await? {
                Event::Error { code, message, .. } => {
                    eprintln!("TWS notice {code}: {message}");
                }
                other => {
                    print_event(&other);
                }
            }
        }
        truefix_twsapi_client::error::TwsApiResult::Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err(CliError::Usage(
            "timed out waiting for initial API callbacks".to_owned(),
        )),
    }
}

async fn read_until<F>(
    client: &mut TwsApiClient,
    timeout_secs: u64,
    mut on_event: F,
) -> Result<bool, CliError>
where
    F: FnMut(Event) -> bool,
{
    let result = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        loop {
            let event = client.read_event().await?;
            if on_event(event) {
                return truefix_twsapi_client::error::TwsApiResult::Ok(true);
            }
        }
    })
    .await;
    match result {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Ok(false),
    }
}

fn print_event(event: &Event) {
    match event {
        Event::CurrentTime { time } => println!("current_time={time}"),
        Event::CurrentTimeInMillis { time_in_millis } => {
            println!("current_time_ms={time_in_millis}")
        }
        Event::MarketDataType {
            req_id,
            market_data_type,
        } => println!("market_data_type req_id={req_id} type={market_data_type}"),
        Event::TickPrice {
            req_id,
            tick_type,
            price,
            attrib,
        } => println!(
            "tick_price req_id={req_id} tick_type={tick_type} price={price} attrib={attrib}"
        ),
        Event::TickSize {
            req_id,
            tick_type,
            size,
        } => println!("tick_size req_id={req_id} tick_type={tick_type} size={size}"),
        Event::TickString {
            req_id,
            tick_type,
            value,
        } => println!("tick_string req_id={req_id} tick_type={tick_type} value={value}"),
        Event::TickOptionComputation {
            req_id,
            tick_type,
            tick_attrib,
            implied_vol,
            delta,
            opt_price,
            pv_dividend,
            gamma,
            vega,
            theta,
            und_price,
        } => println!(
            "tick_option req_id={req_id} tick_type={tick_type} tick_attrib={tick_attrib} implied_vol={implied_vol} delta={delta} opt_price={opt_price} pv_dividend={pv_dividend} gamma={gamma} vega={vega} theta={theta} und_price={und_price}"
        ),
        Event::OrderStatus {
            order_id,
            status,
            filled,
            remaining,
            avg_fill_price,
            perm_id,
            parent_id,
            last_fill_price,
            client_id,
            why_held,
            market_cap_price,
        } => println!(
            "order_status order_id={order_id} status={status} filled={filled} remaining={remaining} avg_fill_price={avg_fill_price} perm_id={perm_id} parent_id={parent_id} last_fill_price={last_fill_price} client_id={client_id} why_held={why_held} market_cap_price={market_cap_price}"
        ),
        Event::OpenOrder {
            order_id,
            contract,
            order,
            ..
        } => println!(
            "open_order order_id={order_id} symbol={} sec_type={} exchange={} action={} qty={} type={}",
            contract.symbol,
            contract.sec_type,
            contract.exchange,
            order.action,
            order.total_quantity,
            order.order_type
        ),
        Event::CompletedOrder {
            contract,
            order,
            order_state,
        } => println!(
            "completed_order symbol={} sec_type={} exchange={} action={} qty={} status={} completed_status={}",
            contract.symbol,
            contract.sec_type,
            contract.exchange,
            order.action,
            order.total_quantity,
            order_state.status,
            order_state.completed_status
        ),
        Event::ContractDetails { req_id, details } => println!(
            "contract_details req_id={req_id} symbol={} sec_type={} exchange={} currency={}",
            details.contract.symbol,
            details.contract.sec_type,
            details.contract.exchange,
            details.contract.currency
        ),
        Event::BondContractDetails { req_id, details } => println!(
            "bond_contract_details req_id={req_id} symbol={} sec_type={} exchange={} currency={}",
            details.contract.symbol,
            details.contract.sec_type,
            details.contract.exchange,
            details.contract.currency
        ),
        Event::ExecutionDetails {
            req_id,
            contract,
            execution,
        } => println!(
            "execution req_id={req_id} symbol={} exec_id={} shares={} price={}",
            contract.symbol, execution.exec_id, execution.shares, execution.price
        ),
        Event::HistoricalData { req_id, bar } => println!(
            "historical_bar req_id={req_id} date={} open={} high={} low={} close={} volume={} wap={} count={}",
            bar.date, bar.open, bar.high, bar.low, bar.close, bar.volume, bar.wap, bar.bar_count
        ),
        Event::HistoricalDataUpdate { req_id, bar } => println!(
            "historical_update req_id={req_id} date={} open={} high={} low={} close={}",
            bar.date, bar.open, bar.high, bar.low, bar.close
        ),
        Event::HistoricalDataEnd { req_id, start, end } => {
            println!("historical_end req_id={req_id} start={start} end={end}")
        }
        Event::RealTimeBar { req_id, time, bar } => println!(
            "real_time_bar req_id={req_id} time={time} open={} high={} low={} close={} volume={} wap={} count={}",
            bar.open, bar.high, bar.low, bar.close, bar.volume, bar.wap, bar.bar_count
        ),
        Event::HeadTimestamp {
            req_id,
            head_timestamp,
        } => println!("head_timestamp req_id={req_id} head_timestamp={head_timestamp}"),
        Event::ScannerData { req_id, rows } => {
            println!("scanner_data req_id={req_id} rows={}", rows.len());
            for row in rows {
                println!(
                    "  rank={} symbol={} sec_type={} market={} distance={} benchmark={} projection={} combo_key={}",
                    row.rank,
                    row.contract.symbol,
                    row.contract.sec_type,
                    row.market_name,
                    row.distance,
                    row.benchmark,
                    row.projection,
                    row.combo_key
                );
            }
        }
        Event::ScannerDataEnd { req_id } => println!("scanner_data_end req_id={req_id}"),
        Event::AccountSummary {
            req_id,
            account,
            tag,
            value,
            currency,
        } => println!(
            "account_summary req_id={req_id} account={account} tag={tag} value={value} currency={currency}"
        ),
        Event::AccountValue {
            key,
            value,
            currency,
            account_name,
        } => println!(
            "account_value account={account_name} key={key} value={value} currency={currency}"
        ),
        Event::PortfolioValue {
            contract,
            position,
            market_price,
            market_value,
            average_cost,
            unrealized_pnl,
            realized_pnl,
            account_name,
        } => println!(
            "portfolio account={account_name} symbol={} position={} market_price={} market_value={} average_cost={} unrealized_pnl={} realized_pnl={}",
            contract.symbol,
            position,
            market_price,
            market_value,
            average_cost,
            unrealized_pnl,
            realized_pnl
        ),
        Event::Position {
            account,
            contract,
            position,
            avg_cost,
        } => println!(
            "position account={account} symbol={} sec_type={} exchange={} position={} avg_cost={}",
            contract.symbol, contract.sec_type, contract.exchange, position, avg_cost
        ),
        Event::PositionMulti {
            req_id,
            account,
            model_code,
            contract,
            position,
            avg_cost,
        } => println!(
            "position_multi req_id={req_id} account={account} model_code={model_code} symbol={} position={} avg_cost={}",
            contract.symbol, position, avg_cost
        ),
        Event::Pnl {
            req_id,
            daily_pnl,
            unrealized_pnl,
            realized_pnl,
        } => println!(
            "pnl req_id={req_id} daily_pnl={daily_pnl} unrealized_pnl={unrealized_pnl} realized_pnl={realized_pnl}"
        ),
        Event::PnlSingle {
            req_id,
            position,
            daily_pnl,
            unrealized_pnl,
            realized_pnl,
            value,
        } => println!(
            "pnl_single req_id={req_id} position={position} daily_pnl={daily_pnl} unrealized_pnl={unrealized_pnl} realized_pnl={realized_pnl} value={value}"
        ),
        Event::ManagedAccounts { accounts } => println!("managed_accounts {accounts}"),
        Event::MarketDepth {
            req_id,
            position,
            operation,
            side,
            price,
            size,
            market_maker,
            is_smart_depth,
        } => println!(
            "market_depth req_id={req_id} position={position} operation={operation} side={side} price={price} size={size} market_maker={market_maker} smart_depth={is_smart_depth}"
        ),
        Event::MarketDepthExchanges { descriptions } => {
            println!("market_depth_exchanges count={}", descriptions.len());
            for description in descriptions {
                println!("  {description}");
            }
        }
        Event::NewsProviders { providers } => {
            println!("news_providers count={}", providers.len());
            for provider in providers {
                println!("  {} {}", provider.0, provider.1);
            }
        }
        Event::ScannerParameters { xml } => println!("{xml}"),
        Event::OpenOrderEnd
        | Event::CompletedOrdersEnd
        | Event::ContractDetailsEnd { .. }
        | Event::ExecutionDetailsEnd { .. }
        | Event::PositionEnd
        | Event::AccountSummaryEnd { .. }
        | Event::AccountDownloadEnd { .. }
        | Event::ConnectionClosed
        | Event::ConnectAck => println!("{event:?}"),
        Event::Error {
            req_id,
            code,
            message,
            advanced_order_reject_json,
            ..
        } => eprintln!(
            "TWS error req_id={req_id} code={code} message={message} advanced={advanced_order_reject_json}"
        ),
        other => println!("{other:?}"),
    }
}

async fn run_current_time(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_current_time().await?;
    read_until(client, 5, |event| match event {
        Event::CurrentTime { .. } | Event::CurrentTimeInMillis { .. } => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            true
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_current_time_millis(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_current_time_in_millis().await?;
    read_until(client, 5, |event| match event {
        Event::CurrentTimeInMillis { .. } => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_request_ids(client: &mut TwsApiClient) -> Result<(), CliError> {
    let num_ids = env_parse("TWS_NUM_IDS", 1i32)?;
    client.req_ids(num_ids).await?;
    read_until(client, 5, |event| match event {
        Event::NextValidId { order_id } => {
            println!("next_valid_id order_id={order_id}");
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_set_server_log_level(client: &mut TwsApiClient) -> Result<(), CliError> {
    let log_level = env_parse("TWS_LOG_LEVEL", 1i32)?;
    client.set_server_log_level(log_level).await?;
    println!("server_log_level={log_level}");
    Ok(())
}

async fn run_market_data(
    client: &mut TwsApiClient,
    symbol: Option<String>,
) -> Result<(), CliError> {
    client
        .req_market_data_type(env_parse("TWS_MARKET_DATA_TYPE", 1i32)?)
        .await?;
    let req_id = req_id(9001)?;
    let symbol = symbol.unwrap_or_else(|| env_string("TWS_SYMBOL", "AAPL"));
    let mut contract = contract_from_env()?;
    contract.symbol = symbol;
    client
        .req_mkt_data(MarketDataRequest {
            req_id,
            contract,
            generic_tick_list: env_string("TWS_GENERIC_TICK_LIST", ""),
            snapshot: env_bool("TWS_SNAPSHOT", false)?,
            regulatory_snapshot: env_bool("TWS_REGULATORY_SNAPSHOT", false)?,
            market_data_options: env_tag_values("TWS_MARKET_DATA_OPTION"),
        })
        .await?;
    let done = read_until(
        client,
        env_parse("TWS_WAIT_SECS", 30u64)?,
        |event| match event {
            Event::TickPrice {
                req_id: event_req_id,
                ..
            }
            | Event::TickSize {
                req_id: event_req_id,
                ..
            }
            | Event::TickString {
                req_id: event_req_id,
                ..
            }
            | Event::TickGeneric {
                req_id: event_req_id,
                ..
            }
            | Event::TickOptionComputation {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                true
            }
            Event::Error {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                true
            }
            Event::Error { code, message, .. } => {
                eprintln!("TWS notice {code}: {message}");
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    if !done {
        let _ = client.cancel_mkt_data(req_id).await;
    }
    Ok(())
}

async fn run_market_depth(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9002)?;
    client
        .req_mkt_depth(MarketDepthRequest {
            req_id,
            contract: contract_from_env()?,
            num_rows: env_parse("TWS_DEPTH_ROWS", 5i32)?,
            is_smart_depth: env_bool("TWS_SMART_DEPTH", false)?,
            market_depth_options: env_tag_values("TWS_DEPTH_OPTION"),
        })
        .await?;
    read_until(
        client,
        env_parse("TWS_WAIT_SECS", 20u64)?,
        |event| match event {
            Event::MarketDepth {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                false
            }
            Event::Error {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                true
            }
            Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    let _ = client
        .cancel_mkt_depth(req_id, env_bool("TWS_SMART_DEPTH", false)?)
        .await;
    Ok(())
}

async fn run_place_order(client: &mut TwsApiClient) -> Result<(), CliError> {
    let mut order = order_from_env()?;
    order_kind_from_env(&mut order)?;
    let request = PlaceOrderRequest {
        order_id: order.order_id,
        contract: contract_from_env()?,
        order,
        extra_fields: env_string("TWS_ORDER_EXTRA_FIELDS", ""),
    };
    client.place_order(request).await?;
    read_until(
        client,
        env_parse("TWS_WAIT_SECS", 15u64)?,
        |event| match event {
            Event::OpenOrder { .. } | Event::OrderStatus { .. } | Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    Ok(())
}

async fn run_cancel_order(client: &mut TwsApiClient) -> Result<(), CliError> {
    client
        .cancel_order(
            req_id(5001)?,
            truefix_twsapi_client::types::OrderCancel::default(),
        )
        .await?;
    read_until(client, 10, |event| match event {
        Event::OrderStatus { .. } | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_open_orders(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_open_orders().await?;
    read_until(client, 20, |event| match event {
        Event::OpenOrder { .. } | Event::OpenOrderEnd => {
            print_event(&event);
            matches!(event, Event::OpenOrderEnd)
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_auto_open_orders(client: &mut TwsApiClient) -> Result<(), CliError> {
    let auto_bind = env_bool("TWS_AUTO_BIND", true)?;
    client.req_auto_open_orders(auto_bind).await?;
    println!("auto_open_orders auto_bind={auto_bind}");
    Ok(())
}

async fn run_all_open_orders(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_all_open_orders().await?;
    read_until(client, 20, |event| match event {
        Event::OpenOrder { .. } | Event::OpenOrderEnd => {
            print_event(&event);
            matches!(event, Event::OpenOrderEnd)
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_global_cancel(client: &mut TwsApiClient) -> Result<(), CliError> {
    client
        .req_global_cancel(truefix_twsapi_client::types::OrderCancel::default())
        .await?;
    println!("global_cancel sent");
    Ok(())
}

async fn run_completed_orders(client: &mut TwsApiClient) -> Result<(), CliError> {
    client
        .req_completed_orders(env_bool("TWS_COMPLETED_API_ONLY", false)?)
        .await?;
    read_until(client, 20, |event| match event {
        Event::CompletedOrder { .. } | Event::CompletedOrdersEnd => {
            print_event(&event);
            matches!(event, Event::CompletedOrdersEnd)
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_account_summary(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9101)?;
    client
        .req_account_summary(
            req_id,
            &env_string("TWS_SUMMARY_GROUP", "All"),
            &env_string(
                "TWS_SUMMARY_TAGS",
                "NetLiquidation,TotalCashValue,BuyingPower,AvailableFunds",
            ),
        )
        .await?;
    read_until(client, 20, |event| match event {
        Event::AccountSummary {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::AccountSummaryEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_account_summary(req_id).await;
    Ok(())
}

async fn run_positions(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_positions().await?;
    read_until(client, 20, |event| match event {
        Event::Position { .. } | Event::PositionEnd => {
            print_event(&event);
            matches!(event, Event::PositionEnd)
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_positions().await;
    Ok(())
}

async fn run_positions_multi(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9102)?;
    client
        .req_positions_multi(
            req_id,
            &env_string("TWS_ACCOUNT", ""),
            &env_string("TWS_MODEL_CODE", ""),
        )
        .await?;
    read_until(client, 20, |event| match event {
        Event::PositionMulti {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::PositionMultiEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_positions_multi(req_id).await;
    Ok(())
}

async fn run_account_updates_multi(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9103)?;
    client
        .req_account_updates_multi(
            req_id,
            &env_string("TWS_ACCOUNT", ""),
            &env_string("TWS_MODEL_CODE", ""),
            env_bool("TWS_LEDGER_AND_NLV", false)?,
        )
        .await?;
    let done = read_until(client, 20, |event| match event {
        Event::AccountUpdateMulti {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::AccountUpdateMultiEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    if !done {
        let _ = client.cancel_account_updates_multi(req_id).await;
    }
    Ok(())
}

async fn run_historical_data(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9201)?;
    client
        .req_historical_data(HistoricalDataRequest {
            req_id,
            contract: contract_from_env()?,
            end_date_time: env_string("TWS_END_DATE_TIME", ""),
            duration_str: env_string("TWS_DURATION_STR", "1 D"),
            bar_size_setting: env_string("TWS_BAR_SIZE_SETTING", "1 day"),
            what_to_show: env_string("TWS_WHAT_TO_SHOW", "TRADES"),
            use_rth: env_parse("TWS_USE_RTH", 1i32)?,
            format_date: env_parse("TWS_FORMAT_DATE", 1i32)?,
            keep_up_to_date: env_bool("TWS_KEEP_UP_TO_DATE", false)?,
            chart_options: env_tag_values("TWS_HIST_OPTION"),
        })
        .await?;
    let done = read_until(
        client,
        env_parse("TWS_WAIT_SECS", 30u64)?,
        |event| match event {
            Event::HistoricalData {
                req_id: event_req_id,
                ..
            }
            | Event::HistoricalDataUpdate {
                req_id: event_req_id,
                ..
            }
            | Event::HistoricalDataEnd {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                matches!(event, Event::HistoricalDataEnd { .. })
            }
            Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    if !done {
        let _ = client.cancel_historical_data(req_id).await;
    }
    Ok(())
}

async fn run_historical_ticks(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9202)?;
    client
        .req_historical_ticks(HistoricalTicksRequest {
            req_id,
            contract: contract_from_env()?,
            start_date_time: env_string("TWS_START_DATE_TIME", ""),
            end_date_time: env_string("TWS_END_DATE_TIME", ""),
            number_of_ticks: env_parse("TWS_NUMBER_OF_TICKS", 1000i32)?,
            what_to_show: env_string("TWS_WHAT_TO_SHOW", "TRADES"),
            use_rth: env_bool("TWS_USE_RTH", true)?,
            ignore_size: env_bool("TWS_IGNORE_SIZE", false)?,
            misc_options: env_tag_values("TWS_HIST_TICKS_OPTION"),
        })
        .await?;
    let done = read_until(
        client,
        env_parse("TWS_WAIT_SECS", 30u64)?,
        |event| match event {
            Event::HistoricalTicks {
                req_id: event_req_id,
                done,
                ..
            }
            | Event::HistoricalTicksBidAsk {
                req_id: event_req_id,
                done,
                ..
            }
            | Event::HistoricalTicksLast {
                req_id: event_req_id,
                done,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                done
            }
            Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    if !done {
        let _ = client.cancel_historical_ticks(req_id).await;
    }
    Ok(())
}

async fn run_head_timestamp(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9301)?;
    client
        .req_head_timestamp(HeadTimestampRequest {
            req_id,
            contract: contract_from_env()?,
            use_rth: env_bool("TWS_USE_RTH", true)?,
            what_to_show: env_string("TWS_WHAT_TO_SHOW", "TRADES"),
            format_date: env_parse("TWS_FORMAT_DATE", 1i32)?,
        })
        .await?;
    read_until(client, 20, |event| match event {
        Event::HeadTimestamp {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_head_timestamp(req_id).await;
    Ok(())
}

async fn run_histogram_data(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9302)?;
    client
        .req_histogram_data(HistogramDataRequest {
            req_id,
            contract: contract_from_env()?,
            use_rth: env_bool("TWS_USE_RTH", true)?,
            time_period: env_string("TWS_TIME_PERIOD", "1 D"),
        })
        .await?;
    read_until(client, 20, |event| match event {
        Event::HistogramData {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_histogram_data(req_id).await;
    Ok(())
}

async fn run_contract_details(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9401)?;
    client
        .req_contract_details(ContractDetailsRequest {
            req_id,
            contract: contract_from_env()?,
        })
        .await?;
    read_until(client, 20, |event| match event {
        Event::ContractDetails {
            req_id: event_req_id,
            ..
        }
        | Event::ContractDetailsEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            matches!(event, Event::ContractDetailsEnd { .. })
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_contract_data(req_id).await;
    Ok(())
}

async fn run_bond_contract_details(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9401)?;
    let mut contract = contract_from_env()?;
    contract.sec_type = "BOND".to_owned();
    client
        .req_contract_details(ContractDetailsRequest { req_id, contract })
        .await?;
    read_until(client, 20, |event| match event {
        Event::BondContractDetails {
            req_id: event_req_id,
            ..
        }
        | Event::ContractDetailsEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            matches!(event, Event::ContractDetailsEnd { .. })
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_contract_data(req_id).await;
    Ok(())
}

async fn run_smart_components(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9402)?;
    client
        .req_smart_components(req_id, &env_string("TWS_BBO_EXCHANGE", "a6"))
        .await?;
    read_until(client, 10, |event| match event {
        Event::SmartComponents {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_market_rule(client: &mut TwsApiClient) -> Result<(), CliError> {
    let market_rule_id = env_parse("TWS_MARKET_RULE_ID", 26i32)?;
    client.req_market_rule(market_rule_id).await?;
    read_until(client, 10, |event| match event {
        Event::MarketRule {
            market_rule_id: event_market_rule_id,
            ..
        } if event_market_rule_id == market_rule_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_calculate_implied_volatility(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9502)?;
    client
        .calculate_implied_volatility(CalculateImpliedVolatilityRequest {
            req_id,
            contract: contract_from_env()?,
            option_price: env_parse("TWS_OPTION_PRICE", 1.0f64)?,
            under_price: env_parse("TWS_UNDER_PRICE", 1.0f64)?,
            options: env_tag_values("TWS_IV_OPTION"),
        })
        .await?;
    let done = read_until(client, 10, |event| match event {
        Event::TickOptionComputation {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    if !done {
        let _ = client.cancel_calculate_implied_volatility(req_id).await;
    }
    Ok(())
}

async fn run_calculate_option_price(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9503)?;
    client
        .calculate_option_price(CalculateOptionPriceRequest {
            req_id,
            contract: contract_from_env()?,
            volatility: env_parse("TWS_VOLATILITY", 0.2f64)?,
            under_price: env_parse("TWS_UNDER_PRICE", 1.0f64)?,
            options: env_tag_values("TWS_OPT_PRICE_OPTION"),
        })
        .await?;
    let done = read_until(client, 10, |event| match event {
        Event::TickOptionComputation {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    if !done {
        let _ = client.cancel_calculate_option_price(req_id).await;
    }
    Ok(())
}

async fn run_exercise_options(client: &mut TwsApiClient) -> Result<(), CliError> {
    let order_id = req_id(9602)?;
    client
        .exercise_options(ExerciseOptionsRequest {
            order_id,
            contract: contract_from_env()?,
            exercise_action: env_parse("TWS_EXERCISE_ACTION", 1i32)?,
            exercise_quantity: env_parse("TWS_EXERCISE_QTY", 1i32)?,
            account: env_string("TWS_ACCOUNT", ""),
            override_system_action: env_bool("TWS_EXERCISE_OVERRIDE", false)?,
            manual_order_time: env_string("TWS_MANUAL_ORDER_TIME", ""),
            customer_account: env_string("TWS_CUSTOMER_ACCOUNT", ""),
            professional_customer: env_bool("TWS_PROFESSIONAL_CUSTOMER", false)?,
        })
        .await?;
    read_until(client, 10, |event| match event {
        Event::OpenOrder { .. } | Event::OrderStatus { .. } | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_executions(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9501)?;
    client
        .req_executions(ExecutionRequest {
            req_id,
            filter: ExecutionFilter {
                client_id: env_parse("TWS_FILTER_CLIENT_ID", 0i32)?,
                acct_code: env_string("TWS_ACCOUNT", ""),
                time: env_string("TWS_EXEC_TIME", ""),
                symbol: env_string("TWS_SYMBOL", ""),
                sec_type: env_string("TWS_SEC_TYPE", ""),
                exchange: env_string("TWS_EXCHANGE", ""),
                side: env_string("TWS_EXEC_SIDE", ""),
            },
        })
        .await?;
    read_until(client, 20, |event| match event {
        Event::ExecutionDetails {
            req_id: event_req_id,
            ..
        }
        | Event::ExecutionDetailsEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            matches!(event, Event::ExecutionDetailsEnd { .. })
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_news_bulletins(client: &mut TwsApiClient) -> Result<(), CliError> {
    let all_messages = env_bool("TWS_NEWS_ALL_MESSAGES", true)?;
    client.req_news_bulletins(all_messages).await?;
    let done = read_until(client, 15, |event| match event {
        Event::NewsBulletin { .. } => {
            print_event(&event);
            false
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    if !done {
        let _ = client.cancel_news_bulletins().await;
    }
    Ok(())
}

async fn run_news_article(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9702)?;
    client
        .req_news_article(
            req_id,
            &env_string("TWS_NEWS_PROVIDER_CODE", "BZ"),
            &env_string("TWS_NEWS_ARTICLE_ID", ""),
            &env_tag_values("TWS_NEWS_OPTION"),
        )
        .await?;
    read_until(client, 10, |event| match event {
        Event::NewsArticle {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_historical_news(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9703)?;
    client
        .req_historical_news(HistoricalNewsRequest {
            req_id,
            con_id: env_parse("TWS_CONID", 0i32)?,
            provider_codes: env_string("TWS_NEWS_PROVIDER_CODES", ""),
            start_date_time: env_string("TWS_START_DATE_TIME", ""),
            end_date_time: env_string("TWS_END_DATE_TIME", ""),
            total_results: env_parse("TWS_TOTAL_RESULTS", 10i32)?,
            options: env_tag_values("TWS_HNEWS_OPTION"),
        })
        .await?;
    read_until(client, 15, |event| match event {
        Event::HistoricalNews {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::HistoricalNewsEnd {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_scanner(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9601)?;
    client
        .req_scanner_subscription(ScannerSubscriptionRequest {
            req_id,
            subscription: ScannerSubscription {
                number_of_rows: env_parse("TWS_SCAN_ROWS", 10i32)?,
                instrument: env_string("TWS_SCAN_INSTRUMENT", "STK"),
                location_code: env_string("TWS_SCAN_LOCATION", "STK.US.MAJOR"),
                scan_code: env_string("TWS_SCAN_CODE", "TOP_PERC_GAIN"),
                above_price: env_parse("TWS_SCAN_ABOVE_PRICE", 0.0f64)?,
                below_price: env_parse("TWS_SCAN_BELOW_PRICE", 0.0f64)?,
                above_volume: env_parse("TWS_SCAN_ABOVE_VOLUME", 0i32)?,
                market_cap_above: env_parse("TWS_SCAN_MARKET_CAP_ABOVE", 0.0f64)?,
                market_cap_below: env_parse("TWS_SCAN_MARKET_CAP_BELOW", 0.0f64)?,
                moody_rating_above: env_string("TWS_SCAN_MOODY_ABOVE", ""),
                moody_rating_below: env_string("TWS_SCAN_MOODY_BELOW", ""),
                sp_rating_above: env_string("TWS_SCAN_SP_ABOVE", ""),
                sp_rating_below: env_string("TWS_SCAN_SP_BELOW", ""),
                maturity_date_above: env_string("TWS_SCAN_MATURITY_ABOVE", ""),
                maturity_date_below: env_string("TWS_SCAN_MATURITY_BELOW", ""),
                coupon_rate_above: env_parse("TWS_SCAN_COUPON_ABOVE", 0.0f64)?,
                coupon_rate_below: env_parse("TWS_SCAN_COUPON_BELOW", 0.0f64)?,
                exclude_convertible: env_bool("TWS_SCAN_EXCLUDE_CONVERTIBLE", false)?,
                average_option_volume_above: env_parse("TWS_SCAN_AVG_OPTION_VOLUME_ABOVE", 0i32)?,
                scanner_setting_pairs: env_string("TWS_SCAN_SETTING_PAIRS", ""),
                stock_type_filter: env_string("TWS_SCAN_STOCK_TYPE_FILTER", ""),
            },
            scanner_subscription_options: env_tag_values("TWS_SCAN_OPTION"),
            scanner_subscription_filter_options: env_tag_values("TWS_SCAN_FILTER"),
        })
        .await?;
    let done = read_until(
        client,
        env_parse("TWS_WAIT_SECS", 20u64)?,
        |event| match event {
            Event::ScannerData {
                req_id: event_req_id,
                ..
            }
            | Event::ScannerDataEnd {
                req_id: event_req_id,
            } if event_req_id == req_id => {
                print_event(&event);
                matches!(event, Event::ScannerDataEnd { .. })
            }
            Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    if !done {
        let _ = client.cancel_scanner_subscription(req_id).await;
    }
    Ok(())
}

async fn run_scanner_parameters(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_scanner_parameters().await?;
    read_until(client, 10, |event| match event {
        Event::ScannerParameters { .. } | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_sec_def_opt_params(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9802)?;
    client
        .req_sec_def_opt_params(
            req_id,
            &env_string("TWS_UNDERLYING_SYMBOL", &env_string("TWS_SYMBOL", "AAPL")),
            &env_string("TWS_FUT_FOP_EXCHANGE", "SMART"),
            &env_string("TWS_UNDERLYING_SEC_TYPE", "STK"),
            env_parse("TWS_UNDERLYING_CON_ID", env_parse("TWS_CONID", 0i32)?)?,
        )
        .await?;
    read_until(client, 20, |event| match event {
        Event::SecurityDefinitionOptionParameter {
            req_id: event_req_id,
            ..
        }
        | Event::SecurityDefinitionOptionParameterEnd {
            req_id: event_req_id,
        } if event_req_id == req_id => {
            print_event(&event);
            matches!(event, Event::SecurityDefinitionOptionParameterEnd { .. })
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_soft_dollar_tiers(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9803)?;
    client.req_soft_dollar_tiers(req_id).await?;
    read_until(client, 10, |event| match event {
        Event::SoftDollarTiers {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_family_codes(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_family_codes().await?;
    read_until(client, 10, |event| match event {
        Event::FamilyCodes { .. } | Event::Error { .. } => {
            print_event(&event);
            true
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_matching_symbols(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9804)?;
    client
        .req_matching_symbols(req_id, &env_string("TWS_MATCH_PATTERN", "AAPL"))
        .await?;
    read_until(client, 10, |event| match event {
        Event::SymbolSamples {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_query_display_groups(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9805)?;
    client.query_display_groups(req_id).await?;
    read_until(client, 10, |event| match event {
        Event::DisplayGroupList {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_subscribe_to_group_events(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9806)?;
    let group_id = env_parse("TWS_GROUP_ID", 1i32)?;
    client.subscribe_to_group_events(req_id, group_id).await?;
    let done = read_until(client, 15, |event| match event {
        Event::DisplayGroupList {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::DisplayGroupUpdated {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    if !done {
        let _ = client.unsubscribe_from_group_events(req_id).await;
    }
    Ok(())
}

async fn run_update_display_group(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9807)?;
    client
        .update_display_group(req_id, &env_string("TWS_CONTRACT_INFO", ""))
        .await?;
    read_until(client, 10, |event| match event {
        Event::DisplayGroupUpdated {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_unsubscribe_from_group_events(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9806)?;
    client.unsubscribe_from_group_events(req_id).await?;
    println!("unsubscribe_from_group_events req_id={req_id}");
    Ok(())
}

async fn run_request_fa(client: &mut TwsApiClient) -> Result<(), CliError> {
    let fa_data_type = env_parse("TWS_FA_DATA_TYPE", 1i32)?;
    client.request_fa(fa_data_type).await?;
    read_until(client, 10, |event| match event {
        Event::ReceiveFa {
            fa_data_type: event_type,
            ..
        } if event_type == fa_data_type => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_replace_fa(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9808)?;
    let fa_data_type = env_parse("TWS_FA_DATA_TYPE", 1i32)?;
    client
        .replace_fa(req_id, fa_data_type, &env_string("TWS_FA_XML", ""))
        .await?;
    read_until(client, 10, |event| match event {
        Event::ReplaceFaEnd {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_wsh_meta_data(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9809)?;
    client.req_wsh_meta_data(req_id).await?;
    read_until(client, 10, |event| match event {
        Event::WshMetaData {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_wsh_meta_data(req_id).await;
    Ok(())
}

async fn run_wsh_event_data(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9810)?;
    client
        .req_wsh_event_data(WshEventDataRequest {
            req_id,
            con_id: env_parse("TWS_CONID", 0i32)?,
            filter: env_string("TWS_WSH_FILTER", ""),
            fill_watchlist: env_bool("TWS_WSH_FILL_WATCHLIST", false)?,
            fill_portfolio: env_bool("TWS_WSH_FILL_PORTFOLIO", false)?,
            fill_competitors: env_bool("TWS_WSH_FILL_COMPETITORS", false)?,
            start_date: env_string("TWS_START_DATE", ""),
            end_date: env_string("TWS_END_DATE", ""),
            total_limit: env_parse("TWS_TOTAL_LIMIT", 100i32)?,
        })
        .await?;
    let done = read_until(client, 10, |event| match event {
        Event::WshEventData {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    if !done {
        let _ = client.cancel_wsh_event_data(req_id).await;
    }
    Ok(())
}

async fn run_user_info(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9811)?;
    client.req_user_info(req_id).await?;
    read_until(client, 10, |event| match event {
        Event::UserInfo {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            true
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}
async fn run_real_time_bars(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9701)?;
    client
        .req_real_time_bars(RealTimeBarsRequest {
            req_id,
            contract: contract_from_env()?,
            bar_size: env_parse("TWS_BAR_SIZE", 5i32)?,
            what_to_show: env_string("TWS_WHAT_TO_SHOW", "TRADES"),
            use_rth: env_bool("TWS_USE_RTH", true)?,
            options: env_tag_values("TWS_RTBAR_OPTION"),
        })
        .await?;
    let done = read_until(
        client,
        env_parse("TWS_WAIT_SECS", 20u64)?,
        |event| match event {
            Event::RealTimeBar {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                false
            }
            Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    if !done {
        let _ = client.cancel_real_time_bars(req_id).await;
    }
    Ok(())
}

async fn run_tick_by_tick(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9801)?;
    client
        .req_tick_by_tick_data(TickByTickRequest {
            req_id,
            contract: contract_from_env()?,
            tick_type: env_string("TWS_TICK_TYPE", "Last"),
            number_of_ticks: env_parse("TWS_NUMBER_OF_TICKS", 0i32)?,
            ignore_size: env_bool("TWS_IGNORE_SIZE", false)?,
        })
        .await?;
    let done = read_until(
        client,
        env_parse("TWS_WAIT_SECS", 20u64)?,
        |event| match event {
            Event::TickByTick {
                req_id: event_req_id,
                ..
            } if event_req_id == req_id => {
                print_event(&event);
                false
            }
            Event::Error { .. } => {
                print_event(&event);
                false
            }
            other => {
                print_event(&other);
                false
            }
        },
    )
    .await?;
    if !done {
        let _ = client.cancel_tick_by_tick_data(req_id).await;
    }
    Ok(())
}

async fn run_pnl(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9901)?;
    client
        .req_pnl(
            req_id,
            &env_string("TWS_ACCOUNT", ""),
            &env_string("TWS_MODEL_CODE", ""),
        )
        .await?;
    read_until(client, 10, |event| match event {
        Event::Pnl {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_pnl(req_id).await;
    Ok(())
}

async fn run_pnl_single(client: &mut TwsApiClient) -> Result<(), CliError> {
    let req_id = req_id(9902)?;
    client
        .req_pnl_single(
            req_id,
            &env_string("TWS_ACCOUNT", ""),
            &env_string("TWS_MODEL_CODE", ""),
            env_parse("TWS_CONID", 0i32)?,
        )
        .await?;
    read_until(client, 10, |event| match event {
        Event::PnlSingle {
            req_id: event_req_id,
            ..
        } if event_req_id == req_id => {
            print_event(&event);
            false
        }
        Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client.cancel_pnl_single(req_id).await;
    Ok(())
}

async fn run_managed_accounts(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_managed_accounts().await?;
    read_until(client, 5, |event| match event {
        Event::ManagedAccounts { .. } | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_disconnect(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.disconnect().await?;
    println!("disconnected");
    Ok(())
}

async fn run_account_updates(client: &mut TwsApiClient) -> Result<(), CliError> {
    client
        .req_account_updates(
            env_bool("TWS_ACCOUNT_SUBSCRIBE", true)?,
            &env_string("TWS_ACCOUNT", ""),
        )
        .await?;
    read_until(client, 10, |event| match event {
        Event::AccountValue { .. }
        | Event::PortfolioValue { .. }
        | Event::AccountUpdateTime { .. }
        | Event::AccountDownloadEnd { .. }
        | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    let _ = client
        .req_account_updates(false, &env_string("TWS_ACCOUNT", ""))
        .await;
    Ok(())
}

async fn run_market_depth_exchanges(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_mkt_depth_exchanges().await?;
    read_until(client, 5, |event| match event {
        Event::MarketDepthExchanges { .. } | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}

async fn run_news_providers(client: &mut TwsApiClient) -> Result<(), CliError> {
    client.req_news_providers().await?;
    read_until(client, 5, |event| match event {
        Event::NewsProviders { .. } | Event::Error { .. } => {
            print_event(&event);
            false
        }
        other => {
            print_event(&other);
            false
        }
    })
    .await?;
    Ok(())
}
