//! A 3D polyline whose vertex handles are not in drawing order.
//!
//! Inserting a vertex into an existing polyline gives that vertex a newer
//! handle. Issue #160: the DWG reader used to emit vertices in handle order,
//! so the inserted vertex moved to the end of the line.

use std::io::Cursor;

use acadrust::entities::{EntityType, Polyline3D, Vertex3DPolyline};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Handle};

const POINTS: [(f64, f64, f64); 6] = [
    (0.0, 0.0, 0.0),
    (10.0, 0.0, 1.0),
    (10.0, 10.0, 2.0),
    (0.0, 10.0, 3.0),
    (0.0, 20.0, 4.0),
    (10.0, 20.0, 5.0),
];

/// Drawing order. The second vertex has the highest handle, as it would after
/// an insert into an existing polyline.
const HANDLES: [u64; 6] = [0x113C, 0x1141, 0x113D, 0x113E, 0x113F, 0x1140];

fn inserted_vertex_polyline(version: DxfVersion) -> CadDocument {
    let mut polyline = Polyline3D::new();
    for (&(x, y, z), &handle) in POINTS.iter().zip(HANDLES.iter()) {
        let mut vertex = Vertex3DPolyline::from_xyz(x, y, z);
        vertex.handle = Handle::new(handle);
        polyline.add_vertex_full(vertex);
    }
    let mut document = CadDocument::with_version(version);
    // Keep later allocations away from the preassigned vertex handles.
    document.header.handle_seed = 0x20_000;
    document
        .add_entity(EntityType::Polyline3D(polyline))
        .expect("add polyline");
    document
}

fn assert_drawing_order(version: DxfVersion) {
    let bytes = DwgWriter::write_to_vec(&inserted_vertex_polyline(version)).expect("write dwg");
    let loaded = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg");
    let polyline = loaded.entities().find_map(|entity| match entity {
        EntityType::Polyline3D(polyline) => Some(polyline),
        _ => None,
    });
    let polyline = polyline.expect("3d polyline");
    let got: Vec<_> = polyline
        .vertices
        .iter()
        .map(|vertex| {
            (
                vertex.handle.value(),
                vertex.position.x,
                vertex.position.y,
                vertex.position.z,
            )
        })
        .collect();
    let expected: Vec<_> = HANDLES
        .iter()
        .zip(POINTS.iter())
        .map(|(&handle, &(x, y, z))| (handle, x, y, z))
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn r2000_polyline3d_keeps_an_inserted_vertex_in_place() {
    assert_drawing_order(DxfVersion::AC1015);
}

#[test]
fn r2004_polyline3d_keeps_an_inserted_vertex_in_place() {
    assert_drawing_order(DxfVersion::AC1018);
}

#[test]
fn r2018_polyline3d_keeps_an_inserted_vertex_in_place() {
    assert_drawing_order(DxfVersion::AC1032);
}
