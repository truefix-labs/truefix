//! T070 (US7, feature 006): shipped `.fixdict` provenance header comments name the current
//! regenerating tool, not the deleted `qfj_xml.rs` (GAP-56).
//!
//! Also covers T026 (US3, feature 011, FR-017): every XML-dictionary *converter module*
//! (`orchestra.rs`/`fix_repository.rs`/`vendor_xml.rs`) documents its own provenance boundary
//! against Constitution Principle III — this is the executable check backing that requirement,
//! which otherwise has no test coverage of its own.

use std::fs;
use std::path::Path;

/// FR-017: every dictionary-format converter module must document, in its own top-of-file doc
/// comment, that it does not copy/translate a third-party FIX engine's bundled private spec
/// files — the same convention `orchestra.rs`/`fix_repository.rs` already established, extended
/// here to cover `vendor_xml.rs` (US3, feature 011).
#[test]
fn every_xml_dictionary_converter_documents_its_provenance_boundary() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for name in ["orchestra.rs", "fix_repository.rs", "vendor_xml.rs"] {
        let content =
            fs::read_to_string(src.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"));
        let doc_header: String = content
            .lines()
            .take_while(|line| line.starts_with("//!"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            doc_header.contains("Principle III"),
            "{name}'s top-of-file doc comment should reference Constitution Principle III \
             (License & Provenance), documenting that it converts an independently-licensed \
             published format rather than copying a third-party engine's bundled spec files"
        );
    }
}

#[test]
fn no_shipped_fixdict_references_the_deleted_qfj_xml_module() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dict-src/normalized");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("read dict-src/normalized") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("fixdict") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read fixdict");
        assert!(
            !content.contains("QuickFIX-XML conversion tool"),
            "{path:?} still names the deleted qfj_xml.rs-era tool in its provenance header"
        );
        checked += 1;
    }
    assert!(
        checked >= 9,
        "expected to check at least 9 shipped .fixdict files, got {checked}"
    );
}
