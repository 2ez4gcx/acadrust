use std::collections::HashSet;
use std::io::Cursor;

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// A drawing whose ACAD application id owns the handle that a fresh document
/// gives to its default `AcadAnnotative` entry (older DWGs carry no such entry).
fn drawing_with_acad_on_annotative_handle(version: DxfVersion) -> (CadDocument, u64) {
    let mut document = CadDocument::with_version(version);
    let annotative = document.app_ids.get("AcadAnnotative").unwrap().handle;
    document.app_ids.remove("AcadAnnotative").unwrap();
    for entry in document.app_ids.iter_mut() {
        if entry.name == "ACAD" {
            entry.handle = annotative;
        }
    }
    (document, annotative.value())
}

#[test]
fn dwg_read_keeps_application_id_handles_unique() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1032] {
        let (document, handle) = drawing_with_acad_on_annotative_handle(version);
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

        let ids: Vec<_> = read
            .app_ids
            .iter()
            .map(|a| (a.name.clone(), a.handle.value()))
            .collect();
        let unique: HashSet<u64> = ids.iter().map(|(_, h)| *h).collect();
        assert_eq!(
            unique.len(),
            ids.len(),
            "{version:?}: duplicate APPID handles {ids:?}"
        );
        assert_eq!(
            read.app_ids.get("ACAD").unwrap().handle.value(),
            handle,
            "{version:?}: the source ACAD keeps its handle"
        );
    }
}
