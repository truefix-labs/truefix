/// Supported OKX instrument product classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentType {
    Spot,
    Margin,
    Swap,
    Futures,
    Option,
    Any,
}
