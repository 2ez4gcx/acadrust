//! Construct a straight wall by rewriting the reversed 2027 field layout.
//!
//! The source drawing is created by AutoCAD Architecture (`WallAdd`) and is not
//! stored in the crate. The test skips when that drawing is absent.

use acadrust::classes::arch::{set_straight_wall, ArchKind};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::types::Vector3;
use acadrust::{DwgReader, DwgWriter};

const DIAGONAL_WALL: &str = r"C:\Users\Hakan\AppData\Local\Temp\aec-made-diag.dwg";

#[test]
fn set_straight_wall_rewrites_start_direction_and_size() {
    let path = std::path::Path::new(DIAGONAL_WALL);
    if !path.is_file() {
        return;
    }
    let mut doc = DwgReader::from_file(path).unwrap().read().unwrap();
    let start = Vector3::new(10.0, 20.0, 0.0);
    let end = Vector3::new(10.0, 80.0, 0.0);
    let mut built = false;
    for entity in doc.entities_mut() {
        let EntityType::Extended(entity) = entity else {
            continue;
        };
        let ExtendedEntityData::Architecture(data) = &entity.data else {
            continue;
        };
        if data.kind != ArchKind::Wall {
            continue;
        }
        let ExtendedEntityData::Architecture(data) = &mut entity.data else {
            continue;
        };
        built = set_straight_wall(data, &mut entity.common.graphic_data, start, end, 8.0, 96.0);
        let profile = data.profiles.first().cloned().unwrap_or_default();
        assert!(
            profile
                .iter()
                .any(|point| (point.x - 6.0).abs() < 1e-4 && (point.y - 20.0).abs() < 1e-4),
            "{profile:?}"
        );
        assert!(
            profile
                .iter()
                .any(|point| (point.x - 14.0).abs() < 1e-4 && (point.y - 80.0).abs() < 1e-4),
            "{profile:?}"
        );
        break;
    }
    assert!(built, "diagonal seed wall was not recognized");
    let out = std::env::temp_dir().join("acadrust-constructed-wall.dwg");
    DwgWriter::write_to_file(&out, &doc).unwrap();
}

#[test]
fn autocad_keeps_the_rewritten_vertical_wall() {
    let path = std::env::temp_dir().join("acadrust-constructed-wall-acad.dwg");
    if !path.is_file() {
        return;
    }
    let doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    let wall = doc
        .entities()
        .find_map(|entity| {
            let EntityType::Extended(entity) = entity else {
                return None;
            };
            let ExtendedEntityData::Architecture(data) = &entity.data else {
                return None;
            };
            (data.kind == ArchKind::Wall).then_some(data.profiles.first().cloned())
        })
        .flatten()
        .expect("wall");
    assert!(
        wall.iter()
            .any(|point| (point.x - 6.0).abs() < 1e-3 && (point.y - 20.0).abs() < 1e-3),
        "AutoCAD rebuilt a different wall: {wall:?}"
    );
    assert!(
        wall.iter()
            .any(|point| (point.x - 14.0).abs() < 1e-3 && (point.y - 80.0).abs() < 1e-3),
        "AutoCAD rebuilt a different wall: {wall:?}"
    );
}
