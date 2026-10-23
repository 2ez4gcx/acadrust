//! The SEQEND that ends a block reference's attributes keeps its handle when the drawing is
//! written as DXF (as the DWG writer keeps it), instead of getting a new handle on every save.

use std::io::Cursor;

use acadrust::entities::{AttributeEntity, EntityType, Insert};
use acadrust::tables::BlockRecord;
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn drawing() -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::new();
    let mut record = BlockRecord::new("TAGGED");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    doc.block_records.add(record).unwrap();

    let mut insert = Insert::new("TAGGED", Vector3::new(5.0, 5.0, 0.0));
    insert.common.handle = doc.allocate_handle();
    let mut attribute = AttributeEntity::new("TAG".into(), "value".into());
    attribute.common.handle = doc.allocate_handle();
    attribute.common.owner_handle = insert.common.handle;
    insert.attributes.push(attribute);
    let seqend = doc.allocate_handle();
    insert.seqend_handle = Some(seqend);
    let insert_handle = doc.add_entity(EntityType::Insert(insert)).unwrap();
    (doc, insert_handle, seqend)
}

/// The handle (group 5) of the SEQEND owned (group 330) by `owner`.
fn seqend_of(dxf: &str, owner: Handle) -> Option<Handle> {
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let pairs: Vec<(&str, &str)> = lines
        .chunks(2)
        .filter_map(|p| Some((*p.first()?, *p.get(1)?)))
        .collect();
    pairs.iter().enumerate().find_map(|(i, &(code, value))| {
        if code != "0" || value != "SEQEND" {
            return None;
        }
        let groups = pairs[i + 1..].iter().take_while(|(code, _)| *code != "0");
        let mut handle = None;
        let mut owned = false;
        for &(code, value) in groups {
            match code {
                "5" => handle = u64::from_str_radix(value, 16).ok().map(Handle::new),
                "330" => owned = u64::from_str_radix(value, 16).ok() == Some(owner.value()),
                _ => {}
            }
        }
        if owned {
            handle
        } else {
            None
        }
    })
}

#[test]
fn an_insert_seqend_keeps_its_handle_in_dxf() {
    let (doc, insert, seqend) = drawing();
    let dxf = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    assert_eq!(seqend_of(&dxf, insert), Some(seqend));

    // ...and keeps it through a DXF read and write.
    let back = DxfReader::from_reader(Cursor::new(dxf.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let dxf = String::from_utf8(DxfWriter::new(&back).write_to_vec().unwrap()).unwrap();
    assert_eq!(seqend_of(&dxf, insert), Some(seqend));
}
