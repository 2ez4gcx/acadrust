//! TABLESTYLE and MLEADERSTYLE must survive legacy DWG writes.
//!
//! Drawings from R2000 through R2013 keep both style dictionaries and both
//! Standard style objects in the named-objects dictionary, with the same
//! handles, so a write to any of those versions has to keep them too. Other
//! records point at those handles, and dropping the styles left the
//! references dangling.

use acadrust::objects::ObjectType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::collections::BTreeMap;

const FIXTURE: &str = "tests/fixtures/dynamic-blocks/point.dwg";
const STYLES: [&str; 2] = ["ACAD_TABLESTYLE", "ACAD_MLEADERSTYLE"];

/// Style counts plus the named-objects entries that reach them.
fn styles(doc: &CadDocument) -> (usize, usize, BTreeMap<String, u64>) {
    let tables = doc
        .objects
        .values()
        .filter(|o| matches!(o, ObjectType::TableStyle(_)))
        .count();
    let leaders = doc
        .objects
        .values()
        .filter(|o| matches!(o, ObjectType::MultiLeaderStyle(_)))
        .count();

    let mut entries = BTreeMap::new();
    let root = doc.header.named_objects_dict_handle;
    if let Some(ObjectType::Dictionary(dictionary)) = doc.objects.get(&root) {
        for (name, handle) in &dictionary.entries {
            if STYLES.contains(&name.as_str()) {
                entries.insert(name.clone(), handle.value());
            }
        }
    }
    (tables, leaders, entries)
}

fn read(path: &str) -> CadDocument {
    DwgReader::from_file(path)
        .and_then(|mut reader| reader.read())
        .expect("read fixture")
}

fn write_at(doc: &CadDocument, version: DxfVersion) -> CadDocument {
    let mut document = doc.clone();
    document.version = version;
    let bytes = DwgWriter::write_to_vec(&document).expect("write");

    let path = std::env::temp_dir().join(format!("iss18_legacy_styles_{version}.dwg"));
    std::fs::write(&path, &bytes).expect("spill written bytes");
    let mut reader = DwgReader::from_file(&path).expect("reopen written bytes");
    let back = reader.read().expect("read back");
    let _ = std::fs::remove_file(&path);
    back
}

#[test]
fn legacy_writes_keep_table_and_multileader_styles() {
    let input = read(FIXTURE);
    let (want_tables, want_leaders, want_entries) = styles(&input);
    assert!(
        want_tables > 0 && want_leaders > 0 && want_entries.len() == STYLES.len(),
        "fixture must carry both styles: tables={want_tables} leaders={want_leaders} entries={want_entries:?}"
    );

    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let back = write_at(&input, version);
        let (tables, leaders, entries) = styles(&back);

        assert_eq!(
            tables, want_tables,
            "{version:?}: TABLESTYLE objects dropped"
        );
        assert_eq!(
            leaders, want_leaders,
            "{version:?}: MLEADERSTYLE objects dropped"
        );
        assert_eq!(
            entries, want_entries,
            "{version:?}: named-objects style entries changed"
        );

        for (name, value) in &want_entries {
            let resolved = back.objects.keys().any(|h| h.value() == *value);
            assert!(
                resolved,
                "{version:?}: {name} entry points at {value:X}, which is not an object"
            );
        }
    }
}
