//! Editable Architecture entity payload.
//!
//! The common entity fields (layer, color, handle) stay on the owning
//! [`crate::entities::ExtendedEntity`]. This struct holds the class identity,
//! the decoded profiles, and the original DWG or DXF body so a save reproduces
//! the record Architecture wrote.

use crate::classes::arch::geometry::{
    decode_profiles, find_straight_wall_fields, map_profile_points, write_bit_double_at,
    WALL_CORNER_BIAS,
};
use crate::types::{DxfVersion, Vector3};

/// Which Architecture entity a class name represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchKind {
    Wall,
    Door,
    Window,
    Opening,
    Stair,
    Railing,
    Slab,
    RoofSlab,
    Roof,
    Space,
    CurtainWall,
    CurtainWallUnit,
    WindowAssembly,
    ColumnGrid,
    CeilingGrid,
    Polygon,
    MassElement,
    MassGroup,
    MultiViewBlock,
    MaskBlock,
    Section,
    SectionLine,
    ElevationLine,
    Dimension,
    Schedule,
    LayoutCurve,
    LayoutGrid,
    Zone,
    Area,
    Camera,
    Slice,
    EntityReference,
    Point,
    DisplayTheme,
    StructuralMember,
    /// An Architecture entity this crate does not give a narrower name.
    Other,
}

/// Class body of an Architecture entity.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchEntityData {
    pub dxf_name: String,
    pub cpp_class_name: String,
    pub kind: ArchKind,
    /// Closed or open profiles in world coordinates. The first profile is the
    /// one Architecture stored in the proxy-graphics preview.
    pub profiles: Vec<Vec<Vector3>>,
    pub dwg_type_code: i16,
    pub dwg_handle_bits: i64,
    pub dwg_source_version: Option<DxfVersion>,
    pub raw_dwg_data: Option<Vec<u8>>,
    pub raw_dxf_codes: Option<Vec<(i32, String)>>,
}

impl ArchEntityData {
    pub fn from_parts(
        dxf_name: impl Into<String>,
        cpp_class_name: impl Into<String>,
        graphic_data: Option<&[u8]>,
    ) -> Self {
        let dxf_name = dxf_name.into();
        let kind = crate::classes::arch::kind_of(&dxf_name).unwrap_or(ArchKind::Other);
        let cpp_class_name = {
            let given = cpp_class_name.into();
            if given.is_empty() {
                crate::classes::arch::cpp_class_name(&dxf_name)
                    .unwrap_or("")
                    .to_string()
            } else {
                given
            }
        };
        Self {
            profiles: decode_profiles(graphic_data),
            dxf_name,
            cpp_class_name,
            kind,
            dwg_type_code: 0,
            dwg_handle_bits: 0,
            dwg_source_version: None,
            raw_dwg_data: None,
            raw_dxf_codes: None,
        }
    }

    /// Move every profile point by `delta`, in the decoded profiles and in the
    /// stored DWG record and preview.
    ///
    /// Returns false when a profile point is not present as raw bytes, leaving
    /// the entity unchanged.
    pub fn translate(&mut self, graphic_data: &mut Option<Vec<u8>>, delta: Vector3) -> bool {
        self.transform_points(graphic_data, |point| point + delta)
    }

    /// Apply `map` to every profile point and to the stored DWG and preview bytes.
    pub fn transform_points(
        &mut self,
        graphic_data: &mut Option<Vec<u8>>,
        map: impl FnMut(Vector3) -> Vector3,
    ) -> bool {
        self.map_points(graphic_data, map)
    }

    /// Replace the preview profile with `points`.
    ///
    /// `points` must have the same length as the current preview profile. Each
    /// old point is rewritten everywhere it occurs in the DWG record and the
    /// preview, which is how a straight wall becomes a new rectangle without
    /// dropping the style, endcaps, or display overrides stored around it.
    pub fn reshape(&mut self, graphic_data: &mut Option<Vec<u8>>, points: &[Vector3]) -> bool {
        let Some(current) = self.profiles.first() else {
            return false;
        };
        if current.len() != points.len() || points.is_empty() {
            return false;
        }
        let pairs: Vec<(Vector3, Vector3)> = current
            .iter()
            .copied()
            .zip(points.iter().copied())
            .collect();
        let ok = self.map_points(graphic_data, |point| {
            pairs
                .iter()
                .find(|(from, _)| *from == point)
                .map(|(_, to)| *to)
                .unwrap_or(point)
        });
        ok
    }

    fn map_points(
        &mut self,
        graphic_data: &mut Option<Vec<u8>>,
        map: impl FnMut(Vector3) -> Vector3,
    ) -> bool {
        if self.profiles.is_empty() {
            return false;
        }
        let before = self.profiles.clone();
        let mut map = map;
        let mapped: Vec<Vec<Vector3>> = before
            .iter()
            .map(|profile| profile.iter().copied().map(&mut map).collect())
            .collect();
        let mut raw = self.raw_dwg_data.clone().unwrap_or_default();
        let mut preview = graphic_data.clone().unwrap_or_default();
        let mut present = 0usize;
        if self.raw_dwg_data.is_some() {
            if !profiles_present(&raw, &before) {
                return false;
            }
            present += 1;
        }
        if graphic_data.is_some() {
            if !profiles_present(&preview, &before) {
                return false;
            }
            present += 1;
        }
        if present == 0 {
            return false;
        }
        let mut buffers: Vec<&mut [u8]> = Vec::new();
        if self.raw_dwg_data.is_some() {
            buffers.push(&mut raw);
        }
        if graphic_data.is_some() {
            buffers.push(&mut preview);
        }
        map_profile_points(&mut buffers, &before, |point| {
            mapped
                .iter()
                .zip(&before)
                .find_map(|(next, previous)| {
                    previous
                        .iter()
                        .zip(next.iter())
                        .find(|(from, _)| **from == point)
                        .map(|(_, to)| *to)
                })
                .unwrap_or(point)
        });
        if self.raw_dwg_data.is_some() {
            self.raw_dwg_data = Some(raw);
        }
        if graphic_data.is_some() {
            *graphic_data = Some(preview);
        }
        self.profiles = mapped;
        true
    }
}

fn profiles_present(buffer: &[u8], profiles: &[Vec<Vector3>]) -> bool {
    profiles.iter().flatten().all(|point| {
        let bytes = {
            let mut encoded = [0u8; 24];
            encoded[0..8].copy_from_slice(&point.x.to_le_bytes());
            encoded[8..16].copy_from_slice(&point.y.to_le_bytes());
            encoded[16..24].copy_from_slice(&point.z.to_le_bytes());
            encoded
        };
        buffer.windows(24).any(|window| window == bytes)
    })
}

/// Build a rectangular wall footprint from a baseline and a width.
///
/// The rectangle is closed (the first point is repeated). `width` is applied
/// to the left of the baseline direction.
pub fn rectangle_profile(start: Vector3, end: Vector3, width: f64) -> Option<Vec<Vector3>> {
    let edge = end - start;
    let length = edge.length();
    if length <= f64::EPSILON || !width.is_finite() || width.abs() <= f64::EPSILON {
        return None;
    }
    let normal = Vector3::new(-edge.y / length, edge.x / length, 0.0) * width;
    Some(vec![start, end, end + normal, start + normal, start])
}

/// Rewrite a 2027 straight wall onto a new baseline.
///
/// The wall's start, direction, perpendicular, length, width, and height are
/// full bit-doubles. Zero components stay full doubles so a later edit does
/// not change the record size. The preview becomes the axis-aligned bounds of
/// the center-justified wall, which is the outline Architecture stores.
pub fn set_straight_wall(
    data: &mut ArchEntityData,
    graphic_data: &mut Option<Vec<u8>>,
    start: Vector3,
    end: Vector3,
    width: f64,
    height: f64,
) -> bool {
    if data.kind != ArchKind::Wall {
        return false;
    }
    let Some(current) = data.profiles.first().cloned() else {
        return false;
    };
    if current.len() != 5 || current.first() != current.last() {
        return false;
    }
    let edge = end - start;
    let length = edge.length();
    if length <= f64::EPSILON || !width.is_finite() || width <= f64::EPSILON || !height.is_finite()
    {
        return false;
    }
    let along = edge * (1.0 / length);
    let Some(points) = centered_wall_profile(start, end, width) else {
        return false;
    };
    let Some(raw) = data.raw_dwg_data.as_ref() else {
        return false;
    };
    let Some(fields) = find_straight_wall_fields(raw) else {
        return false;
    };
    let old_corner = current[0];
    if !data.reshape(graphic_data, &points) {
        return false;
    }
    let Some(raw) = data.raw_dwg_data.as_mut() else {
        return false;
    };
    replace_point(raw, old_corner + corner_bias(), points[0] + corner_bias());
    if let Some(preview) = graphic_data.as_mut() {
        replace_point(
            preview,
            old_corner + corner_bias(),
            points[0] + corner_bias(),
        );
    }
    write_bit_double_at(raw, fields.start_x, start.x)
        && write_bit_double_at(raw, fields.start_y, start.y)
        && write_bit_double_at(raw, fields.dir_x, along.x)
        && write_bit_double_at(raw, fields.dir_y, along.y)
        && write_bit_double_at(raw, fields.normal_x, -along.y)
        && write_bit_double_at(raw, fields.normal_y, along.x)
        && write_bit_double_at(raw, fields.height, height)
        && write_bit_double_at(raw, fields.width, width)
        && write_bit_double_at(raw, fields.length, length)
}

fn corner_bias() -> Vector3 {
    Vector3::new(WALL_CORNER_BIAS, WALL_CORNER_BIAS, 0.0)
}

fn replace_point(buffer: &mut [u8], from: Vector3, to: Vector3) {
    let from = point_bytes(from);
    let to = point_bytes(to);
    let mut offset = 0usize;
    while offset + 24 <= buffer.len() {
        if buffer[offset..offset + 24] == from {
            buffer[offset..offset + 24].copy_from_slice(&to);
            offset += 24;
        } else {
            offset += 1;
        }
    }
}

fn point_bytes(point: Vector3) -> [u8; 24] {
    let mut bytes = [0u8; 24];
    bytes[0..8].copy_from_slice(&point.x.to_le_bytes());
    bytes[8..16].copy_from_slice(&point.y.to_le_bytes());
    bytes[16..24].copy_from_slice(&point.z.to_le_bytes());
    bytes
}

/// Closed preview of a center-justified straight wall.
///
/// Architecture stores the wall's axis-aligned bounds, counterclockwise from
/// the lower-left corner.
pub fn centered_wall_profile(start: Vector3, end: Vector3, width: f64) -> Option<Vec<Vector3>> {
    let edge = end - start;
    let length = edge.length();
    if length <= f64::EPSILON || !width.is_finite() || width <= f64::EPSILON {
        return None;
    }
    let along = edge * (1.0 / length);
    let left = Vector3::new(-along.y, along.x, 0.0) * (width * 0.5);
    let corners = [start - left, end - left, end + left, start + left];
    let mut min = corners[0];
    let mut max = corners[0];
    for corner in corners.into_iter().skip(1) {
        min.x = min.x.min(corner.x);
        min.y = min.y.min(corner.y);
        max.x = max.x.max(corner.x);
        max.y = max.y.max(corner.y);
    }
    Some(vec![
        Vector3::new(min.x, min.y, start.z),
        Vector3::new(max.x, min.y, start.z),
        Vector3::new(max.x, max.y, start.z),
        Vector3::new(min.x, max.y, start.z),
        Vector3::new(min.x, min.y, start.z),
    ])
}

/// Reshape the first profile to a rectangle when it has the same point count.
pub fn reshape_rectangle(
    data: &mut ArchEntityData,
    graphic_data: &mut Option<Vec<u8>>,
    start: Vector3,
    end: Vector3,
    width: f64,
) -> bool {
    let Some(points) = rectangle_profile(start, end, width) else {
        return false;
    };
    data.reshape(graphic_data, &points)
}
