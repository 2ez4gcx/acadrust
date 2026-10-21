//! Frame-only R2018 MTEXT must not shift the annotative and column fields.

use std::collections::HashMap;

use acadrust::entities::MTextColumnData;
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::{entities::read_mtext, DwgObjectReader};
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::io::dwg::dwg_stream_writers::{DwgBitWriter, DwgMergedWriter};
use acadrust::io::dwg::dwg_version::DwgVersion;
use acadrust::tables::BlockRecord;
use acadrust::{CadDocument, Color, DxfVersion, EntityType, MText, Vector3};

const VERSION: DxfVersion = DxfVersion::AC1032;
const INSERTION: Vector3 = Vector3::new(12.0, 23.0, 4.0);

fn columns() -> MTextColumnData {
    MTextColumnData {
        column_type: 2,
        column_count: 2,
        flow_reversed: true,
        auto_height: false,
        width: 12.0,
        gutter: 1.25,
        heights: vec![18.0, 21.0],
    }
}

fn assert_column_tail(reader: &mut DwgMergedReader) {
    assert_eq!(reader.read_bit_short(), 2, "dynamic column type");
    assert_eq!(reader.read_bit_long(), 2, "column height count");
    assert_eq!(reader.read_bit_double(), 12.0);
    assert_eq!(reader.read_bit_double(), 1.25);
    assert!(!reader.read_bit());
    assert!(reader.read_bit());
    assert_eq!(reader.read_bit_double(), 18.0);
    assert_eq!(reader.read_bit_double(), 21.0);
}

fn assert_generated_layout(block_owned: bool) {
    for flags in [0, 1, 16, 17] {
        for annotative in [false, true] {
            let mut doc = CadDocument::with_version(VERSION);
            let owner = if block_owned {
                let mut block = BlockRecord::new("FramedText");
                block.handle = doc.allocate_handle();
                block.block_entity_handle = doc.allocate_handle();
                block.block_end_handle = doc.allocate_handle();
                let owner = block.handle;
                doc.block_records.add(block).unwrap();
                owner
            } else {
                doc.header.model_space_block_handle
            };
            let mut text = MText::with_value("Frame regression", INSERTION);
            text.common.owner_handle = owner;
            text.height = 2.5;
            text.rectangle_width = 30.0;
            text.extents_width = 9.75;
            text.extents_height = 2.75;
            text.background_fill_flags = flags;
            text.background_scale = 2.25;
            text.background_color = Color::Rgb {
                r: 31,
                g: 79,
                b: 127,
            };
            text.background_transparency = 0x0200_0080;
            text.is_annotative = annotative;
            text.column_data = columns();
            let handle = doc.add_entity(EntityType::MText(Box::new(text))).unwrap();

            let (data, handles, _, _) = DwgObjectWriter::new(&doc).unwrap().write();
            let offset = handles
                .iter()
                .find(|(h, _)| *h == handle.value())
                .unwrap()
                .1;
            let handles: HashMap<_, _> = handles
                .into_iter()
                .map(|(h, p)| (h, i64::from(p)))
                .collect();
            let objects = DwgObjectReader::new(data, VERSION, handles).unwrap();
            let (code, mut reader) = objects.read_record_at(offset as usize).unwrap();
            let common = objects.read_common_entity_data(&mut reader, code);
            assert_eq!(common.common.handle, handle.value());
            assert_eq!(common.entity_mode, if block_owned { 0 } else { 2 });
            if block_owned {
                assert_eq!(common.owner_handle, owner.value());
            }

            // Decode independently of read_mtext: a mirrored reader/writer bug
            // must not make an invalid background block appear to round-trip.
            assert_eq!(reader.read_3bit_double(), INSERTION);
            assert_eq!(reader.read_3bit_double(), Vector3::new(0.0, 0.0, 1.0));
            assert_eq!(reader.read_3bit_double(), Vector3::new(1.0, 0.0, 0.0));
            assert_eq!(reader.read_bit_double(), 30.0);
            assert_eq!(reader.read_bit_double(), 0.0);
            assert_eq!(reader.read_bit_double(), 2.5);
            assert_eq!(reader.read_bit_short(), 1);
            assert_eq!(reader.read_bit_short(), 1);
            assert_eq!(reader.read_bit_double(), 2.75);
            assert_eq!(reader.read_bit_double(), 9.75);
            assert_eq!(reader.read_variable_text(), "Frame regression");
            assert_eq!(
                reader.read_handle(),
                doc.text_styles.get("Standard").unwrap().handle.value()
            );
            assert_eq!(reader.read_bit_short(), 1);
            assert_eq!(reader.read_bit_double(), 1.0);
            assert!(!reader.read_bit());
            assert_eq!(reader.read_bit_long(), flags);
            if flags & 1 != 0 {
                assert_eq!(reader.read_bit_double(), 2.25);
                assert_eq!(
                    reader.read_cm_color(),
                    Color::Rgb {
                        r: 31,
                        g: 79,
                        b: 127
                    }
                );
                assert_eq!(reader.read_bit_long(), 0x0200_0080);
            }
            assert_eq!(
                reader.read_bit(),
                !annotative,
                "flags={flags}, annotative={annotative}"
            );
            if !annotative {
                assert_eq!(reader.read_bit_short(), 4);
                assert!(reader.read_bit());
                assert_eq!(reader.read_handle(), 0);
                assert_eq!(reader.read_bit_long(), 1);
                assert_eq!(reader.read_3bit_double(), Vector3::new(1.0, 0.0, 0.0));
                assert_eq!(reader.read_3bit_double(), INSERTION);
                assert_eq!(reader.read_bit_double(), 30.0);
                assert_eq!(reader.read_bit_double(), 0.0);
                let _ = reader.read_bit_double();
                let _ = reader.read_bit_double();
                assert_column_tail(&mut reader);
            }
            assert_eq!(
                reader.main_remaining_bits(),
                0,
                "flags={flags}, annotative={annotative}"
            );
            assert!(reader.check_error().is_ok());
        }
    }
}

#[test]
fn r2018_model_space_mtext_frame_has_no_fill_payload() {
    assert_generated_layout(false);
}

#[test]
fn r2018_block_owned_mtext_frame_has_no_fill_payload() {
    assert_generated_layout(true);
}

fn inline_frame_reader(fill_payload: bool, annotative: bool) -> DwgMergedReader {
    let version = DwgVersion::from_dxf_version(VERSION).unwrap();
    let mut writer = DwgMergedWriter::new(version, VERSION);
    writer.write_3bit_double(INSERTION);
    writer.write_3bit_double(Vector3::new(0.0, 0.0, 1.0));
    writer.write_3bit_double(Vector3::new(1.0, 0.0, 0.0));
    writer.write_bit_double(30.0);
    writer.write_bit_double(0.0);
    writer.write_bit_double(2.5);
    writer.write_bit_short(1);
    writer.write_bit_short(5);
    writer.write_bit_double(2.75);
    writer.write_bit_double(9.75);
    writer.write_variable_text("Inline native frame");
    writer.write_handle(DwgReferenceType::HardPointer, 0x1234);
    writer.write_bit_short(1);
    writer.write_bit_double(1.0);
    writer.write_bit(false);
    writer.write_bit_long(16);
    if fill_payload {
        writer.write_bit_double(2.25);
        writer.write_cm_color(&Color::Rgb {
            r: 31,
            g: 79,
            b: 127,
        });
        writer.write_bit_long(0x0200_0080);
    }
    writer.write_bit(!annotative);
    if !annotative {
        writer.write_bit_short(4);
        writer.write_bit(true);
        writer.write_handle(DwgReferenceType::HardPointer, 0x5678);
        writer.write_bit_long(1);
        writer.write_3bit_double(Vector3::new(1.0, 0.0, 0.0));
        writer.write_3bit_double(INSERTION);
        for value in [30.0, 0.0, 9.75, 2.75] {
            writer.write_bit_double(value);
        }
        writer.write_bit_short(2);
        writer.write_bit_long(2);
        writer.write_bit_double(12.0);
        writer.write_bit_double(1.25);
        writer.write_bit(false);
        writer.write_bit(true);
        writer.write_bit_double(18.0);
        writer.write_bit_double(21.0);
    }
    let end = writer.main().position_in_bits();
    let main = DwgBitReader::new(writer.main().to_bytes_snapshot(), version, VERSION);
    let text = DwgBitReader::new(writer.text().to_bytes_snapshot(), version, VERSION);
    let handles = DwgBitReader::new(writer.handle_writer().to_bytes_snapshot(), version, VERSION);
    let mut reader = DwgMergedReader::from_readers(main, Some(text), Some(handles), VERSION);
    reader.set_main_data_end(end);
    reader
}

#[test]
fn r2018_frame_reader_accepts_inline_payload_variants() {
    for fill_payload in [false, true] {
        for annotative in [false, true] {
            let mut reader = inline_frame_reader(fill_payload, annotative);
            let text = read_mtext(
                &mut reader,
                DwgVersion::from_dxf_version(VERSION).unwrap(),
                VERSION,
            );
            assert_eq!(text.background_flags, 16);
            assert_eq!(text.value, "Inline native frame");
            assert_eq!(text.insertion_point, INSERTION);
            assert_eq!(text.extents_width, 9.75);
            assert_eq!(text.extents_height, 2.75);
            assert_eq!(text.style_handle, 0x1234);
            assert_eq!(text.background_scale, if fill_payload { 2.25 } else { 1.5 });
            assert_eq!(
                text.background_color,
                if fill_payload {
                    Color::Rgb {
                        r: 31,
                        g: 79,
                        b: 127,
                    }
                } else {
                    Color::ByLayer
                }
            );
            assert_eq!(
                text.background_transparency,
                if fill_payload { 0x0200_0080 } else { 0 }
            );
            assert_eq!(text.is_annotative, annotative);
            if !annotative {
                assert_eq!(text.column_type, 2);
                assert_eq!(text.column_count, 2);
                assert_eq!(text.column_width, 12.0);
                assert_eq!(text.column_gutter, 1.25);
                assert_eq!(text.column_heights, vec![18.0, 21.0]);
                assert!(text.column_flow_reversed);
                assert!(!text.column_auto_height);
            }
            assert_eq!(reader.main_remaining_bits(), 0);
            assert_eq!(reader.handle_remaining_bits(), 0);
            assert!(reader.check_error().is_ok());
        }
    }
}

fn skip_mtext_fields_before_background(reader: &mut DwgMergedReader) {
    for _ in 0..3 {
        reader.read_3bit_double();
    }
    for _ in 0..3 {
        reader.read_bit_double();
    }
    for _ in 0..2 {
        reader.read_bit_short();
    }
    for _ in 0..2 {
        reader.read_bit_double();
    }
    reader.read_variable_text();
    reader.read_handle();
    reader.read_bit_short();
    reader.read_bit_double();
    reader.read_bit();
}

#[test]
fn truncated_native_frame_fill_payload_is_rejected() {
    let mut probe = inline_frame_reader(true, false);
    skip_mtext_fields_before_background(&mut probe);
    assert_eq!(probe.read_bit_long(), 16);
    let fill_start = probe.position_in_bits();
    for partial_payload_bits in [2, 32, 70] {
        let mut reader = inline_frame_reader(true, false);
        reader.set_main_data_end(fill_start + partial_payload_bits);
        read_mtext(
            &mut reader,
            DwgVersion::from_dxf_version(VERSION).unwrap(),
            VERSION,
        );
        assert!(
            reader.check_error().is_err(),
            "accepted truncated fill payload of {partial_payload_bits} bits"
        );
    }
}

#[test]
fn r2018_native_frame_raw_record_is_normalized_on_rewrite() {
    let version = DwgVersion::from_dxf_version(VERSION).unwrap();
    let mut doc = CadDocument::with_version(VERSION);
    let mut text = MText::with_value("Existing frame", INSERTION);
    text.background_fill_flags = 17;
    text.background_scale = 2.25;
    let handle = doc.add_entity(EntityType::MText(Box::new(text))).unwrap();
    let (data, handles, _, _) = DwgObjectWriter::new(&doc).unwrap().write();
    let offset = handles
        .iter()
        .find(|(h, _)| *h == handle.value())
        .unwrap()
        .1;
    let objects = DwgObjectReader::new(
        data,
        VERSION,
        handles
            .into_iter()
            .map(|(h, p)| (h, i64::from(p)))
            .collect(),
    )
    .unwrap();
    let (code, mut reader) = objects.read_record_at(offset as usize).unwrap();
    objects.read_common_entity_data(&mut reader, code);
    skip_mtext_fields_before_background(&mut reader);
    let flags_position = reader.position_in_bits();
    assert_eq!(reader.read_bit_long(), 17);

    // Both flags use the same BL width. Keep the generated fill payload to
    // construct a native-style frame-only record without a fixture file.
    let mut raw = DwgBitWriter::new(version, VERSION);
    raw.write_bytes(&reader.raw_merged_data());
    raw.set_position_in_bits(flags_position);
    raw.write_bit_long(16);
    let EntityType::MText(text) = doc.get_entity_mut(handle).unwrap() else {
        panic!("expected MTEXT");
    };
    text.background_fill_flags = 16;
    text.common.raw_record = Some(
        acadrust::entities::RawRecord {
            data: raw.to_bytes_snapshot(),
            handle_bits: reader.get_handle_bits(),
            version: VERSION,
        }
        .into(),
    );

    let (data, handles, _, _) = DwgObjectWriter::new(&doc).unwrap().write();
    let offset = handles
        .iter()
        .find(|(h, _)| *h == handle.value())
        .unwrap()
        .1;
    let objects = DwgObjectReader::new(
        data,
        VERSION,
        handles
            .into_iter()
            .map(|(h, p)| (h, i64::from(p)))
            .collect(),
    )
    .unwrap();
    let (code, mut reader) = objects.read_record_at(offset as usize).unwrap();
    objects.read_common_entity_data(&mut reader, code);
    skip_mtext_fields_before_background(&mut reader);
    assert_eq!(reader.read_bit_long(), 16);
    assert!(
        reader.read_bit(),
        "annotative flag must immediately follow frame flags"
    );
    assert_eq!(reader.read_bit_short(), 4);
}
