use std::io::Cursor;

use acadrust::entities::{AttributeEntity, Block, EntityType, Insert};
use acadrust::tables::BlockRecord;
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn add_block(document: &mut CadDocument, name: &str) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = document.allocate_handle();
    let owner = record.handle;
    document.block_records.add(record).unwrap();
    let mut marker = Block::new(name, Vector3::ZERO);
    marker.common.owner_handle = owner;
    let marker_handle = document.add_entity(EntityType::Block(marker)).unwrap();
    document
        .block_records
        .get_mut(name)
        .unwrap()
        .block_entity_handle = marker_handle;
    owner
}

#[test]
fn nested_attributed_insert_explode_gets_fresh_ids_before_dwg_roundtrip() {
    let mut document = CadDocument::new();
    let child_owner = add_block(&mut document, "CHILD");
    let parent_owner = add_block(&mut document, "PARENT");

    // The parent block contains a nested attributed INSERT.
    let mut child = Insert::new("CHILD", Vector3::ZERO);
    child.common.handle = document.allocate_handle();
    child.common.owner_handle = parent_owner;
    let child_handle = child.common.handle;
    let mut child_attribute = AttributeEntity::new("TAG".into(), "nested".into());
    child_attribute.common.handle = document.allocate_handle();
    child_attribute.common.owner_handle = child_handle;
    child.attributes.push(child_attribute);
    child.seqend_handle = Some(document.allocate_handle());
    document.add_entity(EntityType::Insert(child)).unwrap();

    // Keep the child definition valid even though this test only checks the
    // nested INSERT record emitted by EXPLODE.
    let mut child_line =
        acadrust::entities::Line::from_points(Vector3::ZERO, Vector3::new(1.0, 0.0, 0.0));
    child_line.common.owner_handle = child_owner;
    document.add_entity(EntityType::Line(child_line)).unwrap();

    // Source outer INSERT is retained, while its exploded nested INSERT is
    // added as a separate model-space entity.
    let mut outer = Insert::new("PARENT", Vector3::new(10.0, 20.0, 0.0));
    outer.common.owner_handle = document.header.model_space_block_handle;
    outer.attributes.push({
        let mut attribute = AttributeEntity::new("OUTER".into(), "source".into());
        attribute.common.handle = document.allocate_handle();
        attribute.common.owner_handle = outer.common.handle;
        attribute
    });
    outer.seqend_handle = Some(document.allocate_handle());
    let outer_handle = document.add_entity(EntityType::Insert(outer)).unwrap();

    let source = document.get_entity(outer_handle).unwrap().clone();
    let pieces = document.explode_entity(&source);
    assert_eq!(pieces.len(), 1);
    let nested = pieces
        .iter()
        .find_map(|entity| match entity {
            EntityType::Insert(insert) => Some(insert),
            _ => None,
        })
        .expect("nested INSERT");
    assert_eq!(nested.attributes.len(), 1);
    assert_ne!(nested.common.handle, child_handle);
    assert_ne!(nested.attributes[0].common.handle, Handle::NULL);
    assert_ne!(nested.seqend_handle, Some(Handle::NULL));
    document
        .add_entity(pieces.into_iter().next().unwrap())
        .unwrap();

    let bytes = DwgWriter::write_to_vec(&document).expect("write DWG");
    let reopened = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read DWG");
    let inserts: Vec<_> = reopened
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Insert(insert)
                if insert.common.owner_handle == reopened.header.model_space_block_handle =>
            {
                Some(insert)
            }
            _ => None,
        })
        .collect();
    assert_eq!(inserts.len(), 2);
    assert!(inserts.iter().any(|insert| {
        insert
            .attributes
            .iter()
            .any(|attribute| attribute.value == "source")
    }));
    assert!(inserts.iter().any(|insert| {
        insert
            .attributes
            .iter()
            .any(|attribute| attribute.value == "nested")
    }));

    let attribute_handles: Vec<_> = inserts
        .iter()
        .flat_map(|insert| {
            insert
                .attributes
                .iter()
                .map(|attribute| attribute.common.handle)
        })
        .collect();
    assert_eq!(attribute_handles.len(), 2);
    assert_ne!(attribute_handles[0], attribute_handles[1]);
}
