//! Interactive CLI for the enabled Binance REST modules.
//!
//! It covers every REST endpoint exposed by the official SDK's Spot, Margin Trading, Options,
//! and Convert modules through their public raw-request APIs. Run the example without arguments
//! for the prompt, then type help. Credentials are read only at runtime and are never printed.

use anyhow::{Context, Result, bail};
use binance_sdk::{
    config::{
        ConfigurationRestApi, ConfigurationWebsocketApi, ConfigurationWebsocketStreams, PrivateKey,
    },
    convert::ConvertRestApi,
    derivatives_trading_options::{
        DerivativesTradingOptionsRestApi, DerivativesTradingOptionsWsStreams,
    },
    margin_trading::{MarginTradingRestApi, MarginTradingWsStreams},
    models::RestApiResponse,
    spot::{SpotRestApi, SpotWsApi, SpotWsStreams},
};
use http::Method;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::Path,
};

const DEFAULT_API_KEY_FILE: &str =
    "/Users/jiayin/workspace/dev/dev/rust/truefix-account/binance/readme";
const DEFAULT_PRIVATE_KEY_PATH: &str =
    "/Users/jiayin/workspace/dev/dev/rust/truefix-account/binance/Ed25519/test-prv-key.pem";

#[derive(Clone, Copy)]
enum Module {
    Spot,
    Margin,
    Options,
    Convert,
}

struct Request {
    module: Module,
    method: Method,
    path: String,
    query: BTreeMap<String, Value>,
    body: BTreeMap<String, Value>,
    signed: bool,
    confirm_write: bool,
}

fn api_key(path: &Path) -> Result<String> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let key = contents
        .lines()
        .skip_while(|line| !line.contains("API Key:"))
        .skip(1)
        .map(str::trim)
        .find(|line| !line.is_empty())
        .context("no API key found after API Key:")?;
    if key.chars().any(char::is_whitespace) {
        bail!("the Binance API-key line must contain exactly one key");
    }
    Ok(key.to_owned())
}

fn config() -> Result<ConfigurationRestApi> {
    let key_file =
        std::env::var("BINANCE_API_KEY_FILE").unwrap_or_else(|_| DEFAULT_API_KEY_FILE.to_owned());
    let private_key = std::env::var("BINANCE_PRIVATE_KEY_PATH")
        .unwrap_or_else(|_| DEFAULT_PRIVATE_KEY_PATH.to_owned());
    Ok(ConfigurationRestApi::builder()
        .api_key(api_key(Path::new(&key_file))?)
        .private_key(PrivateKey::File(private_key))
        .build()?)
}

fn websocket_config() -> Result<ConfigurationWebsocketApi> {
    let key_file =
        std::env::var("BINANCE_API_KEY_FILE").unwrap_or_else(|_| DEFAULT_API_KEY_FILE.to_owned());
    let private_key = std::env::var("BINANCE_PRIVATE_KEY_PATH")
        .unwrap_or_else(|_| DEFAULT_PRIVATE_KEY_PATH.to_owned());
    Ok(ConfigurationWebsocketApi::builder()
        .api_key(api_key(Path::new(&key_file))?)
        .private_key(PrivateKey::File(private_key))
        .build()?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty() {
        repl().await
    } else if matches!(arguments[0].as_str(), "help" | "h" | "?") {
        help();
        Ok(())
    } else if matches!(arguments[0].as_str(), "catalog" | "commands") {
        catalog(arguments.get(1).map(String::as_str));
        Ok(())
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "stream")
    {
        stream(&arguments).await
    } else if arguments.first().is_some_and(|argument| argument == "ws") {
        websocket_api(&arguments).await
    } else {
        execute(parse(&arguments)?).await
    }
}

async fn repl() -> Result<()> {
    println!("Binance CLI: Spot / Margin / Options / Convert. Type help for commands.");
    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("binance> ");
        io::stdout().flush()?;
        line.clear();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input, "exit" | "quit" | "q") {
            break;
        }
        let arguments = match split_args(input) {
            Ok(arguments) => arguments,
            Err(error) => {
                eprintln!("{error:#}");
                continue;
            }
        };
        if arguments
            .first()
            .is_some_and(|argument| matches!(argument.as_str(), "help" | "h" | "?"))
        {
            help();
            continue;
        }
        if arguments
            .first()
            .is_some_and(|argument| matches!(argument.as_str(), "catalog" | "commands"))
        {
            catalog(arguments.get(1).map(String::as_str));
            continue;
        }
        if arguments
            .first()
            .is_some_and(|argument| argument == "stream")
        {
            if let Err(error) = stream(&arguments).await {
                eprintln!("{error:#}");
            }
            continue;
        }
        if arguments.first().is_some_and(|argument| argument == "ws") {
            if let Err(error) = websocket_api(&arguments).await {
                eprintln!("{error:#}");
            }
            continue;
        }
        match parse(&arguments) {
            Ok(request) => {
                if let Err(error) = execute(request).await {
                    eprintln!("{error:#}");
                }
            }
            Err(error) => eprintln!("{error:#}"),
        }
    }
    Ok(())
}

async fn stream(arguments: &[String]) -> Result<()> {
    if arguments.len() != 3 {
        bail!("usage: stream <spot|margin|options> <stream-name>");
    }
    let configuration = ConfigurationWebsocketStreams::builder().build()?;
    let stream_name = arguments[2].to_owned();
    match arguments[1].as_str() {
        "spot" => {
            let connection = SpotWsStreams::production(configuration).connect().await?;
            let _events = connection.subscribe_on_ws_events(|event| println!("{event:?}"));
            connection.subscribe(vec![stream_name], None);
            wait_for_disconnect().await;
            connection.disconnect().await?;
        }
        "margin" => {
            let connection = MarginTradingWsStreams::production(configuration)
                .connect()
                .await?;
            let _events = connection.subscribe_on_ws_events(|event| println!("{event:?}"));
            connection.subscribe(vec![stream_name], None);
            wait_for_disconnect().await;
            connection.disconnect().await?;
        }
        "options" => {
            let connection = DerivativesTradingOptionsWsStreams::production(configuration)
                .connect()
                .await?;
            let _events = connection.subscribe_on_ws_events(|event| println!("{event:?}"));
            connection.subscribe(vec![stream_name], None);
            wait_for_disconnect().await;
            connection.disconnect().await?;
        }
        _ => bail!("stream module must be spot, margin, or options"),
    }
    Ok(())
}

async fn wait_for_disconnect() {
    println!("stream active; press Ctrl-C to disconnect");
    let _ = tokio::signal::ctrl_c().await;
}

async fn websocket_api(arguments: &[String]) -> Result<()> {
    if arguments.len() < 2 {
        bail!("usage: ws <method> [PARAMS_JSON] [--public] [--confirm-write]");
    }
    let is_public = arguments.iter().any(|argument| argument == "--public");
    if !is_public && websocket_write(&arguments[1]) && !has_flag(arguments, "--confirm-write") {
        bail!("this WebSocket API action requires --confirm-write");
    }
    let payload = arguments
        .iter()
        .skip(2)
        .find(|argument| !argument.starts_with("--"))
        .map(|value| object(value, "params"))
        .transpose()?
        .unwrap_or_default();
    let config = if is_public {
        ConfigurationWebsocketApi::builder().build()?
    } else {
        websocket_config()?
    };
    let connection = SpotWsApi::production(config).connect().await?;
    let response = if is_public {
        connection
            .send_message::<Value>(&arguments[1], payload)
            .await?
    } else {
        connection
            .send_signed_message::<Value>(&arguments[1], payload)
            .await?
    };
    println!("{}", serde_json::to_string_pretty(&response.data()?)?);
    connection.disconnect().await?;
    Ok(())
}

async fn execute(request: Request) -> Result<()> {
    if request.method != Method::GET && !request.confirm_write {
        bail!("write requests require --confirm-write");
    }
    let configuration = if request.signed {
        config()?
    } else {
        ConfigurationRestApi::builder().build()?
    };
    let response = match request.module {
        Module::Spot => send(SpotRestApi::production(configuration), &request).await?,
        Module::Margin => send(MarginTradingRestApi::production(configuration), &request).await?,
        Module::Options => {
            send(
                DerivativesTradingOptionsRestApi::production(configuration),
                &request,
            )
            .await?
        }
        Module::Convert => send(ConvertRestApi::production(configuration), &request).await?,
    };
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

trait RawClient {
    fn send<'a>(
        &'a self,
        signed: bool,
        path: &'a str,
        method: Method,
        query: BTreeMap<String, Value>,
        body: BTreeMap<String, Value>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<RestApiResponse<Value>>> + Send + 'a>,
    >;
}

macro_rules! raw_client {
    ($type:path) => {
        impl RawClient for $type {
            fn send<'a>(
                &'a self,
                signed: bool,
                path: &'a str,
                method: Method,
                query: BTreeMap<String, Value>,
                body: BTreeMap<String, Value>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<RestApiResponse<Value>>> + Send + 'a>,
            > {
                Box::pin(async move {
                    if signed {
                        Ok(self.send_signed_request(path, method, query, body).await?)
                    } else {
                        Ok(self.send_request(path, method, query, body).await?)
                    }
                })
            }
        }
    };
}

raw_client!(binance_sdk::spot::rest_api::RestApi);
raw_client!(binance_sdk::margin_trading::rest_api::RestApi);
raw_client!(binance_sdk::convert::rest_api::RestApi);
raw_client!(binance_sdk::derivatives_trading_options::rest_api::RestApi);

async fn send(client: impl RawClient, request: &Request) -> Result<Value> {
    Ok(client
        .send(
            request.signed,
            &request.path,
            request.method.clone(),
            request.query.clone(),
            request.body.clone(),
        )
        .await?
        .data()
        .await?)
}

fn parse(arguments: &[String]) -> Result<Request> {
    let arguments = request_alias(arguments)?;
    if arguments.len() < 4 || arguments[0] != "request" {
        bail!(
            "usage: request <spot|margin|options|convert> <METHOD> <PATH> [QUERY_JSON] [BODY_JSON] [--public] [--confirm-write]"
        );
    }
    let module = match arguments[1].as_str() {
        "spot" => Module::Spot,
        "margin" => Module::Margin,
        "options" => Module::Options,
        "convert" => Module::Convert,
        _ => bail!("unknown module; use spot, margin, options, or convert"),
    };
    let values: Vec<&String> = arguments[4..]
        .iter()
        .filter(|argument| !argument.starts_with("--"))
        .collect();
    if values.len() > 2 {
        bail!("only QUERY_JSON and BODY_JSON may be supplied");
    }
    Ok(Request {
        module,
        method: Method::from_bytes(arguments[2].to_ascii_uppercase().as_bytes())?,
        path: arguments[3].clone(),
        query: values
            .first()
            .map(|value| object(value, "query"))
            .transpose()?
            .unwrap_or_default(),
        body: values
            .get(1)
            .map(|value| object(value, "body"))
            .transpose()?
            .unwrap_or_default(),
        signed: !has_flag(&arguments, "--public"),
        confirm_write: has_flag(&arguments, "--confirm-write"),
    })
}

fn object(value: &str, label: &str) -> Result<BTreeMap<String, Value>> {
    serde_json::from_str(value).with_context(|| format!("{label} must be a JSON object"))
}

fn request_alias(arguments: &[String]) -> Result<Vec<String>> {
    let Some(command) = arguments.first() else {
        return Ok(Vec::new());
    };
    let method = match command.as_str() {
        "get" => "GET",
        "post" => "POST",
        "put" => "PUT",
        "delete" | "del" => "DELETE",
        "request" => return Ok(arguments.to_vec()),
        _ => return Ok(arguments.to_vec()),
    };
    if arguments.len() < 3 {
        bail!(
            "usage: {command} <spot|margin|options|convert> <PATH> [QUERY_JSON] [BODY_JSON] [--public] [--confirm-write]"
        );
    }
    Ok(std::iter::once("request".to_owned())
        .chain(std::iter::once(arguments[1].clone()))
        .chain(std::iter::once(method.to_owned()))
        .chain(arguments[2..].iter().cloned())
        .collect())
}

fn has_flag(arguments: &[String], flag: &str) -> bool {
    arguments.iter().any(|argument| argument == flag)
}

fn websocket_write(method: &str) -> bool {
    method.starts_with("order.")
        || method.starts_with("orderList.")
        || method.starts_with("openOrders.")
        || matches!(
            method,
            "session.logout" | "userDataStream.start" | "userDataStream.stop"
        )
}

/// Split a command line while retaining JSON quotes. Quote a JSON document with single quotes
/// when it contains whitespace, e.g. `get spot /api/v3/klines '{"symbol": "BTCUSDT"}' --public`.
fn split_args(input: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut json_depth = 0usize;
    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' && (quote.is_some() || json_depth > 0) {
            current.push(character);
            escaped = true;
        } else if Some(character) == quote {
            quote = None;
        } else if quote.is_none() && json_depth == 0 && matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if quote.is_none() && matches!(character, '{' | '[') {
            json_depth += 1;
            current.push(character);
        } else if quote.is_none() && matches!(character, '}' | ']') {
            json_depth = json_depth
                .checked_sub(1)
                .context("unexpected JSON closing delimiter")?;
            current.push(character);
        } else if quote.is_none() && json_depth == 0 && character.is_whitespace() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if escaped || quote.is_some() || json_depth != 0 {
        bail!("unterminated quoted argument");
    }
    if !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

fn help() {
    println!(
        "request <spot|margin|options|convert> <GET|POST|PUT|DELETE> <PATH> [QUERY_JSON] [BODY_JSON] [--public] [--confirm-write]\n\
         <get|post|put|delete> <module> <PATH> [QUERY_JSON] [BODY_JSON] [--public] [--confirm-write]\n\
         catalog [spot|margin|options|convert]\n\
\n\
Every REST endpoint generated by the four enabled official SDK modules is available.\n\
Examples:\n\
  get spot /api/v3/exchangeInfo {{\"symbol\":\"BTCUSDT\"}} --public\n\
  get spot /api/v3/account\n\
  get margin /sapi/v1/margin/account\n\
  get options /eapi/v1/exchangeInfo --public\n\
  get convert /sapi/v1/convert/exchangeInfo\n\
  stream spot btcusdt@trade\n\
  stream margin btcusdt@bookTicker\n\
  stream options btcusdt@trade\n\
  ws time --public\n\
  ws account.status\n\
\n\
Signed requests are the default. Public endpoints need --public. Every non-GET request requires --confirm-write.\n\
The stream command accepts every raw stream name supported by its official SDK module.\n\
The ws command accesses every Spot WebSocket API method. Use --confirm-write for order/session actions.\n\
In the REPL, JSON may be written directly; quote it with single quotes if it contains whitespace."
    );
}

fn catalog(module: Option<&str>) {
    let requested = module.unwrap_or("all");
    if !matches!(requested, "all" | "spot" | "margin" | "options" | "convert") {
        eprintln!("unknown module; use spot, margin, options, or convert");
        return;
    }
    println!(
        "The generic request command covers every REST endpoint exposed by the enabled official SDK modules."
    );
    if matches!(requested, "all" | "spot") {
        println!(
            "spot: /api/v3/exchangeInfo, /api/v3/ticker/price, /api/v3/depth, /api/v3/account, /api/v3/order"
        );
    }
    if matches!(requested, "all" | "margin") {
        println!(
            "margin: /sapi/v1/margin/account, /sapi/v1/margin/order, /sapi/v1/margin/allOrders, /sapi/v1/margin/isolated/account"
        );
    }
    if matches!(requested, "all" | "options") {
        println!(
            "options: /eapi/v1/exchangeInfo, /eapi/v1/ticker, /eapi/v1/depth, /eapi/v1/account, /eapi/v1/order"
        );
    }
    if matches!(requested, "all" | "convert") {
        println!(
            "convert: /sapi/v1/convert/exchangeInfo, /sapi/v1/convert/getQuote, /sapi/v1/convert/acceptQuote, /sapi/v1/convert/tradeFlow"
        );
    }
    println!(
        "Use `help` for syntax. Verify endpoint parameters and permissions against Binance documentation before sending a request."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_json_with_whitespace_without_losing_its_quotes() {
        assert_eq!(
            split_args("get spot /api/v3/ticker/price {\"symbol\": \"BTCUSDT\"} --public").unwrap(),
            [
                "get",
                "spot",
                "/api/v3/ticker/price",
                "{\"symbol\": \"BTCUSDT\"}",
                "--public"
            ]
        );
    }

    #[test]
    fn expands_short_request_alias() {
        let request = parse(&[
            "get".to_owned(),
            "spot".to_owned(),
            "/api/v3/time".to_owned(),
            "--public".to_owned(),
        ])
        .unwrap();
        assert_eq!(request.method, Method::GET);
        assert!(!request.signed);
    }
}
