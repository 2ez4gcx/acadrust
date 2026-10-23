use acadrust::io::dwg::{
    dwg_stream_readers::{
        bit_reader::DwgBitReader, merged_reader::DwgMergedReader,
        object_reader::associative::read_associative_data,
    },
    dwg_stream_writers::bit_writer::DwgBitWriter,
    DwgVersion,
};
use acadrust::objects::{AssocConstraintNodeData, AssociativeData};
use acadrust::types::{DxfVersion, Vector3};

fn group_stream(
    dxf: DxfVersion,
    seed: i32,
    classes: &[&str],
    node_data: impl FnOnce(&mut DwgBitWriter),
    geometry_handles: usize,
) -> DwgMergedReader {
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let mut main = DwgBitWriter::new(version, dxf);
    let mut text = DwgBitWriter::new(version, dxf);
    let mut handles = DwgBitWriter::new(version, dxf);
    main.write_bit_short(1); // AcDbAssocAction version
    for _ in 0..4 {
        main.write_bit_long(0); // status, index, max dependency index, dependencies
    }
    main.write_bit_long(2); // group version
    main.write_bit(false);
    for point in [Vector3::ZERO, Vector3::UNIT_X, Vector3::UNIT_Y] {
        main.write_3bit_double(point);
    }
    main.write_bit_long(0); // owned actions
    main.write_bit_long(seed);
    for name in classes {
        text.write_variable_text(name);
    }
    node_data(&mut main);
    for _ in 0..3 + geometry_handles {
        handles.write_handle_undefined(0);
    }
    DwgMergedReader::from_readers(
        DwgBitReader::new(main.take_bytes(), version, dxf),
        Some(DwgBitReader::new(text.take_bytes(), version, dxf)),
        Some(DwgBitReader::new(handles.take_bytes(), version, dxf)),
        dxf,
    )
}

fn common(main: &mut DwgBitWriter, id: i32) {
    main.write_bit_long(id);
    main.write_byte(1); // R2010 status precedes connections
    main.write_bit_long(0);
}

fn circle(main: &mut DwgBitWriter) {
    main.write_bit_long(0); // geometry node id; dependency is in the handle stream
    for point in [
        Vector3::new(1.0, 2.0, 0.0),
        Vector3::UNIT_Z,
        Vector3::UNIT_X,
    ] {
        main.write_3bit_double(point);
    }
    for value in [3.0, 0.25, 6.0] {
        main.write_bit_double(value);
    }
}

#[test]
fn r2010_inline_circle_and_arc_have_no_reserved_double() {
    let dxf = DxfVersion::AC1024;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    // Encode the independent AutoCAD R2010 layout directly. A node after each
    // circular payload proves that neither consumes a reserved double.
    let mut reader = group_stream(
        dxf,
        3,
        &[
            "AcConstrainedCircle",
            "AcConstrainedArc",
            "AcFixedConstraint",
        ],
        |main| {
            main.write_bit_long(3);
            common(main, 1);
            circle(main);
            common(main, 2);
            circle(main);
            main.write_3bit_double(Vector3::UNIT_X);
            main.write_3bit_double(Vector3::UNIT_Y);
            common(main, 3);
            main.write_bit_long(0); // owner id
            main.write_bit(false); // implied
            main.write_bit(true); // active
        },
        2,
    );
    let AssociativeData::ConstraintGroup(group) =
        read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap()
    else {
        panic!("expected a typed constraint group");
    };
    reader.check_error().unwrap();
    assert_eq!(group.nodes.len(), 4);
    assert!(matches!(
        group.nodes[1].data,
        AssocConstraintNodeData::Circle {
            radius: 3.0,
            start_parameter: 0.25,
            end_parameter: 6.0,
            reserved: 0.0,
            ..
        }
    ));
    assert!(matches!(
        group.nodes[2].data,
        AssocConstraintNodeData::Arc {
            start_point: Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0
            },
            end_point: Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0
            },
            reserved: 0.0,
            ..
        }
    ));
    assert_eq!(group.nodes[3].node_id, 3);
    assert!(matches!(
        group.nodes[3].data,
        AssocConstraintNodeData::Geometrical {
            owner_id: 0,
            is_implied: false,
            is_active: true
        }
    ));
}

#[test]
fn sparse_inline_node_id_seed_is_not_an_array_count() {
    let dxf = DxfVersion::AC1024;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let mut reader = group_stream(
        dxf,
        1_000_000,
        &["AcFixedConstraint"],
        |main| {
            main.write_bit_long(1);
            common(main, 7);
            main.write_bit_long(0);
            main.write_bit(false);
            main.write_bit(true);
        },
        0,
    );
    let AssociativeData::ConstraintGroup(group) =
        read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap()
    else {
        panic!("expected a typed constraint group")
    };
    reader.check_error().unwrap();
    assert_eq!(group.next_node_id, 1_000_000);
    assert_eq!(group.nodes.len(), 2);
    assert_eq!(group.nodes[1].node_id, 7);
}

#[test]
fn sparse_registry_node_id_seed_is_not_an_array_count() {
    let dxf = DxfVersion::AC1027;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let mut reader = group_stream(
        dxf,
        1_000_000,
        &[],
        |main| {
            main.write_bit_long(0); // root id
            main.write_bit_long(0); // root connections
            main.write_bit(false);
            main.write_bit_long(0); // classes
            main.write_bit_long(0); // registry
        },
        0,
    );
    let AssociativeData::ConstraintGroup(group) =
        read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap()
    else {
        panic!("expected a typed constraint group")
    };
    reader.check_error().unwrap();
    assert_eq!(group.next_node_id, 1_000_000);
    assert_eq!(group.nodes.len(), 1);
}

#[test]
fn registry_circle_keeps_its_reserved_double_across_versions() {
    for dxf in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        let version = DwgVersion::from_dxf_version(dxf).unwrap();
        let mut reader = group_stream(
            dxf,
            1,
            &["AcConstrainedCircle"],
            |main| {
                main.write_bit_long(0); // root id
                main.write_bit_long(0);
                main.write_bit(false);
                main.write_bit_long(1); // classes
                main.write_bit_long(1); // registry
                main.write_bit(false);
                main.write_bit_long(1); // class index
                main.write_bit_long(1); // registered node id
                main.write_bit_long(1); // common node id
                if dxf == DxfVersion::AC1024 {
                    main.write_byte(1);
                }
                main.write_bit_long(0); // connections
                if dxf != DxfVersion::AC1024 {
                    main.write_byte(1);
                }
                circle(main);
                main.write_bit_double(2.5); // registry layout retains this value
            },
            1,
        );
        let AssociativeData::ConstraintGroup(group) =
            read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap()
        else {
            panic!("expected a typed constraint group")
        };
        reader.check_error().unwrap();
        assert!(matches!(
            group.nodes[1].data,
            AssocConstraintNodeData::Circle { reserved: 2.5, .. }
        ));
    }
}

#[test]
fn unknown_inline_node_invalidates_the_incomplete_group() {
    let dxf = DxfVersion::AC1024;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let mut reader = group_stream(
        dxf,
        2,
        &["AcFutureConstraint", "AcFixedConstraint"],
        |main| {
            main.write_bit_long(2);
            common(main, 1);
            main.write_bit_long(123); // unknown payload must not become the next node header
            common(main, 2);
            main.write_bit_long(0);
            main.write_bit(false);
            main.write_bit(true);
        },
        0,
    );
    read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap();
    let error = reader.check_error().unwrap_err().to_string();
    assert!(
        error.contains("Unsupported DWG constraint node class AcFutureConstraint"),
        "{error}"
    );
}

#[test]
fn unknown_registered_node_invalidates_the_incomplete_group() {
    let dxf = DxfVersion::AC1027;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let mut reader = group_stream(
        dxf,
        1,
        &["AcFutureConstraint"],
        |main| {
            main.write_bit_long(0); // root id
            main.write_bit_long(0);
            main.write_bit(false);
            main.write_bit_long(1); // classes
            main.write_bit_long(1); // registry
            main.write_bit(false);
            main.write_bit_long(1); // class index
            main.write_bit_long(1); // registered node id
            main.write_bit_long(1); // common node id
            main.write_bit_long(0); // connections
            main.write_byte(1); // R2013 status follows connections
            main.write_bit_long(123); // unknown payload
        },
        0,
    );
    read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap();
    let error = reader.check_error().unwrap_err().to_string();
    assert!(
        error.contains("Unsupported DWG constraint node class AcFutureConstraint"),
        "{error}"
    );
}

#[test]
fn malformed_inline_node_count_still_fails() {
    let dxf = DxfVersion::AC1024;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let mut reader = group_stream(dxf, 1, &[], |main| main.write_bit_long(1_000_000), 0);
    read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap();
    let error = reader.check_error().unwrap_err().to_string();
    assert!(error.contains("Invalid DWG array count 1000000"), "{error}");
}

#[test]
fn reported_malformed_connection_counts_still_fail() {
    let dxf = DxfVersion::AC1024;
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    for count in [-49, 1_652_754_708, 1_619_069_204] {
        let mut reader = group_stream(
            dxf,
            1,
            &["AcFixedConstraint"],
            |main| {
                main.write_bit_long(1); // inline nodes
                main.write_bit_long(1); // node id
                main.write_byte(0);
                main.write_bit_long(count);
            },
            0,
        );
        read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap();
        let error = reader.check_error().unwrap_err().to_string();
        assert!(
            error.contains(&format!("Invalid DWG array count {count}")),
            "{error}"
        );
    }
}
