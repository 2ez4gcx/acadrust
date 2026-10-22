use std::io::Cursor;

use acadrust::entities::{Face3D, Solid};
use acadrust::io::dwg::DwgVersion;
use acadrust::tables::BlockRecord;
use acadrust::tables::Layer;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{
    Arc, CadDocument, Circle, DwgReader, DwgWriter, DxfVersion, Ellipse, EntityType, Line, MText,
    Point, Text, Vector3,
};

fn bytes_contain(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn ac1009_write_keeps_annotative_application_when_extended_data_names_it() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let mut record = ExtendedDataRecord::new("ACADANNOTATIVE");
    record.add_value(XDataValue::String("AnnotativeData".into()));
    line.common.extended_data.add_record(record);
    document.add_entity(EntityType::Line(line)).unwrap();

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    assert!(
        bytes_contain(&bytes, b"ACADANNOTATIVE") || bytes_contain(&bytes, b"AcadAnnotative"),
        "referenced annotative application was dropped from the R12 table"
    );
    let restored = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let entity = restored.entities().next().expect("line");
    let record = entity
        .common()
        .extended_data
        .records()
        .iter()
        .find(|record| {
            record
                .application_name
                .eq_ignore_ascii_case("ACADANNOTATIVE")
        })
        .expect("annotative extended data");
    assert!(record
        .values
        .iter()
        .any(|value| matches!(value, XDataValue::String(text) if text == "AnnotativeData")));
}

#[test]
fn ac1009_write_omits_unreferenced_annotative_application() {
    let document = CadDocument::with_version(DxfVersion::AC1009);
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    assert!(!bytes_contain(&bytes, b"AcadAnnotative"));
    assert!(!bytes_contain(&bytes, b"ACADANNOTATIVE"));
}

#[test]
fn ac1009_keeps_imported_layout_references_when_applying_legacy_header() {
    use acadrust::objects::ObjectType;

    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document.header.model_space_insertion_base = Vector3::new(3.0, 7.0, 0.0);
    document.header.ortho_mode = true;
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let restored = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert_eq!(
        restored.header.model_space_insertion_base,
        Vector3::new(3.0, 7.0, 0.0)
    );
    assert!(restored.header.ortho_mode);
    let ObjectType::Dictionary(layouts) =
        &restored.objects[&restored.header.acad_layout_dict_handle]
    else {
        panic!("missing imported layout dictionary");
    };
    assert_eq!(layouts.entries.len(), 2);
    for (_, handle) in &layouts.entries {
        let ObjectType::Layout(layout) = &restored.objects[handle] else {
            panic!("dangling layout dictionary entry");
        };
        assert!(restored
            .block_records
            .iter()
            .any(|block| block.handle == layout.block_record));
    }
    assert!(!restored.header.model_space_block_handle.is_null());
    assert!(!restored.header.paper_space_block_handle.is_null());
    assert!(restored
        .objects
        .contains_key(&restored.header.named_objects_dict_handle));
}

#[test]
fn ac1009_writer_roundtrips_a_line() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write AC1009 DWG");
    assert_eq!(&bytes[..6], b"AC1009");
    let info = DwgReader::from_stream(Cursor::new(bytes.clone()))
        .read_file_header()
        .expect("read AC1009 file header");
    assert_eq!(info.version, DwgVersion::AC9);
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read AC1009 DWG");
    assert_eq!(restored.version, DxfVersion::AC1009);
    assert!(restored
        .entities()
        .any(|entity| matches!(entity, EntityType::Line(_))));
}

#[test]
fn ac1009_roundtrips_the_native_entity_set() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Point(Point::from_coords(1.0, 2.0, 0.0)))
        .unwrap();
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
    document
        .add_entity(EntityType::Circle(Circle::from_coords(4.0, 5.0, 0.0, 2.0)))
        .unwrap();
    document
        .add_entity(EntityType::Arc(Arc::from_coords(
            4.0,
            5.0,
            0.0,
            2.0,
            0.0,
            std::f64::consts::PI,
        )))
        .unwrap();
    document
        .add_entity(EntityType::Solid(Solid::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        )))
        .unwrap();
    document
        .add_entity(EntityType::Face3D(Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(0.0, 1.0, 1.0),
        )))
        .unwrap();
    document
        .add_entity(EntityType::Text(Text::with_value(
            "R12",
            Vector3::new(0.0, 0.0, 0.0),
        )))
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write native R12 entities");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read native R12 entities");
    let kinds: Vec<&'static str> = restored
        .entities()
        .map(|entity| match entity {
            EntityType::Point(_) => "POINT",
            EntityType::Line(_) => "LINE",
            EntityType::Circle(_) => "CIRCLE",
            EntityType::Arc(_) => "ARC",
            EntityType::Solid(_) => "SOLID",
            EntityType::Face3D(_) => "3DFACE",
            EntityType::Text(_) => "TEXT",
            _ => "OTHER",
        })
        .collect();
    for expected in ["POINT", "LINE", "CIRCLE", "ARC", "SOLID", "3DFACE", "TEXT"] {
        assert!(kinds.contains(&expected), "missing {expected} in {kinds:?}");
    }
}

#[test]
fn ac1009_rejects_corrupt_entity_crc() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 2.0, 3.0, 0.0,
        )))
        .unwrap();
    let mut bytes = DwgWriter::write_to_vec(&document).unwrap();
    let end = u32::from_le_bytes(bytes[0x18..0x1c].try_into().unwrap()) as usize;
    bytes[end - 1] ^= 0x40;
    let error = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .unwrap_err();
    assert!(error.to_string().contains("CRC"));
}

#[test]
fn ac1009_roundtrips_block_entities() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    let mut block = BlockRecord::new("Door");
    block.handle = document.allocate_handle();
    block.block_entity_handle = document.allocate_handle();
    block.block_end_handle = document.allocate_handle();
    let entity = document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
        )))
        .unwrap();
    block.entity_handles.push(entity);
    document.block_records.add(block).unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write AC1009 block");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read AC1009 block");
    assert!(restored.block_records.get("Door").is_some());
    assert!(restored
        .entities()
        .any(|entity| matches!(entity, EntityType::Line(_))));
}

#[test]
fn ac1009_preserves_custom_layer_names() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document.layers.add(Layer::new("CUT")).unwrap();
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    line.common.layer = "CUT".into();
    document.add_entity(EntityType::Line(line)).unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write custom layer");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read custom layer");
    let EntityType::Line(line) = restored
        .entities()
        .find(|entity| matches!(entity, EntityType::Line(_)))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(line.common.layer, "CUT");
}

#[test]
fn ac1009_decodes_legacy_entity_string_escapes() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document.header.code_page = "ANSI_1252".into();
    document
        .add_entity(EntityType::Text(Text::with_value(
            r"\U+4E2D\U+6587 \M+5BCFE\M+5BAC5",
            Vector3::ZERO,
        )))
        .unwrap();

    let bytes = DwgWriter::write_to_vec(&document).expect("write escaped R12 text");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read escaped R12 text");
    let EntityType::Text(text) = restored
        .entities()
        .find(|entity| matches!(entity, EntityType::Text(_)))
        .expect("missing text")
    else {
        unreachable!();
    };
    assert_eq!(text.value, "中文 件号");
}

#[test]
fn ac1009_downsaves_convertible_modern_entities() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::MText(Box::new(MText::with_value(
            "legacy text",
            Vector3::ZERO,
        ))))
        .unwrap();
    document
        .add_entity(EntityType::Ellipse(Ellipse::from_center_axes(
            Vector3::ZERO,
            Vector3::new(5.0, 0.0, 0.0),
            0.5,
        )))
        .unwrap();

    let bytes = DwgWriter::write_to_vec(&document).expect("down-save modern entities");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read converted AC1009 drawing");
    assert!(restored
        .entities()
        .any(|entity| matches!(entity, EntityType::Text(_))));
    assert!(restored
        .entities()
        .any(|entity| matches!(entity, EntityType::Polyline2D(_))));
}
