use super::*;

pub(super) fn proto_order_to_order(order: Option<protobuf::Order>) -> TwsApiResult<Order> {
    let Some(order) = order else {
        return Ok(Order::default());
    };

    Ok(Order {
        soft_dollar_tier: proto_soft_dollar_tier(order.soft_dollar_tier),
        order_id: order.order_id.unwrap_or_default(),
        client_id: order.client_id.unwrap_or_default(),
        perm_id: order.perm_id.unwrap_or_default(),
        action: order.action.unwrap_or_default(),
        total_quantity: parse_decimal_string(order.total_quantity.as_deref())?,
        order_type: order.order_type.unwrap_or_default(),
        limit_price: order.lmt_price.unwrap_or_default(),
        aux_price: order.aux_price.unwrap_or_default(),
        tif: order.tif.unwrap_or_default(),
        active_start_time: order.active_start_time.unwrap_or_default(),
        active_stop_time: order.active_stop_time.unwrap_or_default(),
        oca_group: order.oca_group.unwrap_or_default(),
        oca_type: order.oca_type.unwrap_or_default(),
        order_ref: order.order_ref.unwrap_or_default(),
        transmit: order.transmit.unwrap_or_default(),
        parent_id: order.parent_id.unwrap_or_default(),
        block_order: order.block_order.unwrap_or_default(),
        sweep_to_fill: order.sweep_to_fill.unwrap_or_default(),
        display_size: order.display_size.unwrap_or_default(),
        trigger_method: order.trigger_method.unwrap_or_default(),
        outside_rth: order.outside_rth.unwrap_or_default(),
        hidden: order.hidden.unwrap_or_default(),
        good_after_time: order.good_after_time.unwrap_or_default(),
        good_till_date: order.good_till_date.unwrap_or_default(),
        rule80a: order.rule80_a.unwrap_or_default(),
        all_or_none: order.all_or_none.unwrap_or_default(),
        min_qty: order.min_qty.unwrap_or_default(),
        percent_offset: order.percent_offset.unwrap_or_default(),
        override_percentage_constraints: order.override_percentage_constraints.unwrap_or_default(),
        trail_stop_price: order.trail_stop_price.unwrap_or_default(),
        trailing_percent: order.trailing_percent.unwrap_or_default(),
        fa_group: order.fa_group.unwrap_or_default(),
        fa_method: order.fa_method.unwrap_or_default(),
        fa_percentage: order.fa_percentage.unwrap_or_default(),
        designated_location: order.designated_location.unwrap_or_default(),
        open_close: order.open_close.unwrap_or_default(),
        origin: match order.origin.unwrap_or_default() {
            1 => Origin::Firm,
            2 => Origin::Unknown,
            _ => Origin::Customer,
        },
        short_sale_slot: order.short_sale_slot.unwrap_or_default(),
        exempt_code: order.exempt_code.unwrap_or_default(),
        discretionary_amount: order.discretionary_amt.unwrap_or_default(),
        opt_out_smart_routing: order.opt_out_smart_routing.unwrap_or_default(),
        starting_price: order.starting_price.unwrap_or_default(),
        stock_ref_price: order.stock_ref_price.unwrap_or_default(),
        delta: order.delta.unwrap_or_default(),
        stock_range_lower: order.stock_range_lower.unwrap_or_default(),
        stock_range_upper: order.stock_range_upper.unwrap_or_default(),
        randomize_price: order.randomize_price.unwrap_or_default(),
        randomize_size: order.randomize_size.unwrap_or_default(),
        volatility: order.volatility.unwrap_or_default(),
        volatility_type: order.volatility_type.unwrap_or_default(),
        delta_neutral_order_type: order.delta_neutral_order_type.unwrap_or_default(),
        delta_neutral_aux_price: order.delta_neutral_aux_price.unwrap_or_default(),
        delta_neutral_con_id: order.delta_neutral_con_id.unwrap_or_default(),
        delta_neutral_settling_firm: order.delta_neutral_settling_firm.unwrap_or_default(),
        delta_neutral_clearing_account: order.delta_neutral_clearing_account.unwrap_or_default(),
        delta_neutral_clearing_intent: order.delta_neutral_clearing_intent.unwrap_or_default(),
        delta_neutral_open_close: order.delta_neutral_open_close.unwrap_or_default(),
        delta_neutral_short_sale: order.delta_neutral_short_sale.unwrap_or_default(),
        delta_neutral_short_sale_slot: order.delta_neutral_short_sale_slot.unwrap_or_default(),
        delta_neutral_designated_location: order
            .delta_neutral_designated_location
            .unwrap_or_default(),
        continuous_update: order.continuous_update.unwrap_or_default(),
        reference_price_type: order.reference_price_type.unwrap_or_default(),
        scale_init_level_size: order.scale_init_level_size.unwrap_or_default(),
        scale_subs_level_size: order.scale_subs_level_size.unwrap_or_default(),
        scale_price_increment: order.scale_price_increment.unwrap_or_default(),
        scale_price_adjust_value: order.scale_price_adjust_value.unwrap_or_default(),
        scale_price_adjust_interval: order.scale_price_adjust_interval.unwrap_or_default(),
        scale_profit_offset: order.scale_profit_offset.unwrap_or_default(),
        scale_auto_reset: order.scale_auto_reset.unwrap_or_default(),
        scale_init_position: order.scale_init_position.unwrap_or_default(),
        scale_init_fill_qty: order.scale_init_fill_qty.unwrap_or_default(),
        scale_random_percent: order.scale_random_percent.unwrap_or_default(),
        scale_table: order.scale_table.unwrap_or_default(),
        hedge_type: order.hedge_type.unwrap_or_default(),
        hedge_param: order.hedge_param.unwrap_or_default(),
        hedge_max_size: order.hedge_max_size.unwrap_or_default(),
        account: order.account.unwrap_or_default(),
        settling_firm: order.settling_firm.unwrap_or_default(),
        clearing_account: order.clearing_account.unwrap_or_default(),
        clearing_intent: order.clearing_intent.unwrap_or_default(),
        model_code: order.model_code.unwrap_or_default(),
        algo_strategy: order.algo_strategy.unwrap_or_default(),
        algo_params: proto_tag_values(order.algo_params),
        smart_combo_routing_params: proto_tag_values(order.smart_combo_routing_params),
        algo_id: order.algo_id.unwrap_or_default(),
        what_if: order.what_if.unwrap_or_default(),
        not_held: order.not_held.unwrap_or_default(),
        solicited: order.solicited.unwrap_or_default(),
        order_misc_options: proto_tag_values(order.order_misc_options),
        reference_contract_id: order.reference_contract_id.unwrap_or_default(),
        pegged_change_amount: order.pegged_change_amount.unwrap_or_default(),
        is_pegged_change_amount_decrease: order
            .is_pegged_change_amount_decrease
            .unwrap_or_default(),
        reference_change_amount: order.reference_change_amount.unwrap_or_default(),
        reference_exchange_id: order.reference_exchange_id.unwrap_or_default(),
        adjusted_order_type: order.adjusted_order_type.unwrap_or_default(),
        trigger_price: order.trigger_price.unwrap_or_default(),
        adjusted_stop_price: order.adjusted_stop_price.unwrap_or_default(),
        adjusted_stop_limit_price: order.adjusted_stop_limit_price.unwrap_or_default(),
        adjusted_trailing_amount: order.adjusted_trailing_amount.unwrap_or_default(),
        adjustable_trailing_unit: order.adjustable_trailing_unit.unwrap_or_default(),
        limit_price_offset: order.lmt_price_offset.unwrap_or_default(),
        conditions_cancel_order: order.conditions_cancel_order.unwrap_or_default(),
        conditions_ignore_rth: order.conditions_ignore_rth.unwrap_or_default(),
        ext_operator: order.ext_operator.unwrap_or_default(),
        cash_qty: order.cash_qty.unwrap_or_default(),
        mifid2_decision_maker: order.mifid2_decision_maker.unwrap_or_default(),
        mifid2_decision_algo: order.mifid2_decision_algo.unwrap_or_default(),
        mifid2_execution_trader: order.mifid2_execution_trader.unwrap_or_default(),
        mifid2_execution_algo: order.mifid2_execution_algo.unwrap_or_default(),
        dont_use_auto_price_for_hedge: order.dont_use_auto_price_for_hedge.unwrap_or_default(),
        is_oms_container: order.is_oms_container.unwrap_or_default(),
        discretionary_up_to_limit_price: order.discretionary_up_to_limit_price.unwrap_or_default(),
        auto_cancel_date: order.auto_cancel_date.unwrap_or_default(),
        filled_quantity: parse_decimal_string(order.filled_quantity.as_deref())?,
        ref_futures_con_id: order.ref_futures_con_id.unwrap_or_default(),
        auto_cancel_parent: order.auto_cancel_parent.unwrap_or_default(),
        shareholder: order.shareholder.unwrap_or_default(),
        imbalance_only: order.imbalance_only.unwrap_or_default(),
        route_marketable_to_bbo: order.route_marketable_to_bbo.unwrap_or_default(),
        parent_perm_id: order.parent_perm_id.unwrap_or_default(),
        use_price_mgmt_algo: order.use_price_mgmt_algo.unwrap_or_default(),
        duration: order.duration.unwrap_or_default(),
        post_to_ats: order.post_to_ats.unwrap_or_default(),
        advanced_error_override: order.advanced_error_override.unwrap_or_default(),
        manual_order_time: order.manual_order_time.unwrap_or_default(),
        min_trade_qty: order.min_trade_qty.unwrap_or_default(),
        min_compete_size: order.min_compete_size.unwrap_or_default(),
        compete_against_best_offset: order.compete_against_best_offset.unwrap_or_default(),
        mid_offset_at_whole: order.mid_offset_at_whole.unwrap_or_default(),
        mid_offset_at_half: order.mid_offset_at_half.unwrap_or_default(),
        customer_account: order.customer_account.unwrap_or_default(),
        professional_customer: order.professional_customer.unwrap_or_default(),
        bond_accrued_interest: order.bond_accrued_interest.unwrap_or_default(),
        include_overnight: order.include_overnight.unwrap_or_default(),
        manual_order_indicator: order.manual_order_indicator.unwrap_or_default(),
        submitter: order.submitter.unwrap_or_default(),
        post_only: order.post_only.unwrap_or_default(),
        allow_pre_open: order.allow_pre_open.unwrap_or_default(),
        ignore_open_auction: order.ignore_open_auction.unwrap_or_default(),
        deactivate: order.deactivate.unwrap_or_default(),
        seek_price_improvement: order.seek_price_improvement.unwrap_or_default(),
        what_if_type: order.what_if_type.unwrap_or_default(),
        ..Order::default()
    })
}

pub(super) fn proto_order_state_to_order_state(
    order_state: Option<protobuf::OrderState>,
) -> OrderState {
    let Some(order_state) = order_state else {
        return OrderState::default();
    };

    OrderState {
        status: order_state.status.unwrap_or_default(),
        init_margin_before: order_state.init_margin_before.unwrap_or_default(),
        maint_margin_before: order_state.maint_margin_before.unwrap_or_default(),
        equity_with_loan_before: order_state.equity_with_loan_before.unwrap_or_default(),
        init_margin_change: order_state.init_margin_change.unwrap_or_default(),
        maint_margin_change: order_state.maint_margin_change.unwrap_or_default(),
        equity_with_loan_change: order_state.equity_with_loan_change.unwrap_or_default(),
        init_margin_after: order_state.init_margin_after.unwrap_or_default(),
        maint_margin_after: order_state.maint_margin_after.unwrap_or_default(),
        equity_with_loan_after: order_state.equity_with_loan_after.unwrap_or_default(),
        commission_and_fees: order_state.commission_and_fees.unwrap_or_default(),
        min_commission_and_fees: order_state.min_commission_and_fees.unwrap_or_default(),
        max_commission_and_fees: order_state.max_commission_and_fees.unwrap_or_default(),
        commission_and_fees_currency: order_state.commission_and_fees_currency.unwrap_or_default(),
        margin_currency: order_state.margin_currency.unwrap_or_default(),
        init_margin_before_outside_rth: order_state
            .init_margin_before_outside_rth
            .unwrap_or_default(),
        maint_margin_before_outside_rth: order_state
            .maint_margin_before_outside_rth
            .unwrap_or_default(),
        equity_with_loan_before_outside_rth: order_state
            .equity_with_loan_before_outside_rth
            .unwrap_or_default(),
        init_margin_change_outside_rth: order_state
            .init_margin_change_outside_rth
            .unwrap_or_default(),
        maint_margin_change_outside_rth: order_state
            .maint_margin_change_outside_rth
            .unwrap_or_default(),
        equity_with_loan_change_outside_rth: order_state
            .equity_with_loan_change_outside_rth
            .unwrap_or_default(),
        init_margin_after_outside_rth: order_state
            .init_margin_after_outside_rth
            .unwrap_or_default(),
        maint_margin_after_outside_rth: order_state
            .maint_margin_after_outside_rth
            .unwrap_or_default(),
        equity_with_loan_after_outside_rth: order_state
            .equity_with_loan_after_outside_rth
            .unwrap_or_default(),
        suggested_size: order_state.suggested_size.unwrap_or_default(),
        reject_reason: order_state.reject_reason.unwrap_or_default(),
        order_allocations: order_state
            .order_allocations
            .into_iter()
            .map(proto_order_allocation_to_order_allocation)
            .collect::<TwsApiResult<Vec<_>>>()
            .unwrap_or_default(),
        warning_text: order_state.warning_text.unwrap_or_default(),
        completed_time: order_state.completed_time.unwrap_or_default(),
        completed_status: order_state.completed_status.unwrap_or_default(),
    }
}

fn proto_order_allocation_to_order_allocation(
    allocation: protobuf::OrderAllocation,
) -> TwsApiResult<OrderAllocation> {
    Ok(OrderAllocation {
        account: allocation.account.unwrap_or_default(),
        position: parse_decimal_string(allocation.position.as_deref())?,
        position_desired: parse_decimal_string(allocation.position_desired.as_deref())?,
        position_after: parse_decimal_string(allocation.position_after.as_deref())?,
        desired_alloc_qty: parse_decimal_string(allocation.desired_alloc_qty.as_deref())?,
        allowed_alloc_qty: parse_decimal_string(allocation.allowed_alloc_qty.as_deref())?,
        is_monetary: allocation.is_monetary.unwrap_or_default(),
    })
}

fn proto_soft_dollar_tier(tier: Option<protobuf::SoftDollarTier>) -> SoftDollarTier {
    let Some(tier) = tier else {
        return SoftDollarTier::default();
    };

    SoftDollarTier {
        name: tier.name.unwrap_or_default(),
        value: tier.value.unwrap_or_default(),
        display_name: tier.display_name.unwrap_or_default(),
    }
}

fn proto_tag_values(values: std::collections::HashMap<String, String>) -> Vec<TagValue> {
    values
        .into_iter()
        .map(|(tag, value)| TagValue { tag, value })
        .collect()
}
