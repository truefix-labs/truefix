//! Binance FIX API client: connects as a FIX.4.4 initiator to any combination of Binance's three
//! endpoints (order-entry, drop-copy, market-data), described by a QuickFIX-style `.cfg` file
//! (parsed via `truefix_config`), authenticates each Logon with an Ed25519 `RawData` signature
//! (Binance's non-standard auth scheme), and implements the full message set from Binance's FIX
//! API docs -- new/cancel/amend/mass-cancel orders, OCO/OTO/OTOCO order lists, rate-limit
//! queries, market-data subscriptions, and instrument-list queries.
//!
//! Docs: <https://developers.binance.com/legacy-docs/binance-spot-api-docs/fix-api>.
//! See `examples/binance/README.md` for setup and `examples/binance/config/binance-testnet.cfg`
//! for the example configuration this binary is normally run against.

mod app;
mod config;
mod display;
mod log_dump;
mod messages;

use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tokio::io::{AsyncBufReadExt, BufReader};

use truefix::config::SessionSettings;
use truefix::transport::{
    Services, SessionHandle, TlsSpec, build_client_config, connect_initiator_tls,
};

use app::BinanceApp;
use config::{BinanceEndpoint, BinanceSessionExt, ensure_unique_endpoints, parse_binance_ext};
use messages::{
    CancelRestrictions, CancelTarget, MdEntryKind, NewOrderOptions, OrderKind,
    OrderRateLimitExceededMode, PegPriceType, TimeInForce, TriggerDirection,
};

enum Mode {
    Run(PathBuf),
    DumpLog(PathBuf),
}

fn parse_args() -> Result<Mode> {
    let mut it = std::env::args().skip(1);
    match it.next() {
        None => {
            print_help();
            bail!("missing config file path (or --dump-log <PATH>)");
        }
        Some(arg) if arg == "--help" || arg == "-h" => {
            print_help();
            std::process::exit(0);
        }
        Some(arg) if arg == "--dump-log" => {
            let path = it.next().context("--dump-log requires a path")?;
            Ok(Mode::DumpLog(PathBuf::from(path)))
        }
        Some(cfg_path) => Ok(Mode::Run(PathBuf::from(cfg_path))),
    }
}

fn print_help() {
    eprintln!(
        "usage: cargo run -p truefix --example binance -- <CONFIG.cfg>\n       \
         cargo run -p truefix --example binance -- --dump-log <PATH.redb>\n\n\
         <CONFIG.cfg> is a QuickFIX-style settings file: one [SESSION] block per Binance\n\
         endpoint (OrderEntry / DropCopy / MarketData) to connect. See\n\
         crates/truefix/examples/binance/config/binance-testnet.cfg for a documented example,\n\
         and crates/truefix/examples/binance/README.md for full setup instructions.\n\n\
         Each [SESSION] independently picks its log backend via BinanceLogBackend=Redb|File\n\
         (default Redb). --dump-log only reads redb databases -- a File-backend session's\n\
         messages.log/event.log are already plain text, so just `cat`/`tail -f` them."
    );
}

/// Builds a client-side TLS config via `truefix::transport::build_client_config`: `trust_store`'s
/// PEM CA bundle if given, else the OS's native trust store (matching QuickFIX/J's own fallback to
/// the JVM's default trust store, rather than a bundled, compile-time-fixed CA list). No client
/// certificate is presented -- Binance's FIX endpoints never require mTLS.
fn build_tls_config(trust_store_path: Option<&Path>) -> Result<Arc<rustls::ClientConfig>> {
    let trust_store_bytes = match trust_store_path {
        Some(path) => Some(
            std::fs::read(path)
                .with_context(|| format!("reading BinanceTrustStore at {}", path.display()))?,
        ),
        None => None,
    };
    build_client_config(&TlsSpec {
        key_store_path: None,
        key_store_bytes: None,
        trust_store_path: None,
        trust_store_bytes,
        need_client_auth: false,
        min_version: None,
        server_name: None,
        cipher_suites: Vec::new(),
    })
    .map_err(|e| anyhow::anyhow!("building TLS config: {e}"))
}

/// The canonical session-id string, matching `truefix::SessionId`'s own `Display` impl -- used
/// both as `BinanceApp`'s lookup key and in error messages. `qualifier` must be passed whenever
/// `SessionConfig::session_qualifier` is set (see `run`'s doc comment on why it always is here).
fn session_label(
    begin_string: &str,
    sender: &str,
    target: &str,
    qualifier: Option<&str>,
) -> String {
    match qualifier {
        Some(q) => format!("{begin_string}:{sender}->{target}:{q}"),
        None => format!("{begin_string}:{sender}->{target}"),
    }
}

fn print_repl_help(handles: &[(BinanceEndpoint, SessionHandle)]) {
    eprintln!(
        "connected endpoints: {}",
        handles
            .iter()
            .map(|(e, _)| e.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    eprintln!(
        "commands:\n\
         \x20\x20order <SYMBOL> <BUY|SELL> <QTY> market [FLAGS]\n\
         \x20\x20order <SYMBOL> <BUY|SELL> <QTY> limit <PRICE> [GTC|IOC|FOK] [FLAGS]\n\
         \x20\x20order <SYMBOL> <BUY|SELL> <QTY> limitmaker <PRICE> [FLAGS]\n\
         \x20\x20order <SYMBOL> <BUY|SELL> <QTY> stop <TRIGGER_PRICE> <UP|DOWN> [FLAGS]\n\
         \x20\x20order <SYMBOL> <BUY|SELL> <QTY> stoplimit <PRICE> <TRIGGER_PRICE> <UP|DOWN> [GTC|IOC|FOK] [FLAGS]\n\
         \x20\x20order <SYMBOL> <BUY|SELL> <QTY> pegged <PEG_OFFSET> [MARKET|PRIMARY] [GTC|IOC|FOK] [FLAGS]\n\
         \x20\x20                                                          NewOrderSingle\n\
         \x20\x20      FLAGS: stp=<NONE|EXPIRE_TAKER|EXPIRE_MAKER|EXPIRE_BOTH|DECREMENT|TRANSFER>\n\
         \x20\x20             iceberg=<QTY> trailingdelta=<BIPS> strategy=<TARGET>:<ID> sor cashqty\n\
         \x20\x20cancel <SYMBOL> <ORIG_CLORDID> [ONLY_NEW|ONLY_PARTIALLY_FILLED]      OrderCancelRequest\n\
         \x20\x20cancellist <SYMBOL> <ORIG_CLLISTID> [ONLY_NEW|ONLY_PARTIALLY_FILLED] OrderCancelRequest (by list)\n\
         \x20\x20masscancel <SYMBOL>                                      OrderMassCancelRequest\n\
         \x20\x20orderlist oco <SYMBOL> <BUY|SELL> <QTY> <PRICE> <STOP_PRICE> [STOP_LIMIT_PRICE] [opo]\n\
         \x20\x20orderlist oto <SYMBOL> <W_SIDE> <W_QTY> <W_PRICE> <P_SIDE> <P_QTY> <P_PRICE> [opo]\n\
         \x20\x20orderlist otoco <SYMBOL> <W_SIDE> <W_QTY> <W_PRICE> <P_SIDE> <P_QTY> <P_LIMIT_PRICE> <P_STOP_PRICE> [P_STOP_LIMIT_PRICE] [opo]\n\
         \x20\x20                                                          NewOrderList (OCO / OTO / OTOCO)\n\
         \x20\x20amend <SYMBOL> <ORIG_CLORDID> <NEW_QTY>                  OrderAmendKeepPriorityRequest\n\
         \x20\x20cancelreplace <SYMBOL> <ORIG_CLORDID> <SIDE> <QTY> [PRICE] [restrict=<...>] [ratelimitmode=<DO_NOTHING|CANCEL_ONLY>]\n\
         \x20\x20                                                          OrderCancelRequestAndNewOrderSingle\n\
         \x20\x20limitquery                                               LimitQuery\n\
         \x20\x20mdreq <SYMBOL> [DEPTH]                                    MarketDataRequest, book depth (subscribe)\n\
         \x20\x20mdreq <SYMBOL> book [DEPTH]                               MarketDataRequest, book depth (subscribe)\n\
         \x20\x20mdreq <SYMBOL> trades                                     MarketDataRequest, trade stream (subscribe)\n\
         \x20\x20unsubscribe <SYMBOL>                                     MarketDataRequest (unsubscribe)\n\
         \x20\x20instruments [SYMBOL]                                     InstrumentListRequest (SYMBOL, or ALL if omitted)\n\
         \x20\x20help                                                     show this text\n\
         \x20\x20quit | exit                                              log out and disconnect"
    );
}

fn find_handle(
    handles: &[(BinanceEndpoint, SessionHandle)],
    endpoint: BinanceEndpoint,
) -> Option<&SessionHandle> {
    handles.iter().find(|(e, _)| *e == endpoint).map(|(_, h)| h)
}

/// Peeks the next token and consumes it only if `parse` accepts it -- used for trailing optional
/// positional args (TIF, peg price type, ...) that sit before a variable-length flag tail.
fn maybe_take<'a, T>(
    parts: &mut std::iter::Peekable<std::str::SplitWhitespace<'a>>,
    parse: impl Fn(&'a str) -> Result<T>,
) -> Option<T> {
    let peeked = *parts.peek()?;
    let value = parse(peeked).ok()?;
    parts.next();
    Some(value)
}

/// Parses the optional trailing `key=value`/bare-word flags shared by every `order` variant.
fn parse_new_order_options<'a>(
    parts: impl Iterator<Item = &'a str>,
) -> Result<NewOrderOptions<'a>> {
    let mut opts = NewOrderOptions::default();
    for tok in parts {
        if let Some((key, value)) = tok.split_once('=') {
            match key {
                "stp" => {
                    opts.self_trade_prevention_mode =
                        Some(messages::SelfTradePreventionMode::parse(value)?)
                }
                "iceberg" => opts.iceberg_qty = Some(value),
                "trailingdelta" => opts.trailing_delta_bips = Some(value),
                "strategy" => {
                    let (target, id) = value
                        .split_once(':')
                        .context("usage: strategy=<TARGET_STRATEGY>:<STRATEGY_ID>")?;
                    opts.strategy = Some((target, id));
                }
                other => bail!("unknown order flag: {other}"),
            }
        } else {
            match tok {
                "sor" => opts.sor = true,
                "cashqty" => opts.cash_order_qty = true,
                other => bail!("unknown order flag: {other} (type 'help')"),
            }
        }
    }
    Ok(opts)
}

/// Parses and dispatches one REPL command. Returns `Ok(true)` to keep reading, `Ok(false)` to quit.
async fn dispatch_command(
    handles: &[(BinanceEndpoint, SessionHandle)],
    subscriptions: &mut HashMap<String, String>,
    line: &str,
) -> Result<bool> {
    let mut parts = line.split_whitespace().peekable();
    let Some(cmd) = parts.next() else {
        return Ok(true); // blank line
    };
    match cmd {
        "quit" | "exit" => return Ok(false),
        "help" => print_repl_help(handles),
        "order" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let (Some(symbol), Some(side), Some(qty), Some(kind_tok)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                bail!(
                    "usage: order <SYMBOL> <BUY|SELL> <QTY> <market|limit|limitmaker|stop|stoplimit|pegged> ... (type 'help')"
                );
            };
            let kind = match kind_tok.to_ascii_lowercase().as_str() {
                "market" => OrderKind::Market,
                "limit" => {
                    let price = parts
                        .next()
                        .context("usage: order ... limit <PRICE> [GTC|IOC|FOK]")?;
                    let tif = maybe_take(&mut parts, TimeInForce::parse)
                        .unwrap_or(TimeInForce::GoodTillCancel);
                    OrderKind::Limit { price, tif }
                }
                "limitmaker" => {
                    let price = parts
                        .next()
                        .context("usage: order ... limitmaker <PRICE>")?;
                    OrderKind::LimitMaker { price }
                }
                "stop" => {
                    let trigger_price = parts
                        .next()
                        .context("usage: order ... stop <TRIGGER_PRICE> <UP|DOWN>")?;
                    let direction = TriggerDirection::parse(
                        parts
                            .next()
                            .context("usage: order ... stop <TRIGGER_PRICE> <UP|DOWN>")?,
                    )?;
                    OrderKind::Stop {
                        trigger_price,
                        direction,
                    }
                }
                "stoplimit" => {
                    let price = parts
                        .next()
                        .context("usage: order ... stoplimit <PRICE> <TRIGGER_PRICE> <UP|DOWN> [GTC|IOC|FOK]")?;
                    let trigger_price = parts
                        .next()
                        .context("usage: order ... stoplimit <PRICE> <TRIGGER_PRICE> <UP|DOWN> [GTC|IOC|FOK]")?;
                    let direction = TriggerDirection::parse(
                        parts
                            .next()
                            .context("usage: order ... stoplimit <PRICE> <TRIGGER_PRICE> <UP|DOWN> [GTC|IOC|FOK]")?,
                    )?;
                    let tif = maybe_take(&mut parts, TimeInForce::parse)
                        .unwrap_or(TimeInForce::GoodTillCancel);
                    OrderKind::StopLimit {
                        price,
                        trigger_price,
                        direction,
                        tif,
                    }
                }
                "pegged" => {
                    let peg_offset = parts.next().context(
                        "usage: order ... pegged <PEG_OFFSET> [MARKET|PRIMARY] [GTC|IOC|FOK]",
                    )?;
                    let price_type =
                        maybe_take(&mut parts, PegPriceType::parse).unwrap_or(PegPriceType::Market);
                    let tif = maybe_take(&mut parts, TimeInForce::parse)
                        .unwrap_or(TimeInForce::GoodTillCancel);
                    OrderKind::Pegged {
                        peg_offset,
                        price_type,
                        tif,
                    }
                }
                other => bail!(
                    "unknown order type: {other} (expected market|limit|limitmaker|stop|stoplimit|pegged)"
                ),
            };
            let opts = parse_new_order_options(parts)?;
            handle
                .send(messages::new_order_single(symbol, side, qty, &kind, &opts)?)
                .await;
        }
        "cancel" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let (Some(symbol), Some(orig)) = (parts.next(), parts.next()) else {
                bail!("usage: cancel <SYMBOL> <ORIG_CLORDID> [ONLY_NEW|ONLY_PARTIALLY_FILLED]");
            };
            let restrictions = parts.next().map(CancelRestrictions::parse).transpose()?;
            handle
                .send(messages::order_cancel_request(
                    symbol,
                    CancelTarget::OrigClOrdId(orig),
                    restrictions,
                ))
                .await;
        }
        "cancellist" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let (Some(symbol), Some(orig_list)) = (parts.next(), parts.next()) else {
                bail!(
                    "usage: cancellist <SYMBOL> <ORIG_CLLISTID> [ONLY_NEW|ONLY_PARTIALLY_FILLED]"
                );
            };
            let restrictions = parts.next().map(CancelRestrictions::parse).transpose()?;
            handle
                .send(messages::order_cancel_request(
                    symbol,
                    CancelTarget::OrigClListId(orig_list),
                    restrictions,
                ))
                .await;
        }
        "masscancel" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let Some(symbol) = parts.next() else {
                bail!("usage: masscancel <SYMBOL>");
            };
            handle
                .send(messages::order_mass_cancel_request(symbol))
                .await;
        }
        "orderlist" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let Some(mode) = parts.next() else {
                bail!("usage: orderlist <oco|oto|otoco> ... (type 'help')");
            };
            let mut message = match mode.to_ascii_lowercase().as_str() {
                "oco" => {
                    let (Some(symbol), Some(side), Some(qty), Some(price), Some(stop_price)) = (
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                    ) else {
                        bail!(
                            "usage: orderlist oco <SYMBOL> <BUY|SELL> <QTY> <PRICE> <STOP_PRICE> [STOP_LIMIT_PRICE] [opo]"
                        );
                    };
                    let stop_limit_price = match parts.peek() {
                        Some(&next) if next != "opo" => parts.next(),
                        _ => None,
                    };
                    messages::new_order_list_oco(
                        symbol,
                        side,
                        qty,
                        price,
                        stop_price,
                        stop_limit_price,
                    )?
                }
                "oto" => {
                    let (
                        Some(symbol),
                        Some(w_side),
                        Some(w_qty),
                        Some(w_price),
                        Some(p_side),
                        Some(p_qty),
                        Some(p_price),
                    ) = (
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                    )
                    else {
                        bail!(
                            "usage: orderlist oto <SYMBOL> <W_SIDE> <W_QTY> <W_PRICE> <P_SIDE> <P_QTY> <P_PRICE> [opo]"
                        );
                    };
                    messages::new_order_list_oto(
                        symbol, w_side, w_qty, w_price, p_side, p_qty, p_price,
                    )?
                }
                "otoco" => {
                    let (
                        Some(symbol),
                        Some(w_side),
                        Some(w_qty),
                        Some(w_price),
                        Some(p_side),
                        Some(p_qty),
                        Some(p_limit),
                        Some(p_stop),
                    ) = (
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                        parts.next(),
                    )
                    else {
                        bail!(
                            "usage: orderlist otoco <SYMBOL> <W_SIDE> <W_QTY> <W_PRICE> <P_SIDE> <P_QTY> <P_LIMIT_PRICE> <P_STOP_PRICE> [P_STOP_LIMIT_PRICE] [opo]"
                        );
                    };
                    let p_stop_limit = match parts.peek() {
                        Some(&next) if next != "opo" => parts.next(),
                        _ => None,
                    };
                    messages::new_order_list_otoco(
                        symbol,
                        messages::OtocoParams {
                            working_side: w_side,
                            working_qty: w_qty,
                            working_price: w_price,
                            pending_side: p_side,
                            pending_qty: p_qty,
                            pending_limit_price: p_limit,
                            pending_stop_price: p_stop,
                            pending_stop_limit_price: p_stop_limit,
                        },
                    )?
                }
                other => bail!("unknown orderlist mode: {other} (expected oco|oto|otoco)"),
            };
            if parts.next() == Some("opo") {
                messages::set_opo(&mut message);
            }
            handle.send(message).await;
        }
        "amend" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let (Some(symbol), Some(orig), Some(new_qty)) =
                (parts.next(), parts.next(), parts.next())
            else {
                bail!("usage: amend <SYMBOL> <ORIG_CLORDID> <NEW_QTY>");
            };
            handle
                .send(messages::order_amend_keep_priority(symbol, orig, new_qty))
                .await;
        }
        "cancelreplace" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .context("no OrderEntry session is configured")?;
            let (Some(symbol), Some(orig), Some(side), Some(qty)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                bail!(
                    "usage: cancelreplace <SYMBOL> <ORIG_CLORDID> <SIDE> <QTY> [PRICE] [restrict=<...>] [ratelimitmode=<...>]"
                );
            };
            let price = match parts.peek() {
                Some(&next) if !next.contains('=') => parts.next(),
                _ => None,
            };
            let mut cancel_restrictions = None;
            let mut rate_limit_mode = None;
            for tok in parts {
                let (key, value) = tok.split_once('=').with_context(|| {
                    format!("unknown flag: {tok} (expected restrict=... or ratelimitmode=...)")
                })?;
                match key {
                    "restrict" => cancel_restrictions = Some(CancelRestrictions::parse(value)?),
                    "ratelimitmode" => {
                        rate_limit_mode = Some(OrderRateLimitExceededMode::parse(value)?)
                    }
                    other => bail!("unknown flag: {other}"),
                }
            }
            handle
                .send(messages::order_cancel_request_and_new_order_single(
                    symbol,
                    orig,
                    side,
                    qty,
                    price,
                    cancel_restrictions,
                    rate_limit_mode,
                )?)
                .await;
        }
        "limitquery" => {
            let handle = find_handle(handles, BinanceEndpoint::OrderEntry)
                .or_else(|| find_handle(handles, BinanceEndpoint::MarketData))
                .context("no OrderEntry or MarketData session is configured")?;
            handle.send(messages::limit_query()).await;
        }
        "mdreq" => {
            let handle = find_handle(handles, BinanceEndpoint::MarketData)
                .context("no MarketData session is configured")?;
            let Some(symbol) = parts.next() else {
                bail!("usage: mdreq <SYMBOL> [DEPTH|trades|book [DEPTH]]");
            };
            let (entry_kinds, depth): (Vec<MdEntryKind>, Option<u32>) = match parts.next() {
                None => (vec![MdEntryKind::Bid, MdEntryKind::Offer], Some(5)),
                Some(tok) if tok.eq_ignore_ascii_case("trades") => (vec![MdEntryKind::Trade], None),
                Some(tok) if tok.eq_ignore_ascii_case("book") => {
                    let depth: u32 = match parts.next() {
                        Some(d) => d.parse().context("DEPTH must be a number")?,
                        None => 5,
                    };
                    (vec![MdEntryKind::Bid, MdEntryKind::Offer], Some(depth))
                }
                Some(tok) => {
                    let depth: u32 = tok
                        .parse()
                        .context("usage: mdreq <SYMBOL> [DEPTH|trades|book [DEPTH]]")?;
                    (vec![MdEntryKind::Bid, MdEntryKind::Offer], Some(depth))
                }
            };
            let mdreqid = messages::next_id("md");
            subscriptions.insert(symbol.to_ascii_uppercase(), mdreqid.clone());
            handle
                .send(messages::market_data_request(
                    &mdreqid,
                    symbol,
                    true,
                    &entry_kinds,
                    depth,
                ))
                .await;
        }
        "unsubscribe" => {
            let handle = find_handle(handles, BinanceEndpoint::MarketData)
                .context("no MarketData session is configured")?;
            let Some(symbol) = parts.next() else {
                bail!("usage: unsubscribe <SYMBOL>");
            };
            let Some(mdreqid) = subscriptions.remove(&symbol.to_ascii_uppercase()) else {
                bail!(
                    "no active subscription tracked for {symbol} (only subscriptions made this session can be unsubscribed)"
                );
            };
            handle
                .send(messages::market_data_request(
                    &mdreqid,
                    symbol,
                    false,
                    &[MdEntryKind::Bid, MdEntryKind::Offer],
                    None,
                ))
                .await;
        }
        "instruments" => {
            let handle = find_handle(handles, BinanceEndpoint::MarketData)
                .context("no MarketData session is configured")?;
            handle
                .send(messages::instrument_list_request(parts.next()))
                .await;
        }
        other => bail!("unknown command: {other} (type 'help')"),
    }
    Ok(true)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    match parse_args()? {
        Mode::DumpLog(path) => return log_dump::dump_log_db(&path),
        Mode::Run(cfg_path) => run(&cfg_path).await,
    }
}

async fn run(cfg_path: &Path) -> Result<()> {
    let cfg_text = std::fs::read_to_string(cfg_path)
        .with_context(|| format!("reading config file {}", cfg_path.display()))?;
    let settings = SessionSettings::parse(&cfg_text)
        .with_context(|| format!("parsing config file {}", cfg_path.display()))?;
    let raw_sessions = settings.sessions();
    let resolved = settings.resolve().context("resolving [SESSION] blocks")?;
    if resolved.is_empty() {
        bail!("{} defines no [SESSION] blocks", cfg_path.display());
    }

    // Binance's three endpoints intentionally share one SenderCompID/TargetCompID (only the
    // host:port differs) -- so the plain "{begin}:{sender}->{target}" form collides across all of
    // them. SessionQualifier is a local-only disambiguator (truefix-config: "no wire tag"), so
    // stamping each session with its BinanceEndpoint as the qualifier gives `BinanceApp` a unique
    // lookup key per session without changing anything Binance actually sees on the wire.
    let mut prepared = Vec::with_capacity(resolved.len());
    for (i, mut rs) in resolved.into_iter().enumerate() {
        let unqualified = session_label(
            &rs.session.begin_string,
            &rs.session.sender_comp_id,
            &rs.session.target_comp_id,
            None,
        );
        let ext = parse_binance_ext(&raw_sessions[i], &unqualified)?;
        rs.session.session_qualifier = Some(ext.endpoint.to_string());
        let label = session_label(
            &rs.session.begin_string,
            &rs.session.sender_comp_id,
            &rs.session.target_comp_id,
            rs.session.session_qualifier.as_deref(),
        );
        prepared.push((label, ext, rs));
    }
    let labeled_exts: Vec<(String, BinanceSessionExt)> = prepared
        .iter()
        .map(|(label, ext, _)| (label.clone(), ext.clone()))
        .collect();
    ensure_unique_endpoints(&labeled_exts)?;

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("failed to install the default TLS crypto provider"))?;

    let app_sessions: HashMap<String, BinanceSessionExt> = labeled_exts.into_iter().collect();
    let app = Arc::new(BinanceApp::new(app_sessions));

    let mut handles: Vec<(BinanceEndpoint, SessionHandle)> = Vec::with_capacity(prepared.len());
    for (label, ext, rs) in prepared {
        let host = rs.address.host().to_owned();
        let port = rs.address.port();

        tracing::info!(session = %label, endpoint = %ext.endpoint, host, port, "connecting");

        let log: Arc<dyn truefix_log::Log> = match ext.log_backend {
            config::LogBackend::Redb => {
                if let Some(dir) = ext.log_db.parent().filter(|d| !d.as_os_str().is_empty()) {
                    std::fs::create_dir_all(dir)
                        .with_context(|| format!("creating log directory {}", dir.display()))?;
                }
                let redb_log =
                    truefix_log::RedbLog::connect_with_config(truefix_log::RedbLogConfig {
                        path: ext.log_db.clone(),
                        include_heartbeats: true,
                        session_id: label.clone(),
                    })
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!("opening log database {}: {e}", ext.log_db.display())
                    })?;
                Arc::new(redb_log)
            }
            config::LogBackend::File => {
                // truefix audit 007, NEW-156/157: FileLog's writes are queued onto a bounded
                // channel and persisted by a background task, so a slow disk never blocks this
                // session's async read/dispatch loop -- and (unlike redb's single ever-growing
                // file) rotation is configurable via MaxFileLogSize/FileLogMaxGenerations/
                // FileLogRollIntervalSecs.
                let retention = if ext.file_log.max_generations.is_some()
                    || ext.file_log.roll_interval_secs.is_some()
                {
                    Some(truefix_log::RetentionPolicy {
                        generations: ext.file_log.max_generations,
                        roll_interval: ext.file_log.roll_interval_secs.map(Duration::from_secs),
                    })
                } else {
                    None
                };
                let file_log = truefix_log::FileLog::open_with_options(
                    &ext.log_db,
                    truefix_log::FileLogOptions {
                        include_heartbeats: ext.file_log.include_heartbeats,
                        include_timestamp: ext.file_log.include_timestamp,
                        include_milliseconds: ext.file_log.include_milliseconds,
                        max_size_bytes: ext.file_log.max_size_bytes,
                        retention,
                    },
                )
                .await
                .map_err(|e| {
                    anyhow::anyhow!("opening log directory {}: {e}", ext.log_db.display())
                })?;
                Arc::new(truefix_log::SessionPrefixLog::new(label.clone(), file_log))
            }
        };
        let services = Services {
            log: Some(log),
            ..Services::default()
        };

        let tls_config = build_tls_config(ext.trust_store.as_deref())?;
        let server_name = rustls::pki_types::ServerName::try_from(host.clone())
            .context("invalid TLS server name")?;
        let addr = (host.as_str(), port)
            .to_socket_addrs()
            .with_context(|| format!("resolving {host}:{port}"))?
            .next()
            .with_context(|| format!("no address found for {host}:{port}"))?;

        let handle = connect_initiator_tls(
            addr,
            rs.session,
            app.clone(),
            services,
            tls_config,
            server_name,
        )
        .await
        .with_context(|| format!("connecting session {label}"))?;
        handles.push((ext.endpoint, handle));
    }

    print_repl_help(&handles);
    let mut subscriptions: HashMap<String, String> = HashMap::new();
    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            line = stdin_lines.next_line() => {
                match line {
                    Ok(Some(line)) => match dispatch_command(&handles, &mut subscriptions, &line).await {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(e) => eprintln!("error: {e:#}"),
                    },
                    Ok(None) => break, // stdin closed (e.g. piped input exhausted)
                    Err(e) => {
                        tracing::warn!(error = %e, "reading stdin");
                        break;
                    }
                }
            }
        }
    }

    tracing::info!("shutting down, logging out");
    for (_, handle) in &handles {
        handle.logout().await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    Ok(())
}
