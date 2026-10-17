//! The DWG writer must not depend on hash-map iteration order.
//!
//! `CadDocument::objects` is a plain `HashMap`, so its iteration order is
//! seeded per `HashMap` instance and differs every time the same file is
//! read. Two places walk it and feed results that end up on disk: the class
//! table, whose entries are positional (`500 + index`) and therefore name the
//! type code of every class-registered record, and the extension-dictionary
//! owner index, whose first match wins. Writing the same drawing twice must
//! produce the same bytes.

use acadrust::types::DxfVersion;
use acadrust::{DwgReader, DwgWriter};

fn write_twice(path: &str, version: DxfVersion) -> (Vec<u8>, Vec<u8>) {
    let mut first = DwgReader::from_file(path).expect("open first read");
    let mut a = first.read().expect("first read");
    a.version = version;
    let a = DwgWriter::write_to_vec(&a).expect("first write");

    let mut second = DwgReader::from_file(path).expect("open second read");
    let mut b = second.read().expect("second read");
    b.version = version;
    let b = DwgWriter::write_to_vec(&b).expect("second write");

    (a, b)
}

#[test]
fn writing_the_same_drawing_twice_yields_the_same_bytes() {
    for name in ["lookup", "point", "polar", "xy"] {
        let path = format!("tests/fixtures/dynamic-blocks/{name}.dwg");
        // AC1032 is the version these fixtures already are; the older ones
        // take the legacy class-table path that renumbers classes positionally.
        for version in [
            DxfVersion::AC1015,
            DxfVersion::AC1018,
            DxfVersion::AC1024,
            DxfVersion::AC1032,
        ] {
            let (a, b) = write_twice(&path, version);
            assert_eq!(
                a.len(),
                b.len(),
                "{name} at {version:?}: two writes of one drawing disagree on length"
            );
            assert!(
                a == b,
                "{name} at {version:?}: two writes of one drawing disagree byte for byte"
            );
        }
    }
}
