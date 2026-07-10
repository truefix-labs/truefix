use super::*;

pub(super) fn decode_open_order_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut reader = FieldReader::new(fields);
    let has_version = reader
        .peek(2)
        .is_some_and(|field| field.parse::<i32>().is_ok());
    let version = if has_version {
        reader.next_i32()?
    } else {
        i32::MAX
    };

    let order_id = reader.next_i32()?;
    let contract = decode_order_contract_prefix(&mut reader, version)?;
    let mut order = decode_order_core_after_contract(&mut reader, order_id)?;
    let mut order_state = OrderState::default();
    order.client_id = reader.next_i32_or_default();
    order.perm_id = i64::from(reader.next_i32_or_default());
    decode_order_common_after_perm_id(&mut reader, version, &mut order)?;
    order.what_if = reader.next_bool();
    decode_order_open_state_fields(&mut reader, &mut order_state)?;
    decode_order_state_tail_fields(&mut reader, &mut order);
    order.imbalance_only = reader.next_bool();

    Ok(Event::OpenOrder {
        order_id,
        contract: Box::new(contract),
        order: Box::new(order),
        order_state: Box::new(order_state),
    })
}

pub(super) fn decode_completed_order_fields(fields: &[String]) -> TwsApiResult<Event> {
    let mut reader = FieldReader::new(fields);
    let version = i32::MAX;
    let contract = decode_order_contract_prefix(&mut reader, version)?;
    let mut order = decode_order_core_after_contract(&mut reader, 0)?;
    order.perm_id = i64::from(reader.next_i32_or_default());
    decode_order_common_after_perm_id(&mut reader, version, &mut order)?;
    order.solicited = reader.next_bool();
    let mut order_state = OrderState {
        status: reader.next_string(),
        ..OrderState::default()
    };
    order.randomize_size = reader.next_bool();
    order.randomize_price = reader.next_bool();
    order.reference_contract_id = reader.next_i32_or_default();
    order.is_pegged_change_amount_decrease = reader.next_bool();
    order.pegged_change_amount = reader.next_f64_or_default();
    order.reference_change_amount = reader.next_f64_or_default();
    order.reference_exchange_id = reader.next_string();
    order.conditions = decode_order_conditions(&mut reader)?;
    order.adjusted_order_type = reader.next_string();
    order.trigger_price = reader.next_f64_or_default();
    order.limit_price_offset = reader.next_f64_or_default();
    order.adjusted_stop_price = reader.next_f64_or_default();
    order.adjusted_stop_limit_price = reader.next_f64_or_default();
    order.adjusted_trailing_amount = reader.next_f64_or_default();
    order.adjustable_trailing_unit = reader.next_i32_or_default();
    order.cash_qty = reader.next_f64_or_default();
    order.dont_use_auto_price_for_hedge = reader.next_bool();
    order.is_oms_container = reader.next_bool();
    order.auto_cancel_date = reader.next_string();
    order.filled_quantity = reader.next_decimal_or_default();
    order.ref_futures_con_id = reader.next_i32_or_default();
    order.auto_cancel_parent = reader.next_bool();
    order.shareholder = reader.next_string();
    order.imbalance_only = reader.next_bool();
    order.route_marketable_to_bbo = if reader.next_bool() { 1 } else { 0 };
    order.parent_perm_id = i64::from(reader.next_i32_or_default());
    decode_order_completed_state_fields(&mut reader, &mut order_state);
    decode_order_state_tail_fields(&mut reader, &mut order);

    Ok(Event::CompletedOrder {
        contract: Box::new(contract),
        order: Box::new(order),
        order_state: Box::new(order_state),
    })
}

fn decode_order_algo_fields(reader: &mut FieldReader<'_>, order: &mut Order) -> TwsApiResult<()> {
    order.algo_strategy = reader.next_string();
    if !order.algo_strategy.is_empty() {
        let algo_count = reader.next_i32_or_default().max(0) as usize;
        order.algo_params = Vec::with_capacity(algo_count);
        for _ in 0..algo_count {
            order.algo_params.push(TagValue {
                tag: reader.next_string(),
                value: reader.next_string(),
            });
        }
    }
    Ok(())
}

fn decode_order_common_after_perm_id(
    reader: &mut FieldReader<'_>,
    version: i32,
    order: &mut Order,
) -> TwsApiResult<()> {
    order.outside_rth = reader.next_bool();
    order.hidden = reader.next_bool();
    order.discretionary_amount = reader.next_f64_or_default();
    order.good_after_time = reader.next_string();
    let _shares_allocation = reader.next_string();
    order.fa_group = reader.next_string();
    order.fa_method = reader.next_string();
    order.fa_percentage = reader.next_string();
    if version < 177 {
        let _fa_profile = reader.next_string();
    }
    if version >= 177 {
        order.model_code = reader.next_string();
    }
    order.good_till_date = reader.next_string();
    order.rule80a = reader.next_string();
    order.percent_offset = reader.next_f64_or_default();
    order.settling_firm = reader.next_string();
    order.short_sale_slot = reader.next_i32_or_default();
    order.designated_location = reader.next_string();
    if version >= 156 {
        order.exempt_code = reader.next_i32_or_default();
    }
    order.oca_type = reader.next_i32_or_default();
    order.rule80a = reader.next_string();
    order.settling_firm = reader.next_string();
    order.all_or_none = reader.next_bool();
    order.min_qty = reader.next_i32_or_default();
    order.percent_offset = reader.next_f64_or_default();
    let _e_trade_only = reader.next_bool();
    let _firm_quote_only = reader.next_bool();
    let _nbbo_price_cap = reader.next_f64_or_default();
    order.parent_id = reader.next_i32_or_default();
    order.trigger_method = reader.next_i32_or_default();
    order.volatility = reader.next_f64_or_default();
    order.volatility_type = reader.next_i32_or_default();
    order.delta_neutral_order_type = reader.next_string();
    order.delta_neutral_aux_price = reader.next_f64_or_default();
    if version >= 27 && !order.delta_neutral_order_type.is_empty() {
        order.delta_neutral_con_id = reader.next_i32_or_default();
        order.delta_neutral_settling_firm = reader.next_string();
        order.delta_neutral_clearing_account = reader.next_string();
        order.delta_neutral_clearing_intent = reader.next_string();
    }
    if version >= 31 && !order.delta_neutral_order_type.is_empty() {
        order.delta_neutral_open_close = reader.next_string();
        order.delta_neutral_short_sale = reader.next_bool();
        order.delta_neutral_short_sale_slot = reader.next_i32_or_default();
        order.delta_neutral_designated_location = reader.next_string();
    }
    order.continuous_update = reader.next_bool();
    order.reference_price_type = reader.next_i32_or_default();
    order.trail_stop_price = reader.next_f64_or_default();
    if version >= 30 {
        order.trailing_percent = reader.next_f64_or_default();
    }
    if version >= 20 {
        order.scale_init_level_size = reader.next_i32_or_default();
        order.scale_subs_level_size = reader.next_i32_or_default();
    } else {
        let _scale_num_components = reader.next_string();
        order.scale_init_level_size = reader.next_i32_or_default();
    }
    order.scale_price_increment = reader.next_f64_or_default();
    if version >= 28 && order.scale_price_increment != 0.0 {
        order.scale_price_adjust_value = reader.next_f64_or_default();
        order.scale_price_adjust_interval = reader.next_i32_or_default();
        order.scale_profit_offset = reader.next_f64_or_default();
        order.scale_auto_reset = reader.next_bool();
        order.scale_init_position = reader.next_i32_or_default();
        order.scale_init_fill_qty = reader.next_i32_or_default();
        order.scale_random_percent = reader.next_bool();
    }
    order.scale_table = reader.next_string();
    order.active_start_time = reader.next_string();
    order.active_stop_time = reader.next_string();
    order.hedge_type = reader.next_string();
    if !order.hedge_type.is_empty() {
        order.hedge_param = reader.next_string();
    }
    order.opt_out_smart_routing = reader.next_bool();
    order.clearing_account = reader.next_string();
    order.clearing_intent = reader.next_string();
    order.not_held = reader.next_bool();
    if version >= 20 {
        let has_delta_neutral = reader.next_bool();
        if has_delta_neutral {
            let _con_id = reader.next_i32_or_default();
            let _delta = reader.next_f64_or_default();
            let _price = reader.next_f64_or_default();
        }
    }
    decode_order_algo_fields(reader, order)?;
    Ok(())
}

fn decode_order_open_state_fields(
    reader: &mut FieldReader<'_>,
    order_state: &mut OrderState,
) -> TwsApiResult<()> {
    order_state.status = reader.next_string();
    order_state.init_margin_before = reader.next_f64_or_default();
    order_state.maint_margin_before = reader.next_f64_or_default();
    order_state.equity_with_loan_before = reader.next_f64_or_default();
    order_state.init_margin_change = reader.next_f64_or_default();
    order_state.maint_margin_change = reader.next_f64_or_default();
    order_state.equity_with_loan_change = reader.next_f64_or_default();
    order_state.init_margin_after = reader.next_f64_or_default();
    order_state.maint_margin_after = reader.next_f64_or_default();
    order_state.equity_with_loan_after = reader.next_f64_or_default();
    order_state.commission_and_fees = reader.next_f64_or_default();
    order_state.min_commission_and_fees = reader.next_f64_or_default();
    order_state.max_commission_and_fees = reader.next_f64_or_default();
    order_state.commission_and_fees_currency = reader.next_string();
    order_state.margin_currency = reader.next_string();
    order_state.init_margin_before_outside_rth = reader.next_f64_or_default();
    order_state.maint_margin_before_outside_rth = reader.next_f64_or_default();
    order_state.equity_with_loan_before_outside_rth = reader.next_f64_or_default();
    order_state.init_margin_change_outside_rth = reader.next_f64_or_default();
    order_state.maint_margin_change_outside_rth = reader.next_f64_or_default();
    order_state.equity_with_loan_change_outside_rth = reader.next_f64_or_default();
    order_state.init_margin_after_outside_rth = reader.next_f64_or_default();
    order_state.maint_margin_after_outside_rth = reader.next_f64_or_default();
    order_state.equity_with_loan_after_outside_rth = reader.next_f64_or_default();
    order_state.suggested_size = reader.next_string();
    order_state.reject_reason = reader.next_string();
    order_state.order_allocations = decode_order_state_allocations(reader)?;
    order_state.warning_text = reader.next_string();
    order_state.completed_time = reader.next_string();
    order_state.completed_status = reader.next_string();
    Ok(())
}

fn decode_order_completed_state_fields(reader: &mut FieldReader<'_>, order_state: &mut OrderState) {
    order_state.completed_time = reader.next_string();
    order_state.completed_status = reader.next_string();
}

fn decode_order_state_tail_fields(reader: &mut FieldReader<'_>, order: &mut Order) {
    order.min_trade_qty = reader.next_i32_or_default();
    order.min_compete_size = reader.next_i32_or_default();
    order.compete_against_best_offset = reader.next_f64_or_default();
    order.mid_offset_at_whole = reader.next_f64_or_default();
    order.mid_offset_at_half = reader.next_f64_or_default();
    order.customer_account = reader.next_string();
    order.professional_customer = reader.next_bool();
    order.bond_accrued_interest = reader.next_string();
    order.include_overnight = reader.next_bool();
    order.manual_order_indicator = reader.next_i32_or_default();
    order.submitter = reader.next_string();
}

fn decode_order_contract_prefix(
    reader: &mut FieldReader<'_>,
    version: i32,
) -> TwsApiResult<Contract> {
    let mut contract = Contract {
        con_id: reader.next_i32()?,
        symbol: reader.next_string(),
        sec_type: reader.next_string(),
        last_trade_date_or_contract_month: reader.next_string(),
        strike: reader.next_f64()?,
        right: reader.next_string(),
        ..Contract::default()
    };
    if version >= 32 {
        contract.multiplier = reader.next_string();
    }
    contract.exchange = reader.next_string();
    contract.currency = reader.next_string();
    contract.local_symbol = reader.next_string();
    if version >= 32 {
        contract.trading_class = reader.next_string();
    }
    Ok(contract)
}

fn decode_order_core_after_contract(
    reader: &mut FieldReader<'_>,
    order_id: i32,
) -> TwsApiResult<Order> {
    let mut order = Order {
        order_id,
        action: reader.next_string(),
        total_quantity: reader.next_decimal()?,
        order_type: reader.next_string(),
        limit_price: reader.next_f64()?,
        aux_price: reader.next_f64()?,
        tif: reader.next_string(),
        oca_group: reader.next_string(),
        account: reader.next_string(),
        ..Order::default()
    };
    order.open_close = reader.next_string();
    order.origin = match reader.next_i32_or_default() {
        1 => Origin::Firm,
        2 => Origin::Unknown,
        _ => Origin::Customer,
    };
    order.order_ref = reader.next_string();
    Ok(order)
}

fn decode_order_condition(reader: &mut FieldReader<'_>) -> TwsApiResult<OrderCondition> {
    let condition_type = reader.next_i32_or_default();
    let is_conjunction_connection = parse_condition_connector(&reader.next_string());

    match condition_type {
        1 => Ok(OrderCondition::Price {
            is_conjunction_connection,
            trigger_method: reader.next_i32()?,
            con_id: reader.next_i32()?,
            exchange: reader.next_string(),
            is_more: reader.next_bool(),
            price: reader.next_f64()?,
        }),
        3 => Ok(OrderCondition::Time {
            is_conjunction_connection,
            is_more: reader.next_bool(),
            time: reader.next_string(),
        }),
        4 => Ok(OrderCondition::Margin {
            is_conjunction_connection,
            is_more: reader.next_bool(),
            percent: reader.next_f64()?,
        }),
        5 => Ok(OrderCondition::Execution {
            is_conjunction_connection,
            sec_type: reader.next_string(),
            exchange: reader.next_string(),
            symbol: reader.next_string(),
        }),
        6 => Ok(OrderCondition::Volume {
            is_conjunction_connection,
            con_id: reader.next_i32()?,
            exchange: reader.next_string(),
            is_more: reader.next_bool(),
            volume: reader.next_i32()?,
        }),
        7 => Ok(OrderCondition::PercentChange {
            is_conjunction_connection,
            con_id: reader.next_i32()?,
            exchange: reader.next_string(),
            is_more: reader.next_bool(),
            change_percent: reader.next_f64()?,
        }),
        _ => Ok(OrderCondition::Time {
            is_conjunction_connection,
            is_more: false,
            time: String::new(),
        }),
    }
}

fn decode_order_conditions(reader: &mut FieldReader<'_>) -> TwsApiResult<Vec<OrderCondition>> {
    let count = reader.next_i32_or_default().max(0) as usize;
    let mut conditions = Vec::with_capacity(count);
    for _ in 0..count {
        conditions.push(decode_order_condition(reader)?);
    }
    Ok(conditions)
}

fn decode_order_state_allocations(
    reader: &mut FieldReader<'_>,
) -> TwsApiResult<Vec<crate::types::OrderAllocation>> {
    let count = reader.next_i32_or_default().max(0) as usize;
    let mut allocations = Vec::with_capacity(count);
    for _ in 0..count {
        allocations.push(crate::types::OrderAllocation {
            account: reader.next_string(),
            position: reader.next_decimal()?,
            position_desired: reader.next_decimal()?,
            position_after: reader.next_decimal()?,
            desired_alloc_qty: reader.next_decimal()?,
            allowed_alloc_qty: reader.next_decimal()?,
            is_monetary: reader.next_bool(),
        });
    }
    Ok(allocations)
}

fn parse_condition_connector(value: &str) -> bool {
    matches!(value, "a" | "A" | "1" | "true" | "True")
}

struct FieldReader<'a> {
    fields: &'a [String],
    index: usize,
}

impl<'a> FieldReader<'a> {
    fn new(fields: &'a [String]) -> Self {
        Self { fields, index: 0 }
    }

    fn peek(&self, offset: usize) -> Option<&'a String> {
        self.fields.get(self.index + offset)
    }

    fn next_string(&mut self) -> String {
        let value = self.fields.get(self.index).cloned().unwrap_or_default();
        self.index += 1;
        value
    }

    fn next_i32(&mut self) -> TwsApiResult<i32> {
        let value = parse_i32(self.fields.get(self.index));
        self.index += 1;
        value
    }

    fn next_i32_or_default(&mut self) -> i32 {
        self.next_i32().unwrap_or_default()
    }

    fn next_f64(&mut self) -> TwsApiResult<f64> {
        let value = parse_f64(self.fields.get(self.index));
        self.index += 1;
        value
    }

    fn next_f64_or_default(&mut self) -> f64 {
        self.next_f64().unwrap_or_default()
    }

    fn next_bool(&mut self) -> bool {
        let value = parse_bool(self.fields.get(self.index));
        self.index += 1;
        value
    }

    fn next_decimal(&mut self) -> TwsApiResult<Decimal> {
        let value = parse_decimal(self.fields.get(self.index));
        self.index += 1;
        value
    }

    fn next_decimal_or_default(&mut self) -> Decimal {
        self.next_decimal().unwrap_or_default()
    }
}
