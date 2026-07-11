//! Interactive CLI for the complete audited OKX REST surface.
//!
//! Run `cargo run -p truefix-okx-client --example okx_cli` and type `help`.
//! `request` accepts only operations registered in the client's 264-operation baseline
//! manifest. It never prints credentials, defaults to OKX Demo, and requires
//! `--confirm-write` for every POST operation.

use std::{
    collections::BTreeMap,
    env, fs,
    io::{self, Write},
    path::Path,
};

use serde_json::Value;
use truefix_okx_client::{
    ClientConfig, Credentials, Environment, LiveTradingConfirmation, OkxClient,
    inventory::{AuthClass, BASELINE_OPERATION_MANIFEST},
};

const DEFAULT_CREDENTIAL_FILE: &str =
    "/Users/jiayin/workspace/dev/dev/rust/truefix-account/okx/okx";

type CliResult<T> = Result<T, Box<dyn std::error::Error>>;

struct Request {
    domain: String,
    operation: String,
    requested_method: Option<String>,
    query: BTreeMap<String, String>,
    body: Option<Value>,
    confirm_write: bool,
}

#[tokio::main]
async fn main() -> CliResult<()> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let client = connect_client()?;
    if arguments.is_empty() {
        repl(&client).await
    } else if matches!(
        arguments.first().map(String::as_str),
        Some("help" | "h" | "?")
    ) {
        print_help();
        Ok(())
    } else if arguments
        .first()
        .is_some_and(|argument| matches!(argument.as_str(), "catalog" | "list"))
    {
        list_operations(arguments.get(1).map(String::as_str));
        Ok(())
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "overview")
    {
        overview(&client, arguments.get(1).map(String::as_str)).await
    } else {
        execute(&client, parse_request(&arguments)?).await
    }
}

fn connect_client() -> CliResult<OkxClient> {
    let credentials = credentials()?;
    let live = env::var("OKX_ENV").is_ok_and(|value| value.eq_ignore_ascii_case("live"));
    let config = if live {
        if !env::var("OKX_CONFIRM_LIVE").is_ok_and(|value| value == "1") {
            return Err("set OKX_CONFIRM_LIVE=1 to select the live trading environment".into());
        }
        let credentials = credentials.ok_or("live mode requires OKX credentials")?;
        ClientConfig::live(credentials, LiveTradingConfirmation::acknowledge_risk())
    } else {
        ClientConfig::demo(credentials)
    };
    let environment = if matches!(config.environment, Environment::Demo) {
        "Demo"
    } else {
        "Live"
    };
    println!("OKX CLI ({environment}); type `help` for commands.");
    Ok(OkxClient::new(config)?)
}

/// Loads credentials only when the caller supplies a passphrase or key environment variable.
/// This keeps public commands usable on hosts without local private credentials.
fn credentials() -> CliResult<Option<Credentials>> {
    let requested = ["OKX_API_KEY", "OKX_SECRET", "OKX_PASSPHRASE"]
        .iter()
        .any(|name| env::var(name).is_ok_and(|value| !value.is_empty()));
    if !requested {
        return Ok(None);
    }
    let credential_file =
        env::var("OKX_CREDENTIAL_FILE").unwrap_or_else(|_| DEFAULT_CREDENTIAL_FILE.to_owned());
    let path = Path::new(&credential_file);
    Ok(Some(Credentials::new(
        environment_or_file("OKX_API_KEY", "apikey", path)?,
        environment_or_file("OKX_SECRET", "secretkey", path)?,
        env::var("OKX_PASSPHRASE")?,
    )?))
}

fn environment_or_file(environment: &str, file_key: &str, path: &Path) -> CliResult<String> {
    match env::var(environment) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => credential_from_file(file_key, path),
    }
}

fn credential_from_file(name: &str, path: &Path) -> CliResult<String> {
    let contents = fs::read_to_string(path)?;
    contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .find_map(|(key, value)| (key.trim().eq_ignore_ascii_case(name)).then(|| value.trim()))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("missing {name} in {}", path.display()).into())
}

async fn repl(client: &OkxClient) -> CliResult<()> {
    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("okx> ");
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
        let arguments = match split_args(input) {
            Ok(arguments) => arguments,
            Err(error) => {
                eprintln!("{error}");
                continue;
            }
        };
        let result = match arguments.first().map(String::as_str) {
            Some("help" | "h" | "?") => {
                print_help();
                Ok(())
            }
            Some("catalog" | "list") => {
                list_operations(arguments.get(1).map(String::as_str));
                Ok(())
            }
            Some("overview") => overview(client, arguments.get(1).map(String::as_str)).await,
            _ => match parse_request(&arguments) {
                Ok(request) => execute(client, request).await,
                Err(error) => Err(error),
            },
        };
        if let Err(error) = result {
            eprintln!("{error}");
        }
    }
    Ok(())
}

async fn execute(client: &OkxClient, request: Request) -> CliResult<()> {
    let entry = BASELINE_OPERATION_MANIFEST
        .iter()
        .find(|entry| entry.domain == request.domain && entry.operation == request.operation)
        .ok_or_else(|| {
            format!(
                "unknown operation {}/{}; use `list`",
                request.domain, request.operation
            )
        })?;
    if entry.method == "POST" && !request.confirm_write {
        return Err("POST operations require --confirm-write".into());
    }
    if let Some(method) = request.requested_method.as_deref()
        && method != entry.method
    {
        return Err(format!(
            "{method} does not match {}/{}; the audited operation uses {}",
            request.domain, request.operation, entry.method
        )
        .into());
    }
    let response = client
        .execute_baseline_operation(
            &request.domain,
            &request.operation,
            request.query,
            request.body.as_ref(),
        )
        .await?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

async fn overview(client: &OkxClient, symbol: Option<&str>) -> CliResult<()> {
    let symbol = symbol.unwrap_or("BTC-USDT");
    let ticker = client.market().ticker(symbol).await?;
    let books = client.market().books(symbol, Some(5)).await?;
    println!("ticker:\n{ticker:#?}");
    println!("top-of-book:\n{books:#?}");
    if client.config().credentials.is_some() {
        let balances = client.account().balances().await?;
        let positions = client.account().positions().await?;
        let orders = client.trade().orders(BTreeMap::new()).await?;
        println!("balances:\n{balances:#?}");
        println!("positions:\n{positions:#?}");
        println!("open orders:\n{orders:#?}");
    } else {
        println!("Set OKX_PASSPHRASE (and configure key/secret) to include private account data.");
    }
    Ok(())
}

fn parse_request(arguments: &[String]) -> CliResult<Request> {
    let requested_method = match arguments.first().map(String::as_str) {
        Some("get") => Some("GET".to_owned()),
        Some("post") => Some("POST".to_owned()),
        _ => None,
    };
    let arguments = request_alias(arguments)?;
    if arguments.len() < 3 || arguments.first().map(String::as_str) != Some("request") {
        return Err(
            "usage: request <domain> <operation> [QUERY_JSON] [BODY_JSON] [--confirm-write]".into(),
        );
    }
    let values = arguments[3..]
        .iter()
        .filter(|argument| !argument.starts_with("--"))
        .collect::<Vec<_>>();
    if values.len() > 2 {
        return Err("only QUERY_JSON and BODY_JSON may be supplied".into());
    }
    Ok(Request {
        domain: arguments[1].clone(),
        operation: arguments[2].clone(),
        requested_method,
        query: values
            .first()
            .map(|value| query_object(value))
            .transpose()?
            .unwrap_or_default(),
        body: values.get(1).map(|value| json_object(value)).transpose()?,
        confirm_write: arguments
            .iter()
            .any(|argument| argument == "--confirm-write"),
    })
}

fn request_alias(arguments: &[String]) -> CliResult<Vec<String>> {
    let Some(command) = arguments.first() else {
        return Ok(Vec::new());
    };
    match command.as_str() {
        "request" => Ok(arguments.to_vec()),
        "get" | "post" => {
            if arguments.len() < 3 {
                return Err(format!(
                    "usage: {command} <domain> <operation> [QUERY_JSON] [BODY_JSON] [--confirm-write]"
                )
                .into());
            }
            Ok(std::iter::once("request".to_owned())
                .chain(arguments[1..].iter().cloned())
                .collect())
        }
        _ => Ok(arguments.to_vec()),
    }
}

fn query_object(value: &str) -> CliResult<BTreeMap<String, String>> {
    let object: serde_json::Map<String, Value> = serde_json::from_str(value)?;
    object
        .into_iter()
        .map(|(key, value)| match value {
            Value::String(value) => Ok((key, value)),
            Value::Number(value) => Ok((key, value.to_string())),
            Value::Bool(value) => Ok((key, value.to_string())),
            _ => {
                Err(format!("query value for `{key}` must be a string, number, or boolean").into())
            }
        })
        .collect()
}

fn json_object(value: &str) -> CliResult<Value> {
    let value: Value = serde_json::from_str(value)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err("BODY_JSON must be a JSON object".into())
    }
}

/// Split a command line while retaining JSON quotes. Quote a JSON document with single quotes
/// when it contains whitespace.
fn split_args(input: &str) -> CliResult<Vec<String>> {
    let mut arguments = Vec::new();
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
                .ok_or("unexpected JSON closing delimiter")?;
            current.push(character);
        } else if quote.is_none() && json_depth == 0 && character.is_whitespace() {
            if !current.is_empty() {
                arguments.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if escaped || quote.is_some() || json_depth != 0 {
        return Err("unterminated quoted argument".into());
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    Ok(arguments)
}

fn list_operations(domain: Option<&str>) {
    for entry in BASELINE_OPERATION_MANIFEST
        .iter()
        .filter(|entry| domain.is_none_or(|domain| entry.domain == domain))
    {
        let auth = match entry.auth {
            AuthClass::Public => "public",
            AuthClass::Private => "private",
        };
        println!(
            "{:<18} {:<42} {:<4} {auth}",
            entry.domain, entry.operation, entry.method
        );
    }
}

fn print_help() {
    println!(
        "request <domain> <operation> [QUERY_JSON] [BODY_JSON] [--confirm-write]\n\
         <get|post> <domain> <operation> [QUERY_JSON] [BODY_JSON] [--confirm-write]\n\
         catalog [domain]\n\
         overview [INST_ID]\n\
\n\
         `request` covers every REST operation audited by truefix-okx-client; `list` prints the\n\
         accepted domain/operation pairs. GET entries use QUERY_JSON; POST entries additionally\n\
         need BODY_JSON and --confirm-write. Authentication is derived from the operation manifest.\n\
\n\
         Examples:\n\
           request market_data get_ticker {{\"instId\":\"BTC-USDT\"}}\n\
           request public_data get_instruments {{\"instType\":\"SPOT\"}}\n\
           request account get_account_balance\n\
           request trade place_order {{}} {{\"instId\":\"BTC-USDT\",\"tdMode\":\"cash\",\"side\":\"buy\",\"ordType\":\"limit\",\"sz\":\"0.001\",\"px\":\"1\"}} --confirm-write\n\
\n\
         Credentials: set OKX_PASSPHRASE; API key and secret come from OKX_API_KEY/OKX_SECRET\n\
         or OKX_CREDENTIAL_FILE. Default is Demo. Live also requires OKX_ENV=live and\n\
         OKX_CONFIRM_LIVE=1."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repl_tokenizer_keeps_json_with_whitespace_together() {
        assert_eq!(
            split_args(r#"request market_data get_ticker {"instId": "BTC-USDT"}"#).unwrap(),
            [
                "request",
                "market_data",
                "get_ticker",
                r#"{"instId": "BTC-USDT"}"#,
            ]
        );
    }

    #[test]
    fn parser_converts_query_scalars_to_strings() {
        let request = parse_request(&[
            "request".to_owned(),
            "market_data".to_owned(),
            "get_ticker".to_owned(),
            r#"{"instId":"BTC-USDT","limit":5}"#.to_owned(),
        ])
        .unwrap();
        assert_eq!(request.query["instId"], "BTC-USDT");
        assert_eq!(request.query["limit"], "5");
    }

    #[test]
    fn get_alias_uses_the_audited_operation_name() {
        let request = parse_request(&[
            "get".to_owned(),
            "market_data".to_owned(),
            "get_ticker".to_owned(),
            r#"{"instId":"BTC-USDT"}"#.to_owned(),
        ])
        .unwrap();
        assert_eq!(request.domain, "market_data");
        assert_eq!(request.operation, "get_ticker");
        assert_eq!(request.requested_method.as_deref(), Some("GET"));
    }
}
