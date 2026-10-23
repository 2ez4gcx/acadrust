use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

use acadrust::objects::{
    Assoc2dConstraintGroup, AssocConstraintNodeData, AssociativeData, ObjectType,
};
use acadrust::{CadDocument, DwgReader, DxfReader, Handle, Vector3};

pub type ValidationResult<T> = Result<T, Box<dyn Error>>;
pub type ConstraintGroups = BTreeMap<Handle, Assoc2dConstraintGroup>;

#[allow(dead_code)] // Used by the optional command-line validator.
pub fn read_strict(path: &Path) -> ValidationResult<CadDocument> {
    let document = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dxf"))
    {
        DxfReader::from_file(path)?.read()?
    } else {
        DwgReader::from_file(path)?.read()?
    };
    if document.notifications.error_count() != 0 {
        return Err(format!(
            "{}: recovered parser errors: {:?}",
            path.display(),
            document.notifications
        )
        .into());
    }
    constraint_groups(&document)?;
    Ok(document)
}

pub fn constraint_groups(document: &CadDocument) -> ValidationResult<ConstraintGroups> {
    let mut groups = BTreeMap::new();
    for (handle, object) in &document.objects {
        match object {
            ObjectType::Associative(value) => {
                if let AssociativeData::ConstraintGroup(group) = &value.data {
                    groups.insert(*handle, group.clone());
                } else if value
                    .cpp_class_name
                    .eq_ignore_ascii_case("AcDbAssoc2dConstraintGroup")
                {
                    return Err(format!("constraint group {handle} has no typed payload").into());
                }
            }
            ObjectType::Unknown { type_name, .. }
                if type_name
                    .to_ascii_uppercase()
                    .contains("ASSOC2DCONSTRAINTGROUP") =>
            {
                return Err(format!("constraint group {handle} was preserved as raw data").into());
            }
            _ => {}
        }
    }
    if groups.is_empty() {
        return Err("drawing contains no typed constraint groups".into());
    }
    Ok(groups)
}

pub fn class_counts(groups: &ConstraintGroups) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for group in groups.values() {
        for node in &group.nodes {
            if !node.class_name.is_empty() {
                *counts.entry(node.class_name.clone()).or_default() += 1;
            }
        }
    }
    counts
}

#[allow(dead_code)] // Shared with the command-line validator, not the integration test.
pub fn print_summary(path: &Path, document: &CadDocument) -> ValidationResult<()> {
    let groups = constraint_groups(document)?;
    println!(
        "{}: {:?}, {} entities, {} typed constraint groups",
        path.display(),
        document.version,
        document.entities().count(),
        groups.len()
    );
    for (handle, group) in &groups {
        println!(
            "  group {handle}: version {}, next id {}, {} nodes",
            group.version,
            group.next_node_id,
            group.nodes.len()
        );
        for (class, count) in class_counts(&BTreeMap::from([(*handle, group.clone())])) {
            println!("    {class}: {count}");
        }
    }
    for notification in &document.notifications {
        eprintln!("  {notification}");
    }
    Ok(())
}

pub fn compare_groups(
    expected: &ConstraintGroups,
    actual: &ConstraintGroups,
) -> ValidationResult<()> {
    if expected.keys().ne(actual.keys()) {
        return Err(format!(
            "constraint group handles changed: {:?} -> {:?}",
            expected.keys().collect::<Vec<_>>(),
            actual.keys().collect::<Vec<_>>()
        )
        .into());
    }
    for (handle, expected) in expected {
        let mut expected = expected.clone();
        let mut actual = actual[handle].clone();
        // The seed can legitimately be raised on save; DXF has no registry bit.
        expected.next_node_id = expected.effective_next_node_id();
        actual.next_node_id = actual.effective_next_node_id();
        for node in expected.nodes.iter_mut().chain(actual.nodes.iter_mut()) {
            node.registry_flag = false;
        }
        for (expected, actual) in expected.work_plane.iter().zip(&mut actual.work_plane) {
            snap_point(expected, actual);
        }
        for (expected, actual) in expected.nodes.iter().zip(&mut actual.nodes) {
            snap_data(&expected.data, &mut actual.data);
        }
        if expected != actual {
            return Err(format!(
                "constraint group {handle} changed\nexpected: {expected:#?}\nactual: {actual:#?}"
            )
            .into());
        }
    }
    Ok(())
}

fn snap_real(expected: f64, actual: &mut f64) {
    // Native ASCII DXF rounds binary coordinates to 16 significant digits.
    if (expected - *actual).abs() <= 1.0e-12 * expected.abs().max(actual.abs()).max(1.0) {
        *actual = expected;
    }
}

fn snap_point(expected: &Vector3, actual: &mut Vector3) {
    snap_real(expected.x, &mut actual.x);
    snap_real(expected.y, &mut actual.y);
    snap_real(expected.z, &mut actual.z);
}

fn snap_data(expected: &AssocConstraintNodeData, actual: &mut AssocConstraintNodeData) {
    use AssocConstraintNodeData::*;
    match (expected, actual) {
        (HelpParameter { value: a, .. }, HelpParameter { value: b, .. }) => snap_real(*a, b),
        (ImplicitPoint { point: Some(a), .. }, ImplicitPoint { point: Some(b), .. })
        | (Point { point: Some(a), .. }, Point { point: Some(b), .. })
        | (
            Distance {
                distance: Some(a), ..
            },
            Distance {
                distance: Some(b), ..
            },
        ) => snap_point(a, b),
        (
            Line {
                point: ap,
                direction: ad,
                ..
            },
            Line {
                point: bp,
                direction: bd,
                ..
            },
        ) => {
            snap_point(ap, bp);
            snap_point(ad, bd);
        }
        (
            BoundedLine {
                point: ap,
                direction: ad,
                start_point: as_,
                end_point: ae,
                ..
            },
            BoundedLine {
                point: bp,
                direction: bd,
                start_point: bs,
                end_point: be,
                ..
            },
        ) => {
            for (a, b) in [(ap, bp), (ad, bd), (as_, bs), (ae, be)] {
                snap_point(a, b);
            }
        }
        (
            Circle {
                center: ac,
                normal: an,
                direction: ad,
                radius: ar,
                start_parameter: as_,
                end_parameter: ae,
                reserved: av,
                ..
            },
            Circle {
                center: bc,
                normal: bn,
                direction: bd,
                radius: br,
                start_parameter: bs,
                end_parameter: be,
                reserved: bv,
                ..
            },
        ) => {
            for (a, b) in [(ac, bc), (an, bn), (ad, bd)] {
                snap_point(a, b);
            }
            for (a, b) in [(ar, br), (as_, bs), (ae, be), (av, bv)] {
                snap_real(*a, b);
            }
        }
        (
            Arc {
                center: ac,
                normal: an,
                direction: ad,
                radius: ar,
                start_parameter: as_,
                end_parameter: ae,
                reserved: av,
                start_point: asp,
                end_point: aep,
                ..
            },
            Arc {
                center: bc,
                normal: bn,
                direction: bd,
                radius: br,
                start_parameter: bs,
                end_parameter: be,
                reserved: bv,
                start_point: bsp,
                end_point: bep,
                ..
            },
        ) => {
            for (a, b) in [(ac, bc), (an, bn), (ad, bd), (asp, bsp), (aep, bep)] {
                snap_point(a, b);
            }
            for (a, b) in [(ar, br), (as_, bs), (ae, be), (av, bv)] {
                snap_real(*a, b);
            }
        }
        (
            Ellipse {
                center: ac,
                major_axis: aa,
                axis_ratio: ar,
                ..
            },
            Ellipse {
                center: bc,
                major_axis: ba,
                axis_ratio: br,
                ..
            },
        ) => {
            snap_point(ac, bc);
            snap_point(aa, ba);
            snap_real(*ar, br);
        }
        (
            BoundedEllipse {
                center: ac,
                major_axis: aa,
                axis_ratio: ar,
                start_point: as_,
                end_point: ae,
                ..
            },
            BoundedEllipse {
                center: bc,
                major_axis: ba,
                axis_ratio: br,
                start_point: bs,
                end_point: be,
                ..
            },
        ) => {
            for (a, b) in [(ac, bc), (aa, ba), (as_, bs), (ae, be)] {
                snap_point(a, b);
            }
            snap_real(*ar, br);
        }
        (RigidSet { transform: a, .. }, RigidSet { transform: b, .. }) => {
            for (a, b) in a.iter().zip(b) {
                snap_real(*a, b);
            }
        }
        (
            Spline {
                knot_tolerance: at,
                knots: ak,
                weights: aw,
                control_points: ap,
                ..
            },
            Spline {
                knot_tolerance: bt,
                knots: bk,
                weights: bw,
                control_points: bp,
                ..
            },
        ) => {
            snap_real(*at, bt);
            for (a, b) in ak.iter().zip(bk).chain(aw.iter().zip(bw)) {
                snap_real(*a, b);
            }
            for (a, b) in ap.iter().zip(bp) {
                snap_point(a, b);
            }
        }
        _ => {}
    }
}

pub fn disable_record_passthrough(document: &mut CadDocument) {
    for entity in document.entities_mut() {
        entity.common_mut().raw_record = None;
    }
    // Associative objects also have an equality-based source-record cache.
    // This storage-only marker invalidates it without changing graph data.
    for object in document.objects.values_mut() {
        if let ObjectType::Associative(object) = object {
            object.source_version = None;
        }
    }
}
