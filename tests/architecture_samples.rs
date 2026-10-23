//! Architecture sample drawings: read the AEC classes, move a wall, and
//! confirm the class record still round-trips.

use acadrust::classes::arch::{is_architecture_name, reshape_rectangle, ArchKind};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::objects::ObjectType;
use acadrust::types::Vector3;
use acadrust::{DwgReader, DwgWriter};

const SHELL: &str = r"C:\Users\Hakan\Documents\Autodesk\My Projects\Sample Project 2027\Constructs\Architectural\Shell\01 Masonry Shell.dwg";

fn shell_path() -> Option<std::path::PathBuf> {
    let path = std::path::PathBuf::from(SHELL);
    path.is_file().then_some(path)
}

#[test]
fn masonry_shell_walls_round_trip_after_move() {
    let Some(path) = shell_path() else {
        return;
    };
    let mut doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    let mut walls = 0usize;
    let mut styled = 0usize;
    for object in doc.objects.values() {
        if let ObjectType::Architecture(object) = object {
            assert!(is_architecture_name(&object.dxf_name));
            assert!(object.raw_dwg_data.is_some());
            styled += 1;
        }
    }
    assert!(styled > 100, "style and display objects: {styled}");

    let delta = Vector3::new(120.0, -40.0, 0.0);
    let mut moved = None;
    for entity in doc.entities_mut() {
        let EntityType::Extended(entity) = entity else {
            continue;
        };
        let ExtendedEntityData::Architecture(data) = &mut entity.data else {
            continue;
        };
        if data.kind != ArchKind::Wall {
            continue;
        }
        walls += 1;
        assert!(!data.profiles.is_empty(), "{}", data.dxf_name);
        assert!(data.profiles[0].len() >= 2);
        if moved.is_none() {
            let before = data.profiles[0][0];
            assert!(data.translate(&mut entity.common.graphic_data, delta));
            moved = Some((entity.common.handle, before + delta, data.profiles[0][0]));
        }
    }
    assert!(walls >= 10, "walls: {walls}");
    let (handle, expected, stored) = moved.unwrap();
    assert_eq!(stored, expected);

    let out = std::env::temp_dir().join("acadrust-arch-shell.dwg");
    DwgWriter::write_to_file(&out, &doc).unwrap();
    let reread = DwgReader::from_file(&out).unwrap().read().unwrap();
    let entity = reread.get_entity(handle).expect("moved wall");
    let EntityType::Extended(entity) = entity else {
        panic!("wall was not kept as an Architecture entity");
    };
    let ExtendedEntityData::Architecture(data) = &entity.data else {
        panic!("wall class payload missing");
    };
    assert_eq!(data.dxf_name, "AEC_WALL");
    assert_eq!(data.kind, ArchKind::Wall);
    assert_eq!(data.profiles[0][0], expected);
}

#[test]
fn masonry_shell_wall_can_be_reshaped() {
    let Some(path) = shell_path() else {
        return;
    };
    let mut doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    let start = Vector3::new(0.0, 0.0, 0.0);
    let end = Vector3::new(240.0, 0.0, 0.0);
    let mut reshaped = false;
    for entity in doc.entities_mut() {
        let EntityType::Extended(entity) = entity else {
            continue;
        };
        let ExtendedEntityData::Architecture(data) = &entity.data else {
            continue;
        };
        if data.kind != ArchKind::Wall || data.profiles.first().map(|profile| profile.len()) != Some(5)
        {
            continue;
        }
        let ExtendedEntityData::Architecture(data) = &mut entity.data else {
            continue;
        };
        assert!(reshape_rectangle(
            data,
            &mut entity.common.graphic_data,
            start,
            end,
            8.0
        ));
        assert_eq!(data.profiles[0][0], start);
        assert_eq!(data.profiles[0][1], end);
        reshaped = true;
        break;
    }
    assert!(reshaped, "no 5-point wall profile to reshape");
    let out = std::env::temp_dir().join("acadrust-arch-reshaped.dwg");
    DwgWriter::write_to_file(&out, &doc).unwrap();
}

#[test]
fn autocad_keeps_architecture_entities_after_save() {
    let path = std::env::temp_dir().join("acadrust-arch-moved-acad.dwg");
    if !path.is_file() {
        return;
    }
    let doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    let mut walls = 0usize;
    let mut doors = 0usize;
    let mut assemblies = 0usize;
    for entity in doc.entities() {
        let EntityType::Extended(entity) = entity else {
            continue;
        };
        let ExtendedEntityData::Architecture(data) = &entity.data else {
            continue;
        };
        match data.kind {
            ArchKind::Wall => walls += 1,
            ArchKind::Door => doors += 1,
            ArchKind::WindowAssembly => assemblies += 1,
            _ => {}
        }
    }
    assert!(walls >= 10, "walls {walls}");
    assert!(doors >= 1, "doors {doors}");
    assert!(assemblies >= 1, "window assemblies {assemblies}");
}
