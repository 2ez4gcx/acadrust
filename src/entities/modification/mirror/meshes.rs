use super::super::transform;
use super::*;

pub(crate) fn mirror_face3d(e: &mut Face3D, transform: &Transform) {
    if !super::valid_transform(transform) {
        return;
    }
    transform::transform_face3d(e, transform);
    if transform::is_reflecting(transform) {
        std::mem::swap(&mut e.first_corner, &mut e.fourth_corner);
        std::mem::swap(&mut e.second_corner, &mut e.third_corner);
        let bits = e.invisible_edges.bits();
        e.invisible_edges =
            InvisibleEdgeFlags::from_bits((bits & !5) | ((bits & 1) << 2) | ((bits & 4) >> 2));
    }
    e.common.raw_record = None;
}

pub(crate) fn mirror_solid(e: &mut Solid, transform: &Transform) {
    if !super::valid_transform(transform) {
        return;
    }
    transform::transform_solid(e, transform);
    e.common.raw_record = None;
}

pub(crate) fn mirror_mesh(e: &mut Mesh, transform: &Transform) {
    if !super::valid_transform(transform) {
        return;
    }
    transform::transform_mesh(e, transform);
    e.common.raw_record = None;
}

pub(crate) fn mirror_polyface_mesh(e: &mut PolyfaceMesh, transform: &Transform) {
    if !super::valid_transform(transform) {
        return;
    }
    transform::transform_polyface_mesh(e, transform);
    if transform::is_reflecting(transform) {
        for face in &mut e.faces {
            face.reverse();
            face.common.raw_record = None;
        }
    }
    for vertex in &mut e.vertices {
        vertex.common.raw_record = None;
    }
    e.common.raw_record = None;
}
