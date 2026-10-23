//! An OLE2FRAME's data is written as DXF binary chunks (group 310): 127 bytes a line as upper
//! case hex, the same text as before the writer encoded them itself instead of making a string
//! per byte (which made a drawing with a 10 MB OLE object take seconds to write).

use acadrust::compound_file::BinaryRecord;
use acadrust::entities::{EntityType, Ole2Frame, OleFrameEnvelope};
use acadrust::{CadDocument, DxfWriter};

#[test]
fn ole_data_is_written_as_upper_case_hex_chunks() {
    let mut doc = CadDocument::new();
    let mut ole = Ole2Frame::new();
    let data: Vec<u8> = (0..1000u32).map(|i| (i * 37 % 256) as u8).collect();
    ole.envelope = OleFrameEnvelope::Legacy {
        records: BinaryRecord::split(&data, 300),
    };
    let payload = ole.encoded_payload();
    assert!(payload.len() >= data.len());
    doc.add_entity(EntityType::Ole2Frame(Box::new(ole)))
        .unwrap();
    let mut out = Vec::new();
    DxfWriter::new(&doc).write_to_writer(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    // The entity (the CLASSES section names OLE2FRAME too, as group 1).
    let at = lines
        .windows(2)
        .position(|w| w[0].trim() == "0" && w[1] == "OLE2FRAME")
        .expect("an OLE2FRAME")
        + 1;
    // (code, value) pairs of the entity, up to the next record.
    let groups = lines[at + 1..].chunks(2).take_while(|g| g[0].trim() != "0");
    let chunks: Vec<&str> = groups
        .filter(|g| g[0].trim() == "310")
        .map(|g| g[1])
        .collect();
    let expected: Vec<String> = payload
        .chunks(127)
        .map(|c| c.iter().map(|b| format!("{b:02X}")).collect())
        .collect();
    assert_eq!(chunks, expected);
}

#[test]
fn ole_data_survives_a_binary_dxf_round_trip() {
    let mut doc = CadDocument::new();
    let mut ole = Ole2Frame::new();
    let data: Vec<u8> = (0..1000u32).map(|i| (i * 53 % 256) as u8).collect();
    ole.envelope = OleFrameEnvelope::Legacy {
        records: BinaryRecord::split(&data, 300),
    };
    let payload = ole.encoded_payload();
    doc.add_entity(EntityType::Ole2Frame(Box::new(ole)))
        .unwrap();
    let mut out = Vec::new();
    DxfWriter::new_binary(&doc)
        .write_to_writer(&mut out)
        .unwrap();
    let back = acadrust::DxfReader::from_reader(std::io::Cursor::new(out))
        .unwrap()
        .read()
        .unwrap();
    let read = back
        .entities()
        .find_map(|entity| match entity {
            EntityType::Ole2Frame(ole) => Some(ole.encoded_payload()),
            _ => None,
        })
        .expect("an OLE2FRAME");
    assert_eq!(read, payload);
}
