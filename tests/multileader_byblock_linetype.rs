//! A multileader's ByBlock leader linetype stays ByBlock through DXF and DWG, and a multileader
//! without one is written ByBlock, AutoCAD's default for leader lines. The DXF writer turned
//! ByBlock into ByLayer, and both writers wrote ByLayer for a missing linetype.

use std::io::Cursor;

use acadrust::entities::{EntityType, MultiLeader};
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn drawing(byblock: bool) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut leader = MultiLeader::with_text(
        "Leader",
        Vector3::new(3.0, 3.0, 0.0),
        vec![Vector3::ZERO, Vector3::new(2.0, 2.0, 0.0)],
    );
    if byblock {
        leader.line_type_handle = Some(doc.header.byblock_linetype_handle);
    }
    doc.add_entity(EntityType::MultiLeader(Box::new(leader)))
        .unwrap();
    doc
}

/// The name of the leader linetype of the drawing's multileader.
fn leader_linetype(doc: &CadDocument) -> Option<String> {
    let handle: Handle = doc.entities().find_map(|entity| match entity {
        EntityType::MultiLeader(leader) => leader.line_type_handle,
        _ => None,
    })?;
    doc.line_types
        .iter()
        .find(|lt| lt.handle == handle)
        .map(|lt| lt.name.to_ascii_uppercase())
}

fn through_dxf(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn through_dwg(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn a_byblock_leader_linetype_stays_byblock() {
    let doc = drawing(true);
    assert_eq!(leader_linetype(&doc).as_deref(), Some("BYBLOCK"));
    assert_eq!(
        leader_linetype(&through_dxf(&doc)).as_deref(),
        Some("BYBLOCK")
    );
    assert_eq!(
        leader_linetype(&through_dwg(&doc)).as_deref(),
        Some("BYBLOCK")
    );
}

#[test]
fn a_missing_leader_linetype_is_written_byblock() {
    let doc = drawing(false);
    assert_eq!(
        leader_linetype(&through_dxf(&doc)).as_deref(),
        Some("BYBLOCK")
    );
    assert_eq!(
        leader_linetype(&through_dwg(&doc)).as_deref(),
        Some("BYBLOCK")
    );
}
