use acadrust::{
    objects::{Assoc2dConstraintGroup, AssocConstraintNodeData, AssociativeData, ObjectType},
    types::{DxfVersion, Vector3},
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter,
};
use std::io::Cursor;

fn group(document: &CadDocument) -> &Assoc2dConstraintGroup {
    document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(object) => match &object.data {
                AssociativeData::ConstraintGroup(group) => Some(group),
                _ => None,
            },
            _ => None,
        })
        .expect("typed synthetic group")
}
#[test]
fn synthetic_r2010_inline_group_loads_and_roundtrips_after_edit() {
    let bytes = include_bytes!("fixtures/associative/r2010-v2-inline-group.dwg");
    let mut document = DwgReader::from_stream(Cursor::new(bytes.as_slice()))
        .read()
        .unwrap();
    assert_eq!(document.version, DxfVersion::AC1024);
    let g = group(&document);
    assert_eq!(g.version, 2);
    assert_eq!(g.next_node_id, 9);
    assert_eq!(g.nodes.len(), 3);
    assert_eq!((g.nodes[1].node_id, g.nodes[1].status), (1, 3));
    assert_eq!((g.nodes[2].node_id, g.nodes[2].status), (4, 7));
    assert_eq!(g.nodes[1].connections, vec![4]);
    assert_eq!(g.nodes[2].connections, vec![1]);
    assert!(
        matches!(g.nodes[1].data,AssocConstraintNodeData::BoundedLine{start_point,end_point,..}if start_point==Vector3::ZERO&&end_point==Vector3::UNIT_X)
    );
    assert!(matches!(
        g.nodes[2].data,
        AssocConstraintNodeData::Geometrical {
            owner_id: 1,
            is_implied: false,
            is_active: true
        }
    ));
    for object in document.objects.values_mut() {
        if let ObjectType::Associative(object) = object {
            if let AssociativeData::ConstraintGroup(g) = &mut object.data {
                g.flag = true;
            }
        }
    }
    let expected = group(&document).clone();
    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&document).unwrap()))
        .read()
        .unwrap();
    let dxf = DxfReader::from_reader(Cursor::new(
        DxfWriter::new(&document).write_to_vec().unwrap(),
    ))
    .unwrap()
    .read()
    .unwrap();
    assert_eq!(group(&dwg), &expected);
    assert_eq!(group(&dxf), &expected);
}
