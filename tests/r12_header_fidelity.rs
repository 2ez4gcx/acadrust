use std::io::Cursor;

use acadrust::document::HeaderVariables;
use acadrust::io::dwg::crc::{crc16, CRC16_SEED};
use acadrust::io::dwg::r12::header::{
    read_header, write_header, HeaderLayout, HeaderReferences, TableLocator, ENTITIES_BEGIN,
    ENTITIES_END,
};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Vector3};

// Fixed offsets come from the independent CC0 AC1009 Kaitai layout:
// https://github.com/michal-josef-spacek/CAD-Format-DWG-AC1009/blob/main/dwg_ac1009.ksy
fn put_short(data: &mut [u8], offset: usize, value: i16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_long(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_double(data: &mut [u8], offset: usize, value: f64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_point(data: &mut [u8], offset: usize, point: Vector3) {
    put_double(data, offset, point.x);
    put_double(data, offset + 8, point.y);
    put_double(data, offset + 16, point.z);
}

fn header_fixture(count: u16) -> Vec<u8> {
    let variables_end = if count == 204 { 0x6bb } else { 0x6bd };
    let mut data = vec![0; variables_end];
    data[..6].copy_from_slice(b"AC1009");
    data[0xb] = 7;
    data[0xc] = 1;
    put_short(&mut data, 0xd, 3);
    put_short(&mut data, 0xf, 5);
    put_short(&mut data, 0x11, count as i16);
    put_long(&mut data, 0x14, (variables_end + 18) as u32);
    put_long(&mut data, 0x18, (variables_end + 18) as u32);
    put_point(&mut data, 0x5e, Vector3::new(12.0, -34.0, 56.0));
    put_short(&mut data, 0x128, 1); // ORTHOMODE
    put_double(&mut data, 0x132, 2.75); // LTSCALE
    put_short(&mut data, 0x164, 4); // LUNITS
    put_short(&mut data, 0x190, 513); // OSMODE
    put_double(&mut data, 0x229, 9.25); // ELEVATION
    put_short(&mut data, 0x31d, -1); // CELTYPE / ByLayer
    put_short(&mut data, 0x3f9, 30); // ANSI_1252
    put_point(&mut data, 0x3fb, Vector3::new(2.0, 3.0, 4.0)); // UCSORG
    put_double(&mut data, 0x45b, 85.0); // LENSLENGTH
    put_double(&mut data, 0x463, 0.75); // VIEWTWIST
    put_short(&mut data, 0x4ec, 1); // HANDLING
    data[0x4ee..0x4f6].copy_from_slice(&42_u64.to_be_bytes());
    put_short(&mut data, 0x510, -1); // UCSNAME
    put_short(&mut data, 0x62f, -1); // PUCSNAME
    put_short(&mut data, 0x631, 1); // TILEMODE
    put_short(&mut data, 0x6a9, 12); // MAXACTVP
    put_double(&mut data, 0x6ab, 0.45); // DIMGAP
    put_double(&mut data, 0x6b3, 4.5); // PELEVATION
    if count == 205 {
        put_short(&mut data, 0x6bb, 0); // VISRETAIN
    }
    finish_header(data)
}

fn finish_header(mut data: Vec<u8>) -> Vec<u8> {
    let checksum = crc16(CRC16_SEED, &data);
    data.extend_from_slice(&checksum.to_le_bytes());
    data.extend_from_slice(&ENTITIES_BEGIN);
    data.extend_from_slice(&ENTITIES_END);
    data
}

fn rewrite_crc(data: &mut [u8], count: u16) {
    let end = if count == 204 { 0x6bb } else { 0x6bd };
    let checksum = crc16(CRC16_SEED, &data[..end]);
    data[end..end + 2].copy_from_slice(&checksum.to_le_bytes());
}

fn read_document(data: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(data)).read().unwrap()
}

#[test]
fn independent_204_and_205_headers_decode_nondefault_values_and_honor_edits() {
    for count in [204, 205] {
        let fixture = header_fixture(count);
        let decoded = read_header(&fixture).unwrap();
        assert_eq!(decoded.num_header_variables, count);
        assert_eq!(decoded.maintenance_version, 7);
        assert_eq!(
            decoded.variables.model_space_insertion_base,
            Vector3::new(12.0, -34.0, 56.0)
        );
        assert!(decoded.variables.ortho_mode);
        assert_eq!(decoded.variables.linetype_scale, 2.75);
        assert_eq!(decoded.variables.linear_unit_format, 4);
        assert_eq!(decoded.variables.object_snap_mode, 513);
        assert_eq!(decoded.variables.elevation, 9.25);
        assert_eq!(
            decoded.variables.model_space_ucs_origin,
            Vector3::new(2.0, 3.0, 4.0)
        );
        assert_eq!(decoded.variables.lens_length, 85.0);
        assert_eq!(decoded.variables.view_twist, 0.75);
        assert_eq!(decoded.variables.max_active_viewports, 12);
        assert_eq!(decoded.variables.dim_line_gap, 0.45);
        assert_eq!(decoded.variables.paper_elevation, 4.5);
        assert_eq!(decoded.variables.retain_xref_visibility, count == 204);

        let mut document = read_document(fixture);
        document.header.linetype_scale = 6.25;
        document.header.view_twist = 1.5;
        document.header.ortho_mode = false;
        let saved = DwgWriter::write_to_vec(&document).unwrap();
        assert_eq!(&saved[0x132..0x13a], &6.25_f64.to_le_bytes());
        assert_eq!(&saved[0x463..0x46b], &1.5_f64.to_le_bytes());
        assert_eq!(&saved[0x128..0x12a], &[0, 0]);
        assert_eq!(
            read_header(&saved).unwrap().num_header_variables,
            if count == 204 { 204 } else { 205 }
        );
        let restored = read_document(saved);
        assert_eq!(restored.header.linetype_scale, 6.25);
        assert_eq!(restored.header.view_twist, 1.5);
        assert!(!restored.header.ortho_mode);
    }
}

#[test]
fn document_saves_preserve_every_unmapped_native_header_span() {
    let spans = [
        0xc8..0x128,  // active view, snap and grid
        0x14c..0x156, // converted legacy colors and unknown field
        0x15a..0x164, // unknown field and view aspect ratio
        0x168..0x17a, // axis mode and spacing
        0x1fb..0x1fc, // reserved byte before MENU continuation
        0x239..0x2e3, // direction, view coordinate bases and 3D flag
        0x317..0x319, // circle zoom
        0x341..0x343, // fast zoom
        0x345..0x353, // legacy date components
        0x443..0x45b, // view target
        0x46b..0x47d, // clipping and view mode
        0x4cb..0x4ec, // unknown fixed string
        0x50a..0x50c, // flatland
        0x50e..0x510, // UCS icon
        0x51e..0x522, // unknown fields before DIMSTYLE locator
        0x52c..0x531, // unknown fields after DIMSTYLE locator
        0x53b..0x53d, // unknown field before typed user-unit settings
        0x635..0x637, // unknown paper-space field
    ];
    for count in [204, 205] {
        let mut fixture = header_fixture(count);
        for span in &spans {
            for (index, byte) in fixture[span.clone()].iter_mut().enumerate() {
                *byte = ((span.start + index) % 251 + 1) as u8;
            }
        }
        rewrite_crc(&mut fixture, count);
        let mut document = read_document(fixture.clone());
        document.header.linetype_scale = 12.5;
        for _ in 0..3 {
            let saved = DwgWriter::write_to_vec(&document).unwrap();
            for span in &spans {
                assert_eq!(
                    &saved[span.clone()],
                    &fixture[span.clone()],
                    "span {span:x?}"
                );
            }
            assert_eq!(&saved[0x132..0x13a], &12.5_f64.to_le_bytes());
            document = read_document(saved);
        }
    }
}

#[test]
fn menu_joins_independently_encoded_spans_before_decoding() {
    let samples: &[(&[u8], i16, &str)] = &[
        (
            b"12345678901234\x82\xa0suffix",
            22,
            "12345678901234\u{3042}suffix",
        ),
        (
            b"12345678901234\\U+4E2Dsuffix",
            30,
            "12345678901234\u{4e2d}suffix",
        ),
        (
            b"123456789012345abcdefghijklmnop",
            30,
            "123456789012345abcdefghijklmnop",
        ),
    ];
    for &(bytes, page, expected) in samples {
        // Native fixed spans accept both a terminated prefix and all 15 bytes.
        for split in [14, 15] {
            let mut fixture = header_fixture(205);
            put_short(&mut fixture, 0x3f9, page);
            fixture[0x194..0x194 + split].copy_from_slice(&bytes[..split]);
            fixture[0x1fc..0x1fc + bytes.len() - split].copy_from_slice(&bytes[split..]);
            rewrite_crc(&mut fixture, 205);
            assert_eq!(read_header(&fixture).unwrap().variables.menu_name, expected);
            let document = read_document(fixture);
            assert_eq!(document.header.menu_name, expected);
            let saved = DwgWriter::write_to_vec(&document).unwrap();
            assert_eq!(read_document(saved).header.menu_name, expected);
        }
    }
}

#[test]
fn menu_writes_the_full_native_capacity_and_clears_stale_continuation() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    for length in [0, 14, 15, 16, 59, 60] {
        document.header.menu_name = "M".repeat(length);
        let saved = DwgWriter::write_to_vec(&document).unwrap();
        let split = length.min(15);
        assert_eq!(&saved[0x194..0x194 + split], &vec![b'M'; split]);
        assert_eq!(
            &saved[0x1fc..0x1fc + length - split],
            &vec![b'M'; length - split]
        );
        document = read_document(saved);
        assert_eq!(document.header.menu_name, "M".repeat(length));
    }
    document.header.menu_name = "NEW".into();
    let saved = DwgWriter::write_to_vec(&document).unwrap();
    assert_eq!(&saved[0x194..0x1a3], b"NEW\0\0\0\0\0\0\0\0\0\0\0\0");
    assert_eq!(&saved[0x1fc..0x229], &[0; 45]);
}

#[test]
fn menu_limits_use_encoded_bytes_and_reject_nul() {
    let mut variables = HeaderVariables::default();
    variables.code_page = "ANSI_1252".into();
    for sample in ["M".repeat(61), "M".repeat(54) + "\u{4e2d}", "A\0B".into()] {
        variables.menu_name = sample;
        let error = write_header(
            &variables,
            HeaderLayout::default(),
            &[TableLocator::default(); 10],
            HeaderReferences::default(),
            0,
            None,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("MENU must fit in 60 encoded bytes"));
    }
    variables.menu_name = "M".repeat(53) + "\u{4e2d}";
    let saved = write_header(
        &variables,
        HeaderLayout::default(),
        &[TableLocator::default(); 10],
        HeaderReferences::default(),
        0,
        None,
    )
    .unwrap();
    assert_eq!(
        read_header(&saved).unwrap().variables.menu_name,
        variables.menu_name
    );
}
