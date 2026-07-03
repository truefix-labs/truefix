//! T070 (US7, feature 006): shipped `.fixdict` provenance header comments name the current
//! regenerating tool, not the deleted `qfj_xml.rs` (GAP-56).

use std::fs;
use std::path::Path;

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
