#[path = "support/issue135_graphs.rs"]
mod graphs;
#[path = "support/issue135_stream.rs"]
mod native_stream;
#[path = "support/issue135_validation.rs"]
mod validation;

use std::io::Cursor;

use acadrust::objects::{AssociativeData, ObjectType};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter};
use validation::{class_counts, compare_groups, constraint_groups, disable_record_passthrough};

fn assert_native_coverage(document: &CadDocument) {
    let groups = constraint_groups(document).unwrap();
    assert_eq!(groups.len(), 13);
    assert_eq!(
        groups
            .values()
            .map(|group| group.nodes.len())
            .sum::<usize>(),
        122
    );
    let counts = class_counts(&groups);
    for (class, expected) in [
        ("AcConstrainedBoundedLine", 11),
        ("AcConstrainedImplicitPoint", 40),
        ("AcConstrainedDatumLine", 2),
        ("AcConstrainedCircle", 5),
        ("AcConstrainedArc", 1),
        ("AcConstrainedEllipse", 1),
        ("AcConstrainedBoundedEllipse", 1),
        ("AcConstrainedSpline", 1),
        ("AcPointCurveConstraint", 26),
        ("AcCenterPointConstraint", 8),
        ("AcHorizontalConstraint", 1),
        ("AcVerticalConstraint", 1),
        ("AcParallelConstraint", 1),
        ("AcPerpendicularConstraint", 1),
        ("AcColinearConstraint", 1),
        ("AcEqualLengthConstraint", 1),
        ("AcConcentricConstraint", 1),
        ("AcEqualRadiusConstraint", 1),
        ("AcTangentConstraint", 1),
        ("AcFixedConstraint", 4),
    ] {
        assert_eq!(counts.get(class), Some(&expected), "{class}");
    }
    assert_eq!(counts.len(), 20);
    for group in groups.values() {
        assert_eq!(group.nodes[0].node_id, 0);
        for node in group.nodes.iter().skip(1) {
            assert!(!node.class_name.is_empty());
        }
    }
}

fn assert_codec_routes(source: &CadDocument) {
    let expected = constraint_groups(source).unwrap();
    let mut source = source.clone();
    disable_record_passthrough(&mut source);
    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
        .read()
        .unwrap();
    assert_eq!(
        dwg.notifications.error_count(),
        0,
        "{:?}",
        dwg.notifications
    );
    compare_groups(&expected, &constraint_groups(&dwg).unwrap()).unwrap();
    for (handle, group) in constraint_groups(&dwg).unwrap() {
        assert_eq!(
            group.next_node_id,
            expected[&handle].effective_next_node_id()
        );
    }
    for writer in [DxfWriter::new(&source), DxfWriter::new_binary(&source)] {
        let dxf = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(
            dxf.notifications.error_count(),
            0,
            "{:?}",
            dxf.notifications
        );
        compare_groups(&expected, &constraint_groups(&dxf).unwrap()).unwrap();
        let through_dxf =
            DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&dxf).unwrap()))
                .read()
                .unwrap();
        assert_eq!(
            through_dxf.notifications.error_count(),
            0,
            "{:?}",
            through_dxf.notifications
        );
        compare_groups(&expected, &constraint_groups(&through_dxf).unwrap()).unwrap();
    }
}

fn assert_native_routes(version: DxfVersion) {
    let source = graphs::document(version);
    assert_native_coverage(&source);
    let mut decoded = source.clone();
    for (handle, expected) in constraint_groups(&source).unwrap() {
        let actual = native_stream::read_native_group(&expected, version);
        assert_eq!(actual, expected, "native layout for {version:?}, {handle}");
        let Some(ObjectType::Associative(object)) = decoded.objects.get_mut(&handle) else {
            unreachable!()
        };
        object.data = AssociativeData::ConstraintGroup(actual);
    }
    assert_native_coverage(&decoded);
    assert_codec_routes(&source);
    assert_codec_routes(&decoded);
}

#[test]
fn native_r2010_constraint_groups_preserve_all_nodes_through_all_codecs() {
    assert_native_routes(DxfVersion::AC1024);
}

#[test]
fn native_r2013_constraint_groups_preserve_all_nodes_through_all_codecs() {
    assert_native_routes(DxfVersion::AC1027);
}

#[test]
fn native_r2018_constraint_groups_preserve_all_nodes_through_all_codecs() {
    assert_native_routes(DxfVersion::AC1032);
}
