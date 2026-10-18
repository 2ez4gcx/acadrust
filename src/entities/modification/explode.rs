//! Entity explosion — decompose complex entities into simpler primitives.
//!
//! The [`EntityType::explode`] method breaks a compound entity into its
//! constituent parts (e.g. a polyline into individual line/arc segments).
//!
//! Returned entities carry [`Handle::NULL`]; callers must assign valid
//! handles before inserting them into a document.  The convenience method
//! [`CadDocument::explode_entity`](crate::document::CadDocument::explode_entity)
//! does this automatically.

use std::f64::consts::PI;

use crate::entities::*;
use crate::types::{Handle, Matrix3, Vector2, Vector3};

// ============================================================================
// Helpers
// ============================================================================

/// Create a new [`EntityCommon`] that inherits visual properties from *source*
/// but has [`Handle::NULL`] (the caller must allocate a real handle later).
pub(crate) fn inherit_common(source: &EntityCommon) -> EntityCommon {
    EntityCommon {
        handle: Handle::NULL,
        layer: source.layer.clone(),
        layer_handle: source.layer_handle,
        color: source.color,
        line_weight: source.line_weight,
        linetype: source.linetype.clone(),
        linetype_scale: source.linetype_scale,
        transparency: source.transparency,
        color_name: source.color_name.clone(),
        color_book_handle: source.color_book_handle,
        linetype_handle: source.linetype_handle,
        full_visual_style_handle: source.full_visual_style_handle,
        face_visual_style_handle: source.face_visual_style_handle,
        edge_visual_style_handle: source.edge_visual_style_handle,
        material_flags: source.material_flags,
        material_handle: source.material_handle,
        shadow_flags: source.shadow_flags,
        plotstyle_flags: source.plotstyle_flags,
        plotstyle_handle: source.plotstyle_handle,
        entity_mode: source.entity_mode,
        invisible: source.invisible,
        owner_handle: source.owner_handle,
        ..EntityCommon::new()
    }
}

/// Normalize an angle into the range [0, 2π).
fn normalize_angle(a: f64) -> f64 {
    let mut a = a % (2.0 * PI);
    if a < 0.0 {
        a += 2.0 * PI;
    }
    a
}

/// Given two 2-D points and a DXF *bulge* value, produce an [`Arc`] entity.
///
/// `elevation` is the Z coordinate shared by both points (OCS).
fn arc_from_bulge(
    p1x: f64,
    p1y: f64,
    p2x: f64,
    p2y: f64,
    bulge: f64,
    elevation: f64,
    thickness: f64,
    normal: Vector3,
    common: &EntityCommon,
) -> EntityType {
    let dx = p2x - p1x;
    let dy = p2y - p1y;
    let d = (dx * dx + dy * dy).sqrt();

    if d < 1e-10 {
        // Degenerate segment — return a point instead.
        return EntityType::Point(Point {
            common: inherit_common(common),
            location: Vector3::new(p1x, p1y, elevation),
            thickness,
            normal,
            x_axis_angle: 0.0,
        });
    }

    let b_sq = bulge * bulge;
    let radius = d * (1.0 + b_sq) / (4.0 * bulge.abs());

    // Perpendicular signed offset from midpoint to centre.
    let perp_offset = d * (1.0 - b_sq) / (4.0 * bulge);
    let mx = (p1x + p2x) / 2.0;
    let my = (p1y + p2y) / 2.0;
    // Left-perpendicular of chord direction.
    let px = -dy / d;
    let py = dx / d;
    let cx = mx + px * perp_offset;
    let cy = my + py * perp_offset;

    let angle_p1 = (p1y - cy).atan2(p1x - cx);
    let angle_p2 = (p2y - cy).atan2(p2x - cx);

    // DXF arcs always run CCW from start_angle to end_angle.
    let (sa, ea) = if bulge > 0.0 {
        (angle_p1, angle_p2)
    } else {
        (angle_p2, angle_p1)
    };

    EntityType::Arc(Arc {
        common: inherit_common(common),
        center: Vector3::new(cx, cy, elevation),
        radius: radius.abs(),
        start_angle: normalize_angle(sa),
        end_angle: normalize_angle(ea),
        thickness,
        normal,
    })
}

/// Build a [`Line`] entity between two 3-D points.
fn line_entity(
    start: Vector3,
    end: Vector3,
    thickness: f64,
    normal: Vector3,
    common: &EntityCommon,
) -> EntityType {
    EntityType::Line(Line {
        common: inherit_common(common),
        start,
        end,
        thickness,
        normal,
    })
}

fn dimension_arrow(tip: Vector3, toward: Vector3, size: f64, common: &EntityCommon, normal: Vector3) -> EntityType {
    let direction = (toward - tip).normalize();
    if direction.length_squared() <= 1e-20 || size <= 0.0 {
        return EntityType::Solid(Solid::triangle(tip, tip, tip));
    }
    let side = normal.normalize().cross(&direction).normalize() * (size * 0.28);
    let base = tip + direction * size;
    let mut solid = Solid::triangle(tip, base + side, base - side);
    solid.common = inherit_common(common);
    EntityType::Solid(solid)
}

fn dimension_mtext(base: &DimensionBase, value: String) -> EntityType {
    let mut text = MText::with_value(value, base.text_middle_point)
        .with_height(2.5)
        .with_width(0.0);
    text.common = inherit_common(&base.common);
    text.style = base.style_name.clone();
    text.rotation = base.text_rotation;
    text.normal = base.normal;
    text.attachment_point = AttachmentPoint::MiddleCenter;
    EntityType::MText(Box::new(text))
}

// ============================================================================
// Per-entity explode functions
// ============================================================================

fn explode_lwpolyline(pl: &LwPolyline) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let seg_count = if pl.is_closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);
    let to_wcs = Matrix3::arbitrary_axis(pl.normal);

    for i in 0..seg_count {
        let v1 = &pl.vertices[i];
        let v2 = &pl.vertices[(i + 1) % n];

        if v1.bulge.abs() < 1e-10 {
            result.push(line_entity(
                to_wcs * Vector3::new(v1.location.x, v1.location.y, pl.elevation),
                to_wcs * Vector3::new(v2.location.x, v2.location.y, pl.elevation),
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        } else {
            result.push(arc_from_bulge(
                v1.location.x,
                v1.location.y,
                v2.location.x,
                v2.location.y,
                v1.bulge,
                pl.elevation,
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        }
    }
    result
}

fn explode_polyline2d(pl: &Polyline2D) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.is_closed();
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);
    let to_wcs = Matrix3::arbitrary_axis(pl.normal);

    for i in 0..seg_count {
        let v1 = &pl.vertices[i];
        let v2 = &pl.vertices[(i + 1) % n];

        if v1.bulge.abs() < 1e-10 {
            result.push(line_entity(
                to_wcs * v1.location,
                to_wcs * v2.location,
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        } else {
            result.push(arc_from_bulge(
                v1.location.x,
                v1.location.y,
                v2.location.x,
                v2.location.y,
                v1.bulge,
                v1.location.z,
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        }
    }
    result
}

fn explode_polyline3d(pl: &Polyline) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.is_closed();
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let start = pl.vertices[i].location;
        let end = pl.vertices[(i + 1) % n].location;
        result.push(line_entity(start, end, 0.0, Vector3::UNIT_Z, &pl.common));
    }
    result
}

fn explode_polyline3d_new(pl: &Polyline3D) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.closed;
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let start = pl.vertices[i].position;
        let end = pl.vertices[(i + 1) % n].position;
        result.push(line_entity(start, end, 0.0, Vector3::UNIT_Z, &pl.common));
    }
    result
}

fn explode_circle(circle: &Circle) -> Vec<EntityType> {
    // A circle explodes into a single full-circle arc.
    vec![EntityType::Arc(Arc {
        common: inherit_common(&circle.common),
        center: circle.center,
        radius: circle.radius,
        start_angle: 0.0,
        end_angle: 2.0 * PI,
        thickness: circle.thickness,
        normal: circle.normal,
    })]
}

fn explode_ellipse(ellipse: &Ellipse) -> Vec<EntityType> {
    // Approximate the ellipse as a spline (same approach AutoCAD uses).
    let a = ellipse.major_axis_length();
    let b = ellipse.minor_axis_length();
    if a < 1e-10 {
        return Vec::new();
    }

    // Direction of major / minor axes in WCS.
    let major_dir = ellipse.major_axis.normalize();
    let minor_dir = ellipse.normal.cross(&major_dir).normalize();

    let is_full = ellipse.is_full();
    // Approximate with N points.
    let num_points: usize = if is_full { 36 } else { 24 };
    let (t_start, t_end) = if is_full {
        (0.0, 2.0 * PI)
    } else {
        (ellipse.start_parameter, ellipse.end_parameter)
    };

    let mut points = Vec::with_capacity(num_points + 1);
    for i in 0..=num_points {
        let t = t_start + (t_end - t_start) * (i as f64 / num_points as f64);
        let pt = ellipse.center + major_dir * (a * t.cos()) + minor_dir * (b * t.sin());
        points.push(pt);
    }

    let spline = Spline::from_control_points(3, points);
    let mut s = spline;
    s.common = inherit_common(&ellipse.common);
    if is_full {
        s.flags.closed = true;
    }
    vec![EntityType::Spline(s)]
}

fn explode_solid(solid: &Solid) -> Vec<EntityType> {
    // Explode a filled solid into its edge lines.
    let common = &solid.common;
    let corners = solid.boundary_corners();
    let n = corners.len();
    let mut result = Vec::with_capacity(n);
    let to_wcs = Matrix3::arbitrary_axis(solid.normal);
    for i in 0..n {
        result.push(line_entity(
            to_wcs * corners[i],
            to_wcs * corners[(i + 1) % n],
            solid.thickness,
            solid.normal,
            common,
        ));
    }
    result
}

fn explode_face3d(face: &Face3D) -> Vec<EntityType> {
    // Explode a 3D face into its edge lines (only visible edges).
    let common = &face.common;
    let corners = face.corners();
    let n = corners.len();
    let mut result = Vec::with_capacity(n);

    let invisible = [
        face.invisible_edges.is_first_invisible(),
        face.invisible_edges.is_second_invisible(),
        face.invisible_edges.is_third_invisible(),
        face.invisible_edges.is_fourth_invisible(),
    ];

    for i in 0..n {
        if !invisible[i] {
            result.push(line_entity(
                corners[i],
                corners[(i + 1) % n],
                0.0,
                Vector3::UNIT_Z,
                common,
            ));
        }
    }
    result
}

/// Points on a NURBS curve (de Boor, rational when `weights` is given).
///
/// `segments` is the number of samples per non-empty knot span. Knot vectors
/// that do not match the control points are replaced by a clamped uniform one.
pub(crate) fn nurbs_points(
    degree: usize,
    control_points: &[Vector3],
    weights: &[f64],
    knots: &[f64],
    segments: usize,
) -> Vec<Vector3> {
    let n = control_points.len();
    if n < 2 {
        return control_points.to_vec();
    }
    let p = degree.clamp(1, n - 1);
    let knots: Vec<f64> = if knots.len() == n + p + 1 && knots.windows(2).all(|w| w[1] >= w[0]) {
        knots.to_vec()
    } else {
        // clamped uniform knot vector
        let inner = n - p;
        (0..n + p + 1)
            .map(|i| {
                if i <= p {
                    0.0
                } else if i >= n {
                    1.0
                } else {
                    (i - p) as f64 / inner as f64
                }
            })
            .collect()
    };
    let rational = weights.len() == n && weights.iter().any(|w| (*w - 1.0).abs() > 1e-12);
    // homogeneous control points
    let hom: Vec<[f64; 4]> = control_points
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = if rational && weights[i] > 0.0 {
                weights[i]
            } else {
                1.0
            };
            [c.x * w, c.y * w, c.z * w, w]
        })
        .collect();
    let eval = |u: f64, k: usize| -> Vector3 {
        let mut d: Vec<[f64; 4]> = (0..=p).map(|j| hom[j + k - p]).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = j + k - p;
                let denom = knots[i + p + 1 - r] - knots[i];
                let alpha = if denom.abs() < 1e-15 {
                    0.0
                } else {
                    (u - knots[i]) / denom
                };
                let (prev, cur) = (d[j - 1], d[j]);
                d[j] = std::array::from_fn(|c| (1.0 - alpha) * prev[c] + alpha * cur[c]);
            }
        }
        let w = if d[p][3].abs() < 1e-15 { 1.0 } else { d[p][3] };
        Vector3::new(d[p][0] / w, d[p][1] / w, d[p][2] / w)
    };
    let segments = segments.max(1);
    let mut points = Vec::new();
    for k in p..n {
        let (u0, u1) = (knots[k], knots[k + 1]);
        if u1 - u0 <= 1e-15 {
            continue;
        }
        let first = if points.is_empty() { 0 } else { 1 };
        for s in first..=segments {
            let u = u0 + (u1 - u0) * s as f64 / segments as f64;
            points.push(eval(u, k));
        }
    }
    if points.is_empty() {
        return control_points.to_vec();
    }
    points
}

/// Points along a spline entity: the NURBS curve when control points are
/// present, otherwise the fit points.
pub(crate) fn spline_points(spline: &Spline) -> Vec<Vector3> {
    if spline.control_points.len() >= 2 {
        let degree = usize::try_from(spline.degree).unwrap_or(3).max(1);
        let mut points = nurbs_points(
            degree,
            &spline.control_points,
            &spline.weights,
            &spline.knots,
            16,
        );
        if spline.flags.closed && points.len() > 2 && points[0] != points[points.len() - 1] {
            points.push(points[0]);
        }
        points
    } else {
        let mut points = spline.fit_points.clone();
        if spline.flags.closed && points.len() > 2 {
            points.push(points[0]);
        }
        points
    }
}

fn explode_spline(spline: &Spline) -> Vec<EntityType> {
    // Approximate the curve by line segments between points on the NURBS curve.
    let points = spline_points(spline);
    if points.len() < 2 {
        return Vec::new();
    }
    points
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .map(|pair| line_entity(pair[0], pair[1], 0.0, spline.normal, &spline.common))
        .collect()
}

/// Wraps one paragraph at `width` using an average character width.
fn wrap_paragraph(text: &str, width: f64, char_width: f64) -> Vec<String> {
    if width <= 0.0 || char_width <= 0.0 {
        return vec![text.to_string()];
    }
    let max_chars = ((width / char_width).floor() as usize).max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if candidate.chars().count() > max_chars && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    lines.push(line);
    lines
}

fn explode_mtext(mtext: &MText) -> Vec<EntityType> {
    // One TEXT per line: formatting codes removed, paragraphs and (approximate)
    // word wrapping at the reference rectangle width, aligned by the attachment point.
    let plain = crate::entities::mtext_format::parse_mtext(&mtext.value, true).to_plain_text();
    let height = if mtext.height > 0.0 {
        mtext.height
    } else {
        1.0
    };
    // average character width of typical CAD fonts
    let char_width = height * 0.6;
    let lines: Vec<String> = plain
        .split('\n')
        .flat_map(|paragraph| wrap_paragraph(paragraph, mtext.rectangle_width, char_width))
        .collect();
    let factor = if mtext.line_spacing_factor > 0.0 {
        mtext.line_spacing_factor
    } else {
        1.0
    };
    let spacing = height * 5.0 / 3.0 * factor;
    let total = height + spacing * (lines.len().saturating_sub(1)) as f64;
    let attachment = mtext.attachment_point as i32;
    let column = (attachment - 1).rem_euclid(3);
    let row = (attachment - 1).div_euclid(3).clamp(0, 2);
    // baseline of the first line, measured from the insertion point (y up)
    let first = match row {
        0 => -height,
        1 => -height + total / 2.0,
        _ => -height + total,
    };
    let rotation = match mtext.dwg_x_direction {
        Some(d) if d.x.abs() + d.y.abs() > 1e-12 => d.y.atan2(d.x),
        _ => mtext.rotation,
    };
    let (sin, cos) = rotation.sin_cos();
    let horizontal = match column {
        0 => TextHorizontalAlignment::Left,
        1 => TextHorizontalAlignment::Center,
        _ => TextHorizontalAlignment::Right,
    };
    lines
        .into_iter()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            let y = first - spacing * i as f64;
            let point = Vector3::new(
                mtext.insertion_point.x - y * sin,
                mtext.insertion_point.y + y * cos,
                mtext.insertion_point.z,
            );
            EntityType::Text(Text {
                common: inherit_common(&mtext.common),
                value: line,
                insertion_point: point,
                alignment_point: (column != 0).then_some(point),
                height,
                rotation,
                width_factor: 1.0,
                oblique_angle: 0.0,
                style: mtext.style.clone(),
                horizontal_alignment: horizontal,
                vertical_alignment: TextVerticalAlignment::Baseline,
                normal: mtext.normal,
                thickness: 0.0,
                generation_flags: 0,
            })
        })
        .collect()
}

fn explode_leader(leader: &Leader) -> Vec<EntityType> {
    let n = leader.vertices.len();
    if n < 2 {
        return Vec::new();
    }
    let size = if leader.arrow_size > 1e-9 { leader.arrow_size } else { 0.18 };
    let mut path = leader.vertices.clone();
    let mut result = Vec::new();
    if leader.arrow_enabled {
        let arrow = dimension_arrow(path[0], path[1], size, &leader.common, leader.normal);
        let direction = (path[1] - path[0]).normalize();
        if direction.length_squared() > 1e-20 {
            path[0] = path[0] + direction * size;
        }
        result.push(arrow);
    }
    if path.len() < 2 {
        return result;
    }
    match leader.path_type {
        LeaderPathType::Spline if path.len() >= 3 => {
            let mut spline = Spline::from_control_points(
                (path.len().min(4) - 1) as i32,
                path,
            );
            spline.common = inherit_common(&leader.common);
            spline.normal = leader.normal;
            result.push(EntityType::Spline(spline));
        }
        _ => {
            let mut polyline = LwPolyline::new();
            polyline.common = inherit_common(&leader.common);
            polyline.normal = leader.normal;
            for point in path {
                polyline.add_point(Vector2::new(point.x, point.y));
            }
            result.push(EntityType::LwPolyline(polyline));
        }
    }
    result
}

fn flatten_leader(leader: &Leader) -> Vec<EntityType> {
    leader
        .vertices
        .windows(2)
        .map(|pair| line_entity(pair[0], pair[1], 0.0, leader.normal, &leader.common))
        .collect()
}

/// Length-to-half-width ratio of AutoCAD's default `_ClosedFilled` arrowhead:
/// a unit-long triangle whose base spans `[-1/6, 1/6]`.
const CLOSED_FILLED_HALF_WIDTH: f64 = 1.0 / 6.0;

/// Whether an arrowhead handle names a real block. A null handle is how a
/// file says "no block": the default `_ClosedFilled` arrowhead.
fn names_arrow_block(handle: Option<Handle>) -> bool {
    handle.is_some_and(|handle| handle != Handle::NULL)
}

/// The arrowhead block a leader line uses: its own when it overrides the
/// entity's, the entity's otherwise.
fn line_arrowhead_handle(ml: &MultiLeader, line: &LeaderLine) -> Option<Handle> {
    if line
        .override_flags
        .contains(LeaderLinePropertyOverrideFlags::ARROWHEAD)
    {
        line.arrowhead_handle
    } else {
        ml.arrowhead_handle
    }
}

/// The arrowhead size a leader line uses, already scaled by the annotation.
fn line_arrowhead_size(ml: &MultiLeader, line: &LeaderLine) -> f64 {
    if line
        .override_flags
        .contains(LeaderLinePropertyOverrideFlags::ARROWHEAD_SIZE)
    {
        line.arrowhead_size
    } else {
        ml.context.arrowhead_size
    }
}

/// A leader line's full path: its own vertices, arrow tip first, then the
/// root's connection point, where the landing starts. AutoCAD stores that last
/// point on the root only, so a straight leader is a single-vertex line.
fn leader_line_path(root: &LeaderRoot, line: &LeaderLine) -> Vec<Vector3> {
    let mut path = line.points.clone();

    if path
        .last()
        .is_some_and(|last| last.distance(&root.connection_point) > f64::EPSILON)
    {
        path.push(root.connection_point);
    }

    path
}

/// The default `_ClosedFilled` arrowhead at the start of `path`, pointing
/// away from the second vertex, and the point where the shaft now starts.
///
/// No arrowhead when the size is not positive or when the first segment is
/// shorter than the arrow itself, which is also what keeps the shaft from
/// being trimmed past its own end.
fn closed_filled_arrowhead(
    path: &[Vector3],
    size: f64,
    common: &EntityCommon,
) -> Option<(EntityType, Vector3)> {
    let (tip, next) = (*path.first()?, *path.get(1)?);
    let first_segment = tip - next;
    let length = first_segment.length();

    if size <= 0.0 || length < size {
        return None;
    }

    let direction = first_segment * (1.0 / length);
    let normal = Vector3::new(-direction.y, direction.x, 0.0) * (size * CLOSED_FILLED_HALF_WIDTH);
    let base = tip - direction * size;
    let mut solid = Solid::triangle(tip, base + normal, base - normal);
    solid.common = inherit_common(common);

    Some((EntityType::Solid(solid), base))
}

/// The landing: from the connection point, along the root's direction, for
/// the landing distance. Nothing when the dogleg is disabled, has no length,
/// or has no direction.
fn landing_line(ml: &MultiLeader, root: &LeaderRoot) -> Option<EntityType> {
    let direction_length = root.direction.length();

    if !ml.enable_dogleg || root.landing_distance <= 0.0 || direction_length <= f64::EPSILON {
        return None;
    }

    let end = root.connection_point + root.direction * (root.landing_distance / direction_length);

    Some(line_entity(
        root.connection_point,
        end,
        0.0,
        Vector3::UNIT_Z,
        &ml.common,
    ))
}

/// Explode one leader line: its arrowhead (default arrowhead only: a custom
/// block is left to the caller, which knows the document) and its segments.
fn explode_leader_line(
    ml: &MultiLeader,
    root: &LeaderRoot,
    line: &LeaderLine,
    result: &mut Vec<EntityType>,
) {
    let mut path = leader_line_path(root, line);
    let arrowhead = if names_arrow_block(line_arrowhead_handle(ml, line)) {
        None
    } else {
        closed_filled_arrowhead(&path, line_arrowhead_size(ml, line), &ml.common)
    };

    if let Some((solid, base)) = arrowhead {
        result.push(solid);
        path[0] = base;
    }

    for pair in path.windows(2) {
        result.push(line_entity(
            pair[0],
            pair[1],
            0.0,
            Vector3::UNIT_Z,
            &ml.common,
        ));
    }
}

fn explode_multileader(ml: &MultiLeader) -> Vec<EntityType> {
    // Decompose MultiLeader the way AutoCAD's EXPLODE does: per leader line,
    // the arrowhead and the segments down to the root's connection point;
    // per root, the landing; then the text.
    let mut result = Vec::new();

    if ml.path_type != MultiLeaderPathType::Invisible {
        for root in &ml.context.leader_roots {
            for line in &root.lines {
                explode_leader_line(ml, root, line, &mut result);
            }

            result.extend(landing_line(ml, root));
        }
    }

    // Add text content if available.
    if ml.context.has_text_contents && !ml.context.text_string.is_empty() {
        let text = MText {
            common: inherit_common(&ml.common),
            value: ml.context.text_string.clone(),
            insertion_point: ml.context.text_location,
            height: ml.context.text_height,
            rectangle_width: ml.context.text_width,
            rectangle_height: None,
            rotation: ml.context.text_rotation,
            style: "Standard".to_string(),
            attachment_point: AttachmentPoint::TopLeft,
            drawing_direction: DrawingDirection::LeftToRight,
            line_spacing_factor: ml.context.line_spacing_factor,
            normal: ml.context.text_normal,
            ..MText::new()
        };
        result.push(EntityType::MText(Box::new(text)));
    }

    result
}

fn explode_mline(mline: &MLine) -> Vec<EntityType> {
    // Context-free entities do not carry their MLINESTYLE definition.  Keep
    // the center path here; the document-aware API can resolve style offsets.
    let n = mline.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        result.push(line_entity(
            mline.vertices[i].position,
            mline.vertices[i + 1].position,
            0.0,
            mline.normal,
            &mline.common,
        ));
    }

    // Close if the MLine is closed.
    if mline.flags.contains(MLineFlags::CLOSED) && n > 2 {
        result.push(line_entity(
            mline.vertices[n - 1].position,
            mline.vertices[0].position,
            0.0,
            mline.normal,
            &mline.common,
        ));
    }

    result
}

fn explode_wipeout(wipeout: &Wipeout) -> Vec<EntityType> {
    let mut points = if wipeout.is_rectangular() {
        wipeout.corners().to_vec()
    } else {
        wipeout.world_boundary_vertices()
    };
    if points.len() < 2 {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(points.len());
    points.push(points[0]);
    for pair in points.windows(2) {
        result.push(line_entity(
            pair[0],
            pair[1],
            0.0,
            Vector3::UNIT_Z,
            &wipeout.common,
        ));
    }
    result
}

fn explode_table(table: &Table) -> Vec<EntityType> {
    if table.rows.is_empty() || table.columns.is_empty() {
        return Vec::new();
    }
    let normal = if table.normal.length_squared() > 1e-20 {
        table.normal.normalize()
    } else {
        Vector3::UNIT_Z
    };
    let horizontal = if table.horizontal_direction.length_squared() > 1e-20 {
        table.horizontal_direction.normalize()
    } else {
        Vector3::UNIT_X
    };
    let vertical = normal.cross(&horizontal).normalize();
    let origin = table.insertion_point;
    let x: Vec<f64> = std::iter::once(0.0)
        .chain(table.columns.iter().scan(0.0, |sum, column| {
            *sum += column.width;
            Some(*sum)
        }))
        .collect();
    let y: Vec<f64> = std::iter::once(0.0)
        .chain(table.rows.iter().scan(0.0, |sum, row| {
            *sum += row.height;
            Some(*sum)
        }))
        .collect();
    let point = |u: f64, v: f64| origin + horizontal * u + vertical * v;
    let mut result = Vec::new();
    for u in &x {
        result.push(line_entity(
            point(*u, 0.0),
            point(*u, *y.last().unwrap()),
            0.0,
            normal,
            &table.common,
        ));
    }
    for v in &y {
        result.push(line_entity(
            point(0.0, *v),
            point(*x.last().unwrap(), *v),
            0.0,
            normal,
            &table.common,
        ));
    }
    for (row_index, row) in table.rows.iter().enumerate() {
        for (column_index, cell) in row.cells.iter().enumerate().take(table.columns.len()) {
            let value = cell.text_value();
            if value.is_empty() {
                continue;
            }
            let mut text = MText::with_value(
                value,
                point((x[column_index] + x[column_index + 1]) * 0.5,
                    (y[row_index] + y[row_index + 1]) * 0.5),
            )
            .with_height(2.5)
            .with_width((x[column_index + 1] - x[column_index]).max(0.0));
            text.common = inherit_common(&table.common);
            text.normal = normal;
            text.attachment_point = AttachmentPoint::MiddleCenter;
            result.push(EntityType::MText(Box::new(text)));
        }
    }
    result
}

fn explode_dimension(dim: &Dimension) -> Vec<EntityType> {
    // Simplified decomposition: produce extension lines, dimension line,
    // and a text entity with the measurement value.
    let base = dim.base();
    let common = &base.common;

    let text_value = base
        .text_override()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{:.4}", dim.measurement()));

    let mut result = Vec::new();
    let mut arrows = Vec::new();

    // Dimension geometry is defined by two measured points and a point on the
    // dimension line.  Project each measured point onto that line so the
    // exploded extension and dimension lines meet at their endpoints.
    let projected_pair =
        |first: Vector3, second: Vector3, on_line: Vector3, line_direction: Vector3| {
            let direction = line_direction.normalize();
            if direction.length_squared() <= 1e-20 {
                return (first, second);
            }
            let project = |point: Vector3| {
                let offset = point - on_line;
                on_line + direction * offset.dot(&direction)
            };
            (project(first), project(second))
        };

    match dim {
        Dimension::Linear(d) => {
            let direction = Matrix3::arbitrary_axis(base.normal)
                * Vector3::new(d.rotation.cos(), d.rotation.sin(), 0.0);
            let (first_on_line, second_on_line) =
                projected_pair(d.first_point, d.second_point, d.definition_point, direction);
            result.push(line_entity(
                d.first_point,
                first_on_line,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.second_point,
                second_on_line,
                0.0,
                base.normal,
                common,
            ));
            // Dimension line between first and second projected points.
            result.push(line_entity(
                first_on_line,
                second_on_line,
                0.0,
                base.normal,
                common,
            ));
            arrows.push((first_on_line, second_on_line));
            arrows.push((second_on_line, first_on_line));
        }
        Dimension::Aligned(d) => {
            let (first_on_line, second_on_line) = projected_pair(
                d.first_point,
                d.second_point,
                d.definition_point,
                d.second_point - d.first_point,
            );
            result.push(line_entity(
                d.first_point,
                first_on_line,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.second_point,
                second_on_line,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                first_on_line,
                second_on_line,
                0.0,
                base.normal,
                common,
            ));
            arrows.push((first_on_line, second_on_line));
            arrows.push((second_on_line, first_on_line));
        }
        Dimension::Radius(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Diameter(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Angular2Ln(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.first_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.angle_vertex,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Angular3Pt(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.first_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.angle_vertex,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Ordinate(d) => {
            let points = d.leader_polyline(0.0, 0.0, None);
            for pair in points.windows(2) {
                if (pair[1] - pair[0]).length() > 1e-12 {
                    result.push(line_entity(pair[0], pair[1], 0.0, base.normal, common));
                }
            }
        }
        Dimension::Arc(d) => {
            result.push(line_entity(
                d.center_point,
                d.first_extension_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.center_point,
                d.second_extension_point,
                0.0,
                base.normal,
                common,
            ));
            if d.has_leader {
                result.push(line_entity(
                    d.first_leader_point,
                    d.second_leader_point,
                    0.0,
                    base.normal,
                    common,
                ));
            }
        }
        Dimension::LargeRadial(d) => {
            result.push(line_entity(
                d.definition_point,
                d.jog_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.jog_point,
                d.chord_point,
                0.0,
                base.normal,
                common,
            ));
        }
    }

    for (tip, toward) in arrows {
        result.push(dimension_arrow(tip, toward, 2.5, common, base.normal));
    }
    result.push(dimension_mtext(base, text_value));

    result
}

fn explode_hatch(hatch: &Hatch) -> Vec<EntityType> {
    // Decompose hatch boundary paths into line/arc entities.
    let common = &hatch.common;
    let elevation = hatch.elevation;
    let normal = hatch.normal;
    let mut result = Vec::new();

    for path in &hatch.paths {
        for edge in &path.edges {
            match edge {
                BoundaryEdge::Line(le) => {
                    result.push(line_entity(
                        Vector3::new(le.start.x, le.start.y, elevation),
                        Vector3::new(le.end.x, le.end.y, elevation),
                        0.0,
                        normal,
                        common,
                    ));
                }
                BoundaryEdge::CircularArc(arc_edge) => {
                    let (sa, ea) = if arc_edge.counter_clockwise {
                        (arc_edge.start_angle, arc_edge.end_angle)
                    } else {
                        (arc_edge.end_angle, arc_edge.start_angle)
                    };
                    result.push(EntityType::Arc(Arc {
                        common: inherit_common(common),
                        center: Vector3::new(arc_edge.center.x, arc_edge.center.y, elevation),
                        radius: arc_edge.radius,
                        start_angle: normalize_angle(sa),
                        end_angle: normalize_angle(ea),
                        thickness: 0.0,
                        normal,
                    }));
                }
                BoundaryEdge::EllipticArc(ea_edge) => {
                    let (start_parameter, end_parameter) = if ea_edge.counter_clockwise {
                        (ea_edge.start_angle, ea_edge.end_angle)
                    } else {
                        // Clockwise HATCH edges store mirrored parameters.
                        // Reverse the geometric endpoints for CCW ELLIPSE.
                        (-ea_edge.end_angle, -ea_edge.start_angle)
                    };
                    result.push(EntityType::Ellipse(Ellipse {
                        common: inherit_common(common),
                        center: Vector3::new(ea_edge.center.x, ea_edge.center.y, elevation),
                        major_axis: Vector3::new(
                            ea_edge.major_axis_endpoint.x,
                            ea_edge.major_axis_endpoint.y,
                            0.0,
                        ),
                        minor_axis_ratio: ea_edge.minor_axis_ratio,
                        start_parameter,
                        end_parameter,
                        normal,
                    }));
                }
                BoundaryEdge::Spline(sp_edge) => {
                    let control_points: Vec<Vector3> = sp_edge
                        .control_points
                        .iter()
                        .map(|cp| Vector3::new(cp.x, cp.y, elevation))
                        .collect();
                    if control_points.len() >= 2 {
                        let mut spline =
                            Spline::from_control_points(sp_edge.degree, control_points);
                        spline.common = inherit_common(common);
                        spline.flags.rational = sp_edge.rational;
                        spline.flags.periodic = sp_edge.periodic;
                        spline.knots = sp_edge.knots.clone();
                        result.push(EntityType::Spline(spline));
                    }
                }
                BoundaryEdge::Polyline(pl_edge) => {
                    // Each vertex has (x, y, bulge) stored in Vector3.
                    let verts = &pl_edge.vertices;
                    let n = verts.len();
                    let seg_count = if pl_edge.is_closed {
                        n
                    } else {
                        n.saturating_sub(1)
                    };
                    for i in 0..seg_count {
                        let v1 = &verts[i];
                        let v2 = &verts[(i + 1) % n];
                        let bulge = v1.z; // bulge stored in z component
                        if bulge.abs() < 1e-10 {
                            result.push(line_entity(
                                Vector3::new(v1.x, v1.y, elevation),
                                Vector3::new(v2.x, v2.y, elevation),
                                0.0,
                                normal,
                                common,
                            ));
                        } else {
                            result.push(arc_from_bulge(
                                v1.x, v1.y, v2.x, v2.y, bulge, elevation, 0.0, normal, common,
                            ));
                        }
                    }
                }
            }
        }
    }

    result
}

fn explode_mesh(mesh: &Mesh) -> Vec<EntityType> {
    // Decompose Mesh into Face3D entities (one per face).
    let common = &mesh.common;
    let mut result = Vec::with_capacity(mesh.faces.len());

    for face in &mesh.faces {
        let n = face.vertices.len();
        if n < 3 {
            continue;
        }
        let v = &face.vertices;
        let get =
            |idx: usize| -> Vector3 { mesh.vertices.get(idx).copied().unwrap_or(Vector3::ZERO) };

        if n == 3 {
            result.push(EntityType::Face3D(Face3D::triangle(
                get(v[0]),
                get(v[1]),
                get(v[2]),
            )));
        } else {
            // For quads and n-gons, fan-triangulate from vertex 0.
            for i in 1..n - 1 {
                let mut face3d = Face3D::triangle(get(v[0]), get(v[i]), get(v[i + 1]));
                face3d.common = inherit_common(common);
                result.push(EntityType::Face3D(face3d));
            }
            continue;
        }

        // Set common for properly constructed faces.
        if let Some(EntityType::Face3D(ref mut f)) = result.last_mut() {
            f.common = inherit_common(common);
        }
    }

    result
}

/// AutoCAD emits one 3DFACE for each mesh face, retaining quads instead of
/// introducing a diagonal that was not present in the source mesh.
fn explode_mesh_autocad(mesh: &Mesh) -> Vec<EntityType> {
    let mut result = Vec::with_capacity(mesh.faces.len());
    for face in &mesh.faces {
        let indices = &face.vertices;
        if !(3..=4).contains(&indices.len())
            || indices.iter().any(|&index| index >= mesh.vertices.len())
        {
            continue;
        }
        let points: Vec<Vector3> = indices.iter().map(|&index| mesh.vertices[index]).collect();
        let mut output = if points.len() == 3 {
            Face3D::triangle(points[0], points[1], points[2])
        } else {
            Face3D::new(points[0], points[1], points[2], points[3])
        };
        output.common = inherit_common(&mesh.common);
        result.push(EntityType::Face3D(output));
    }
    result
}

fn explode_polyface_mesh(mesh: &PolyfaceMesh) -> Vec<EntityType> {
    // Each face references vertices by 1-based index.
    let common = &mesh.common;
    let mut result = Vec::with_capacity(mesh.faces.len());

    let get_vertex = |idx: i16| -> Vector3 {
        let abs_idx = idx.unsigned_abs() as usize;
        if abs_idx == 0 || abs_idx > mesh.vertices.len() {
            Vector3::ZERO
        } else {
            mesh.vertices[abs_idx - 1].location
        }
    };

    for face in &mesh.faces {
        let p1 = get_vertex(face.index1);
        let p2 = get_vertex(face.index2);
        let p3 = get_vertex(face.index3);

        let mut face3d = if face.is_triangle() {
            Face3D::triangle(p1, p2, p3)
        } else {
            let p4 = get_vertex(face.index4);
            Face3D::new(p1, p2, p3, p4)
        };

        // Mark invisible edges from face index signs.
        if face.index1 < 0 {
            face3d.invisible_edges.set_first_invisible(true);
        }
        if face.index2 < 0 {
            face3d.invisible_edges.set_second_invisible(true);
        }
        if face.index3 < 0 {
            face3d.invisible_edges.set_third_invisible(true);
        }
        if face.index4 < 0 {
            face3d.invisible_edges.set_fourth_invisible(true);
        }

        face3d.common = inherit_common(common);
        result.push(EntityType::Face3D(face3d));
    }

    result
}

fn explode_polyface_mesh_autocad(mesh: &PolyfaceMesh) -> Vec<EntityType> {
    let get_vertex = |index: i16| -> Option<Vector3> {
        let index = index.unsigned_abs() as usize;
        (index > 0 && index <= mesh.vertices.len()).then(|| mesh.vertices[index - 1].location)
    };
    let mut result = Vec::new();
    for face in &mesh.faces {
        let indices = face.vertex_indices();
        let points: Vec<Vector3> = indices
            .iter()
            .filter_map(|&index| get_vertex(index))
            .collect();
        if points.len() != indices.len() {
            continue;
        }
        match points.as_slice() {
            [] => {}
            [point] => result.push(EntityType::Point(Point {
                common: inherit_common(&face.common),
                location: *point,
                ..Point::new()
            })),
            [start, end] => result.push(EntityType::Line(Line {
                common: inherit_common(&face.common),
                start: *start,
                end: *end,
                ..Line::new()
            })),
            [a, b, c] => {
                let mut output = Face3D::triangle(*a, *b, *c);
                output.common = inherit_common(&face.common);
                output.invisible_edges.set_first_invisible(face.index1 < 0);
                output.invisible_edges.set_second_invisible(face.index2 < 0);
                output.invisible_edges.set_third_invisible(face.index3 < 0);
                if let Some(color) = face.color {
                    output.common.color = color;
                }
                result.push(EntityType::Face3D(output));
            }
            [a, b, c, d] => {
                let mut output = Face3D::new(*a, *b, *c, *d);
                output.common = inherit_common(&face.common);
                output.invisible_edges.set_first_invisible(face.index1 < 0);
                output.invisible_edges.set_second_invisible(face.index2 < 0);
                output.invisible_edges.set_third_invisible(face.index3 < 0);
                output.invisible_edges.set_fourth_invisible(face.index4 < 0);
                if let Some(color) = face.color {
                    output.common.color = color;
                }
                result.push(EntityType::Face3D(output));
            }
            _ => {}
        }
    }
    result
}

fn explode_polygon_mesh(mesh: &PolygonMeshEntity) -> Vec<EntityType> {
    // Decompose a polygon mesh grid (M × N) into Face3D entities.
    let common = &mesh.common;
    let m = mesh.m_vertex_count as usize;
    let n = mesh.n_vertex_count as usize;
    if m < 2 || n < 2 || mesh.vertices.len() < m * n {
        return Vec::new();
    }

    let get = |mi: usize, ni: usize| -> Vector3 { mesh.vertices[mi * n + ni].location };

    let mut result = Vec::with_capacity((m - 1) * (n - 1));
    for i in 0..m - 1 {
        for j in 0..n - 1 {
            let mut face = Face3D::new(get(i, j), get(i, j + 1), get(i + 1, j + 1), get(i + 1, j));
            face.common = inherit_common(common);
            result.push(EntityType::Face3D(face));
        }
    }

    result
}

fn explode_polygon_mesh_autocad(mesh: &PolygonMeshEntity) -> Vec<EntityType> {
    let Ok(m) = usize::try_from(mesh.m_vertex_count) else {
        return Vec::new();
    };
    let Ok(n) = usize::try_from(mesh.n_vertex_count) else {
        return Vec::new();
    };
    if m < 2
        || n < 2
        || m.checked_mul(n)
            .is_none_or(|count| mesh.vertices.len() < count)
    {
        return Vec::new();
    }
    let close_m = mesh.flags.contains(PolygonMeshFlags::CLOSED_M);
    let close_n = mesh.flags.contains(PolygonMeshFlags::CLOSED_N);
    let m_cells = if close_m { m } else { m - 1 };
    let n_cells = if close_n { n } else { n - 1 };
    let get = |i: usize, j: usize| -> Vector3 { mesh.vertices[(i % m) * n + (j % n)].location };
    let mut result = Vec::with_capacity(m_cells.saturating_mul(n_cells));
    for i in 0..m_cells {
        for j in 0..n_cells {
            let mut face = Face3D::new(get(i, j), get(i, j + 1), get(i + 1, j + 1), get(i + 1, j));
            face.common = inherit_common(&mesh.common);
            result.push(EntityType::Face3D(face));
        }
    }
    result
}

// ============================================================================
// EntityType::explode
// ============================================================================

impl EntityType {
    /// Explode this entity using AutoCAD's standalone-object rules.
    ///
    /// Primitive curves and faces that AutoCAD refuses to explode return an
    /// empty vector. Legacy exporters can use flatten instead.
    pub fn explode(&self) -> Vec<EntityType> {
        match self {
            EntityType::Helix(helix) => {
                let mut spline = helix.spline.clone();
                spline.common = inherit_common(&helix.common);
                vec![EntityType::Spline(spline)]
            }
            EntityType::MText(e) => explode_mtext(e),
            EntityType::LwPolyline(e) => explode_lwpolyline(e),
            EntityType::Polyline2D(e) => explode_polyline2d(e),
            EntityType::Polyline(e) => explode_polyline3d(e),
            EntityType::Polyline3D(e) => explode_polyline3d_new(e),
            EntityType::Dimension(e) => explode_dimension(e),
            EntityType::Leader(e) => explode_leader(e),
            EntityType::MultiLeader(e) => explode_multileader(e),
            EntityType::MLine(e) => explode_mline(e),
            EntityType::Table(e) => explode_table(e),
            EntityType::Wipeout(e) => explode_wipeout(e),
            EntityType::Hatch(e) => super::explode_hatch::explode_hatch(e),
            EntityType::Mesh(e) => explode_mesh_autocad(e),
            EntityType::PolyfaceMesh(e) => explode_polyface_mesh_autocad(e),
            EntityType::PolygonMesh(e) => explode_polygon_mesh_autocad(e),
            EntityType::Circle(_)
            | EntityType::Arc(_)
            | EntityType::Ellipse(_)
            | EntityType::Spline(_)
            | EntityType::Solid(_)
            | EntityType::Face3D(_)
            | EntityType::Point(_)
            | EntityType::Line(_)
            | EntityType::Text(_)
            | EntityType::Ray(_)
            | EntityType::XLine(_)
            | EntityType::Viewport(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_)
            | EntityType::Insert(_)
            | EntityType::AttributeDefinition(_)
            | EntityType::AttributeEntity(_)
            | EntityType::RasterImage(_)
            | EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_)
            | EntityType::Tolerance(_)
            | EntityType::Shape(_)
            | EntityType::Underlay(_)
            | EntityType::Ole2Frame(_)
            | EntityType::Light(_)
            | EntityType::SectionSymbol(_)
            | EntityType::ViewBorder(_)
            | EntityType::Extended(_)
            | EntityType::Unknown(_) => Vec::new(),
        }
    }

    /// Flatten this entity into the library's historical primitive geometry.
    /// This is retained for format conversion such as R12.
    pub fn flatten(&self) -> Vec<EntityType> {
        match self {
            // Complex entities that decompose.
            EntityType::Circle(e) => explode_circle(e),
            EntityType::Ellipse(e) => explode_ellipse(e),
            EntityType::LwPolyline(e) => explode_lwpolyline(e),
            EntityType::Polyline2D(e) => explode_polyline2d(e),
            EntityType::Polyline(e) => explode_polyline3d(e),
            EntityType::Polyline3D(e) => explode_polyline3d_new(e),
            EntityType::Solid(e) => explode_solid(e),
            EntityType::Face3D(e) => explode_face3d(e),
            EntityType::Spline(e) => explode_spline(e),
            EntityType::Helix(e) => explode_spline(&e.spline),
            EntityType::MText(e) => explode_mtext(e),
            EntityType::Dimension(e) => explode_dimension(e),
            EntityType::Leader(e) => flatten_leader(e),
            EntityType::MultiLeader(e) => explode_multileader(e),
            EntityType::MLine(e) => explode_mline(e),
            EntityType::Hatch(e) => explode_hatch(e),
            EntityType::Mesh(e) => explode_mesh(e),
            EntityType::PolyfaceMesh(e) => explode_polyface_mesh(e),
            EntityType::PolygonMesh(e) => explode_polygon_mesh(e),

            // Atomic / non-decomposable entities.
            EntityType::Point(_)
            | EntityType::Line(_)
            | EntityType::Arc(_)
            | EntityType::Text(_)
            | EntityType::Ray(_)
            | EntityType::XLine(_)
            | EntityType::Viewport(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_)
            | EntityType::Insert(_)
            | EntityType::AttributeDefinition(_)
            | EntityType::AttributeEntity(_)
            | EntityType::RasterImage(_)
            | EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_)
            | EntityType::Table(_)
            | EntityType::Tolerance(_)
            | EntityType::Wipeout(_)
            | EntityType::Shape(_)
            | EntityType::Underlay(_)
            | EntityType::Ole2Frame(_)
            | EntityType::Light(_)
            | EntityType::SectionSymbol(_)
            | EntityType::ViewBorder(_)
            | EntityType::Extended(_)
            | EntityType::Unknown(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Vector2;

    #[test]
    fn test_atomic_entities_return_empty() {
        let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 0.0));
        assert!(line.flatten().is_empty());

        let arc = EntityType::Arc(Arc::new());
        assert!(arc.flatten().is_empty());

        let point = EntityType::Point(Point::new());
        assert!(point.flatten().is_empty());

        let text = EntityType::Text(Text::new());
        assert!(text.flatten().is_empty());
    }

    #[test]
    fn test_explode_circle() {
        let circle = Circle::from_center_radius(Vector3::new(5.0, 5.0, 0.0), 10.0);
        let entity = EntityType::Circle(circle);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            EntityType::Arc(arc) => {
                assert_eq!(arc.center, Vector3::new(5.0, 5.0, 0.0));
                assert!((arc.radius - 10.0).abs() < 1e-10);
                assert!((arc.start_angle - 0.0).abs() < 1e-10);
                assert!((arc.end_angle - 2.0 * PI).abs() < 1e-10);
            }
            _ => panic!("Expected Arc from circle explosion"),
        }
    }

    #[test]
    fn test_explode_lwpolyline_lines_only() {
        let mut pl = LwPolyline::new();
        pl.add_point(Vector2::new(0.0, 0.0));
        pl.add_point(Vector2::new(10.0, 0.0));
        pl.add_point(Vector2::new(10.0, 10.0));

        let entity = EntityType::LwPolyline(pl);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], EntityType::Line(_)));
        assert!(matches!(&parts[1], EntityType::Line(_)));
    }

    #[test]
    fn test_explode_lwpolyline_closed() {
        let mut pl = LwPolyline::new();
        pl.add_point(Vector2::new(0.0, 0.0));
        pl.add_point(Vector2::new(10.0, 0.0));
        pl.add_point(Vector2::new(10.0, 10.0));
        pl.close();

        let entity = EntityType::LwPolyline(pl);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 3); // 3 segments for closed triangle
    }

    #[test]
    fn test_explode_lwpolyline_with_bulge() {
        let mut pl = LwPolyline::new();
        pl.add_point_with_bulge(Vector2::new(0.0, 0.0), 1.0); // semicircle
        pl.add_point(Vector2::new(10.0, 0.0));

        let entity = EntityType::LwPolyline(pl);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Arc(_)));
    }

    #[test]
    fn test_explode_planar_lines_convert_ocs_to_wcs() {
        let normal = Vector3::UNIT_Y;
        let to_wcs = Matrix3::arbitrary_axis(normal);
        let start = Vector3::new(1.0, 2.0, 3.0);
        let end = Vector3::new(4.0, 5.0, 3.0);

        let mut lightweight = LwPolyline::new();
        lightweight.normal = normal;
        lightweight.elevation = 3.0;
        lightweight.add_point(Vector2::new(start.x, start.y));
        lightweight.add_point(Vector2::new(end.x, end.y));
        let mut heavy = Polyline2D::new();
        heavy.normal = normal;
        heavy.add_vertex(Vertex2D::new(start));
        heavy.add_vertex(Vertex2D::new(end));
        let solid = Solid::triangle(start, end, Vector3::new(7.0, 8.0, 3.0)).with_normal(normal);
        for entity in [
            EntityType::LwPolyline(lightweight),
            EntityType::Polyline2D(heavy),
            EntityType::Solid(solid),
        ] {
            let parts = entity.flatten();
            let EntityType::Line(line) = &parts[0] else {
                panic!("expected a WCS line");
            };
            assert_eq!(line.start, to_wcs * start);
            assert_eq!(line.end, to_wcs * end);
        }
    }

    #[test]
    fn test_explode_hatch_clockwise_elliptic_edge_uses_geometric_parameters() {
        let mut path = BoundaryPath::new();
        path.edges.push(BoundaryEdge::EllipticArc(
            crate::entities::hatch::EllipticArcEdge {
                center: Vector2::ZERO,
                major_axis_endpoint: Vector2::new(4.0, 0.0),
                minor_axis_ratio: 0.5,
                start_angle: 0.3,
                end_angle: 1.2,
                counter_clockwise: false,
            },
        ));
        let mut hatch = Hatch::new();
        hatch.paths.push(path);
        let parts = EntityType::Hatch(Box::new(hatch)).flatten();
        let EntityType::Ellipse(ellipse) = &parts[0] else {
            panic!("expected ellipse");
        };
        let point = |parameter: f64| {
            ellipse.center
                + ellipse.major_axis * parameter.cos()
                + ellipse.normal.cross(&ellipse.major_axis)
                    * (ellipse.minor_axis_ratio * parameter.sin())
        };
        let expected_start = Vector3::new(4.0 * 1.2f64.cos(), -2.0 * 1.2f64.sin(), 0.0);
        let expected_end = Vector3::new(4.0 * 0.3f64.cos(), -2.0 * 0.3f64.sin(), 0.0);
        assert!(point(ellipse.start_parameter).distance(&expected_start) < 1e-12);
        assert!(point(ellipse.end_parameter).distance(&expected_end) < 1e-12);
        assert!((ellipse.end_parameter - ellipse.start_parameter - 0.9).abs() < 1e-12);
    }

    #[test]
    fn test_explode_solid_quad() {
        let first = Vector3::new(0.0, 0.0, 0.0);
        let second = Vector3::new(10.0, 0.0, 0.0);
        let third = Vector3::new(0.0, 10.0, 0.0);
        let fourth = Vector3::new(10.0, 10.0, 0.0);
        let solid = Solid::new(first, second, third, fourth);
        let entity = EntityType::Solid(solid);
        let parts = entity.flatten();
        let expected = [
            (first, second),
            (second, fourth),
            (fourth, third),
            (third, first),
        ];
        assert_eq!(parts.len(), expected.len());
        for (part, (start, end)) in parts.iter().zip(expected) {
            let EntityType::Line(line) = part else {
                panic!("expected line")
            };
            assert_eq!((line.start, line.end), (start, end));
        }
    }

    #[test]
    fn test_explode_solid_triangle() {
        let solid = Solid::triangle(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 0.0),
        );
        let entity = EntityType::Solid(solid);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 3); // 3 edge lines
    }

    #[test]
    fn test_explode_face3d() {
        let face = Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(10.0, 10.0, 0.0),
            Vector3::new(0.0, 10.0, 0.0),
        );
        let entity = EntityType::Face3D(face);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 4); // All edges visible
    }

    #[test]
    fn test_explode_face3d_invisible_edges() {
        let mut face = Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(10.0, 10.0, 0.0),
            Vector3::new(0.0, 10.0, 0.0),
        );
        face.invisible_edges.set_first_invisible(true);
        face.invisible_edges.set_third_invisible(true);

        let entity = EntityType::Face3D(face);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 2); // Only 2 visible edges
    }

    #[test]
    fn test_explode_polyline3d() {
        let pl = Polyline::from_points(vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(10.0, 10.0, 0.0),
        ]);
        let entity = EntityType::Polyline(pl);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], EntityType::Line(_)));
    }

    #[test]
    fn test_explode_leader() {
        let mut leader = Leader::new();
        leader.vertices.push(Vector3::new(0.0, 0.0, 0.0));
        leader.vertices.push(Vector3::new(10.0, 10.0, 0.0));
        leader.vertices.push(Vector3::new(20.0, 10.0, 0.0));

        let entity = EntityType::Leader(leader);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], EntityType::Line(_)));
    }

    /// A callout the way AutoCAD stores it: the leader line holds only the
    /// arrow tip, the root holds the bend (connection point) and the landing.
    fn callout(tips: &[Vector3]) -> MultiLeader {
        let mut ml = MultiLeader::new();
        ml.context.arrowhead_size = 1.0;
        let mut root = LeaderRoot::new(0);
        root.connection_point = Vector3::new(10.0, 10.0, 0.0);
        root.direction = Vector3::new(2.0, 0.0, 0.0);
        root.landing_distance = 5.0;

        for (index, tip) in tips.iter().enumerate() {
            let mut line = LeaderLine::new(index as i32);
            line.points.push(*tip);
            root.lines.push(line);
        }

        ml.context.leader_roots.push(root);
        ml
    }

    fn line_ends(parts: &[EntityType]) -> Vec<(Vector3, Vector3)> {
        parts
            .iter()
            .filter_map(|part| match part {
                EntityType::Line(line) => Some((line.start, line.end)),
                _ => None,
            })
            .collect()
    }

    fn solids(parts: &[EntityType]) -> Vec<&Solid> {
        parts
            .iter()
            .filter_map(|part| match part {
                EntityType::Solid(solid) => Some(solid),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_explode_multileader_single_vertex_line() {
        let parts = EntityType::MultiLeader(Box::new(callout(&[Vector3::ZERO]))).flatten();
        let arrows = solids(&parts);
        let lines = line_ends(&parts);

        // The arrowhead: tip at the first point, one unit long, 1/3 wide.
        assert_eq!(arrows.len(), 1);
        let base = Vector3::new(1.0, 1.0, 0.0) * (1.0 / 2f64.sqrt());
        assert_eq!(arrows[0].first_corner, Vector3::ZERO);
        let half_width = arrows[0].second_corner.distance(&arrows[0].third_corner) / 2.0;
        assert!((half_width - 1.0 / 6.0).abs() < 1e-9);
        assert!(((arrows[0].second_corner + arrows[0].third_corner) * 0.5).distance(&base) < 1e-9);

        // The shaft starts at the arrow base and reaches the connection
        // point; the landing follows the normalized direction.
        assert_eq!(lines.len(), 2);
        assert!(lines[0].0.distance(&base) < 1e-9);
        assert_eq!(lines[0].1, Vector3::new(10.0, 10.0, 0.0));
        assert_eq!(
            lines[1],
            (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0))
        );
    }

    #[test]
    fn test_explode_multileader_does_not_repeat_the_connection_point() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.context.arrowhead_size = 0.0;
        ml.context.leader_roots[0].lines[0]
            .points
            .extend([Vector3::new(5.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)]);

        let lines = line_ends(&EntityType::MultiLeader(Box::new(ml)).flatten());

        assert_eq!(
            lines,
            vec![
                (Vector3::ZERO, Vector3::new(5.0, 0.0, 0.0)),
                (Vector3::new(5.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)),
                (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0)),
            ]
        );
    }

    #[test]
    fn test_explode_multileader_lines_share_one_landing() {
        let ml = callout(&[Vector3::ZERO, Vector3::new(20.0, 0.0, 0.0)]);
        let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

        // One arrowhead per leader line, never one on the landing.
        assert_eq!(solids(&parts).len(), 2);
        let landings = line_ends(&parts)
            .into_iter()
            .filter(|(start, _)| *start == Vector3::new(10.0, 10.0, 0.0))
            .count();
        assert_eq!(landings, 1);
    }

    #[test]
    fn test_explode_multileader_each_root_draws_its_own_path() {
        let mut ml = callout(&[Vector3::ZERO]);
        let mut second = ml.context.leader_roots[0].clone();
        second.connection_point = Vector3::new(30.0, 10.0, 0.0);
        ml.context.leader_roots.push(second);

        let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

        assert_eq!(solids(&parts).len(), 2);
        assert_eq!(line_ends(&parts).len(), 4);
    }

    #[test]
    fn test_explode_multileader_invisible_path_keeps_only_the_text() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.path_type = MultiLeaderPathType::Invisible;
        ml.context.has_text_contents = true;
        ml.context.text_string = "NOTE".to_string();

        let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::MText(_)));
    }

    #[test]
    fn test_explode_multileader_without_dogleg_has_no_landing() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.enable_dogleg = false;

        let lines = line_ends(&EntityType::MultiLeader(Box::new(ml)).flatten());

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1, Vector3::new(10.0, 10.0, 0.0));
    }

    #[test]
    fn test_explode_multileader_custom_arrow_block_is_not_drawn_as_the_default() {
        let mut entity_level = callout(&[Vector3::ZERO]);
        entity_level.arrowhead_handle = Some(Handle::new(0x2A));

        let mut line_level = callout(&[Vector3::ZERO]);
        let line = &mut line_level.context.leader_roots[0].lines[0];
        line.arrowhead_handle = Some(Handle::new(0x2A));
        line.override_flags = LeaderLinePropertyOverrideFlags::ARROWHEAD;

        for ml in [entity_level, line_level] {
            let parts = EntityType::MultiLeader(Box::new(ml)).flatten();
            assert!(solids(&parts).is_empty());
            // The shaft is not trimmed when no arrowhead covers its start.
            assert_eq!(line_ends(&parts)[0].0, Vector3::ZERO);
        }
    }

    #[test]
    fn test_explode_multileader_null_arrow_handle_is_the_default_arrow() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.arrowhead_handle = Some(Handle::NULL);

        assert_eq!(
            solids(&EntityType::MultiLeader(Box::new(ml)).flatten()).len(),
            1
        );
    }

    #[test]
    fn test_explode_multileader_line_overrides_the_arrow_size() {
        let mut ml = callout(&[Vector3::ZERO]);
        let line = &mut ml.context.leader_roots[0].lines[0];
        line.arrowhead_size = 2.0;
        line.override_flags = LeaderLinePropertyOverrideFlags::ARROWHEAD_SIZE;

        let parts = EntityType::MultiLeader(Box::new(ml)).flatten();
        let base = (solids(&parts)[0].second_corner + solids(&parts)[0].third_corner) * 0.5;

        assert!((base.length() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_explode_multileader_arrow_longer_than_its_segment_is_dropped() {
        let mut ml = callout(&[Vector3::new(9.5, 10.0, 0.0)]);
        ml.context.arrowhead_size = 1.0;

        let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

        assert!(solids(&parts).is_empty());
        assert_eq!(line_ends(&parts)[0].0, Vector3::new(9.5, 10.0, 0.0));
    }

    #[test]
    fn test_explode_mtext() {
        let mtext = MText::with_value("Hello World", Vector3::new(5.0, 5.0, 0.0));
        let entity = EntityType::MText(Box::new(mtext));
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            EntityType::Text(t) => {
                assert_eq!(t.value, "Hello World");
                // default attachment is top left: the baseline lies one text height below
                assert_eq!(t.insertion_point, Vector3::new(5.0, 5.0 - t.height, 0.0));
            }
            _ => panic!("Expected Text from MText explosion"),
        }
    }

    #[test]
    fn test_explode_mtext_paragraphs_and_formatting() {
        let mut mtext =
            MText::with_value("{\\fArial|b1;Line 1}\\PLine 2", Vector3::new(0.0, 0.0, 0.0));
        mtext.height = 2.0;
        let parts = EntityType::MText(Box::new(mtext)).flatten();
        let values: Vec<&str> = parts
            .iter()
            .map(|p| match p {
                EntityType::Text(t) => t.value.as_str(),
                _ => panic!("Expected Text"),
            })
            .collect();
        assert_eq!(values, ["Line 1", "Line 2"]);
        // second line one line spacing (5/3 of the height) lower
        let y = |i: usize| match &parts[i] {
            EntityType::Text(t) => t.insertion_point.y,
            _ => unreachable!(),
        };
        assert!((y(0) - y(1) - 2.0 * 5.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_explode_mesh_triangles() {
        let mesh = Mesh::from_triangles(
            vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(5.0, 10.0, 0.0),
            ],
            &[(0, 1, 2)],
        );
        let entity = EntityType::Mesh(mesh);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Face3D(_)));
    }

    #[test]
    fn test_explode_ellipse() {
        let ellipse = Ellipse::from_center_axes(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0), 0.5);
        let entity = EntityType::Ellipse(ellipse);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Spline(_)));
    }

    #[test]
    fn test_explode_inherits_common_properties() {
        let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
        circle.common.layer = "WALLS".to_string();
        circle.common.color = Color::Index(1);
        circle.common.linetype = "DASHED".to_string();

        let entity = EntityType::Circle(circle);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        let common = parts[0].common();
        assert_eq!(common.layer, "WALLS");
        assert_eq!(common.color, Color::Index(1));
        assert_eq!(common.linetype, "DASHED");
        assert!(common.handle.is_null()); // handle not assigned yet
    }

    #[test]
    fn test_explode_spline() {
        let pts = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(3.0, 10.0, 0.0),
            Vector3::new(7.0, 10.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
        ];
        let spline = Spline::from_control_points(3, pts);
        let entity = EntityType::Spline(spline);
        let parts = entity.flatten();
        // one clamped cubic span sampled with 16 segments
        assert_eq!(parts.len(), 16);
        let ends: Vec<(Vector3, Vector3)> = parts
            .iter()
            .map(|p| match p {
                EntityType::Line(l) => (l.start, l.end),
                _ => panic!("Expected Line"),
            })
            .collect();
        // the curve, not the control polygon: starts/ends at the end points
        // and passes through the Bezier midpoint (5, 7.5)
        assert_eq!(ends[0].0, Vector3::new(0.0, 0.0, 0.0));
        assert!((ends[15].1 - Vector3::new(10.0, 0.0, 0.0)).length() < 1e-9);
        assert!((ends[7].1 - Vector3::new(5.0, 7.5, 0.0)).length() < 1e-9);
    }

    #[test]
    fn test_nurbs_rational_quarter_circle() {
        // quadratic rational Bezier with weights 1, sqrt(2)/2, 1 is an exact quarter circle
        let pts = [
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];
        let w = [1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0];
        let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        for p in nurbs_points(2, &pts, &w, &knots, 8) {
            assert!((p.length() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn test_explode_dimension() {
        let dim = DimensionLinear::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
        let entity = EntityType::Dimension(Box::new(Dimension::Linear(dim)));
        let parts = entity.flatten();
        // Should contain dimension lines + text
        assert!(parts.len() >= 2);
        assert!(parts.iter().any(|p| matches!(p, EntityType::MText(_))));
        assert!(parts.iter().any(|p| matches!(p, EntityType::Line(_))));
    }

    #[test]
    fn test_explode_linear_dimension_projects_onto_offset_line() {
        let mut dimension =
            DimensionLinear::horizontal(Vector3::new(1.0, 2.0, 0.0), Vector3::new(11.0, 4.0, 0.0));
        dimension.definition_point = Vector3::new(11.0, 8.0, 0.0);
        let parts = EntityType::Dimension(Box::new(Dimension::Linear(dimension))).flatten();
        assert_eq!(
            line_ends(&parts),
            vec![
                (Vector3::new(1.0, 2.0, 0.0), Vector3::new(1.0, 8.0, 0.0)),
                (Vector3::new(11.0, 4.0, 0.0), Vector3::new(11.0, 8.0, 0.0)),
                (Vector3::new(1.0, 8.0, 0.0), Vector3::new(11.0, 8.0, 0.0)),
            ]
        );
    }

    #[test]
    fn test_explode_aligned_dimension_projects_onto_offset_line() {
        let mut dimension = DimensionAligned::new(Vector3::ZERO, Vector3::new(3.0, 4.0, 0.0));
        dimension.definition_point = Vector3::new(-1.0, 7.0, 0.0);
        let parts = EntityType::Dimension(Box::new(Dimension::Aligned(dimension))).flatten();
        let lines = line_ends(&parts);
        assert!(lines[0].1.distance(&Vector3::new(-4.0, 3.0, 0.0)) < 1e-12);
        assert!(lines[1].1.distance(&Vector3::new(-1.0, 7.0, 0.0)) < 1e-12);
        assert_eq!(lines[2], (lines[0].1, lines[1].1));
    }

    #[test]
    fn test_explode_polyface_mesh() {
        let mut mesh = PolyfaceMesh::new();
        mesh.add_vertex(PolyfaceVertex::new(Vector3::new(0.0, 0.0, 0.0)));
        mesh.add_vertex(PolyfaceVertex::new(Vector3::new(10.0, 0.0, 0.0)));
        mesh.add_vertex(PolyfaceVertex::new(Vector3::new(5.0, 10.0, 0.0)));
        mesh.add_face(PolyfaceFace::triangle(1, 2, 3));

        let entity = EntityType::PolyfaceMesh(mesh);
        let parts = entity.flatten();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Face3D(_)));
    }

    #[test]
    fn test_document_explode_entity_assigns_handles() {
        use crate::document::CadDocument;

        let mut doc = CadDocument::new();
        let mut polyline = LwPolyline::new();
        polyline.add_point(Vector2::ZERO);
        polyline.add_point(Vector2::new(5.0, 0.0));
        polyline.common.owner_handle = Handle::new(0x100);

        let entity = EntityType::LwPolyline(polyline);
        let parts = doc.explode_entity(&entity);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].common().handle.is_valid());
        assert_eq!(parts[0].common().owner_handle, Handle::new(0x100));
    }
}
