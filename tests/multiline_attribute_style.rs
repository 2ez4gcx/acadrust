use std::io::Cursor;

use acadrust::entities::{AttributeDefinition, AttributeEntity, EntityType, Insert, MText};
use acadrust::tables::TextStyle;
use acadrust::{CadDocument, Color, DwgReader, DwgWriter, DxfVersion, Vector3};

#[test]
fn multiline_attribute_and_definition_styles_survive_a_dwg_roundtrip() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for name in ["OuterStyle", "EmbeddedStyle"] {
        let mut style = TextStyle::new(name);
        style.handle = doc.allocate_handle();
        doc.text_styles.add(style).unwrap();
    }
    let mut mtext = MText::with_value("First\\PSecond", Vector3::ZERO);
    mtext.style = "EmbeddedStyle".into();
    mtext.common.color = Color::from_rgb(20, 40, 60);
    mtext.common.shadow_flags = 3;
    let mut att = AttributeEntity::simple("TITLE", "First");
    att.text_style = "OuterStyle".into();
    att.is_multiline = true;
    att.embedded_mtext = Some(Box::new(mtext.clone()));
    let mut insert = Insert::new("*Model_Space", Vector3::ZERO);
    insert.attributes.push(att);
    let insert_handle = doc.add_entity(EntityType::Insert(insert)).unwrap();
    let mut def = AttributeDefinition::simple("TITLE");
    def.text_style = "OuterStyle".into();
    def.is_multiline = true;
    def.embedded_mtext = Some(Box::new(mtext));
    let definition = doc
        .add_entity(EntityType::AttributeDefinition(def))
        .unwrap();

    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let EntityType::Insert(insert) = loaded.get_entity(insert_handle).unwrap() else {
        panic!()
    };
    let attr = &insert.attributes[0];
    assert_eq!(attr.text_style, "OuterStyle");
    assert_eq!(attr.embedded_mtext.as_ref().unwrap().style, "EmbeddedStyle");
    assert_eq!(attr.value, "First\\PSecond");
    assert_eq!(attr.tag, "TITLE");
    let EntityType::AttributeDefinition(def) = loaded.get_entity(definition).unwrap() else {
        panic!()
    };
    assert_eq!(def.text_style, "OuterStyle");
    assert_eq!(def.embedded_mtext.as_ref().unwrap().style, "EmbeddedStyle");
    assert_eq!(def.default_value, "First\\PSecond");
    assert_eq!(def.tag, "TITLE");
}

#[test]
fn empty_multiline_values_override_stale_single_line_text() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut att = AttributeEntity::simple("EMPTY", "stale value");
    att.is_multiline = true;
    att.embedded_mtext = Some(Box::new(MText::with_value("", Vector3::ZERO)));
    let mut insert = Insert::new("*Model_Space", Vector3::ZERO);
    insert.attributes.push(att);
    let insert = doc.add_entity(EntityType::Insert(insert)).unwrap();
    let mut def = AttributeDefinition::simple("EMPTY");
    def.default_value = "stale default".into();
    def.is_multiline = true;
    def.embedded_mtext = Some(Box::new(MText::with_value("", Vector3::ZERO)));
    let definition = doc
        .add_entity(EntityType::AttributeDefinition(def))
        .unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    let EntityType::Insert(i) = loaded.get_entity(insert).unwrap() else {
        panic!()
    };
    assert_eq!(i.attributes[0].value, "");
    let EntityType::AttributeDefinition(d) = loaded.get_entity(definition).unwrap() else {
        panic!()
    };
    assert_eq!(d.default_value, "");
}

#[test]
fn legacy_multiline_tag_comes_from_the_roundtrip_record() {
    use acadrust::objects::{XRecordEntry, XRecordValue};
    let mut doc = CadDocument::with_version(DxfVersion::AC1021);
    let handle = doc
        .add_entity(EntityType::AttributeDefinition(
            AttributeDefinition::simple("NOTE_001"),
        ))
        .unwrap();
    doc.ensure_xrecord(handle, "ACAD_MLATT");
    doc.xrecord_mut(handle, "ACAD_MLATT").unwrap().entries = vec![
        XRecordEntry {
            code: 70,
            value: XRecordValue::Int16(4),
        },
        XRecordEntry {
            code: 2,
            value: XRecordValue::String("NOTE".into()),
        },
        XRecordEntry {
            code: 1,
            value: XRecordValue::String("Embedded Object".into()),
        },
    ];
    let loaded = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    let EntityType::AttributeDefinition(d) = loaded.get_entity(handle).unwrap() else {
        panic!()
    };
    assert_eq!(d.tag, "NOTE");
    assert!(d.common.raw_record.is_some(), "Derived tag resolution must preserve raw records");
    assert!(loaded.xrecord(handle, "ACAD_MLATT").is_some());
}
