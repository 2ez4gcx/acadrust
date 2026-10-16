//! Table entries added to a document after it was built or read
//! (`Layer::new` + `layers.add`, `AppId::new` + `app_ids.add`) carry NULL
//! handles. The DWG writer assigns them on its output copy; the DXF writer
//! wrote them as they were: one `5 / 0` record per entry, duplicates of one
//! another, and because `extension_dictionary_handle(NULL)` resolves to the
//! root dictionary (whose owner is NULL), every one of them also claimed the
//! root dictionary as its `{ACAD_XDICTIONARY`. Strict readers reject such a
//! file; a Leica field controller refuses to list its layers.

use acadrust::entities::{EntityType, Line};
use acadrust::tables::{AppId, Layer};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn read_dxf(bytes: &[u8]) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes.to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

/// Record handles outside the HEADER, the extension dictionaries named in
/// the TABLES section, and the root dictionary (a DICTIONARY owned by 0).
/// Walked in code/value pairs, so a value line is never read as a code.
fn scan(dxf: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let lines: Vec<&str> = dxf.split("\r\n").collect();
    let (mut handles, mut xdicts, mut roots) = (Vec::new(), Vec::new(), Vec::new());
    let (mut section, mut record, mut handle) = (String::new(), String::new(), String::new());
    for i in (0..lines.len().saturating_sub(1)).step_by(2) {
        let (code, value) = (lines[i].trim(), lines[i + 1].trim());
        match code {
            "0" => record = value.to_string(),
            "2" if record == "SECTION" => section = value.to_string(),
            "5" | "105" if section != "HEADER" => {
                handle = value.to_string();
                handles.push(handle.clone());
            }
            "360" if section == "TABLES" => xdicts.push(value.to_string()),
            "330" if section == "OBJECTS" && record == "DICTIONARY" && value == "0" => {
                roots.push(handle.clone())
            }
            _ => {}
        }
    }
    (handles, xdicts, roots)
}

#[test]
fn table_entries_added_after_read_get_handles_in_dxf() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1032,
    ] {
        // A drawing as read from a file: every record has a handle.
        let mut source = CadDocument::with_version(version);
        source
            .add_entity(EntityType::Line(Line::from_coords(
                0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
            )))
            .unwrap();
        let mut doc = read_dxf(&DxfWriter::new(&source).write_to_vec().unwrap());

        // An application then registers its own app ids and adds a layer.
        doc.app_ids.add(AppId::new("MY_APP")).unwrap();
        doc.app_ids.add(AppId::new("MY_OTHER_APP")).unwrap();
        doc.layers.add(Layer::new("MY_LAYER")).unwrap();

        let output = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
        let (handles, xdicts, roots) = scan(&output);

        assert!(
            !handles.iter().any(|h| h == "0"),
            "{version:?}: a record was written with handle 0"
        );
        let mut unique = handles.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            handles.len(),
            "{version:?}: duplicate record handles"
        );
        assert!(!roots.is_empty(), "{version:?}: no root dictionary found");
        assert!(
            !xdicts.iter().any(|x| roots.contains(x)),
            "{version:?}: a table entry claims the root dictionary as its extension dictionary"
        );

        // The entries survive with real handles; the caller's document keeps
        // its NULL handles (only the output copy is repaired).
        let loaded = read_dxf(output.as_bytes());
        for name in ["MY_APP", "MY_OTHER_APP"] {
            assert!(
                !loaded.app_ids.get(name).unwrap().handle.is_null(),
                "{version:?}: {name} has no handle after reading back"
            );
            assert!(doc.app_ids.get(name).unwrap().handle.is_null());
        }
        assert!(
            !loaded.layers.get("MY_LAYER").unwrap().handle.is_null(),
            "{version:?}: MY_LAYER has no handle after reading back"
        );
        assert!(doc.layers.get("MY_LAYER").unwrap().handle.is_null());
    }
}
