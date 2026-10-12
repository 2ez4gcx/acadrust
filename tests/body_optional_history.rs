use acadrust::entities::UnknownEntity;
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::types::Transparency;
use acadrust::{CadDocument, Color, DwgReader, DwgWriter, DxfVersion, EntityType, Handle};
use std::io::Cursor;

// Assemble a small BODY independently of the production BODY writer. Native
// R2007 records can omit the history slot entirely; a truncated present slot
// must still fail validation.
#[test]
fn r2007_body_accepts_absent_history_but_rejects_a_truncated_handle() {
    for history in [0, 1, 2] {
        let version = DxfVersion::AC1021;
        let mut doc = CadDocument::with_version(version);
        let id = doc.allocate_handle();
        let mut writer =
            DwgMergedWriter::new(DwgVersion::from_dxf_version(version).unwrap(), version);
        writer.write_object_type(39);
        writer.save_position_for_size();
        writer.main_mut().write_handle_undefined(id.value());
        writer.write_bit_short(0); // no EED
        writer.write_bit(false); // no preview
        writer.main_mut().write_2bits(2); // model space
        writer.write_bit_long(0); // no reactors
        writer.write_bit(true); // no extension dictionary
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
        writer.write_bit_short(0); // visible
        writer.write_byte(29); // ByLayer lineweight
        writer.write_bit(true); // empty ACIS
        writer.write_bit(false); // no wireframe
        writer.write_bit(true); // no extra body
        writer.write_bit_long(0); // trailing R2007 value
        if history == 1 {
            writer.write_handle(DwgReferenceType::SoftPointer, 0x1234);
        } else if history == 2 {
            writer.handle_mut().write_byte(0x45); // announces five absent bytes
        }
        let raw = writer.merge();
        let mut entity = UnknownEntity::new("BODY");
        entity.common.handle = id;
        entity.dwg_type_code = 39;
        entity.raw_dwg_data = Some(raw);
        entity.dwg_source_version = Some(version);
        doc.add_entity(EntityType::Unknown(entity)).unwrap();
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let result = DwgReader::from_stream(Cursor::new(bytes)).read();
        if history == 2 {
            assert!(
                result.is_err(),
                "a present malformed history handle must fail"
            );
            continue;
        }
        let restored = result.unwrap();
        let EntityType::Body(body) = restored.get_entity(id).unwrap() else {
            panic!("missing BODY")
        };
        assert_eq!(
            body.history_handle,
            (history == 1).then(|| Handle::new(0x1234))
        );
    }
}
