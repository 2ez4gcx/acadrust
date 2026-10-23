//! `$DIMBLK`, `$DIMBLK1`, `$DIMBLK2`, `$DIMLDRBLK`, `$DIMARCSYM` and `$DIMFXLON`
//! survive DXF, and a DWG whose header holds the block handles comes back out
//! as the names AutoCAD writes (`ArchTick` for the block `_ArchTick`).

use std::io::Cursor;

use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn arrow_block(doc: &mut CadDocument, name: &str) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let handle = record.handle;
    doc.block_records.add(record).unwrap();
    handle
}

fn header_value(dxf: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    lines.windows(4).find_map(|window| {
        if window[0] == "9" && window[1] == name {
            Some(window[3].to_string())
        } else {
            None
        }
    })
}

fn drawing() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let arch = arrow_block(&mut doc, "_ArchTick");
    let dot = arrow_block(&mut doc, "_Dot");
    let open = arrow_block(&mut doc, "_Open");
    let small = arrow_block(&mut doc, "_Small");
    doc.header.dim_arrow_block_handle = arch;
    doc.header.dim_arrow_block1_handle = dot;
    doc.header.dim_arrow_block2_handle = open;
    doc.header.dim_leader_arrow_block_handle = small;
    doc.header.dim_arc_symbol = 1;
    doc.header.dim_fixed_ext_line_on = true;
    doc.header.dim_fixed_ext_line_length = 1.5;
    doc
}

#[test]
fn dxf_writes_the_autocad_arrow_names_and_reads_them_back() {
    let doc = drawing();
    let dxf = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    assert_eq!(header_value(&dxf, "$DIMBLK").as_deref(), Some("ArchTick"));
    assert_eq!(header_value(&dxf, "$DIMBLK1").as_deref(), Some("Dot"));
    assert_eq!(header_value(&dxf, "$DIMBLK2").as_deref(), Some("Open"));
    assert_eq!(header_value(&dxf, "$DIMLDRBLK").as_deref(), Some("Small"));
    assert_eq!(header_value(&dxf, "$DIMARCSYM").as_deref(), Some("1"));
    assert_eq!(header_value(&dxf, "$DIMFXLON").as_deref(), Some("1"));

    let back = DxfReader::from_reader(Cursor::new(dxf.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(back.header.dim_arrow_block, "ArchTick");
    assert_eq!(back.header.dim_arrow_block1, "Dot");
    assert_eq!(back.header.dim_arrow_block2, "Open");
    assert_eq!(back.header.dim_leader_arrow_block, "Small");
    assert_eq!(back.header.dim_arc_symbol, 1);
    assert!(back.header.dim_fixed_ext_line_on);
    let name = |handle: Handle| {
        back.block_records
            .iter()
            .find(|record| record.handle == handle)
            .map(|record| record.name.clone())
    };
    assert_eq!(
        name(back.header.dim_arrow_block_handle).as_deref(),
        Some("_ArchTick")
    );
    assert_eq!(
        name(back.header.dim_arrow_block1_handle).as_deref(),
        Some("_Dot")
    );
    assert_eq!(
        name(back.header.dim_arrow_block2_handle).as_deref(),
        Some("_Open")
    );
    assert_eq!(
        name(back.header.dim_leader_arrow_block_handle).as_deref(),
        Some("_Small")
    );
}

#[test]
fn a_dwg_round_trip_still_writes_those_dxf_names() {
    let doc = drawing();
    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    let from_dwg = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    assert_eq!(from_dwg.header.dim_arrow_block, "ArchTick");
    assert_eq!(from_dwg.header.dim_arc_symbol, 1);
    assert!(from_dwg.header.dim_fixed_ext_line_on);

    let dxf = String::from_utf8(DxfWriter::new(&from_dwg).write_to_vec().unwrap()).unwrap();
    assert_eq!(header_value(&dxf, "$DIMBLK").as_deref(), Some("ArchTick"));
    assert_eq!(header_value(&dxf, "$DIMLDRBLK").as_deref(), Some("Small"));
    assert_eq!(header_value(&dxf, "$DIMARCSYM").as_deref(), Some("1"));
    assert_eq!(header_value(&dxf, "$DIMFXLON").as_deref(), Some("1"));

    let again = DxfReader::from_reader(Cursor::new(dxf.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let dwg = DwgWriter::write_to_vec(&again).unwrap();
    let back = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    assert_eq!(back.header.dim_leader_arrow_block, "Small");
    assert_eq!(back.header.dim_arc_symbol, 1);
    assert!(back.header.dim_fixed_ext_line_on);
    let leader = back
        .block_records
        .iter()
        .find(|record| record.handle == back.header.dim_leader_arrow_block_handle)
        .map(|record| record.name.as_str());
    assert_eq!(leader, Some("_Small"));
}
