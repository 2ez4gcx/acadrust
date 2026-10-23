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
            crate::entities::ProxyGraphicRecord::Unknown { record_type: 6, data } => {
                polyline_from_type6(&data)
            }
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
        if let Some(buffer) = buffers.iter_mut().find(|buffer| buffer.as_ptr() as usize == buffer_id)
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
        let written = map_profile_points(&mut [&mut raw, &mut preview], &[decoded.clone()], |point| {
            point + delta
        });
        assert!(written >= points.len());
        let moved = polyline_from_type6(&raw).unwrap();
        assert_eq!(moved[0], points[0] + delta);
        assert_eq!(polyline_from_type6(&preview).unwrap()[1], points[1] + delta);
    }
}
