//! Writing a DWG takes time in proportion to the drawing: four times the blocks and entities
//! take well under sixteen times as long. The entity mode of every entity (model space, paper
//! space, or owned by a block) was found by searching the block records for its owner, so a
//! drawing of many blocks (every dimension has one) was quadratic to write.

use std::time::{Duration, Instant};

use acadrust::entities::{EntityType, Line};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// A drawing of `n` blocks of two lines each, and `n` lines in model space.
fn drawing(n: usize) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for b in 0..n {
        let mut record = BlockRecord::new(format!("B{b}"));
        record.handle = doc.allocate_handle();
        record.block_entity_handle = doc.allocate_handle();
        record.block_end_handle = doc.allocate_handle();
        let owner = record.handle;
        doc.block_records.add(record).unwrap();
        for k in 0..2 {
            let mut line = Line::from_points(
                Vector3::new(k as f64, 0.0, 0.0),
                Vector3::new(k as f64, 1.0, 0.0),
            );
            line.common.owner_handle = owner;
            doc.add_entity(EntityType::Line(line)).unwrap();
        }
    }
    for i in 0..n {
        let x = i as f64;
        doc.add_entity(EntityType::Line(Line::from_points(
            Vector3::new(x, 0.0, 0.0),
            Vector3::new(x, 1.0, 0.0),
        )))
        .unwrap();
    }
    doc
}

fn fastest_write(doc: &CadDocument) -> (Duration, Vec<u8>) {
    let mut best: Option<(Duration, Vec<u8>)> = None;
    for _ in 0..3 {
        let t = Instant::now();
        let bytes = DwgWriter::write_to_vec(doc).unwrap();
        let e = t.elapsed();
        if best.as_ref().is_none_or(|(b, _)| e < *b) {
            best = Some((e, bytes));
        }
    }
    best.unwrap()
}

#[test]
fn writing_many_blocks_scales_linearly() {
    const N: usize = 5_000;
    let (small, large) = (drawing(N), drawing(N * 4));
    let (a, _) = fastest_write(&small);
    let (b, bytes) = fastest_write(&large);
    let back = DwgReader::from_stream(std::io::Cursor::new(bytes))
        .read()
        .unwrap();
    assert_eq!(
        back.block_records.get("B7").map(|r| r.entity_handles.len()),
        Some(2)
    );
    // Linear is 4x, quadratic 16x; below a few milliseconds the ratio is noise.
    let ratio = b.as_secs_f64() / a.as_secs_f64().max(0.005);
    eprintln!("{N} blocks {a:?}, {} blocks {b:?}, ratio {ratio:.1}", N * 4);
    assert!(
        ratio < 9.0,
        "4x the blocks took {ratio:.1}x as long to write ({a:?} → {b:?})"
    );
}
