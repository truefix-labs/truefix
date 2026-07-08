//! Outbound message builders for Binance's FIX API.
//!
//! Docs: <https://developers.binance.com/legacy-docs/binance-spot-api-docs/fix-api>. Tag numbers
//! and enum values below are taken from Binance's published dictionaries
//! (`spot-fix-oe.xml`/`spot-fix-md.xml`).

use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, bail};
use truefix::{Field, FieldMap, Group, Message};

/// Monotonic suffix so ClOrdID/MDReqID/... stay unique even for commands issued within the same
/// millisecond.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_id(prefix: &str) -> String {
    let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!(
        "{prefix}-{}-{n}",
        time::OffsetDateTime::now_utc().unix_timestamp()
    )
}

fn side_code(side: &str) -> Result<&'static str> {
    match side.to_ascii_uppercase().as_str() {
        "BUY" | "1" => Ok("1"),
        "SELL" | "2" => Ok("2"),
        other => bail!("side must be BUY or SELL, got {other}"),
    }
}

/// `TimeInForce(59)` for a Limit/Stop-Limit order.
#[derive(Clone, Copy)]
pub enum TimeInForce {
    GoodTillCancel,
    ImmediateOrCancel,
    FillOrKill,
}

impl TimeInForce {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "GTC" => Ok(Self::GoodTillCancel),
            "IOC" => Ok(Self::ImmediateOrCancel),
            "FOK" => Ok(Self::FillOrKill),
            other => bail!("time in force must be GTC, IOC, or FOK, got {other}"),
        }
    }

    fn tag_value(self) -> &'static str {
        match self {
            Self::GoodTillCancel => "1",
            Self::ImmediateOrCancel => "3",
            Self::FillOrKill => "4",
        }
    }
}

/// `TriggerPriceDirection(1109)`: which way the price must move to activate a stop/OCO leg.
#[derive(Clone, Copy)]
pub enum TriggerDirection {
    Up,
    Down,
}

impl TriggerDirection {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "UP" => Ok(Self::Up),
            "DOWN" => Ok(Self::Down),
            other => bail!("trigger direction must be UP or DOWN, got {other}"),
        }
    }

    fn tag_value(self) -> &'static str {
        match self {
            Self::Up => "U",
            Self::Down => "D",
        }
    }
}

/// `PegPriceType(1094)` for a Pegged order.
#[derive(Clone, Copy)]
pub enum PegPriceType {
    Market,
    Primary,
}

impl PegPriceType {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "MARKET" => Ok(Self::Market),
            "PRIMARY" => Ok(Self::Primary),
            other => bail!("peg price type must be MARKET or PRIMARY, got {other}"),
        }
    }

    fn tag_value(self) -> &'static str {
        match self {
            Self::Market => "4",
            Self::Primary => "5",
        }
    }
}

/// `SelfTradePreventionMode(25001)`.
#[derive(Clone, Copy)]
pub enum SelfTradePreventionMode {
    None,
    ExpireTaker,
    ExpireMaker,
    ExpireBoth,
    Decrement,
    Transfer,
}

impl SelfTradePreventionMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "NONE" => Ok(Self::None),
            "EXPIRE_TAKER" => Ok(Self::ExpireTaker),
            "EXPIRE_MAKER" => Ok(Self::ExpireMaker),
            "EXPIRE_BOTH" => Ok(Self::ExpireBoth),
            "DECREMENT" => Ok(Self::Decrement),
            "TRANSFER" => Ok(Self::Transfer),
            other => bail!(
                "self-trade prevention mode must be one of NONE|EXPIRE_TAKER|EXPIRE_MAKER|EXPIRE_BOTH|DECREMENT|TRANSFER, got {other}"
            ),
        }
    }

    fn tag_value(self) -> &'static str {
        match self {
            Self::None => "1",
            Self::ExpireTaker => "2",
            Self::ExpireMaker => "3",
            Self::ExpireBoth => "4",
            Self::Decrement => "5",
            Self::Transfer => "6",
        }
    }
}

/// `CancelRestrictions(25002)`.
#[derive(Clone, Copy)]
pub enum CancelRestrictions {
    OnlyNew,
    OnlyPartiallyFilled,
}

impl CancelRestrictions {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "ONLY_NEW" => Ok(Self::OnlyNew),
            "ONLY_PARTIALLY_FILLED" => Ok(Self::OnlyPartiallyFilled),
            other => {
                bail!("cancel restrictions must be ONLY_NEW or ONLY_PARTIALLY_FILLED, got {other}")
            }
        }
    }

    fn tag_value(self) -> &'static str {
        match self {
            Self::OnlyNew => "1",
            Self::OnlyPartiallyFilled => "2",
        }
    }
}

/// `OrderRateLimitExceededMode(25038)`, for `OrderCancelRequestAndNewOrderSingle`.
#[derive(Clone, Copy)]
pub enum OrderRateLimitExceededMode {
    DoNothing,
    CancelOnly,
}

impl OrderRateLimitExceededMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_uppercase().as_str() {
            "DO_NOTHING" => Ok(Self::DoNothing),
            "CANCEL_ONLY" => Ok(Self::CancelOnly),
            other => bail!(
                "order rate limit exceeded mode must be DO_NOTHING or CANCEL_ONLY, got {other}"
            ),
        }
    }

    fn tag_value(self) -> &'static str {
        match self {
            Self::DoNothing => "1",
            Self::CancelOnly => "2",
        }
    }
}

/// Which order type/pricing model a `NewOrderSingle` (or a `NewOrderList` leg) uses.
pub enum OrderKind<'a> {
    Market,
    Limit {
        price: &'a str,
        tif: TimeInForce,
    },
    /// Limit + `ExecInst(18)=PARTICIPATE_DONT_INITIATE` (post-only).
    LimitMaker {
        price: &'a str,
    },
    /// Market order once `trigger_price` is crossed.
    Stop {
        trigger_price: &'a str,
        direction: TriggerDirection,
    },
    /// Limit order once `trigger_price` is crossed.
    StopLimit {
        price: &'a str,
        trigger_price: &'a str,
        direction: TriggerDirection,
        tif: TimeInForce,
    },
    Pegged {
        peg_offset: &'a str,
        price_type: PegPriceType,
        tif: TimeInForce,
    },
}

/// The less commonly needed `NewOrderSingle` fields, all optional.
#[derive(Default)]
pub struct NewOrderOptions<'a> {
    pub self_trade_prevention_mode: Option<SelfTradePreventionMode>,
    /// `MaxFloor(111)`: the visible quantity of an iceberg order.
    pub iceberg_qty: Option<&'a str>,
    /// `TriggerTrailingDeltaBips(25009)`: a trailing stop offset, in bips.
    pub trailing_delta_bips: Option<&'a str>,
    /// `(TargetStrategy(847), StrategyID(7940))`.
    pub strategy: Option<(&'a str, &'a str)>,
    /// `SOR(25032)`: route through Smart Order Routing.
    pub sor: bool,
    /// When true, `qty` is sent as `CashOrderQty(152)` (quote-asset amount) instead of
    /// `OrderQty(38)` (base-asset amount).
    pub cash_order_qty: bool,
}

fn apply_order_kind(body: &mut FieldMap, kind: &OrderKind) {
    match kind {
        OrderKind::Market => {
            body.set(Field::string(40, "1")); // OrdType = Market
        }
        OrderKind::Limit { price, tif } => {
            body.set(Field::string(40, "2")); // OrdType = Limit
            body.set(Field::string(44, price));
            body.set(Field::string(59, tif.tag_value()));
        }
        OrderKind::LimitMaker { price } => {
            body.set(Field::string(40, "2")); // OrdType = Limit
            body.set(Field::string(44, price));
            body.set(Field::string(18, "6")); // ExecInst = PARTICIPATE_DONT_INITIATE (post-only)
        }
        OrderKind::Stop {
            trigger_price,
            direction,
        } => {
            body.set(Field::string(40, "3")); // OrdType = Stop
            set_trigger(body, trigger_price, *direction);
        }
        OrderKind::StopLimit {
            price,
            trigger_price,
            direction,
            tif,
        } => {
            body.set(Field::string(40, "4")); // OrdType = Stop Limit
            body.set(Field::string(44, price));
            body.set(Field::string(59, tif.tag_value()));
            set_trigger(body, trigger_price, *direction);
        }
        OrderKind::Pegged {
            peg_offset,
            price_type,
            tif,
        } => {
            body.set(Field::string(40, "P")); // OrdType = Pegged
            body.set(Field::string(211, peg_offset)); // PegOffsetValue
            body.set(Field::string(1094, price_type.tag_value())); // PegPriceType
            body.set(Field::string(835, "1")); // PegMoveType = FIXED (the only enum value the schema defines)
            body.set(Field::string(836, "3")); // PegOffsetType = PRICE_TIER (the only enum value the schema defines)
            // Discovered live against testnet: Binance rejects a Pegged order with -1102
            // "Required tag 'TimeInForce (59)' missing" if this is omitted, even though the
            // schema itself marks TimeInForce optional for NewOrderSingle in general.
            body.set(Field::string(59, tif.tag_value()));
        }
    }
}

/// Stamps the `TriggeringInstruction` component (`TriggerType`/`TriggerAction`/`TriggerPrice`/
/// `TriggerPriceType`/`TriggerPriceDirection`) used by Stop, Stop-Limit, and OCO stop legs.
fn set_trigger(body: &mut FieldMap, trigger_price: &str, direction: TriggerDirection) {
    body.set(Field::string(1100, "4")); // TriggerType = PRICE_MOVEMENT
    body.set(Field::string(1101, "1")); // TriggerAction = ACTIVATE
    body.set(Field::string(1102, trigger_price)); // TriggerPrice
    body.set(Field::string(1107, "2")); // TriggerPriceType = LAST_TRADE
    body.set(Field::string(1109, direction.tag_value())); // TriggerPriceDirection
}

fn apply_new_order_options(body: &mut FieldMap, opts: &NewOrderOptions) {
    if let Some(stp) = opts.self_trade_prevention_mode {
        body.set(Field::string(25001, stp.tag_value()));
    }
    if let Some(qty) = opts.iceberg_qty {
        body.set(Field::string(111, qty)); // MaxFloor
    }
    if let Some(bips) = opts.trailing_delta_bips {
        body.set(Field::string(25009, bips)); // TriggerTrailingDeltaBips
    }
    if let Some((target, id)) = opts.strategy {
        body.set(Field::string(847, target)); // TargetStrategy
        body.set(Field::string(7940, id)); // StrategyID
    }
    if opts.sor {
        body.set(Field::string(25032, "Y")); // SOR
    }
}

/// Builds a NewOrderSingle (35=D).
pub fn new_order_single(
    symbol: &str,
    side: &str,
    qty: &str,
    kind: &OrderKind,
    opts: &NewOrderOptions,
) -> Result<Message> {
    let side_code = side_code(side)?;
    let mut m = Message::new();
    m.header.set(Field::string(35, "D"));
    m.body.set(Field::string(11, &next_id("cli"))); // ClOrdID
    m.body.set(Field::string(55, symbol));
    m.body.set(Field::string(54, side_code));
    // No TransactTime (60): standard FIX 4.4 requires it, but Binance rejects it on requests
    // (-1170, "not defined for this message type") -- it only appears in ExecutionReports.
    if opts.cash_order_qty {
        m.body.set(Field::string(152, qty)); // CashOrderQty
    } else {
        m.body.set(Field::string(38, qty)); // OrderQty
    }
    apply_order_kind(&mut m.body, kind);
    apply_new_order_options(&mut m.body, opts);
    Ok(m)
}

/// Which order a `OrderCancelRequest`/`OrderCancelRequestAndNewOrderSingle` targets.
pub enum CancelTarget<'a> {
    OrigClOrdId(&'a str),
    OrigClListId(&'a str),
}

/// Builds an OrderCancelRequest (35=F): cancel the order (or order list) identified by `target`.
pub fn order_cancel_request(
    symbol: &str,
    target: CancelTarget,
    restrictions: Option<CancelRestrictions>,
) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "F"));
    m.body.set(Field::string(11, &next_id("cxl"))); // ClOrdID
    match target {
        CancelTarget::OrigClOrdId(id) => {
            m.body.set(Field::string(41, id)); // OrigClOrdID
        }
        CancelTarget::OrigClListId(id) => {
            m.body.set(Field::string(25015, id)); // OrigClListID
        }
    }
    m.body.set(Field::string(55, symbol));
    if let Some(r) = restrictions {
        m.body.set(Field::string(25002, r.tag_value())); // CancelRestrictions
    }
    m
}

/// Builds an OrderMassCancelRequest (35=q): cancel every open order on `symbol`.
pub fn order_mass_cancel_request(symbol: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "q"));
    m.body.set(Field::string(11, &next_id("mcxl"))); // ClOrdID
    m.body.set(Field::string(55, symbol));
    m.body.set(Field::string(530, "1")); // MassCancelRequestType = CANCEL_SYMBOL_ORDERS
    m
}

/// Builds an OrderAmendKeepPriorityRequest (35=XAK): reduce the open quantity of
/// `orig_cl_ord_id` to `new_qty` while keeping its place in the order book queue.
pub fn order_amend_keep_priority(symbol: &str, orig_cl_ord_id: &str, new_qty: &str) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "XAK"));
    m.body.set(Field::string(11, &next_id("amd"))); // ClOrdID
    m.body.set(Field::string(41, orig_cl_ord_id)); // OrigClOrdID
    m.body.set(Field::string(55, symbol));
    m.body.set(Field::string(38, new_qty)); // OrderQty (the new, reduced quantity)
    m
}

/// Builds an OrderCancelRequestAndNewOrderSingle (35=XCN): atomically cancel `orig_cl_ord_id` and
/// place a new order, in `STOP_ON_FAILURE` mode (if the cancel fails, the new order is not
/// placed).
pub fn order_cancel_request_and_new_order_single(
    symbol: &str,
    orig_cl_ord_id: &str,
    side: &str,
    qty: &str,
    price: Option<&str>,
    cancel_restrictions: Option<CancelRestrictions>,
    rate_limit_mode: Option<OrderRateLimitExceededMode>,
) -> Result<Message> {
    let side_code = side_code(side)?;
    let mut m = Message::new();
    m.header.set(Field::string(35, "XCN"));
    m.body.set(Field::string(25033, "1")); // OrderCancelRequestAndNewOrderSingleMode = STOP_ON_FAILURE
    m.body.set(Field::string(25034, &next_id("cxlrep"))); // CancelClOrdID
    m.body.set(Field::string(41, orig_cl_ord_id)); // OrigClOrdID (the order being replaced)
    if let Some(r) = cancel_restrictions {
        m.body.set(Field::string(25002, r.tag_value())); // CancelRestrictions
    }
    if let Some(rl) = rate_limit_mode {
        m.body.set(Field::string(25038, rl.tag_value())); // OrderRateLimitExceededMode
    }
    // The `NewOrder` component's fields, spliced flat into the body (it's a reused field group,
    // not a repeating group).
    m.body.set(Field::string(11, &next_id("cli"))); // ClOrdID (the new order's own id)
    m.body.set(Field::string(38, qty)); // OrderQty
    m.body.set(Field::string(54, side_code));
    m.body.set(Field::string(55, symbol));
    match price {
        Some(p) => {
            m.body.set(Field::string(40, "2")); // OrdType = Limit
            m.body.set(Field::string(44, p));
            m.body.set(Field::string(59, "1")); // TimeInForce = GTC
        }
        None => {
            m.body.set(Field::string(40, "1")); // OrdType = Market
        }
    }
    Ok(m)
}

/// Marks a `NewOrderList` message as an OPO (Opposite Position Order) list.
pub fn set_opo(message: &mut Message) {
    message.body.set(Field::string(25046, "Y")); // OPO
}

fn oco_trigger_direction(side_code: &str) -> TriggerDirection {
    // A SELL stop-limit fires as the price falls through the trigger; a BUY stop-limit fires as
    // the price rises through it.
    if side_code == "2" {
        TriggerDirection::Down
    } else {
        TriggerDirection::Up
    }
}

fn build_leg(id_prefix: &str, symbol: &str, side_code: &str, qty: &str) -> FieldMap {
    let mut leg = FieldMap::new();
    leg.set(Field::string(11, &next_id(id_prefix))); // ClOrdID
    leg.set(Field::string(54, side_code));
    leg.set(Field::string(55, symbol));
    leg.set(Field::string(38, qty));
    leg
}

/// Appends the `ListTriggeringInstruction` component (`NoListTriggeringInstructions(25010)`) to
/// one `NewOrderList` leg entry. `instructions` are `(ListTriggerType, ListTriggerTriggerIndex,
/// ListTriggerAction)` triples -- e.g. `("3", 0, "1")` = "release this order once the order at
/// index 0 is FILLED".
fn add_triggering_instructions(entry: &mut FieldMap, instructions: &[(&str, u32, &str)]) {
    let mut group = Group::new(25010); // NoListTriggeringInstructions
    for (trigger_type, index, action) in instructions {
        let mut instr = FieldMap::new();
        instr.set(Field::string(25011, trigger_type)); // ListTriggerType
        instr.set(Field::int(25012, i64::from(*index))); // ListTriggerTriggerIndex
        instr.set(Field::string(25013, action)); // ListTriggerAction
        group.add_entry(instr);
    }
    entry.add_group(group);
}

/// Builds a NewOrderList (35=E) implementing a spot OCO: a Limit order plus a Stop-Limit order,
/// same side, one canceling the other on execution (`ContingencyType=ONE_CANCELS_THE_OTHER`).
/// `stop_limit_price` defaults to `stop_price` when absent (a simple stop-limit leg).
pub fn new_order_list_oco(
    symbol: &str,
    side: &str,
    qty: &str,
    price: &str,
    stop_price: &str,
    stop_limit_price: Option<&str>,
) -> Result<Message> {
    let side_code = side_code(side)?;
    let mut m = Message::new();
    m.header.set(Field::string(35, "E"));
    m.body.set(Field::string(25014, &next_id("oco"))); // ClListID
    m.body.set(Field::string(1385, "1")); // ContingencyType = ONE_CANCELS_THE_OTHER

    let mut orders = Group::new(73); // NoOrders

    // Discovered live against testnet, by trial against both sides: Binance's OCO cross-cancel
    // `ListTriggerType` isn't symmetric between the two legs -- whichever leg sits *above* the
    // market (SELL: the Limit leg; BUY: the Stop-Limit leg) must reference
    // `ACTIVATED(1)`/... actually the *above* leg wants `PARTIALLY_FILLED(2)` and the *below* leg
    // wants `ACTIVATED(1)`; Binance's own -1174 error recommended exactly this pairing for each
    // side. A SELL OCO's Limit leg sits above the market (take-profit) and its Stop-Limit leg
    // sits below (stop-loss); a BUY OCO is the mirror image.
    let (limit_trigger_type, stop_trigger_type) = if side_code == "2" {
        ("2", "1") // SELL: Limit is the "above" leg, Stop-Limit is "below"
    } else {
        ("1", "2") // BUY: Limit is the "below" leg, Stop-Limit is "above"
    };

    // index 0
    let mut limit_leg = build_leg("oco-lmt", symbol, side_code, qty);
    limit_leg.set(Field::string(40, "2")); // OrdType = Limit
    limit_leg.set(Field::string(44, price));
    // Discovered live against testnet: Binance rejects a plain Limit leg in an OCO with -1158
    // "Order type not supported in OCO" -- the non-stop leg must be a LIMIT_MAKER (post-only),
    // which in turn must NOT carry TimeInForce (-1106 "sent when not required"), matching the
    // standalone `OrderKind::LimitMaker` order's own field set.
    limit_leg.set(Field::string(18, "6")); // ExecInst = PARTICIPATE_DONT_INITIATE
    // Also discovered live: every OCO leg needs its own `ListTriggeringInstruction` naming its
    // sibling, or Binance rejects with -1174 ("Component 'ListTriggeringInstruction' is
    // incorrectly populated").
    add_triggering_instructions(&mut limit_leg, &[(limit_trigger_type, 1, "2")]); // cancel this leg once the sibling (index 1) crosses the threshold
    orders.add_entry(limit_leg);

    // index 1
    let mut stop_leg = build_leg("oco-stp", symbol, side_code, qty);
    stop_leg.set(Field::string(40, "4")); // OrdType = Stop Limit
    stop_leg.set(Field::string(44, stop_limit_price.unwrap_or(stop_price))); // Price (the limit price of the stop-limit leg)
    stop_leg.set(Field::string(59, "1")); // TimeInForce = GTC
    set_trigger(&mut stop_leg, stop_price, oco_trigger_direction(side_code));
    add_triggering_instructions(&mut stop_leg, &[(stop_trigger_type, 0, "2")]); // cancel this leg once the sibling (index 0) crosses the threshold
    orders.add_entry(stop_leg);

    m.body.add_group(orders);
    Ok(m)
}

/// Builds a NewOrderList (35=E) implementing a spot OTO (One-Triggers-Other): a working Limit
/// order (index 0) that, once filled, releases a second, pending Limit order (index 1).
pub fn new_order_list_oto(
    symbol: &str,
    working_side: &str,
    working_qty: &str,
    working_price: &str,
    pending_side: &str,
    pending_qty: &str,
    pending_price: &str,
) -> Result<Message> {
    let working_side_code = side_code(working_side)?;
    let pending_side_code = side_code(pending_side)?;
    let mut m = Message::new();
    m.header.set(Field::string(35, "E"));
    m.body.set(Field::string(25014, &next_id("oto"))); // ClListID
    m.body.set(Field::string(1385, "2")); // ContingencyType = ONE_TRIGGERS_THE_OTHER

    let mut orders = Group::new(73); // NoOrders

    let mut working = build_leg("oto-working", symbol, working_side_code, working_qty);
    working.set(Field::string(40, "2")); // OrdType = Limit
    working.set(Field::string(44, working_price));
    working.set(Field::string(59, "1")); // TimeInForce = GTC
    orders.add_entry(working);

    let mut pending = build_leg("oto-pending", symbol, pending_side_code, pending_qty);
    pending.set(Field::string(40, "2")); // OrdType = Limit
    pending.set(Field::string(44, pending_price));
    pending.set(Field::string(59, "1")); // TimeInForce = GTC
    add_triggering_instructions(&mut pending, &[("3", 0, "1")]); // released when order[0] is FILLED
    orders.add_entry(pending);

    m.body.add_group(orders);
    Ok(m)
}

/// Parameters for [`new_order_list_otoco`] (bundled into a struct since there are too many for a
/// readable positional argument list).
pub struct OtocoParams<'a> {
    pub working_side: &'a str,
    pub working_qty: &'a str,
    pub working_price: &'a str,
    pub pending_side: &'a str,
    pub pending_qty: &'a str,
    pub pending_limit_price: &'a str,
    pub pending_stop_price: &'a str,
    pub pending_stop_limit_price: Option<&'a str>,
}

/// Builds a NewOrderList (35=E) implementing a spot OTOCO (One-Triggers-OCO): a working Limit
/// order (index 0) that, once filled, releases an OCO pair (index 1 Limit, index 2 Stop-Limit).
///
/// **Best-effort**: Binance's published schema has a single, list-wide `ContingencyType(1385)`
/// field, so the "OTO" (working -> pending) and "OCO" (pending leg vs. pending leg) relationships
/// are both expressed only through each pending leg's own `NoListTriggeringInstructions` entries
/// (release on the working order's `FILLED`, cancel on the sibling leg's `FILLED`) -- this isn't
/// spelled out field-by-field in the docs and hasn't been exercised against a live matching
/// engine; validate on testnet before relying on it.
pub fn new_order_list_otoco(symbol: &str, params: OtocoParams) -> Result<Message> {
    let OtocoParams {
        working_side,
        working_qty,
        working_price,
        pending_side,
        pending_qty,
        pending_limit_price,
        pending_stop_price,
        pending_stop_limit_price,
    } = params;
    let working_side_code = side_code(working_side)?;
    let pending_side_code = side_code(pending_side)?;
    let mut m = Message::new();
    m.header.set(Field::string(35, "E"));
    m.body.set(Field::string(25014, &next_id("otoco"))); // ClListID
    m.body.set(Field::string(1385, "2")); // ContingencyType = ONE_TRIGGERS_THE_OTHER

    let mut orders = Group::new(73); // NoOrders

    let mut working = build_leg("otoco-working", symbol, working_side_code, working_qty);
    working.set(Field::string(40, "2")); // OrdType = Limit
    working.set(Field::string(44, working_price));
    working.set(Field::string(59, "1")); // TimeInForce = GTC
    orders.add_entry(working);

    // Same asymmetric cross-cancel `ListTriggerType` requirement discovered live for
    // `new_order_list_oco` -- confirmed by testing both sides end-to-end: the "above the market"
    // pending leg (SELL: Limit; BUY: Stop-Limit) needs `PARTIALLY_FILLED(2)`, the "below" one
    // needs `ACTIVATED(1)`.
    let (limit_trigger_type, stop_trigger_type) = if pending_side_code == "2" {
        ("2", "1") // pending side SELL: Limit is "above", Stop-Limit is "below"
    } else {
        ("1", "2") // pending side BUY: Limit is "below", Stop-Limit is "above"
    };

    let mut limit_leg = build_leg("otoco-lmt", symbol, pending_side_code, pending_qty);
    limit_leg.set(Field::string(40, "2")); // OrdType = Limit
    limit_leg.set(Field::string(44, pending_limit_price));
    // Same OCO-leg requirement as `new_order_list_oco`'s limit leg (discovered live against
    // testnet): must be a LIMIT_MAKER (post-only, no TimeInForce), or Binance rejects the whole
    // list (-1158 then -1106).
    limit_leg.set(Field::string(18, "6")); // ExecInst = PARTICIPATE_DONT_INITIATE
    add_triggering_instructions(
        &mut limit_leg,
        &[
            ("3", 0, "1"),                // released when the working order (index 0) is FILLED
            (limit_trigger_type, 2, "2"), // canceled once its OCO sibling (index 2) crosses the threshold
        ],
    );
    orders.add_entry(limit_leg);

    let mut stop_leg = build_leg("otoco-stp", symbol, pending_side_code, pending_qty);
    stop_leg.set(Field::string(40, "4")); // OrdType = Stop Limit
    stop_leg.set(Field::string(
        44,
        pending_stop_limit_price.unwrap_or(pending_stop_price),
    ));
    stop_leg.set(Field::string(59, "1")); // TimeInForce = GTC
    set_trigger(
        &mut stop_leg,
        pending_stop_price,
        oco_trigger_direction(pending_side_code),
    );
    add_triggering_instructions(
        &mut stop_leg,
        &[
            ("3", 0, "1"),               // released when the working order (index 0) is FILLED
            (stop_trigger_type, 1, "2"), // canceled once its OCO sibling (index 1) crosses the threshold
        ],
    );
    orders.add_entry(stop_leg);

    m.body.add_group(orders);
    Ok(m)
}

/// Builds a LimitQuery (35=XLQ): ask for the current message/order rate-limit usage.
pub fn limit_query() -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "XLQ"));
    m.body.set(Field::string(6136, &next_id("lq"))); // ReqID
    m
}

/// `MDEntryType(269)` values a `MarketDataRequest` can subscribe to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MdEntryKind {
    Bid,
    Offer,
    Trade,
}

impl MdEntryKind {
    fn tag_value(self) -> &'static str {
        match self {
            Self::Bid => "0",
            Self::Offer => "1",
            Self::Trade => "2",
        }
    }
}

/// Builds a MarketDataRequest (35=V). `subscribe = false` sends an unsubscribe for `mdreqid`
/// (Binance requires echoing the *original* subscription's MDReqID to cancel it). `depth`: `Some(1)`
/// for the best bid/offer only (book ticker), `Some(2..=5000)` for a depth snapshot+updates stream,
/// or `None` for a plain trade stream (no `MarketDepth`/`AggregatedBook`).
pub fn market_data_request(
    mdreqid: &str,
    symbol: &str,
    subscribe: bool,
    entry_kinds: &[MdEntryKind],
    depth: Option<u32>,
) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "V"));
    m.body.set(Field::string(262, mdreqid)); // MDReqID
    m.body
        .set(Field::string(263, if subscribe { "1" } else { "2" })); // SubscriptionRequestType
    if let Some(d) = depth {
        m.body.set(Field::int(264, i64::from(d))); // MarketDepth
        m.body.set(Field::string(266, "Y")); // AggregatedBook, required for depth streams
    }

    let mut entry_types = Group::new(267); // NoMDEntryTypes
    for kind in entry_kinds {
        let mut entry = FieldMap::new();
        entry.set(Field::string(269, kind.tag_value())); // MDEntryType
        entry_types.add_entry(entry);
    }
    m.body.add_group(entry_types);

    let mut related_sym = Group::new(146); // NoRelatedSym
    let mut sym_entry = FieldMap::new();
    sym_entry.set(Field::string(55, symbol)); // Symbol
    related_sym.add_entry(sym_entry);
    m.body.add_group(related_sym);

    m
}

/// Builds an InstrumentListRequest (35=x). `symbol` present -> query that one symbol's trading
/// rules; absent -> query every instrument.
pub fn instrument_list_request(symbol: Option<&str>) -> Message {
    let mut m = Message::new();
    m.header.set(Field::string(35, "x"));
    m.body.set(Field::string(320, &next_id("il"))); // InstrumentReqID
    match symbol {
        Some(s) => {
            m.body.set(Field::string(559, "0")); // InstrumentListRequestType = SINGLE_INSTRUMENT
            m.body.set(Field::string(55, s));
        }
        None => {
            m.body.set(Field::string(559, "4")); // InstrumentListRequestType = ALL_INSTRUMENTS
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_str(m: &Message, tag: u32) -> String {
        m.body
            .get(tag)
            .and_then(|f| f.as_str().ok())
            .unwrap_or_default()
            .to_owned()
    }

    #[test]
    fn new_order_single_limit() {
        let m = new_order_single(
            "BTCUSDT",
            "BUY",
            "0.001",
            &OrderKind::Limit {
                price: "50000",
                tif: TimeInForce::GoodTillCancel,
            },
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "D");
        assert_eq!(body_str(&m, 55), "BTCUSDT");
        assert_eq!(body_str(&m, 54), "1");
        assert!(m.body.get(60).is_none()); // Binance rejects TransactTime on requests
        assert_eq!(body_str(&m, 40), "2");
        assert_eq!(body_str(&m, 44), "50000");
        assert_eq!(body_str(&m, 59), "1");
    }

    #[test]
    fn new_order_single_market() {
        let m = new_order_single(
            "ETHUSDT",
            "SELL",
            "1",
            &OrderKind::Market,
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(body_str(&m, 54), "2");
        assert_eq!(body_str(&m, 40), "1");
        assert!(m.body.get(44).is_none());
    }

    #[test]
    fn new_order_single_ioc_tif() {
        let m = new_order_single(
            "BTCUSDT",
            "BUY",
            "1",
            &OrderKind::Limit {
                price: "100",
                tif: TimeInForce::ImmediateOrCancel,
            },
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(body_str(&m, 59), "3");
    }

    #[test]
    fn new_order_single_bad_side() {
        assert!(
            new_order_single(
                "BTCUSDT",
                "HOLD",
                "1",
                &OrderKind::Market,
                &NewOrderOptions::default()
            )
            .is_err()
        );
    }

    #[test]
    fn new_order_single_limit_maker_sets_post_only() {
        let m = new_order_single(
            "BTCUSDT",
            "BUY",
            "1",
            &OrderKind::LimitMaker { price: "100" },
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(body_str(&m, 40), "2");
        assert_eq!(body_str(&m, 18), "6");
    }

    #[test]
    fn new_order_single_stop_sets_trigger() {
        let m = new_order_single(
            "BTCUSDT",
            "SELL",
            "1",
            &OrderKind::Stop {
                trigger_price: "90",
                direction: TriggerDirection::Down,
            },
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(body_str(&m, 40), "3");
        assert_eq!(body_str(&m, 1102), "90");
        assert_eq!(body_str(&m, 1109), "D");
        assert!(m.body.get(44).is_none()); // Stop (market) has no limit Price
    }

    #[test]
    fn new_order_single_stop_limit_sets_price_and_trigger() {
        let m = new_order_single(
            "BTCUSDT",
            "BUY",
            "1",
            &OrderKind::StopLimit {
                price: "105",
                trigger_price: "100",
                direction: TriggerDirection::Up,
                tif: TimeInForce::GoodTillCancel,
            },
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(body_str(&m, 40), "4");
        assert_eq!(body_str(&m, 44), "105");
        assert_eq!(body_str(&m, 1102), "100");
        assert_eq!(body_str(&m, 1109), "U");
    }

    #[test]
    fn new_order_single_pegged_sets_peg_fields() {
        let m = new_order_single(
            "BTCUSDT",
            "BUY",
            "1",
            &OrderKind::Pegged {
                peg_offset: "5",
                price_type: PegPriceType::Primary,
                tif: TimeInForce::GoodTillCancel,
            },
            &NewOrderOptions::default(),
        )
        .unwrap();
        assert_eq!(body_str(&m, 40), "P");
        assert_eq!(body_str(&m, 211), "5");
        assert_eq!(body_str(&m, 1094), "5");
        assert_eq!(body_str(&m, 59), "1");
    }

    #[test]
    fn new_order_single_options_all_set() {
        let opts = NewOrderOptions {
            self_trade_prevention_mode: Some(SelfTradePreventionMode::ExpireTaker),
            iceberg_qty: Some("0.1"),
            trailing_delta_bips: Some("50"),
            strategy: Some(("1000001", "42")),
            sor: true,
            cash_order_qty: true,
        };
        let m = new_order_single("BTCUSDT", "BUY", "100", &OrderKind::Market, &opts).unwrap();
        assert_eq!(body_str(&m, 25001), "2");
        assert_eq!(body_str(&m, 111), "0.1");
        assert_eq!(body_str(&m, 25009), "50");
        assert_eq!(body_str(&m, 847), "1000001");
        assert_eq!(body_str(&m, 7940), "42");
        assert_eq!(body_str(&m, 25032), "Y");
        assert_eq!(body_str(&m, 152), "100"); // CashOrderQty, not OrderQty
        assert!(m.body.get(38).is_none());
    }

    #[test]
    fn order_cancel_request_by_orig_cl_ord_id() {
        let m = order_cancel_request("BTCUSDT", CancelTarget::OrigClOrdId("orig-1"), None);
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "F");
        assert_eq!(body_str(&m, 41), "orig-1");
        assert_eq!(body_str(&m, 55), "BTCUSDT");
        assert!(m.body.get(25015).is_none());
    }

    #[test]
    fn order_cancel_request_by_orig_cl_list_id_with_restriction() {
        let m = order_cancel_request(
            "BTCUSDT",
            CancelTarget::OrigClListId("list-1"),
            Some(CancelRestrictions::OnlyPartiallyFilled),
        );
        assert_eq!(body_str(&m, 25015), "list-1");
        assert!(m.body.get(41).is_none());
        assert_eq!(body_str(&m, 25002), "2");
    }

    #[test]
    fn order_mass_cancel_request_encodes() {
        let m = order_mass_cancel_request("BTCUSDT");
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "q");
        assert_eq!(body_str(&m, 530), "1");
    }

    #[test]
    fn order_amend_keep_priority_encodes() {
        let m = order_amend_keep_priority("BTCUSDT", "orig-1", "0.5");
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "XAK");
        assert_eq!(body_str(&m, 41), "orig-1");
        assert_eq!(body_str(&m, 38), "0.5");
    }

    #[test]
    fn order_cancel_request_and_new_order_single_encodes() {
        let m = order_cancel_request_and_new_order_single(
            "BTCUSDT",
            "orig-1",
            "BUY",
            "1",
            Some("100"),
            Some(CancelRestrictions::OnlyNew),
            Some(OrderRateLimitExceededMode::CancelOnly),
        )
        .unwrap();
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "XCN");
        assert_eq!(body_str(&m, 25033), "1");
        assert_eq!(body_str(&m, 41), "orig-1");
        assert_eq!(body_str(&m, 54), "1");
        assert_eq!(body_str(&m, 44), "100");
        assert_eq!(body_str(&m, 25002), "1");
        assert_eq!(body_str(&m, 25038), "2");
    }

    #[test]
    fn new_order_list_oco_encodes_two_legs() {
        let m = new_order_list_oco("BTCUSDT", "SELL", "1", "51000", "49000", None).unwrap();
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "E");
        assert_eq!(body_str(&m, 1385), "1");
        let entries = m.body.group(73).expect("NoOrders group");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].get(40).unwrap().as_str().unwrap(), "2"); // Limit
        assert_eq!(entries[0].get(44).unwrap().as_str().unwrap(), "51000");
        assert_eq!(entries[0].get(18).unwrap().as_str().unwrap(), "6"); // ExecInst = LIMIT_MAKER (required by Binance for OCO's limit leg)
        assert!(entries[0].get(59).is_none()); // LIMIT_MAKER must not carry TimeInForce
        assert_eq!(entries[1].get(40).unwrap().as_str().unwrap(), "4"); // Stop Limit
        assert_eq!(entries[1].get(44).unwrap().as_str().unwrap(), "49000");
        assert_eq!(entries[1].get(1102).unwrap().as_str().unwrap(), "49000"); // TriggerPrice
        assert_eq!(entries[1].get(1109).unwrap().as_str().unwrap(), "D"); // SELL -> triggers on the way down

        let limit_leg_instructions = entries[0].group(25010).expect("limit leg instructions");
        assert_eq!(limit_leg_instructions.len(), 1);
        assert_eq!(
            limit_leg_instructions[0]
                .get(25011)
                .unwrap()
                .as_str()
                .unwrap(),
            "2"
        ); // PARTIALLY_FILLED
        assert_eq!(
            limit_leg_instructions[0]
                .get(25012)
                .unwrap()
                .as_int()
                .unwrap(),
            1
        ); // sibling index 1
        assert_eq!(
            limit_leg_instructions[0]
                .get(25013)
                .unwrap()
                .as_str()
                .unwrap(),
            "2"
        ); // CANCEL
        let stop_leg_instructions = entries[1].group(25010).expect("stop leg instructions");
        assert_eq!(
            stop_leg_instructions[0]
                .get(25011)
                .unwrap()
                .as_str()
                .unwrap(),
            "1"
        ); // ACTIVATED (asymmetric vs. the Limit leg's "2" -- confirmed live for both sides)
        assert_eq!(
            stop_leg_instructions[0]
                .get(25012)
                .unwrap()
                .as_int()
                .unwrap(),
            0
        ); // sibling index 0
    }

    #[test]
    fn new_order_list_oco_buy_uses_mirrored_trigger_types() {
        let m = new_order_list_oco("BTCUSDT", "BUY", "1", "49000", "51000", None).unwrap();
        let entries = m.body.group(73).expect("NoOrders group");
        let limit_leg_instructions = entries[0].group(25010).expect("limit leg instructions");
        assert_eq!(
            limit_leg_instructions[0]
                .get(25011)
                .unwrap()
                .as_str()
                .unwrap(),
            "1"
        ); // ACTIVATED
        let stop_leg_instructions = entries[1].group(25010).expect("stop leg instructions");
        assert_eq!(
            stop_leg_instructions[0]
                .get(25011)
                .unwrap()
                .as_str()
                .unwrap(),
            "2"
        ); // PARTIALLY_FILLED
    }

    #[test]
    fn new_order_list_oto_encodes_trigger_instruction() {
        let m = new_order_list_oto("BTCUSDT", "BUY", "1", "100", "SELL", "1", "110").unwrap();
        assert_eq!(body_str(&m, 1385), "2"); // ONE_TRIGGERS_THE_OTHER
        let entries = m.body.group(73).expect("NoOrders group");
        assert_eq!(entries.len(), 2);
        let pending = &entries[1];
        let instructions = pending.group(25010).expect("NoListTriggeringInstructions");
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].get(25011).unwrap().as_str().unwrap(), "3"); // FILLED
        assert_eq!(instructions[0].get(25012).unwrap().as_int().unwrap(), 0);
        assert_eq!(instructions[0].get(25013).unwrap().as_str().unwrap(), "1"); // RELEASE
    }

    #[test]
    fn new_order_list_otoco_encodes_three_legs_with_cross_triggers() {
        let m = new_order_list_otoco(
            "BTCUSDT",
            OtocoParams {
                working_side: "BUY",
                working_qty: "1",
                working_price: "100",
                pending_side: "SELL",
                pending_qty: "1",
                pending_limit_price: "110",
                pending_stop_price: "90",
                pending_stop_limit_price: None,
            },
        )
        .unwrap();
        let entries = m.body.group(73).expect("NoOrders group");
        assert_eq!(entries.len(), 3);
        assert!(entries[0].get(18).is_none()); // working leg: plain Limit, no ExecInst needed
        assert_eq!(entries[1].get(18).unwrap().as_str().unwrap(), "6"); // pending limit leg: LIMIT_MAKER required
        assert!(entries[1].get(59).is_none()); // LIMIT_MAKER must not carry TimeInForce
        let limit_leg_instructions = entries[1].group(25010).expect("limit leg instructions");
        assert_eq!(limit_leg_instructions.len(), 2);
        let stop_leg_instructions = entries[2].group(25010).expect("stop leg instructions");
        assert_eq!(stop_leg_instructions.len(), 2);
    }

    #[test]
    fn set_opo_stamps_flag() {
        let mut m = new_order_list_oco("BTCUSDT", "BUY", "1", "100", "90", None).unwrap();
        set_opo(&mut m);
        assert_eq!(body_str(&m, 25046), "Y");
    }

    #[test]
    fn limit_query_encodes() {
        let m = limit_query();
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "XLQ");
        assert!(m.body.get(6136).is_some());
    }

    #[test]
    fn market_data_request_subscribe_book_encodes() {
        let m = market_data_request(
            "md-1",
            "BTCUSDT",
            true,
            &[MdEntryKind::Bid, MdEntryKind::Offer],
            Some(5),
        );
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "V");
        let bytes = m.encode();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("262=md-1"));
        assert!(text.contains("263=1"));
        assert!(text.contains("266=Y"));
        assert!(text.contains("267=2"));
        assert!(text.contains("269=0"));
        assert!(text.contains("269=1"));
        assert!(text.contains("146=1"));
        assert!(text.contains("55=BTCUSDT"));
    }

    #[test]
    fn market_data_request_trades_mode_omits_depth() {
        let m = market_data_request("md-1", "BTCUSDT", true, &[MdEntryKind::Trade], None);
        assert!(m.body.get(264).is_none());
        assert!(m.body.get(266).is_none());
        let entries = m.body.group(267).expect("NoMDEntryTypes");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].get(269).unwrap().as_str().unwrap(), "2");
    }

    #[test]
    fn market_data_request_unsubscribe_encodes() {
        let m = market_data_request(
            "md-1",
            "BTCUSDT",
            false,
            &[MdEntryKind::Bid, MdEntryKind::Offer],
            None,
        );
        assert_eq!(body_str(&m, 263), "2");
    }

    #[test]
    fn instrument_list_request_single_encodes() {
        let m = instrument_list_request(Some("BTCUSDT"));
        assert_eq!(m.header.get(35).unwrap().as_str().unwrap(), "x");
        assert_eq!(body_str(&m, 559), "0");
        assert_eq!(body_str(&m, 55), "BTCUSDT");
    }

    #[test]
    fn instrument_list_request_all_encodes() {
        let m = instrument_list_request(None);
        assert_eq!(body_str(&m, 559), "4");
        assert!(m.body.get(55).is_none());
    }
}
