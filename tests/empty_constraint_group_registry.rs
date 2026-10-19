//! An ASSOC2DCONSTRAINTGROUP without nodes keeps the R2013+ registry layout in DWG: after the
//! next node id, the root node (id, connections, status), the class table and the registry
//! follow with zero counts, as AutoCAD and BricsCAD write them. A group that ended after the
//! next node id made BricsCAD refuse the whole drawing ("Object improperly read"). The reader
//! tolerated the short form, so a DWG read and written back kept its original bytes and only a
//! group that came through DXF or the API was affected.
use acadrust::io::dwg::dwg_stream_readers::object_reader::associative::read_associative_data;
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::objects::{
    Assoc2dConstraintGroup, AssocAction, AssociativeData, AssociativeObject, ObjectType,
};
use acadrust::types::{DxfVersion, Handle};
use acadrust::CadDocument;

fn document_with_empty_group(version: DxfVersion) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let handle = document.allocate_handle();
    let owner = document.header.named_objects_dict_handle;
    document.objects.insert(
        handle,
        ObjectType::Associative(AssociativeObject {
            handle,
            owner,
            dxf_name: "ASSOC2DCONSTRAINTGROUP".to_string(),
            cpp_class_name: "AcDbAssoc2dConstraintGroup".to_string(),
            data: AssociativeData::ConstraintGroup(Assoc2dConstraintGroup {
                action: AssocAction {
                    class_version: 2,
                    ..Default::default()
                },
                version: 2,
                ..Default::default()
            }),
            ..Default::default()
        }),
    );
    (document, handle)
}

#[test]
fn empty_constraint_group_keeps_the_registry_layout_in_r2013_dwg() {
    let (document, handle) = document_with_empty_group(DxfVersion::AC1032);
    let (bytes, records, _, _) = DwgObjectWriter::new(&document).unwrap().write();
    let offsets = records
        .into_iter()
        .map(|(handle, offset)| (handle, offset as i64))
        .collect();
    let objects = DwgObjectReader::new(bytes, document.version, offsets).unwrap();
    let offset = objects.offset_for(handle.value()).unwrap() as usize;
    let (type_code, mut reader) = objects.read_record_at(offset).unwrap();
    objects.read_common_non_entity_data(&mut reader, type_code);
    let AssociativeData::ConstraintGroup(group) = read_associative_data(
        &mut reader,
        "ASSOC2DCONSTRAINTGROUP",
        objects.version(),
        document.version,
    )
    .unwrap() else {
        panic!("not a constraint group")
    };
    assert!(group.nodes.is_empty());
    assert_eq!(group.next_node_id, 0);
    // The reader stops after the next node id. The root node, the class table and the
    // registry still follow, with zero counts, and nothing else precedes the handles.
    assert_eq!(reader.main_remaining_bits(), 9, "root id, connections, status, class types, registry");
    assert_eq!(reader.read_bit_long(), 0, "root node id");
    assert_eq!(reader.read_bit_long(), 0, "root node connections");
    assert!(!reader.read_bit(), "root node status");
    assert_eq!(reader.read_bit_long(), 0, "class types");
    assert_eq!(reader.read_bit_long(), 0, "registered nodes");
    assert_eq!(reader.main_remaining_bits(), 0);
    reader.check_error().unwrap();
}

#[test]
fn empty_constraint_group_round_trips_in_dwg() {
    use std::io::Cursor;
    let (document, handle) = document_with_empty_group(DxfVersion::AC1032);
    let bytes = acadrust::DwgWriter::write_to_vec(&document).unwrap();
    let read = acadrust::DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let Some(ObjectType::Associative(object)) = read.objects.get(&handle) else {
        panic!("group lost")
    };
    let AssociativeData::ConstraintGroup(group) = &object.data else {
        panic!("not a constraint group")
    };
    assert!(group.nodes.is_empty());
    assert_eq!((group.version, group.next_node_id), (2, 0));
}
