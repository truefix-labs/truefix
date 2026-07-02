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

pub mod codegen;
pub mod fixt;
mod hash;
pub mod model;
#[cfg(feature = "dict-tooling")]
pub mod orchestra;
pub mod parser;
mod validate;

pub use codegen::CodegenError;
pub use fixt::FixtDictionaries;
pub use model::{
    ComponentDef, DataDictionary, DictMergeConflict, FieldDef, FieldType, GroupDef, MessageDef,
    RejectReason, ValidationError, ValidationOptions,
};
pub use parser::{parse, ParseError};

/// An error loading a dictionary file from disk (FR-010).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DictLoadError {
    /// The file could not be read.
    #[error("could not read dictionary file {path}: {reason}")]
    Io {
        /// The path that failed to read.
        path: String,
        /// The underlying I/O error, stringified.
        reason: String,
    },
    /// The file's contents failed to parse.
    #[error("could not parse dictionary file {path}: {source}")]
    Parse {
        /// The path whose contents failed to parse.
        path: String,
        /// The underlying parse error.
        #[source]
        source: ParseError,
    },
}

/// Load and parse a dictionary from a file on disk (FR-010; a runtime, custom-dictionary
/// counterpart to the bundled `load_fix*` loaders).
pub fn load_from_file(path: impl AsRef<std::path::Path>) -> Result<DataDictionary, DictLoadError> {
    let path_ref = path.as_ref();
    let text = std::fs::read_to_string(path_ref).map_err(|e| DictLoadError::Io {
        path: path_ref.display().to_string(),
        reason: e.to_string(),
    })?;
    parse(&text).map_err(|source| DictLoadError::Parse {
        path: path_ref.display().to_string(),
        source,
    })
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// Bundled normalized dictionary sources (the single source of truth for both tracks).
pub const FIX40_DICT: &str = include_str!("../dict-src/normalized/FIX40.fixdict");
/// FIX 4.1 dictionary source.
pub const FIX41_DICT: &str = include_str!("../dict-src/normalized/FIX41.fixdict");
/// FIX 4.2 dictionary source.
pub const FIX42_DICT: &str = include_str!("../dict-src/normalized/FIX42.fixdict");
/// FIX 4.3 dictionary source.
pub const FIX43_DICT: &str = include_str!("../dict-src/normalized/FIX43.fixdict");
/// FIX 4.4 dictionary source.
pub const FIX44_DICT: &str = include_str!("../dict-src/normalized/FIX44.fixdict");
/// FIXT 1.1 transport (session) dictionary source.
pub const FIXT11_DICT: &str = include_str!("../dict-src/normalized/FIXT11.fixdict");
/// FIX 5.0 application dictionary source.
pub const FIX50_DICT: &str = include_str!("../dict-src/normalized/FIX50.fixdict");
/// FIX 5.0 SP1 application dictionary source.
pub const FIX50SP1_DICT: &str = include_str!("../dict-src/normalized/FIX50SP1.fixdict");
/// FIX 5.0 SP2 application dictionary source.
pub const FIX50SP2_DICT: &str = include_str!("../dict-src/normalized/FIX50SP2.fixdict");
/// FIX Latest dictionary source (US9; sourced from FIX Orchestra XML via the `orchestra`
/// conversion tool, `--features dict-tooling`).
pub const FIXLATEST_DICT: &str = include_str!("../dict-src/normalized/FIXLATEST.fixdict");

/// Parse the bundled FIX 4.0 dictionary.
pub fn load_fix40() -> Result<DataDictionary, ParseError> {
    parse(FIX40_DICT)
}

/// Parse the bundled FIX 4.1 dictionary.
pub fn load_fix41() -> Result<DataDictionary, ParseError> {
    parse(FIX41_DICT)
}

/// Parse the bundled FIX 4.2 dictionary.
pub fn load_fix42() -> Result<DataDictionary, ParseError> {
    parse(FIX42_DICT)
}

/// Parse the bundled FIX 4.3 dictionary.
pub fn load_fix43() -> Result<DataDictionary, ParseError> {
    parse(FIX43_DICT)
}

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

/// Parse the bundled FIX 5.0 SP1 application dictionary.
pub fn load_fix50sp1() -> Result<DataDictionary, ParseError> {
    parse(FIX50SP1_DICT)
}

/// Parse the bundled FIX 5.0 SP2 application dictionary.
pub fn load_fix50sp2() -> Result<DataDictionary, ParseError> {
    parse(FIX50SP2_DICT)
}

/// Parse the bundled FIX Latest dictionary (US9, FR-012).
pub fn load_fixlatest() -> Result<DataDictionary, ParseError> {
    parse(FIXLATEST_DICT)
}

/// All bundled dictionaries as `(version, source)` pairs, for iteration.
pub const ALL_DICTS: &[(&str, &str)] = &[
    ("FIX.4.0", FIX40_DICT),
    ("FIX.4.1", FIX41_DICT),
    ("FIX.4.2", FIX42_DICT),
    ("FIX.4.3", FIX43_DICT),
    ("FIX.4.4", FIX44_DICT),
    ("FIXT.1.1", FIXT11_DICT),
    ("FIX.5.0", FIX50_DICT),
    ("FIX.5.0SP1", FIX50SP1_DICT),
    ("FIX.5.0SP2", FIX50SP2_DICT),
    ("FIX.Latest", FIXLATEST_DICT),
];
