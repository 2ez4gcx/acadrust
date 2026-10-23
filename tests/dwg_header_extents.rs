//! EXTMIN/EXTMAX survive a DWG round trip. The writer replaced them with the union of the
//! model-space entities' bounding boxes, which made a drawing with an XLINE or RAY billions
//! of units wide and gave a drawing read from a file the writer's own extents. Only a
//! document without extents (the 1e20 defaults) gets computed ones.

use std::io::Cursor;

use acadrust::entities::{EntityType, Line, XLine};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn drawing(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Line(Line::from_points(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 5.0, 0.0),
    )))
    .unwrap();
    doc
}

fn round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn extents_the_document_holds_are_kept() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1032] {
        let mut doc = drawing(version);
        // An unbounded construction line must not widen the extents.
        doc.add_entity(EntityType::XLine(XLine::new(
            Vector3::new(5.0, 2.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
        )))
        .unwrap();
        let (min, max) = (Vector3::new(-1.0, -2.0, 0.0), Vector3::new(11.0, 6.0, 0.0));
        doc.header.model_space_extents_min = min;
        doc.header.model_space_extents_max = max;

        let back = round_trip(&doc);
        assert_eq!(back.header.model_space_extents_min, min, "{version:?}");
        assert_eq!(back.header.model_space_extents_max, max, "{version:?}");
    }
}

#[test]
fn a_document_without_extents_gets_computed_ones() {
    let doc = drawing(DxfVersion::AC1032);
    assert!(doc.header.model_space_extents_min.x > doc.header.model_space_extents_max.x);

    let back = round_trip(&doc);
    let (min, max) = (
        back.header.model_space_extents_min,
        back.header.model_space_extents_max,
    );
    assert!(min.x <= 0.0 && min.y <= 0.0, "{min:?}");
    assert!(max.x >= 10.0 && max.y >= 5.0, "{max:?}");
    assert!(max.x < 1e3 && max.y < 1e3, "{max:?}");
}
