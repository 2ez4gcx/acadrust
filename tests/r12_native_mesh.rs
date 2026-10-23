use std::{fs, io::Cursor, path::PathBuf};

use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, EntityType, Vector3};

#[derive(Debug, PartialEq)]
struct NativeGeometry {
    mesh_flags: i16,
    mesh_grid: [i16; 2],
    mesh_vertices: Vec<(Vector3, i16)>,
    polyface_flags: i16,
    polyface_vertices: Vec<Vector3>,
    polyface_faces: Vec<[i16; 4]>,
    polyline3d_flags: i32,
    polyline3d_vertices: Vec<(Vector3, i32)>,
    closed_polyline_vertices: Vec<Vector3>,
}

fn native_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r12/cad-created")
        .join(name)
}

/// Reads a native BricsCAD DWG. These binaries are not in the repository (see
/// `tests/fixtures/r12/cad-created/README.md`), so the tests that read them
/// are ignored.
fn native_dwg(name: &str) -> CadDocument {
    let path = native_path(name);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn native_dxf(name: &str) -> CadDocument {
    DxfReader::from_file(native_path(name))
        .unwrap()
        .read()
        .unwrap()
}

fn geometry(document: &CadDocument) -> NativeGeometry {
    assert_eq!(document.version, DxfVersion::AC1009);
    let meshes: Vec<_> = document
        .entities()
        .filter_map(|entity| match entity {
            EntityType::PolygonMesh(mesh) => Some(mesh),
            _ => None,
        })
        .collect();
    let polyfaces: Vec<_> = document
        .entities()
        .filter_map(|entity| match entity {
            EntityType::PolyfaceMesh(mesh) => Some(mesh),
            _ => None,
        })
        .collect();
    let polylines3d: Vec<_> = document
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Polyline3D(polyline) => Some(polyline),
            _ => None,
        })
        .collect();
    let closed_polylines: Vec<_> = document
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Polyline2D(polyline) if polyline.flags.is_closed() => Some(polyline),
            _ => None,
        })
        .collect();
    assert_eq!(meshes.len(), 1, "native 2-by-3 polygon mesh");
    assert_eq!(polyfaces.len(), 1, "native four-vertex polyface");
    assert_eq!(polylines3d.len(), 1, "native 3D polyline");
    assert_eq!(closed_polylines.len(), 1, "native closed 2D polyline");
    NativeGeometry {
        mesh_flags: meshes[0].flags.bits(),
        mesh_grid: [meshes[0].m_vertex_count, meshes[0].n_vertex_count],
        mesh_vertices: meshes[0]
            .vertices
            .iter()
            .map(|vertex| (vertex.location, vertex.flags))
            .collect(),
        polyface_flags: polyfaces[0].flags.bits(),
        polyface_vertices: polyfaces[0]
            .vertices
            .iter()
            .map(|vertex| vertex.location)
            .collect(),
        polyface_faces: polyfaces[0]
            .faces
            .iter()
            .map(|face| [face.index1, face.index2, face.index3, face.index4])
            .collect(),
        polyline3d_flags: polylines3d[0].flags.to_bits(),
        polyline3d_vertices: polylines3d[0]
            .vertices
            .iter()
            .map(|vertex| (vertex.position, vertex.flags))
            .collect(),
        closed_polyline_vertices: closed_polylines[0]
            .vertices
            .iter()
            .map(|vertex| vertex.location)
            .collect(),
    }
}

fn expected_geometry() -> NativeGeometry {
    NativeGeometry {
        mesh_flags: 16,
        mesh_grid: [2, 3],
        mesh_vertices: [
            (30.0, 0.0, 0.0),
            (30.0, 5.0, 1.0),
            (30.0, 10.0, 2.0),
            (40.0, 0.0, 3.0),
            (40.0, 5.0, 4.0),
            (40.0, 10.0, 5.0),
        ]
        .into_iter()
        .map(|(x, y, z)| (Vector3::new(x, y, z), 64))
        .collect(),
        polyface_flags: 64,
        polyface_vertices: [
            (50.0, 0.0, 0.0),
            (60.0, 0.0, 0.0),
            (60.0, 10.0, 3.0),
            (50.0, 10.0, 0.0),
        ]
        .into_iter()
        .map(|(x, y, z)| Vector3::new(x, y, z))
        .collect(),
        polyface_faces: vec![[1, 2, 3, 4]],
        polyline3d_flags: 8,
        polyline3d_vertices: [(0.0, 15.0, 0.0), (5.0, 17.0, 3.0), (10.0, 15.0, 7.0)]
            .into_iter()
            .map(|(x, y, z)| (Vector3::new(x, y, z), 32))
            .collect(),
        closed_polyline_vertices: [(0.0, 0.0, 0.0), (5.0, 2.0, 0.0), (10.0, 0.0, 0.0)]
            .into_iter()
            .map(|(x, y, z)| Vector3::new(x, y, z))
            .collect(),
    }
}

fn assert_repeated_saves_are_deterministic(mut restored: CadDocument) {
    for cycle in 0..3 {
        let bytes = DwgWriter::write_to_vec(&restored).unwrap();
        assert_eq!(bytes, DwgWriter::write_to_vec(&restored).unwrap());
        restored = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(
            geometry(&restored),
            expected_geometry(),
            "save cycle {cycle}"
        );
    }
}

#[test]
fn ac1009_native_dxf_keeps_distinct_mesh_topologies() {
    for suffix in ["", "-edited"] {
        let dxf = native_dxf(&format!("native-geometry{suffix}.dxf"));
        assert_eq!(geometry(&dxf), expected_geometry(), "native DXF{suffix}");
    }
}

#[test]
#[ignore = "needs native-geometry.dwg and native-geometry-edited.dwg in tests/fixtures/r12/cad-created"]
fn ac1009_native_dwg_keeps_distinct_mesh_topologies() {
    for suffix in ["", "-edited"] {
        let dwg = native_dwg(&format!("native-geometry{suffix}.dwg"));
        assert_eq!(geometry(&dwg), expected_geometry(), "native DWG{suffix}");
    }
}

#[test]
fn ac1009_native_dxf_mesh_topologies_survive_repeated_deterministic_saves() {
    for suffix in ["", "-edited"] {
        let dxf = native_dxf(&format!("native-geometry{suffix}.dxf"));
        assert_repeated_saves_are_deterministic(dxf);
    }
}

#[test]
#[ignore = "needs native-geometry.dwg and native-geometry-edited.dwg in tests/fixtures/r12/cad-created"]
fn ac1009_native_dwg_mesh_topologies_survive_repeated_deterministic_saves() {
    for suffix in ["", "-edited"] {
        let dwg = native_dwg(&format!("native-geometry{suffix}.dwg"));
        assert_repeated_saves_are_deterministic(dwg);
    }
}
