use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::entities::{ExtendedEntityData, Line, ProxyEntityData, UnknownEntity};
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::objects::{ProxyPayload, ProxyReferenceKind};
use acadrust::tables::AppId;
use acadrust::types::Transparency;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{
    CadDocument, Color, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, EntityType, Handle,
    Vector3,
};

fn graphics() -> Vec<u8> {
    let mut payload = 8u32.to_le_bytes().to_vec();
    for index in 0..8 {
        for value in [index as f64, (index % 2) as f64, 0.0] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
    }
    let mut record = ((payload.len() + 8) as u32).to_le_bytes().to_vec();
    record.extend_from_slice(&6u32.to_le_bytes());
    record.extend(payload);
    let mut result = ((record.len() + 8) as u32).to_le_bytes().to_vec();
    result.extend_from_slice(&1u32.to_le_bytes());
    result.extend(record);
    result
}

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn proxy(doc: &CadDocument, handle: Handle) -> &ProxyEntityData {
    let Some(EntityType::Extended(entity)) = doc.get_entity(handle) else {
        panic!("unknown entity {handle} disappeared during DXF conversion");
    };
    let ExtendedEntityData::Proxy(data) = &entity.data else {
        panic!("expected a standard proxy entity");
    };
    data
}

#[test]
fn graphics_only_unknown_entities_survive_ascii_and_binary_dxf() {
    for version in [
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut doc = CadDocument::with_version(version);
        let line = doc
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::ZERO,
                Vector3::UNIT_X,
            )))
            .unwrap();
        let mut app = AppId::new("VENDOR_DATA");
        app.handle = doc.allocate_handle();
        doc.app_ids.add(app).unwrap();
        let mut custom = UnknownEntity::new("MYVENDOR_ENTITY");
        custom.common.graphic_data = Some(graphics());
        custom.common.color = Color::from_rgb(12, 34, 56);
        custom.common.linetype_scale = 2.5;
        let mut xdata = ExtendedDataRecord::new("VENDOR_DATA");
        xdata.values.push(XDataValue::String("preserved".into()));
        custom.common.extended_data.add_record(xdata);
        assert_eq!(custom.common.proxy_graphics().unwrap().records.len(), 1);
        let handle = doc.add_entity(EntityType::Unknown(custom)).unwrap();
        for binary in [false, true] {
            let writer = if binary {
                DxfWriter::new_binary(&doc)
            } else {
                DxfWriter::new(&doc)
            };
            let bytes = writer.write_to_vec().unwrap();
            if !binary {
                let text = std::str::from_utf8(&bytes).unwrap();
                let lines: Vec<_> = text.lines().collect();
                let pairs: Vec<_> = lines
                    .chunks_exact(2)
                    .map(|pair| (pair[0].trim().parse::<i32>().unwrap(), pair[1].trim()))
                    .collect();
                let start = pairs
                    .iter()
                    .position(|pair| *pair == (0, "ACAD_PROXY_ENTITY"))
                    .unwrap();
                let subclass = pairs[start..]
                    .iter()
                    .position(|pair| *pair == (100, "AcDbProxyEntity"))
                    .unwrap()
                    + start;
                assert!(!pairs[start..subclass]
                    .iter()
                    .any(|(code, _)| [92, 160, 310].contains(code)));
                let end = pairs[subclass..]
                    .iter()
                    .position(|(code, _)| *code == 0)
                    .unwrap()
                    + subclass;
                assert_eq!(
                    pairs[subclass..end]
                        .iter()
                        .filter(|(code, _)| *code == 310)
                        .count(),
                    graphics().len().div_ceil(127)
                );
            }
            let loaded = read_dxf(bytes);
            let data = proxy(&loaded, handle);
            assert_eq!(data.proxy_id, 498);
            assert_eq!(data.class_id, 499);
            assert_eq!(data.graphics.data(), graphics());
            assert_eq!(data.payload.bit_count, 0);
            assert!(loaded.get_entity(line).is_some());
            let common = loaded.get_entity(handle).unwrap().common();
            assert_eq!(common.owner_handle, doc.header.model_space_block_handle);
            assert_eq!(common.color, Color::from_rgb(12, 34, 56));
            assert_eq!(common.linetype_scale, 2.5);
            assert!(common
                .extended_data
                .records()
                .iter()
                .any(|record| record.application_name == "VENDOR_DATA"
                    && record
                        .values
                        .contains(&XDataValue::String("preserved".into()))));
        }
    }
}

fn opaque_drawing(
    version: DxfVersion,
    with_graphics: bool,
) -> (CadDocument, Handle, ProxyPayload, ProxyPayload, Handle) {
    let mut doc = CadDocument::with_version(version);
    let reference = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::UNIT_X,
        )))
        .unwrap();
    doc.classes
        .add_or_update(DxfClass::new_entity("MYVENDOR_ENTITY", "MyVendorEntity"));
    let class_id = doc
        .classes
        .get_by_name("MYVENDOR_ENTITY")
        .unwrap()
        .class_number;
    let handle = doc.allocate_handle();
    let dwg = DwgVersion::from_dxf_version(version).unwrap();
    let mut writer = DwgMergedWriter::new(dwg, version);
    writer.write_object_type(class_id);
    writer.main_mut().write_handle_undefined(handle.value());
    writer.write_bit_short(0); // no EED
    writer.write_bit(with_graphics);
    if with_graphics {
        writer.write_bit_long_long(graphics().len() as i64);
        writer.write_bytes(&graphics());
    }
    writer.main_mut().write_2bits(2); // model space
    writer.write_bit_long(0); // no reactors
    writer.write_bit(true); // no extension dictionary
    writer.write_bit(false); // no data-store record
    writer.write_en_color(&Color::ByLayer, &Transparency::ByLayer);
    writer.write_bit_double(1.0);
    writer.write_handle(
        DwgReferenceType::HardPointer,
        doc.layers.get("0").unwrap().handle.value(),
    );
    writer.main_mut().write_2bits(0); // ByLayer linetype
    writer.main_mut().write_2bits(0); // ByLayer material
    writer.write_byte(0); // shadow flags
    writer.main_mut().write_2bits(0); // ByLayer plot style
    for _ in 0..3 {
        writer.write_bit(false);
    }
    writer.write_bit_short(0); // visible
    writer.write_byte(29); // ByLayer lineweight
    let mut expected = DwgMergedWriter::new(dwg, version);
    for index in 0..13 {
        let bit = [0xb6u8, 0xa8][index / 8] & (0x80 >> (index % 8)) != 0;
        writer.write_bit(bit);
        expected.write_bit(bit);
    }
    writer.write_variable_text("vendor \u{03a9}");
    expected.write_variable_text("vendor \u{03a9}");
    writer.write_handle(DwgReferenceType::HardPointer, reference.value());
    let expected_main = ProxyPayload::from_bits(
        &expected.main().to_bytes_snapshot(),
        expected.main().position_in_bits() as u32,
    );
    let expected_text = ProxyPayload::from_bits(
        &expected.text().to_bytes_snapshot(),
        expected.text().position_in_bits() as u32,
    );
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    let mut custom = UnknownEntity::new("MYVENDOR_ENTITY");
    custom.common.handle = handle;
    custom.common.owner_handle = doc.header.model_space_block_handle;
    custom.common.graphic_data = with_graphics.then(graphics);
    custom.dwg_type_code = class_id;
    custom.raw_dwg_data = Some(raw);
    custom.dwg_handle_bits = handle_bits;
    custom.dwg_source_version = Some(version);
    doc.add_entity(EntityType::Unknown(custom)).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    assert!(matches!(
        loaded.get_entity(handle),
        Some(EntityType::Unknown(_))
    ));
    (loaded, handle, expected_main, expected_text, reference)
}

#[test]
fn unknown_dwg_class_payload_text_and_references_survive_dxf_conversion() {
    for version in [DxfVersion::AC1027, DxfVersion::AC1032] {
        for with_graphics in [false, true] {
            let (doc, handle, expected_main, expected_text, reference) =
                opaque_drawing(version, with_graphics);
            let EntityType::Unknown(original) = doc.get_entity(handle).unwrap() else {
                panic!()
            };
            for target in [DxfVersion::AC1021, DxfVersion::AC1024, DxfVersion::AC1032] {
                for binary in [false, true] {
                    let writer = if binary {
                        DxfWriter::new_binary(&doc)
                    } else {
                        DxfWriter::new(&doc)
                    }
                    .with_version(target);
                    let loaded = read_dxf(writer.write_to_vec().unwrap());
                    let data = proxy(&loaded, handle);
                    assert_eq!(data.class_id, i32::from(original.dwg_type_code));
                    assert_eq!(data.payload.bit_count, expected_main.bit_count);
                    assert_eq!(data.payload.data(), expected_main.data());
                    assert_eq!(data.text_payload.bit_count, expected_text.bit_count);
                    assert_eq!(data.text_payload.data(), expected_text.data());
                    assert_eq!(
                        data.graphics.data(),
                        if with_graphics {
                            graphics()
                        } else {
                            Vec::new()
                        }
                    );
                    assert_eq!(data.object_ids.len(), 1);
                    assert_eq!(data.object_ids[0].handle, reference);
                    assert_eq!(data.object_ids[0].kind, ProxyReferenceKind::HardPointer);
                    assert_eq!(
                        data.dwg_version,
                        if version == DxfVersion::AC1032 {
                            33
                        } else {
                            31
                        }
                    );
                    assert_eq!(data.maintenance_version, i32::from(doc.maintenance_version));
                }
            }
        }
    }
}

#[test]
fn unusable_opaque_data_keeps_graphics_without_fabricating_a_payload() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut custom = UnknownEntity::new("BROKEN_VENDOR");
    custom.common.graphic_data = Some(graphics());
    custom.raw_dwg_data = Some(vec![0xff]);
    custom.dwg_source_version = Some(DxfVersion::AC1032);
    let handle = doc.add_entity(EntityType::Unknown(custom)).unwrap();
    let loaded = read_dxf(DxfWriter::new(&doc).write_to_vec().unwrap());
    let data = proxy(&loaded, handle);
    assert_eq!(data.graphics.data(), graphics());
    assert_eq!(data.payload.bit_count, 0);
    assert_eq!(data.text_payload.bit_count, 0);
}

#[test]
fn unknown_entities_without_a_usable_representation_return_an_error() {
    for raw in [None, Some(vec![0xff])] {
        let mut doc = CadDocument::with_version(DxfVersion::AC1032);
        let mut custom = UnknownEntity::new("NO_REPRESENTATION");
        custom.raw_dwg_data = raw;
        custom.dwg_source_version = Some(DxfVersion::AC1032);
        let handle = doc.add_entity(EntityType::Unknown(custom)).unwrap();
        let error = DxfWriter::new(&doc)
            .write_to_vec()
            .err()
            .expect("entity loss was not reported")
            .to_string();
        assert!(
            error.contains("NO_REPRESENTATION") && error.contains(&handle.to_string()),
            "{error}"
        );
    }
}

#[test]
fn existing_raw_dxf_entities_keep_their_original_representation() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut custom = UnknownEntity::new("MY_DXF_ENTITY");
    custom.raw_dxf_codes = Some(vec![
        (100, "MyEntity".into()),
        (1, "preserved payload".into()),
    ]);
    let handle = doc.add_entity(EntityType::Unknown(custom)).unwrap();
    let loaded = read_dxf(DxfWriter::new(&doc).write_to_vec().unwrap());
    let Some(EntityType::Unknown(entity)) = loaded.get_entity(handle) else {
        panic!("raw DXF representation changed")
    };
    assert_eq!(entity.dxf_name, "MY_DXF_ENTITY");
    assert!(entity
        .raw_dxf_codes
        .as_ref()
        .unwrap()
        .contains(&(1, "preserved payload".into())));
}
