/// TWS API's sentinel for "no valid request/order id".
pub const NO_VALID_ID: i32 = -1;

/// Maximum message length used by the official client: 16 MiB minus one byte.
pub const MAX_MSG_LEN: usize = 0xFF_FFFF;

/// TWS API integer unset sentinel.
pub const UNSET_INTEGER: i32 = i32::MAX;

/// TWS API double unset sentinel.
pub const UNSET_DOUBLE: f64 = f64::MAX;

/// TWS API long unset sentinel.
pub const UNSET_LONG: i64 = i64::MAX;

/// String representation used by the official client for positive infinity.
pub const INFINITY_STR: &str = "Infinity";
