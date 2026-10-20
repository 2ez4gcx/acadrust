use std::io::Cursor;

use acadrust::entities::Mesh;
use acadrust::{CadDocument, DwgReadOptions, DwgReader, DwgWriter, EntityType, Vector3};

fn dense_mesh() -> Mesh {
    let mut mesh = Mesh::new();
    for i in 0..6 {
        mesh.add_vertex(Vector3::new(i as f64, (i % 2) as f64, 0.0));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    mesh.add_triangle(2, 3, 4);
    mesh.add_triangle(3, 4, 5);
    mesh.add_triangle(0, 2, 5);
    mesh
}

fn write_mesh() -> Vec<u8> {
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Mesh(dense_mesh())).unwrap();
    DwgWriter::write_to_vec(&doc).unwrap()
}

fn faces(doc: &CadDocument) -> Vec<Vec<usize>> {
    doc.entities()
        .find_map(|entity| match entity {
            EntityType::Mesh(mesh) => Some(
                mesh.faces
                    .iter()
                    .map(|face| face.vertices.clone())
                    .collect(),
            ),
            _ => None,
        })
        .expect("MESH entity")
}

#[test]
fn default_read_keeps_every_face_of_a_small_mesh() {
    let mut reader = DwgReader::from_stream(Cursor::new(write_mesh()));
    let doc = reader.read().unwrap();
    assert_eq!(faces(&doc).len(), 5);
    assert!(doc
        .notifications
        .iter()
        .all(|note| !note.message.contains("face budget")));
}

#[test]
fn explicit_caps_truncate_whole_faces_and_report_the_budget() {
    let bytes = write_mesh();
    let mut reader = DwgReader::from_stream_with_options(
        Cursor::new(bytes.clone()),
        DwgReadOptions::default().with_mesh_limits(2, usize::MAX),
    );
    let outcome = reader.read_with_stats().unwrap();
    assert_eq!(faces(&outcome.document), vec![vec![0, 1, 2], vec![1, 2, 3]]);
    assert!(outcome
        .stats
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "mesh-face-budget"));

    let mut reader = DwgReader::from_stream_with_options(
        Cursor::new(bytes),
        DwgReadOptions::default().with_mesh_limits(usize::MAX, 7),
    );
    let doc = reader.read().unwrap();
    let stored = faces(&doc);
    assert_eq!(stored, vec![vec![0, 1, 2], vec![1, 2, 3]]);
    assert!(doc
        .notifications
        .iter()
        .any(|note| note.message.contains("face budget")));
}

#[test]
fn unlimited_option_stores_every_face() {
    let mut reader = DwgReader::from_stream_with_options(
        Cursor::new(write_mesh()),
        DwgReadOptions::default().with_unlimited_mesh(),
    );
    let doc = reader.read().unwrap();
    assert_eq!(faces(&doc).len(), 5);
    assert!(doc
        .notifications
        .iter()
        .all(|note| !note.message.contains("face budget")));
}
