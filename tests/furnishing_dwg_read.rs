use acadrust::{DwgReader, EntityType, Handle};
use std::path::PathBuf;

#[test]
#[ignore = "needs the furnishing drawings; set ACADRUST_FURNISHING_DIR to their folder"]
fn furnishing_modeler_records_read_without_recovery() {
    let root = std::env::var_os("ACADRUST_FURNISHING_DIR")
        .expect("set ACADRUST_FURNISHING_DIR to the folder holding the furnishing drawings");
    let mut failures = Vec::new();
    for (folder, handle) in [
        ("9925129", 0xADFE),
        ("9925132", 0xAD0C),
        ("Bardak Takim3", 0x14E2C0),
        ("DACKA", 0x5CF4B),
    ] {
        let path = PathBuf::from(&root).join(folder).join(".dwg");
        let result = DwgReader::from_file(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .read_with_stats();
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
