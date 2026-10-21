use acadrust::io::dwg::{
    dwg_stream_readers::{
        bit_reader::DwgBitReader, merged_reader::DwgMergedReader,
        object_reader::associative::read_associative_data,
    },
    dwg_stream_writers::bit_writer::DwgBitWriter,
    DwgVersion,
};
use acadrust::objects::{
    Assoc2dConstraintGroup, AssocConstraintNode, AssocConstraintNodeData, AssociativeData,
};
use acadrust::DxfVersion;

fn node_payload(
    main: &mut DwgBitWriter,
    handles: &mut DwgBitWriter,
    data: &AssocConstraintNodeData,
    modern: bool,
) {
    use AssocConstraintNodeData::*;
    match data {
        Geometrical {
            owner_id,
            is_implied,
            is_active,
        } => {
            main.write_bit_long(*owner_id);
            main.write_bit(*is_implied);
            main.write_bit(*is_active);
        }
        Parallel {
            owner_id,
            is_implied,
            is_active,
            datum_line_index,
        } => {
            main.write_bit_long(*owner_id);
            main.write_bit(*is_implied);
            main.write_bit(*is_active);
            if let Some(id) = datum_line_index {
                main.write_bit_long(*id);
            }
        }
        ImplicitPoint {
            geometry_dependency,
            geometry_node_id,
            point,
            point_type,
            point_index,
            curve_id,
        } => {
            handles.write_handle_undefined(geometry_dependency.value());
            main.write_bit_long(*geometry_node_id);
            if !geometry_dependency.is_null() {
                main.write_3bit_double(point.expect("dependent point coordinates"));
            }
            main.write_byte(*point_type);
            main.write_bit_long(*point_index);
            main.write_bit_long(*curve_id);
        }
        Line {
            geometry_dependency,
            geometry_node_id,
            point,
            direction,
        } => {
            handles.write_handle_undefined(geometry_dependency.value());
            main.write_bit_long(*geometry_node_id);
            main.write_3bit_double(*point);
            main.write_3bit_double(*direction);
        }
        BoundedLine {
            geometry_dependency,
            geometry_node_id,
            point,
            direction,
            is_ray,
            start_point,
            end_point,
        } => {
            handles.write_handle_undefined(geometry_dependency.value());
            main.write_bit_long(*geometry_node_id);
            main.write_3bit_double(*point);
            main.write_3bit_double(*direction);
            main.write_bit(*is_ray);
            main.write_3bit_double(*start_point);
            main.write_3bit_double(*end_point);
        }
        Circle {
            geometry_dependency,
            geometry_node_id,
            center,
            normal,
            direction,
            radius,
            start_parameter,
            end_parameter,
            reserved,
        }
        | Arc {
            geometry_dependency,
            geometry_node_id,
            center,
            normal,
            direction,
            radius,
            start_parameter,
            end_parameter,
            reserved,
            ..
        } => {
            handles.write_handle_undefined(geometry_dependency.value());
            main.write_bit_long(*geometry_node_id);
            for value in [center, normal, direction] {
                main.write_3bit_double(*value);
            }
            for value in [radius, start_parameter, end_parameter] {
                main.write_bit_double(*value);
            }
            if modern {
                main.write_bit_double(*reserved);
            }
            if let Arc {
                start_point,
                end_point,
                ..
            } = data
            {
                main.write_3bit_double(*start_point);
                main.write_3bit_double(*end_point);
            }
        }
        Ellipse {
            geometry_dependency,
            geometry_node_id,
            center,
            major_axis,
            axis_ratio,
        }
        | BoundedEllipse {
            geometry_dependency,
            geometry_node_id,
            center,
            major_axis,
            axis_ratio,
            ..
        } => {
            handles.write_handle_undefined(geometry_dependency.value());
            main.write_bit_long(*geometry_node_id);
            main.write_3bit_double(*center);
            main.write_3bit_double(*major_axis);
            main.write_bit_double(*axis_ratio);
            if let BoundedEllipse {
                start_point,
                end_point,
                ..
            } = data
            {
                main.write_3bit_double(*start_point);
                main.write_3bit_double(*end_point);
            }
        }
        Spline {
            geometry_dependency,
            geometry_node_id,
            rational,
            periodic,
            degree,
            knot_tolerance,
            knot_physical_length,
            knot_grow_length,
            knots,
            weight_physical_length,
            weight_grow_length,
            weights,
            control_point_physical_length,
            control_point_grow_length,
            control_points,
            implicit_point_ids,
        } => {
            handles.write_handle_undefined(geometry_dependency.value());
            main.write_bit_long(*geometry_node_id);
            main.write_bit(*rational);
            main.write_bit(*periodic);
            main.write_bit_long(*degree);
            main.write_bit_double(*knot_tolerance);
            for (values, physical, growth) in [
                (knots, knot_physical_length, knot_grow_length),
                (weights, weight_physical_length, weight_grow_length),
            ] {
                main.write_bit_long(values.len() as i32);
                main.write_bit_long(*physical);
                main.write_bit_long(*growth);
                for value in values {
                    main.write_bit_double(*value);
                }
            }
            main.write_bit_long(control_points.len() as i32);
            main.write_bit_long(*control_point_physical_length);
            main.write_bit_long(*control_point_grow_length);
            for value in control_points {
                main.write_3bit_double(*value);
            }
            main.write_bit_long(implicit_point_ids.len() as i32);
            for id in implicit_point_ids {
                main.write_bit_long(*id);
            }
        }
        _ => panic!("unexpected node payload in issue135 corpus: {data:?}"),
    }
}

fn common(main: &mut DwgBitWriter, node: &AssocConstraintNode, modern: bool) {
    main.write_bit_long(node.node_id);
    if !modern {
        main.write_byte(node.status);
    }
    main.write_bit_long(node.connections.len() as i32);
    for id in &node.connections {
        main.write_bit_long(*id);
    }
    if modern {
        main.write_byte(node.status);
    }
}

// Native layouts encoded independently of the production associative writer.
pub fn read_native_group(
    group: &Assoc2dConstraintGroup,
    dxf: DxfVersion,
) -> Assoc2dConstraintGroup {
    let version = DwgVersion::from_dxf_version(dxf).unwrap();
    let modern = dxf >= DxfVersion::AC1027;
    let mut main = DwgBitWriter::new(version, dxf);
    let mut text = DwgBitWriter::new(version, dxf);
    let mut handles = DwgBitWriter::new(version, dxf);
    let action = &group.action;
    assert!(
        action.dependencies.is_empty()
            && action.owned_parameters.is_empty()
            && action.values.is_empty()
    );
    main.write_bit_short(action.class_version);
    main.write_bit_long(action.geometry_status);
    handles.write_handle_undefined(action.owning_network.value());
    handles.write_handle_undefined(action.action_body.value());
    main.write_bit_long(action.action_index);
    main.write_bit_long(action.max_dependency_index);
    main.write_bit_long(0);
    if action.class_version > 1 {
        for _ in 0..2 {
            main.write_bit_short(0);
            main.write_bit_long(0);
        }
    }
    main.write_bit_long(group.version);
    main.write_bit(group.flag);
    for value in group.work_plane {
        main.write_3bit_double(value);
    }
    handles.write_handle_undefined(group.dependency.value());
    main.write_bit_long(group.actions.len() as i32);
    for value in &group.actions {
        handles.write_handle_undefined(value.value());
    }
    main.write_bit_long(group.next_node_id);
    let registered: Vec<_> = group
        .nodes
        .iter()
        .filter(|node| !node.class_name.is_empty())
        .collect();
    if modern {
        let root = group.nodes.first().expect("native root");
        main.write_bit_long(root.node_id);
        main.write_bit_long(root.connections.len() as i32);
        for id in &root.connections {
            main.write_bit_long(*id);
        }
        main.write_bit(root.status != 0);
        let mut classes = Vec::new();
        for node in &registered {
            if !classes.contains(&node.class_name.as_str()) {
                classes.push(node.class_name.as_str());
            }
        }
        main.write_bit_long(classes.len() as i32);
        for class in &classes {
            text.write_variable_text(class);
        }
        main.write_bit_long(registered.len() as i32);
        for node in &registered {
            main.write_bit(node.registry_flag);
            main.write_bit_long(
                classes
                    .iter()
                    .position(|class| *class == node.class_name)
                    .unwrap() as i32
                    + 1,
            );
            main.write_bit_long(node.node_id);
        }
    } else {
        main.write_bit_long(registered.len() as i32);
    }
    for node in registered {
        if !modern {
            text.write_variable_text(&node.class_name);
        }
        common(&mut main, node, modern);
        node_payload(&mut main, &mut handles, &node.data, modern);
    }
    let mut reader = DwgMergedReader::from_readers(
        DwgBitReader::new(main.take_bytes(), version, dxf),
        Some(DwgBitReader::new(text.take_bytes(), version, dxf)),
        Some(DwgBitReader::new(handles.take_bytes(), version, dxf)),
        dxf,
    );
    let value = read_associative_data(&mut reader, "ASSOC2DCONSTRAINTGROUP", version, dxf).unwrap();
    reader.check_error().unwrap();
    assert!(reader.main_remaining_bits() < 8);
    assert!(reader.text_remaining_bits() < 8);
    assert!(reader.handle_remaining_bits() < 8);
    match value {
        AssociativeData::ConstraintGroup(value) => value,
        _ => unreachable!(),
    }
}
