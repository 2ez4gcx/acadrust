//! XDATA attached to the structural BLOCK marker survives DXF and DWG round trips.

use std::io::Cursor;

use acadrust::entities::EntityType;
use acadrust::{DwgReader, DwgWriter, DxfReader, DxfWriter};

const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nAPPID\n70\n1\n0\nAPPID\n2\nACME\n70\n0\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n8\n0\n2\nBX\n70\n0\n10\n0.0\n20\n0.0\n30\n0.0\n3\nBX\n1\n\n1001\nACME\n1000\nblock-entity-xdata\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n1.0\n21\n1.0\n31\n0.0\n0\nENDBLK\n8\n0\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nINSERT\n8\n0\n2\nBX\n10\n5.0\n20\n5.0\n30\n0.0\n0\nENDSEC\n0\nEOF\n";

fn assert_block_marker_xdata(dxf: &[u8], stage: &str) {
    let document = DxfReader::from_reader(Cursor::new(dxf.to_vec()))
        .unwrap()
        .read()
        .unwrap();
    assert_document_marker_xdata(&document, stage);
}

fn assert_document_marker_xdata(document: &acadrust::CadDocument, stage: &str) {
    let record = document.block_records.get("BX").expect("BX block record");
    let Some(EntityType::Block(marker)) = document.get_entity(record.block_entity_handle) else {
        panic!("{stage}: BLOCK marker not retained");
    };
    let data = marker
        .common
        .extended_data
        .get_record("ACME")
        .unwrap_or_else(|| panic!("{stage}: BLOCK marker has no ACME XDATA"));
    assert_eq!(data.values.len(), 1, "{stage}");
    assert_eq!(
        data.values[0],
        acadrust::xdata::XDataValue::String("block-entity-xdata".into()),
        "{stage}"
    );
}

#[test]
fn block_marker_xdata_survives_dxf_and_dwg_roundtrips() {
    let document = DxfReader::from_reader(Cursor::new(DXF.as_bytes()))
        .unwrap()
        .read()
        .unwrap();
    assert_document_marker_xdata(&document, "DXF input");
    let dxf = DxfWriter::new(&document).write_to_vec().unwrap();
    assert!(
        String::from_utf8_lossy(&dxf).contains("block-entity-xdata"),
        "XDATA value missing from DXF output"
    );
    assert_block_marker_xdata(&dxf, "DXF round trip");

    let dwg = DwgWriter::write_to_vec(&document).unwrap();
    let read = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    let dxf = DxfWriter::new(&read).write_to_vec().unwrap();
    assert!(
        String::from_utf8_lossy(&dxf).contains("block-entity-xdata"),
        "XDATA value missing after DXF -> DWG -> DXF"
    );
    assert_block_marker_xdata(&dxf, "DWG round trip");
}
