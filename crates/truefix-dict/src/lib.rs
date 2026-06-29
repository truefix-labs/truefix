//! `truefix-dict` — the dual-track FIX data dictionary.
//!
//! One normalized dictionary source feeds **both** tracks (Constitution Principle IV):
//! - the runtime [`DataDictionary`] (parsed at runtime, used by [`DataDictionary::validate`]);
//! - build-time **codegen** (`build.rs`) of per-version MsgType constants + a content hash.
//!
//! The generated `*_DICT_HASH` constants equal the runtime-parsed [`DataDictionary::hash`], which
//! proves the two tracks derive from the same source (see the `dual_track` test).
//!
//! Dictionaries are derived from the FIX Trading Community specifications; QuickFIX/J XML files are
//! not copied (Constitution Principle III).
//!
//! Design: `specs/001-fix-engine-parity/`.
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod fixt;
mod hash;
pub mod model;
pub mod parser;
mod validate;

pub use fixt::FixtDictionaries;
pub use model::{
    DataDictionary, FieldDef, FieldType, MessageDef, RejectReason, ValidationError,
    ValidationOptions,
};
pub use parser::{parse, ParseError};

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// Bundled normalized dictionary sources (the single source of truth for both tracks).
pub const FIX44_DICT: &str = include_str!("../dict-src/normalized/FIX44.fixdict");
/// FIXT 1.1 transport (session) dictionary source.
pub const FIXT11_DICT: &str = include_str!("../dict-src/normalized/FIXT11.fixdict");
/// FIX 5.0 application dictionary source.
pub const FIX50_DICT: &str = include_str!("../dict-src/normalized/FIX50.fixdict");

/// Parse the bundled FIX 4.4 dictionary.
pub fn load_fix44() -> Result<DataDictionary, ParseError> {
    parse(FIX44_DICT)
}

/// Parse the bundled FIXT 1.1 transport dictionary.
pub fn load_fixt11() -> Result<DataDictionary, ParseError> {
    parse(FIXT11_DICT)
}

/// Parse the bundled FIX 5.0 application dictionary.
pub fn load_fix50() -> Result<DataDictionary, ParseError> {
    parse(FIX50_DICT)
}
