use acadrust::io::dxf::{DxfBinaryWriter, DxfStreamWriter};

const BINARY_DXF_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

#[derive(Clone, Copy, Debug)]
enum IntegerWriter {
    Byte,
    I16,
    I32,
    I64,
}

const INTEGER_WRITERS: [IntegerWriter; 4] = [
    IntegerWriter::Byte,
    IntegerWriter::I16,
    IntegerWriter::I32,
    IntegerWriter::I64,
];

const ISSUE_CODES: [i32; 15] = [70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 90, 92, 93, 170, 370];

// These ranges encode the DXF binary layout independently from acadrust.
fn expected_width(code: i32) -> usize {
    match code {
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => 8,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => 2,
        90..=99 | 420..=429 | 440..=459 | 1071 => 4,
        160..=169 => 8,
        290..=299 => 1,
        _ => panic!("test group code {code} is not in the width table"),
    }
}

fn source_value(writer: IntegerWriter, code: i32) -> i64 {
    if (290..=299).contains(&code) {
        return 1;
    }

    match writer {
        IntegerWriter::Byte => 0xa5,
        IntegerWriter::I16 => -0x1234,
        IntegerWriter::I32 | IntegerWriter::I64 if expected_width(code) == 2 => -0x1234,
        IntegerWriter::I32 => -0x1234_5678,
        IntegerWriter::I64 if expected_width(code) == 4 => -0x1234_5678,
        IntegerWriter::I64 => -0x0123_4567_89ab_cdef,
    }
}

fn write_integer(
    writer: &mut impl DxfStreamWriter,
    writer_kind: IntegerWriter,
    code: i32,
    value: i64,
) -> acadrust::Result<()> {
    match writer_kind {
        IntegerWriter::Byte => writer.write_byte(code, value as u8),
        IntegerWriter::I16 => writer.write_i16(code, value as i16),
        IntegerWriter::I32 => writer.write_i32(code, value as i32),
        IntegerWriter::I64 => writer.write_i64(code, value),
    }
}

fn write_record(writer_kind: IntegerWriter, code: i32, value: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut writer = DxfBinaryWriter::new(&mut bytes).expect("sentinel write should succeed");
        write_integer(&mut writer, writer_kind, code, value)
            .expect("integer group should write successfully");
    }
    bytes
}

fn expected_record(code: i32, value: i64) -> Vec<u8> {
    let mut expected = BINARY_DXF_SENTINEL.to_vec();
    expected.extend_from_slice(&(code as i16).to_le_bytes());

    match (code, expected_width(code)) {
        (10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059, 8) => {
            expected.extend_from_slice(&(value as f64).to_le_bytes())
        }
        (_, 1) => expected.push(value as u8),
        (_, 2) => expected.extend_from_slice(&(value as i16).to_le_bytes()),
        (_, 4) => expected.extend_from_slice(&(value as i32).to_le_bytes()),
        (_, 8) => expected.extend_from_slice(&value.to_le_bytes()),
        (_, width) => panic!("unsupported expected width {width}"),
    }

    expected
}

#[test]
fn integer_writers_follow_binary_widths_for_all_issue_136_codes() {
    for writer_kind in INTEGER_WRITERS {
        for code in ISSUE_CODES {
            let value = source_value(writer_kind, code);
            let actual = write_record(writer_kind, code, value);
            let expected = expected_record(code, value);
            assert_eq!(
                actual, expected,
                "{writer_kind:?} group {code} must use its DXF binary width"
            );
        }
    }
}

#[test]
fn integer_writers_follow_numeric_group_range_boundaries() {
    let boundary_codes = [
        10, 59, 60, 79, 90, 99, 110, 149, 160, 169, 170, 179, 210, 239, 270, 279, 280, 289, 290,
        299, 370, 389, 400, 409, 420, 429, 440, 449, 450, 459, 460, 469, 1010, 1059, 1060, 1070,
        1071,
    ];
    for writer_kind in INTEGER_WRITERS {
        for code in boundary_codes {
            let value = source_value(writer_kind, code);
            let actual = write_record(writer_kind, code, value);
            let expected = expected_record(code, value);
            assert_eq!(
                actual, expected,
                "{writer_kind:?} group {code} must use its DXF binary width"
            );
        }
    }
}

#[test]
fn integer_writers_reject_nonnumeric_groups_without_partial_records() {
    for writer_kind in INTEGER_WRITERS {
        for code in [0, 5, 100, 310, 330, 999, 1000, -1, 1080] {
            let mut writer = DxfBinaryWriter::new(Vec::new()).unwrap();
            let result = write_integer(&mut writer, writer_kind, code, 1);
            assert!(
                matches!(result, Err(acadrust::DxfError::InvalidDxfCode(value)) if value == code),
                "{writer_kind:?} must reject nonnumeric group {code}"
            );
            assert_eq!(
                writer.into_inner(),
                BINARY_DXF_SENTINEL,
                "{writer_kind:?} must not partially write nonnumeric group {code}"
            );
        }
    }
}

#[test]
fn write_bool_still_uses_group_widths() {
    for (code, expected_value) in [(290, vec![1]), (280, vec![1, 0]), (70, vec![1, 0])] {
        let mut actual = Vec::new();
        {
            let mut writer = DxfBinaryWriter::new(&mut actual).unwrap();
            writer.write_bool(code, true).unwrap();
        }

        let mut expected = BINARY_DXF_SENTINEL.to_vec();
        expected.extend_from_slice(&(code as i16).to_le_bytes());
        expected.extend_from_slice(&expected_value);
        assert_eq!(actual, expected, "boolean group {code} has the wrong width");
    }
}
