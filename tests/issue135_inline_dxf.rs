use std::io::Cursor;

use acadrust::objects::{
    Assoc2dConstraintGroup, AssocConstraintNodeData, AssociativeData, ObjectType,
};
use acadrust::{CadDocument, DxfReader, DxfWriter, Vector3};

type Pairs = Vec<(i32, String)>;

fn pair(pairs: &mut Pairs, code: i32, value: impl ToString) {
    pairs.push((code, value.to_string()));
}

fn point(pairs: &mut Pairs, code: i32, point: Vector3) {
    pair(pairs, code, point.x);
    pair(pairs, code + 10, point.y);
    pair(pairs, code + 20, point.z);
}

fn group_header(separate_group_section: bool) -> Pairs {
    let mut pairs = Vec::new();
    for (code, value) in [
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1024"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "OBJECTS"),
        (0, "ACDBASSOC2DCONSTRAINTGROUP"),
        (5, "ABCD"),
        (330, "10"),
        (100, "AcDbAssocAction"),
        (90, "1"),
        (90, "0"),
        (330, "10"),
        (360, "0"),
        (90, "1"),
        (90, "0"),
        (90, "0"),
    ] {
        pair(&mut pairs, code, value);
    }
    if separate_group_section {
        pair(&mut pairs, 100, "AcDbAssoc2dConstraintGroup");
    }
    pair(&mut pairs, 90, 2);
    pair(&mut pairs, 70, 0);
    for value in [Vector3::ZERO, Vector3::UNIT_X, Vector3::UNIT_Y] {
        point(&mut pairs, 10, value);
    }
    pair(&mut pairs, 360, 0);
    pair(&mut pairs, 90, 0); // actions
    pair(&mut pairs, 90, 1_000_000); // sparse ID seed, not an allocation length
    pairs
}

fn common(pairs: &mut Pairs, id: i32) {
    pair(pairs, 90, id);
    pair(pairs, 70, 4);
    pair(pairs, 90, 1);
    pair(pairs, 90, 7);
}

fn circular_data(pairs: &mut Pairs) {
    pair(pairs, 330, "30");
    pair(pairs, 90, 0);
    for value in [
        Vector3::new(1.0, 2.0, 0.0),
        Vector3::UNIT_Z,
        Vector3::UNIT_X,
    ] {
        point(pairs, 10, value);
    }
    for value in [3.0, 0.25, 6.0] {
        pair(pairs, 40, value);
    }
}

fn read_pairs(mut pairs: Pairs) -> CadDocument {
    pair(&mut pairs, 0, "ENDSEC");
    pair(&mut pairs, 0, "EOF");
    let text = pairs
        .into_iter()
        .map(|(code, value)| format!("{code}\n{value}\n"))
        .collect::<String>();
    DxfReader::from_reader(Cursor::new(text.into_bytes()))
        .unwrap()
        .read()
        .unwrap()
}

fn constraint_group(document: &CadDocument) -> &Assoc2dConstraintGroup {
    document
        .objects
        .values()
        .find_map(|value| match value {
            ObjectType::Associative(value) => match &value.data {
                AssociativeData::ConstraintGroup(group) => Some(group),
                _ => None,
            },
            _ => None,
        })
        .expect("typed constraint group")
}

#[test]
fn native_inline_r2010_dxf_keeps_circle_arc_and_following_constraint() {
    let mut pairs = group_header(false);
    pair(&mut pairs, 90, 3);
    for (name, id) in [("AcConstrainedCircle", 1), ("AcConstrainedArc", 2)] {
        pair(&mut pairs, 1, name);
        pair(&mut pairs, 330, "ABCD");
        common(&mut pairs, id);
        circular_data(&mut pairs);
        if id == 2 {
            point(&mut pairs, 10, Vector3::UNIT_X);
            point(&mut pairs, 11, Vector3::UNIT_Y);
        }
    }
    pair(&mut pairs, 1, "AcFixedConstraint");
    pair(&mut pairs, 330, "ABCD");
    common(&mut pairs, 7);
    pair(&mut pairs, 90, 0);
    pair(&mut pairs, 290, 0);
    pair(&mut pairs, 290, 1);
    let document = read_pairs(pairs);
    let group = constraint_group(&document);
    assert_eq!(group.next_node_id, 1_000_000);
    assert_eq!(group.nodes.len(), 4);
    assert_eq!(group.nodes[0], Default::default());
    assert_eq!(group.nodes[1].node_id, 1);
    assert_eq!(group.nodes[1].status, 4);
    assert_eq!(group.nodes[1].connections, vec![7]);
    assert!(
        matches!(group.nodes[1].data, AssocConstraintNodeData::Circle {
        geometry_dependency, radius: 3.0, reserved: 0.0, ..
    } if geometry_dependency.value() == 0x30)
    );
    assert!(matches!(group.nodes[2].data, AssocConstraintNodeData::Arc {
        start_point, end_point, reserved: 0.0, ..
    } if start_point == Vector3::UNIT_X && end_point == Vector3::UNIT_Y));
    assert_eq!(group.nodes[3].node_id, 7);
    assert!(matches!(
        group.nodes[3].data,
        AssocConstraintNodeData::Geometrical {
            owner_id: 0,
            is_implied: false,
            is_active: true
        }
    ));
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let roundtrip = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(constraint_group(&roundtrip), group);
}

#[test]
fn legacy_synthetic_r2010_registry_dxf_stays_readable() {
    for separate_group_section in [false, true] {
        let mut pairs = group_header(separate_group_section);
        pair(&mut pairs, 90, 0); // root
        pair(&mut pairs, 90, 0);
        pair(&mut pairs, 290, 0);
        pair(&mut pairs, 90, 1); // registry
        pair(&mut pairs, 1, "AcConstrainedCircle");
        pair(&mut pairs, 90, 7);
        common(&mut pairs, 7);
        circular_data(&mut pairs);
        pair(&mut pairs, 40, 2.5); // legacy codec registry keeps this value
        let document = read_pairs(pairs);
        let group = constraint_group(&document);
        assert_eq!(group.next_node_id, 1_000_000);
        assert_eq!(group.nodes.len(), 2);
        assert_eq!(group.nodes[0], Default::default());
        assert_eq!(group.nodes[1].node_id, 7);
        assert!(matches!(
            group.nodes[1].data,
            AssocConstraintNodeData::Circle { reserved: 2.5, .. }
        ));
    }
}

#[test]
fn empty_r2010_inline_group_keeps_its_sparse_seed_without_a_root() {
    let mut pairs = group_header(false);
    pair(&mut pairs, 90, 0);
    let document = read_pairs(pairs);
    let group = constraint_group(&document);
    assert_eq!(group.next_node_id, 1_000_000);
    assert!(group.nodes.is_empty());
}

#[test]
fn unsupported_dxf_nodes_cannot_silently_become_partial_typed_groups() {
    for registry in [false, true] {
        let mut pairs = group_header(registry);
        if registry {
            pair(&mut pairs, 90, 0); // root
            pair(&mut pairs, 90, 0); // connections
            pair(&mut pairs, 290, 0);
        }
        pair(&mut pairs, 90, 1); // node count
        pair(&mut pairs, 1, "AcFutureConstraint");
        if registry {
            pair(&mut pairs, 90, 7); // registered node id
        } else {
            pair(&mut pairs, 330, "ABCD");
        }
        common(&mut pairs, 7);
        pair(&mut pairs, 40, 12.5); // unsupported payload
        pair(&mut pairs, 0, "ENDSEC");
        pair(&mut pairs, 0, "EOF");
        let text = pairs
            .into_iter()
            .map(|(code, value)| format!("{code}\n{value}\n"))
            .collect::<String>();
        let error = DxfReader::from_reader(Cursor::new(text.into_bytes()))
            .unwrap()
            .read()
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Unsupported DXF constraint node class AcFutureConstraint"),
            "{error}"
        );
    }
}
