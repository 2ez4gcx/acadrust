use std::io::Cursor;
use acadrust::{CadDocument, DxfReader, DxfVersion, DxfWriter};

#[test]
fn binary_dxf_byte_groups_roundtrip_without_desynchronizing_records() {
    let document = CadDocument::with_version(DxfVersion::AC1015);
    let bytes = DxfWriter::new_binary(&document).write_to_vec().unwrap();
    let restored = DxfReader::from_reader(Cursor::new(bytes)).unwrap().read().unwrap();
    assert_eq!(restored.version, DxfVersion::AC1015);
}
