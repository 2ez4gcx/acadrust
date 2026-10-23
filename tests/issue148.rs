//! Fixture-free coverage for the large, mixed-content DWG workload reported in #148.
use std::io::Cursor;

use acadrust::entities::{Circle, EntityType, Line, MText};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Vector3};

fn benchmark_scene() -> CadDocument {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    for index in 0..512u32 {
        let x = f64::from(index % 32) * 12.5;
        let y = f64::from(index / 32) * 8.75;
        document
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 9.0, y + f64::from(index % 5), 0.0),
            )))
            .unwrap();
    }
    for index in 0..192u32 {
        let x = f64::from(index % 24) * 17.0;
        let y = f64::from(index / 24) * 13.0;
        document
            .add_entity(EntityType::Circle(Circle::from_center_radius(
                Vector3::new(x, y, f64::from(index % 3)),
                1.0 + f64::from(index % 7) * 0.25,
            )))
            .unwrap();
    }
    for index in 0..96u32 {
        let mut text = MText::new();
        text.value = format!(
            "{{\\fArial|b1;Station {index}}}\\PFlow={{\\H0.75x;{:.2}}}\\P{{\\C3;Notes}}",
            f64::from(index) * 1.25
        );
        text.insertion_point = Vector3::new(
            f64::from(index % 16) * 24.0,
            f64::from(index / 16) * 19.0,
            0.0,
        );
        text.height = 2.5 + f64::from(index % 4) * 0.5;
        text.rectangle_width = 35.0;
        document
            .add_entity(EntityType::MText(Box::new(text)))
            .unwrap();
    }
    document
}

#[test]
fn mixed_benchmark_scene_roundtrips_through_dwg_without_entity_loss() {
    let document = benchmark_scene();
    let entity_count = document.entities().count();
    assert_eq!(entity_count, 800);

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    assert!(!bytes.is_empty());
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

    assert_eq!(loaded.version, DxfVersion::AC1032);
    assert_eq!(loaded.entities().count(), entity_count);
    assert_eq!(
        loaded
            .entities()
            .filter(|entity| matches!(entity, EntityType::Line(_)))
            .count(),
        512
    );
    assert_eq!(
        loaded
            .entities()
            .filter(|entity| matches!(entity, EntityType::Circle(_)))
            .count(),
        192
    );
    assert_eq!(
        loaded
            .entities()
            .filter(|entity| matches!(entity, EntityType::MText(_)))
            .count(),
        96
    );
}
