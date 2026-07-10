use rust_decimal::Decimal;

#[test]
fn decimal_wire_values_preserve_precision() {
    let value = "0.00000001".parse::<Decimal>().unwrap();
    assert_eq!(value.to_string(), "0.00000001");
}

#[test]
fn spot_margin_swap_and_option_values_round_trip_without_float_conversion() {
    for wire in [
        "0.00000001",
        "123456789.12345678",
        "0.125",
        "99999.99999999",
    ] {
        let value = wire.parse::<Decimal>().unwrap();
        assert_eq!(value.to_string(), wire);
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: Decimal = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.to_string(), wire);
    }
}
