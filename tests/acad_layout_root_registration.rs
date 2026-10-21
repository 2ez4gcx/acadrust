//! Regression: the root dictionary's `ACAD_LAYOUT` registration must be
//! restored when a pre-R2000 DWG is loaded (PaySwap finding F1).
//!
//! The R13/R14 DWG writer intentionally strips the root dictionary's
//! `ACAD_LAYOUT` entry (that named-object-dictionary reference does not
//! exist in the R14 format) while the layout dictionary itself survives as
//! a regular object owned by the root. The reader must invert the strip
//! when building the canonical document model; otherwise every later save —
//! DXF or DWG, any version — serializes the layout dictionary as an orphan
//! of the root, and strict handle-graph verification rejects the output.
//!
//! These tests build the document in memory (no fixture files): a fresh
//! document with a second layout, exactly the shape of the vertical-slice
//! drawing that exposed the finding.

use acadrust::objects::ObjectType;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter};
use std::io::Cursor;

/// A fresh document with Model + Layout1 plus a session-created layout
/// ("SliceSheet"), the vertical-slice shape.
fn document_with_extra_layout() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc.add_layout("SliceSheet").expect("add_layout");
    doc
}

fn root_dict<'a>(doc: &'a CadDocument) -> &'a acadrust::objects::Dictionary {
    match doc.objects.get(&doc.header.named_objects_dict_handle) {
        Some(ObjectType::Dictionary(dict)) => dict,
        _ => panic!("root named-objects dictionary missing"),
    }
}

fn root_acad_layout_entry(doc: &CadDocument) -> Option<acadrust::Handle> {
    root_dict(doc).get("ACAD_LAYOUT")
}

fn layout_dictionary_handle(doc: &CadDocument) -> acadrust::Handle {
    // The dictionary that owns LAYOUT objects is the ACAD_LAYOUT dictionary.
    let mut candidates: Vec<acadrust::Handle> = doc
        .objects
        .iter()
        .filter_map(|(_handle, object)| match object {
            ObjectType::Layout(layout) if !layout.owner.is_null() => Some(layout.owner),
            _ => None,
        })
        .filter(|owner| matches!(doc.objects.get(owner), Some(ObjectType::Dictionary(_))))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    assert_eq!(
        candidates.len(),
        1,
        "exactly one layout dictionary expected"
    );
    candidates[0]
}

fn dwg_roundtrip(doc: &CadDocument, version: DxfVersion) -> CadDocument {
    let mut copy = doc.clone();
    copy.version = version;
    let bytes = DwgWriter::write_to_vec(&copy).expect("write dwg");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("parse dwg")
}

fn dxf_roundtrip(doc: &CadDocument, version: DxfVersion) -> CadDocument {
    let mut copy = doc.clone();
    copy.version = version;
    let bytes = DxfWriter::new(&copy)
        .with_version(version)
        .write_to_vec()
        .expect("write dxf");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("parse dxf")
}

fn assert_canonical_layout_registration(doc: &CadDocument, context: &str) {
    let layout_dict = layout_dictionary_handle(doc);
    let entry = root_acad_layout_entry(doc)
        .unwrap_or_else(|| panic!("{context}: root dictionary lost its ACAD_LAYOUT entry"));
    assert_eq!(
        entry, layout_dict,
        "{context}: root ACAD_LAYOUT entry must point at the layout dictionary"
    );
    assert_eq!(
        doc.header.acad_layout_dict_handle, layout_dict,
        "{context}: header.acad_layout_dict_handle must resolve to the layout dictionary"
    );
    match doc.objects.get(&layout_dict) {
        Some(ObjectType::Dictionary(dict)) => {
            assert_eq!(
                dict.owner, doc.header.named_objects_dict_handle,
                "{context}: layout dictionary must be owned by the root"
            );
            for name in ["Model", "SliceSheet"] {
                assert!(
                    dict.entries.iter().any(|(key, _)| key == name),
                    "{context}: layout dictionary must still list {name}"
                );
            }
        }
        _ => panic!("{context}: layout dictionary vanished"),
    }
}

/// F1 core: writing a pre-R2000 DWG strips the root entry by design; reading
/// it back must restore the canonical registration.
#[test]
fn r14_roundtrip_restores_root_registration() {
    let doc = document_with_extra_layout();
    assert_canonical_layout_registration(&doc, "fresh document");

    let reloaded = dwg_roundtrip(&doc, DxfVersion::AC1014);
    assert_canonical_layout_registration(&reloaded, "after R14 write->read");
}

/// The upgrade path: R14 -> R2018 must not propagate the strip.
#[test]
fn r14_to_r2018_upgrade_restores_registration() {
    let doc = document_with_extra_layout();
    let r14 = dwg_roundtrip(&doc, DxfVersion::AC1014);
    let r2018 = dwg_roundtrip(&r14, DxfVersion::AC1032);
    assert_canonical_layout_registration(&r2018, "after R14 -> R2018 upgrade");
}

/// The original finding's shape: DWG-reopened document saved as DXF must
/// produce a DXF whose layout dictionary is registered in the root (the
/// orphan that `save_verified` rejected).
#[test]
fn r14_reopen_dxf_save_keeps_layout_dictionary_reachable() {
    let doc = document_with_extra_layout();
    let r14 = dwg_roundtrip(&doc, DxfVersion::AC1014);

    let reloaded = dxf_roundtrip(&r14, DxfVersion::AC1032);
    assert_canonical_layout_registration(&reloaded, "after R14 -> DXF save -> read");
}

/// Repeated mixed-version cycles stay stable (no drift, no duplicate
/// entries, no handle churn).
#[test]
fn repeated_mixed_cycles_are_stable() {
    let mut doc = document_with_extra_layout();
    for cycle in 0..5 {
        doc = dwg_roundtrip(&doc, DxfVersion::AC1014);
        assert_canonical_layout_registration(&doc, &format!("cycle {cycle}: after R14"));
        doc = dwg_roundtrip(&doc, DxfVersion::AC1032);
        assert_canonical_layout_registration(&doc, &format!("cycle {cycle}: after R2018"));
        doc = dxf_roundtrip(&doc, DxfVersion::AC1032);
        assert_canonical_layout_registration(&doc, &format!("cycle {cycle}: after DXF"));
    }
    // No duplicate ACAD_LAYOUT entries accumulated.
    let count = root_dict(&doc)
        .entries
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("ACAD_LAYOUT"))
        .count();
    assert_eq!(count, 1, "exactly one ACAD_LAYOUT root entry");
}

/// R2000+ files are unaffected: the entry they already carry is preserved
/// (and the header pointer is kept in sync with it).
#[test]
fn r2018_roundtrip_preserves_existing_registration() {
    let doc = document_with_extra_layout();
    let reloaded = dwg_roundtrip(&doc, DxfVersion::AC1032);
    assert_canonical_layout_registration(&reloaded, "after R2018 write->read");
    let again = dxf_roundtrip(&reloaded, DxfVersion::AC1032);
    assert_canonical_layout_registration(&again, "after DXF write->read");
}
