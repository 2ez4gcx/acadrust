//! The header's dimension arrow blocks (DIMBLK, DIMBLK1, DIMBLK2, DIMLDRBLK), arc length
//! symbol (DIMARCSYM) and fixed extension line switch (DIMFXLON) survive a DWG round trip.
//! The reader skipped them and the writer wrote null handles and defaults in their place.

use std::io::Cursor;

use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

const VERSIONS: [DxfVersion; 6] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// An empty block definition, as an arrow block is referenced from the header.
fn arrow_block(doc: &mut CadDocument, name: &str) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let handle = record.handle;
    doc.block_records.add(record).unwrap();
    handle
}

fn block_name(doc: &CadDocument, handle: Handle) -> Option<String> {
    doc.block_records
        .iter()
        .find(|record| record.handle == handle)
        .map(|record| record.name.clone())
}

fn round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn dimension_arrow_blocks_survive_a_dwg_round_trip() {
    for version in VERSIONS {
        let mut doc = CadDocument::with_version(version);
        doc.header.dim_arrow_block_handle = arrow_block(&mut doc, "_ARCHTICK");
        doc.header.dim_arrow_block1_handle = arrow_block(&mut doc, "_DOT");
        doc.header.dim_arrow_block2_handle = arrow_block(&mut doc, "_OPEN");
        doc.header.dim_leader_arrow_block_handle = arrow_block(&mut doc, "_SMALL");
        doc.header.dim_arc_symbol = 1;
        doc.header.dim_fixed_ext_line_on = true;

        let back = round_trip(&doc);
        let h = &back.header;
        let names = [
            block_name(&back, h.dim_arrow_block_handle),
            block_name(&back, h.dim_arrow_block1_handle),
            block_name(&back, h.dim_arrow_block2_handle),
            block_name(&back, h.dim_leader_arrow_block_handle),
        ];
        let expected = ["_ARCHTICK", "_DOT", "_OPEN", "_SMALL"].map(|n| Some(n.to_string()));
        assert_eq!(names, expected, "{version:?}");

        // DIMARCSYM and DIMFXLON are in the header from R2007 on.
        if version >= DxfVersion::AC1021 {
            assert_eq!(h.dim_arc_symbol, 1, "{version:?}");
            assert!(h.dim_fixed_ext_line_on, "{version:?}");
        }
    }
}

#[test]
fn an_arrow_handle_naming_no_block_is_written_null() {
    for version in VERSIONS {
        let mut doc = CadDocument::with_version(version);
        doc.header.dim_arrow_block_handle = Handle::new(0x7FFF_FFF0);
        doc.header.dim_leader_arrow_block_handle = arrow_block(&mut doc, "_DOT");

        let back = round_trip(&doc);
        assert!(back.header.dim_arrow_block_handle.is_null(), "{version:?}");
        assert_eq!(
            block_name(&back, back.header.dim_leader_arrow_block_handle).as_deref(),
            Some("_DOT"),
            "{version:?}"
        );
    }
}
