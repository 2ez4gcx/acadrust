use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::objects::ObjectType;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle, Layer};

#[test]
fn audit_is_read_only_and_clean_for_a_default_document() {
    let document = CadDocument::new();
    let snapshot = document.clone();

    let report = document.audit();

    assert_eq!(document, snapshot);
    assert_eq!(report.scanned_entities, 0);
    assert!(report.scanned_table_entries > 0);
    assert!(
        report.is_clean(),
        "default document issues: {:?}",
        report.issues
    );
}

#[test]
fn repair_fixes_unambiguous_identity_state_and_is_idempotent() {
    let mut document = CadDocument::new();
    let layer = Layer::new("BROKEN");
    document.layers.add(layer).unwrap();
    document.layers.get_mut("BROKEN").unwrap().name = "RENAMED".into();
    let entity = document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
        )))
        .unwrap();
    document.get_entity_mut(entity).unwrap().common_mut().handle = Handle::NULL;

    let before = document.audit();
    assert!(before.count(acadrust::AuditIssueCode::StaleTableKey) > 0);
    assert!(before.count(acadrust::AuditIssueCode::EntityIndexMismatch) > 0);

    let repaired = document.repair();
    assert!(!repaired.actions.is_empty());
    assert!(
        repaired.after.is_clean(),
        "repair left issues: {:?}",
        repaired.after.issues
    );
    let second = document.repair();
    assert_eq!(repaired.after, second.before);
    assert!(second.actions.iter().all(|action| {
        !matches!(
            action,
            acadrust::RepairAction::ResyncTableKeys { count } if *count > 0
        )
    }));
}

#[test]
fn recover_reads_and_repairs_a_valid_dxf() {
    let mut source = CadDocument::new();
    source
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 0.0, 0.0,
        )))
        .unwrap();
    let bytes = DxfWriter::new(&source).write_to_vec().unwrap();

    let recovered = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .recover()
        .unwrap();

    assert!(recovered.stats.has_usable_drawing_data());
    assert_eq!(recovered.document.entity_count(), 1);
    assert!(recovered.repair.after.scanned_entities > 0);
}

#[test]
fn recover_rejects_a_stream_without_usable_records() {
    let result = DxfReader::from_reader(Cursor::new(Vec::<u8>::new()))
        .unwrap()
        .recover();

    assert!(matches!(result, Err(acadrust::DxfError::RecoveryFailed(_))));
}

#[test]
fn recover_reads_and_repairs_a_valid_dwg() {
    let mut source = CadDocument::new();
    source
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 0.0, 0.0,
        )))
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&source).unwrap();

    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    let recovered = reader.recover().unwrap();

    assert!(recovered.stats.has_usable_drawing_data());
    assert_eq!(recovered.document.entity_count(), 1);
    assert!(recovered.repair.after.scanned_entities > 0);
}

#[test]
fn audit_reports_raw_object_records_without_mutating_them() {
    let mut document = CadDocument::new();
    let object_handle = Handle::new(0x9000);
    document.objects.insert(
        object_handle,
        ObjectType::Unknown {
            type_name: "CUSTOM".into(),
            handle: object_handle,
            owner: Handle::NULL,
            raw_dxf_codes: Some(vec![(1, "payload".into())]),
            raw_dwg_data: None,
            raw_dwg_handle_bits: 0,
            raw_dwg_version: None,
        },
    );
    let snapshot = document.clone();

    let report = document.audit();

    assert_eq!(document, snapshot);
    assert_eq!(report.count(acadrust::AuditIssueCode::UnsupportedRecord), 1);
}

#[test]
fn audit_reports_dangling_object_owner_and_layer_references() {
    let mut document = CadDocument::new();
    let handle = document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
        )))
        .unwrap();
    let entity = document.get_entity_mut(handle).unwrap();
    entity.common_mut().layer = "MISSING_LAYER".into();
    entity.common_mut().owner_handle = Handle::new(0xdead);

    let report = document.audit();

    assert!(report.count(acadrust::AuditIssueCode::DanglingReference) >= 2);
}
