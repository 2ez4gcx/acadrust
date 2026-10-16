use opencadcodec::io::dwg::r12::header::read_header;
use opencadcodec::{
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, EntityType, Vector2, Vector3,
};
use std::{fs, io::Cursor, path::PathBuf};

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/r12/cad-created")
        .join(name)
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn native(name: &str) -> Option<CadDocument> {
    Some(read(fs::read(path(name)).ok()?))
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

fn assert_viewports(document: &CadDocument, edited: bool) {
    let viewports: Vec<_> = document
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Viewport(viewport) if viewport.width == 120.0 => Some(viewport),
            _ => None,
        })
        .collect();
    assert_eq!(viewports.len(), 2);
    for viewport in viewports {
        assert_eq!(
            viewport.common.owner_handle,
            document.header.paper_space_block_handle
        );
        assert_eq!(viewport.view_target, Vector3::new(4.0, 5.0, 6.0));
        assert_eq!(viewport.view_direction, Vector3::new(1.0, 2.0, 3.0));
        close(
            viewport.twist_angle,
            if edited && viewport.center.x == 210.0 {
                std::f64::consts::FRAC_PI_4
            } else {
                std::f64::consts::FRAC_PI_6
            },
        );
        close(viewport.snap_angle, std::f64::consts::PI / 9.0);
        assert_eq!(viewport.snap_base, Vector3::new(2.0, 3.0, 0.0));
        assert_eq!(viewport.snap_spacing, Vector3::new(0.25, 0.75, 0.0));
        assert_eq!(viewport.grid_spacing, Vector3::new(5.0, 10.0, 0.0));
        assert!(viewport.status.snap_on && viewport.status.grid_on);
        close(
            viewport.custom_scale,
            viewport.height / viewport.view_height,
        );
        assert_eq!(viewport.lens_length, 65.0);
    }
}

#[test]
fn cad_created_dwg_and_dxf_restore_mview_in_radians() {
    for suffix in ["", "-edited"] {
        if let Some(document) = native(&format!("native-viewports{suffix}.dwg")) {
            assert_viewports(&document, !suffix.is_empty());
        }
        let dxf = DxfReader::from_file(path(&format!("native-viewports{suffix}.dxf")))
            .unwrap()
            .read()
            .unwrap();
        assert_viewports(&dxf, !suffix.is_empty());
    }
}

#[test]
fn vx_references_resolve_and_rebase_across_three_saves() {
    let Some(mut document) = native("native-viewports-edited.dwg") else {
        return;
    };
    for _ in 0..3 {
        assert_viewports(&document, true);
        assert_eq!(document.vx_table.len(), 5);
        assert_eq!(
            document
                .vx_table
                .iter()
                .filter(|vx| vx.viewport.is_valid())
                .count(),
            4
        );
        for vx in document.vx_table.iter().filter(|vx| vx.viewport.is_valid()) {
            assert!(matches!(
                document.get_entity(vx.viewport),
                Some(EntityType::Viewport(_))
            ));
            assert!(
                vx.previous_entry.is_null()
                    || document
                        .vx_table
                        .iter()
                        .any(|other| other.handle == vx.previous_entry)
            );
        }
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        assert_eq!(bytes, DwgWriter::write_to_vec(&document).unwrap());
        let header = read_header(&bytes).unwrap();
        for index in 1..5 {
            let offset =
                header.tables[9].address as usize + index * header.tables[9].record_size as usize;
            let address =
                u16::from_le_bytes(bytes[offset + 35..offset + 37].try_into().unwrap()) as usize;
            assert_eq!(bytes[address], 24, "VX address must point at VIEWPORT");
        }
        document = read(bytes);
    }
}

#[test]
fn shared_vx_block_sentinel_is_validated_without_weakening_crc_checks() {
    let Ok(bytes) = fs::read(path("native-viewports.dwg")) else {
        return;
    };
    let header = read_header(&bytes).unwrap();
    assert_eq!(
        header.tables[9].address
            + u32::from(header.tables[9].count) * u32::from(header.tables[9].record_size),
        header.layout.blocks_start
    );
    let mut corrupt = bytes.clone();
    corrupt[header.tables[9].address as usize - 16] ^= 1;
    assert!(DwgReader::from_stream(Cursor::new(corrupt))
        .read()
        .unwrap_err()
        .to_string()
        .contains("sentinels"));
    let mut corrupt = bytes;
    corrupt[header.tables[9].address as usize + 1] ^= 1;
    assert!(DwgReader::from_stream(Cursor::new(corrupt))
        .read()
        .unwrap_err()
        .to_string()
        .contains("CRC"));
}

#[test]
fn native_block_name_is_resolved_from_its_table_locator() {
    let Some(document) = native("native-geometry.dwg") else {
        return;
    };
    assert!(document
        .block_records
        .iter()
        .all(|block| !block.name.is_empty()));
    assert!(
        document
            .block_records
            .get("NATIVE_PART")
            .unwrap()
            .flags
            .has_attributes
    );
    assert!(
        document
            .block_records
            .get("NATIVE_PART")
            .unwrap()
            .entity_handles
            .len()
            >= 3
    );
}

#[test]
fn native_header_drafting_state_is_typed_and_honors_edits() {
    for suffix in ["", "-edited"] {
        let Some(mut document) = native(&format!("native-header{suffix}.dwg")) else {
            continue;
        };
        let state = document.header.legacy.as_ref().unwrap();
        assert_eq!(state.snap_mode, 1);
        assert_eq!(state.grid_mode, 1);
        // BricsCAD's R12 DWG export resets UNITMODE, while its DXF retains 1.
        assert_eq!(state.unit_mode, 0);
        assert_eq!(state.snap_base, Vector2::new(2.0, 3.0));
        assert_eq!(
            state.snap_spacing,
            if suffix.is_empty() {
                Vector2::new(0.25, 0.75)
            } else {
                Vector2::new(0.5, 1.5)
            }
        );
        assert_eq!(
            state.grid_spacing,
            if suffix.is_empty() {
                Vector2::new(5.0, 10.0)
            } else {
                Vector2::new(6.0, 12.0)
            }
        );
        close(state.snap_angle, std::f64::consts::PI / 9.0);
        assert_eq!(state.circle_zoom, 0);
        assert_eq!(document.vports.get("*ACTIVE").unwrap().circle_zoom, 1200);
        document.header.legacy.as_mut().unwrap().snap_spacing = Vector2::new(1.25, 2.5);
        document.header.legacy.as_mut().unwrap().unit_mode = 1;
        let restored = read(DwgWriter::write_to_vec(&document).unwrap());
        assert_eq!(
            restored.header.legacy.as_ref().unwrap().snap_spacing,
            Vector2::new(1.25, 2.5)
        );
        assert_eq!(restored.header.legacy.as_ref().unwrap().unit_mode, 1);
        let dxf = DxfReader::from_reader(Cursor::new(
            DxfWriter::new(&document).write_to_vec().unwrap(),
        ))
        .unwrap()
        .read()
        .unwrap();
        assert_eq!(dxf.header.legacy.as_ref().unwrap().unit_mode, 1);
    }
}

#[cfg(feature = "serde")]
#[test]
fn native_header_and_viewport_state_survive_serde() {
    let Some(document) = native("native-header.dwg") else {
        return;
    };
    let bytes = serde_json::to_vec(&document).unwrap();
    let restored: CadDocument = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored.header.legacy, document.header.legacy);
    let reread = read(DwgWriter::write_to_vec(&restored).unwrap());
    assert_eq!(reread.header.legacy, document.header.legacy);
    let Some(document) = native("native-viewports-edited.dwg") else {
        return;
    };
    let bytes = serde_json::to_vec(&document).unwrap();
    let restored: CadDocument = serde_json::from_slice(&bytes).unwrap();
    assert_viewports(&restored, true);
}
