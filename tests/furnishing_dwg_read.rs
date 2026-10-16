use acadrust::{DwgReader, EntityType, Handle};
use std::path::PathBuf;

#[test]
fn furnishing_modeler_records_read_without_recovery() {
    let Some(root) = std::env::var_os("ACADRUST_FURNISHING_DIR") else {
        return;
    };
    let mut failures = Vec::new();
    for (folder, handle) in [
        ("9925129", 0xADFE),
        ("9925132", 0xAD0C),
        ("Bardak Takim3", 0x14E2C0),
        ("DACKA", 0x5CF4B),
    ] {
        let path = PathBuf::from(&root).join(folder).join(".dwg");
        let result = DwgReader::from_file(&path).unwrap().read_with_stats();
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        assert!(outcome.stats.stream_completed);
        assert_eq!(outcome.stats.skipped_source_records, 0);
        assert_eq!(outcome.stats.recovered_errors, 0);
        let data = match outcome.document.get_entity(Handle::new(handle)).unwrap() {
            EntityType::Solid3D(entity) => &entity.acis_data,
            EntityType::Body(entity) => &entity.acis_data,
            _ => panic!("missing modeler entity in {folder}"),
        };
        assert!(data.has_data(), "missing modeler data in {folder}");
        if folder == "Bardak Takim3" {
            assert_eq!(data.materials.len(), 1);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
