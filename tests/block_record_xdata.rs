//! A block record's own XDATA (a dynamic block's AcDbDynamicBlockTrueName,
//! AcDbDynamicBlockGUID and AcDbBlockRepETag tags) survives DXF and DWG.

use acadrust::tables::{AppId, BlockRecord};
use acadrust::types::DxfVersion;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

fn tagged() -> (CadDocument, acadrust::Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for name in ["AcDbDynamicBlockTrueName", "AcDbBlockRepETag"] {
        let mut app = AppId::new(name);
        app.handle = doc.allocate_handle();
        doc.app_ids.add(app).unwrap();
    }
    let mut record = BlockRecord::new("door");
    record.handle = doc.allocate_handle();
    let handle = record.handle;
    doc.block_records.add(record).unwrap();
    let mut name = ExtendedDataRecord::new("AcDbDynamicBlockTrueName");
    name.add_value(XDataValue::String("door".into()));
    doc.set_object_xdata_record(handle, name);
    let mut tag = ExtendedDataRecord::new("AcDbBlockRepETag");
    tag.add_value(XDataValue::Integer16(1));
    tag.add_value(XDataValue::Integer32(5));
    doc.set_object_xdata_record(handle, tag);
    (doc, handle)
}

fn check(read: &CadDocument, what: &str) {
    let record = read.block_records.get("door").expect("block record");
    let xdata = read
        .object_xdata(record.handle)
        .unwrap_or_else(|| panic!("{what}: no xdata"));
    let name = xdata
        .get_record("AcDbDynamicBlockTrueName")
        .expect("true name");
    assert_eq!(
        name.values,
        vec![XDataValue::String("door".into())],
        "{what}"
    );
    let tag = xdata.get_record("AcDbBlockRepETag").expect("tag");
    assert_eq!(
        tag.values,
        vec![XDataValue::Integer16(1), XDataValue::Integer32(5)],
        "{what}"
    );
}

#[test]
fn block_record_xdata_round_trips_dxf_and_dwg() {
    let (doc, _) = tagged();
    let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
    let read = DxfReader::from_reader(Cursor::new(dxf))
        .unwrap()
        .read()
        .unwrap();
    check(&read, "dxf");
    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    let read = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    check(&read, "dwg");

    // A record replaced after a DWG read is the one written next time.
    let mut read = read;
    let handle = read.block_records.get("door").unwrap().handle;
    let mut tag = ExtendedDataRecord::new("AcDbBlockRepETag");
    tag.add_value(XDataValue::Integer16(1));
    tag.add_value(XDataValue::Integer32(9));
    read.set_object_xdata_record(handle, tag);
    let dwg = DwgWriter::write_to_vec(&read).unwrap();
    let again = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    let handle = again.block_records.get("door").unwrap().handle;
    let tag = again
        .object_xdata(handle)
        .unwrap()
        .get_record("AcDbBlockRepETag")
        .unwrap();
    assert_eq!(tag.values[1], XDataValue::Integer32(9));
}
