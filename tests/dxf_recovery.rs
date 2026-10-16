use std::io::Cursor;

use acadrust::{DxfReader, DxfReaderConfiguration};

fn malformed_entities_dxf() -> Vec<u8> {
    br#"0
SECTION
2
ENTITIES
0
LINE
5
1
10
0
BAD-CODE
x
0
POINT
5
2
10
3
20
4
0
ENDSEC
0
EOF
"#
    .to_vec()
}

#[test]
fn strict_reader_stops_on_a_malformed_record() {
    let result = DxfReader::from_reader(Cursor::new(malformed_entities_dxf()))
        .unwrap()
        .read();
    assert!(result.is_err());
}

#[test]
fn failsafe_reader_resynchronizes_to_the_next_entity() {
    let reader = DxfReader::from_reader(Cursor::new(malformed_entities_dxf()))
        .unwrap()
        .with_configuration(DxfReaderConfiguration {
            failsafe: true,
            ..Default::default()
        });
    let outcome = reader.read_with_stats().unwrap();

    assert_eq!(outcome.document.entity_count(), 1);
    assert_eq!(outcome.stats.decoded_source_records, 1);
    assert_eq!(outcome.stats.skipped_source_records, 1);
    assert!(outcome
        .stats
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "record-read-failed"));
}

#[test]
fn recover_exposes_the_surviving_entity_and_repair_report() {
    let outcome = DxfReader::from_reader(Cursor::new(malformed_entities_dxf()))
        .unwrap()
        .recover()
        .unwrap();

    assert_eq!(outcome.document.entity_count(), 1);
    assert!(outcome.stats.has_usable_drawing_data());
    assert!(outcome.stats.recovered());
    assert_eq!(outcome.stats.skipped_source_records, 1);
    assert!(outcome.repair.after.scanned_entities > 0);
}
