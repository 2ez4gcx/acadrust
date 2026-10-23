//! Profile geometry stored inside Architecture entities.
//!
//! A profile is a run of world points. AutoCAD stores each run as a count
//! followed by little-endian XYZ doubles, both in the class record and in the
//! entity's proxy-graphics preview (record type 6). Editing replaces every
//! exact copy of those 24-byte points so the class data and the preview move
//! together.

use crate::types::Vector3;

const POINT_BYTES: usize = 24;
const MAX_POINTS: usize = 100_000;

/// Decode a type-6 profile payload: `u32` count, then `count` XYZ doubles.
pub fn polyline_from_type6(data: &[u8]) -> Option<Vec<Vector3>> {
    if data.len() < 4 {
        return None;
    }
    let count = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    if count < 2 || count > MAX_POINTS {
        return None;
    }
    let needed = 4usize.checked_add(count.checked_mul(POINT_BYTES)?)?;
    if needed > data.len() {
        return None;
    }
    let mut points = Vec::with_capacity(count);
    for index in 0..count {
        let offset = 4 + index * POINT_BYTES;
        let point = read_point(data, offset)?;
        if !point_is_plausible(point) {
            return None;
        }
        points.push(point);
    }
    Some(points)
}

/// Profiles carried by an entity's proxy-graphics blob.
pub fn decode_profiles(graphic_data: Option<&[u8]>) -> Vec<Vec<Vector3>> {
    let Some(graphic_data) = graphic_data else {
        return Vec::new();
    };
    let Some(graphics) = crate::entities::ProxyGraphics::decode(graphic_data) else {
        return Vec::new();
    };
    graphics
        .records
        .into_iter()
        .filter_map(|record| match record {
            crate::entities::ProxyGraphicRecord::Unknown {
                record_type: 6,
                data,
            } => polyline_from_type6(&data),
            _ => None,
        })
        .collect()
}

/// Replace every stored copy of `profiles` with `map(point)`.
///
/// Returns the number of points rewritten. A profile point that does not occur
/// verbatim is left unchanged and is not counted.
pub fn map_profile_points(
    buffers: &mut [&mut [u8]],
    profiles: &[Vec<Vector3>],
    mut map: impl FnMut(Vector3) -> Vector3,
) -> usize {
    let mut replacements = Vec::new();
    for profile in profiles {
        for point in profile {
            let from = write_point(*point);
            let to = write_point(map(*point));
            if from == to {
                continue;
            }
            for buffer in buffers.iter() {
                let mut offset = 0usize;
                while offset + POINT_BYTES <= buffer.len() {
                    if buffer[offset..offset + POINT_BYTES] == from {
                        replacements.push((buffer.as_ptr() as usize, offset, to));
                        offset += POINT_BYTES;
                    } else {
                        offset += 1;
                    }
                }
            }
        }
    }
    let mut written = 0usize;
    for (buffer_id, offset, bytes) in replacements {
        if let Some(buffer) = buffers
            .iter_mut()
            .find(|buffer| buffer.as_ptr() as usize == buffer_id)
        {
            buffer[offset..offset + POINT_BYTES].copy_from_slice(&bytes);
            written += 1;
        }
    }
    written
}

fn read_point(data: &[u8], offset: usize) -> Option<Vector3> {
    let x = read_f64(data, offset)?;
    let y = read_f64(data, offset + 8)?;
    let z = read_f64(data, offset + 16)?;
    Some(Vector3::new(x, y, z))
}

fn read_f64(data: &[u8], offset: usize) -> Option<f64> {
    let bytes: [u8; 8] = data.get(offset..offset + 8)?.try_into().ok()?;
    let value = f64::from_le_bytes(bytes);
    value.is_finite().then_some(value)
}

/// World-axis bias Architecture stores next to a wall's first preview corner.
pub(crate) const WALL_CORNER_BIAS: f64 = 0.046875;

/// Bit distance from a straight wall's width field back to its height field.
pub(crate) const WALL_HEIGHT_BEFORE_WIDTH: i64 = 132;
/// Bit distance from a straight wall's width field forward to its length field.
pub(crate) const WALL_LENGTH_AFTER_WIDTH: i64 = 222;

/// Located parameter fields of a 2027 straight wall whose start and direction
/// are stored as full bit-doubles.
#[derive(Debug, Clone, Copy)]
pub(crate) struct StraightWallFields {
    pub start_x: i64,
    pub start_y: i64,
    pub dir_x: i64,
    pub dir_y: i64,
    pub normal_x: i64,
    pub normal_y: i64,
    pub height: i64,
    pub width: i64,
    pub length: i64,
}

/// Find the straight-wall parameters measured from AutoCAD Architecture 2027.
///
/// A nonzero direction is the pair of full bit-doubles `(dx, dy)` 66 bits
/// apart with `dx² + dy² = 1`. The start point is the full-double pair 68 and
/// 134 bits before it. The left normal `(-dy, dx)` follows the direction's
/// zero Z. Height, width, and length follow the direction by the distances
/// measured on walls of different sizes.
pub(crate) fn find_straight_wall_fields(data: &[u8]) -> Option<StraightWallFields> {
    let limit = (data.len() as i64).saturating_mul(8).saturating_sub(700);
    let mut bit = 134i64;
    while bit < limit {
        let (Some(dir_x), Some(dir_y)) = (
            read_bit_double_at(data, bit),
            read_bit_double_at(data, bit + 66),
        ) else {
            bit += 1;
            continue;
        };
        if (dir_x * dir_x + dir_y * dir_y - 1.0).abs() > 1e-3 {
            bit += 1;
            continue;
        }
        let normal_x = bit + 134;
        let normal_y = normal_x + 66;
        let (Some(left_x), Some(left_y)) = (
            read_bit_double_at(data, normal_x),
            read_bit_double_at(data, normal_y),
        ) else {
            bit += 1;
            continue;
        };
        if (left_x + dir_y).abs() > 1e-3 || (left_y - dir_x).abs() > 1e-3 {
            bit += 1;
            continue;
        }
        let start_y = bit - 68;
        let start_x = start_y - 66;
        if read_bit_double_at(data, start_x).is_none()
            || read_bit_double_at(data, start_y).is_none()
        {
            bit += 1;
            continue;
        }
        let width = bit + 416;
        let height = width - WALL_HEIGHT_BEFORE_WIDTH;
        let length = width + WALL_LENGTH_AFTER_WIDTH;
        if read_bit_double_at(data, width)
            .filter(|value| *value > 0.0)
            .is_none()
            || read_bit_double_at(data, height)
                .filter(|value| *value > 0.0)
                .is_none()
            || read_bit_double_at(data, length)
                .filter(|value| *value > 0.0)
                .is_none()
        {
            bit += 1;
            continue;
        }
        return Some(StraightWallFields {
            start_x,
            start_y,
            dir_x: bit,
            dir_y: bit + 66,
            normal_x,
            normal_y,
            height,
            width,
            length,
        });
    }
    None
}

pub(crate) fn read_bit_double_at(data: &[u8], bit: i64) -> Option<f64> {
    use crate::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
    use crate::io::dwg::dwg_version::DwgVersion;
    use crate::types::DxfVersion;
    if bit < 0 {
        return None;
    }
    let mut reader = DwgBitReader::new(data.to_vec(), DwgVersion::AC24, DxfVersion::AC1032);
    reader.set_position_in_bits(bit);
    if reader.read_2bits() != 0 {
        return None;
    }
    let value = reader.read_raw_double();
    value.is_finite().then_some(value)
}

pub(crate) fn write_bit_double_at(data: &mut [u8], bit: i64, value: f64) -> bool {
    if read_bit_double_at(data, bit).is_none() {
        return false;
    }
    write_double_at_bit(data, (bit + 2) as usize, value);
    true
}

pub(crate) fn write_double_at_bit(data: &mut [u8], bit_index: usize, value: f64) {
    for (byte_index, byte) in value.to_le_bytes().into_iter().enumerate() {
        for bit in 0..8 {
            let on = byte & (1 << (7 - bit)) != 0;
            let at = bit_index + byte_index * 8 + bit;
            let Some(slot) = data.get_mut(at / 8) else {
                return;
            };
            let mask = 1u8 << (7 - (at % 8));
            if on {
                *slot |= mask;
            } else {
                *slot &= !mask;
            }
        }
    }
}

fn write_point(point: Vector3) -> [u8; POINT_BYTES] {
    let mut bytes = [0u8; POINT_BYTES];
    bytes[0..8].copy_from_slice(&point.x.to_le_bytes());
    bytes[8..16].copy_from_slice(&point.y.to_le_bytes());
    bytes[16..24].copy_from_slice(&point.z.to_le_bytes());
    bytes
}

fn point_is_plausible(point: Vector3) -> bool {
    point.x.abs() < 1.0e9 && point.y.abs() < 1.0e9 && point.z.abs() < 1.0e9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type6_round_trip_and_translate() {
        let points = vec![
            Vector3::new(-151.0, 1016.0, 0.0),
            Vector3::new(72.101423, 1016.0, 0.0),
            Vector3::new(72.101423, 1033.0, 0.0),
            Vector3::new(-151.0, 1033.0, 0.0),
            Vector3::new(-151.0, 1016.0, 0.0),
        ];
        let mut payload = Vec::new();
        payload.extend_from_slice(&(points.len() as u32).to_le_bytes());
        for point in &points {
            payload.extend_from_slice(&write_point(*point));
        }
        let decoded = polyline_from_type6(&payload).unwrap();
        assert_eq!(decoded, points);

        let delta = Vector3::new(100.0, -20.0, 5.0);
        let mut raw = payload.clone();
        let mut preview = payload.clone();
        let written =
            map_profile_points(&mut [&mut raw, &mut preview], &[decoded.clone()], |point| {
                point + delta
            });
        assert!(written >= points.len());
        let moved = polyline_from_type6(&raw).unwrap();
        assert_eq!(moved[0], points[0] + delta);
        assert_eq!(polyline_from_type6(&preview).unwrap()[1], points[1] + delta);
    }

    #[test]
    fn bit_double_round_trip_keeps_the_prefix_code() {
        let mut data = vec![0u8; 16];
        // Code 00 occupies bits 2 and 3. The raw double starts at bit 4.
        super::write_double_at_bit(&mut data, 4, 10.0);
        let value = super::read_bit_double_at(&data, 2).unwrap();
        assert!((value - 10.0).abs() < 1e-9);
        assert!(super::write_bit_double_at(&mut data, 2, 6.5));
        let value = super::read_bit_double_at(&data, 2).unwrap();
        assert!((value - 6.5).abs() < 1e-9);
    }
}
