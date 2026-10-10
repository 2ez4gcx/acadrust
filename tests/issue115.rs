use std::io::Cursor;

use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const RGB: Color = Color::from_rgb(200, 100, 50);

fn document() -> CadDocument {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document.layers.add(Layer::with_color("L1", RGB)).unwrap();
    document
}

fn layer_aci(dxf: &[u8]) -> i16 {
    let text = String::from_utf8_lossy(dxf);
    let table = text
        .split_once("2\nLAYER\n")
        .and_then(|(_, rest)| rest.split_once("0\nENDTAB"))
        .map(|(table, _)| table)
        .expect("LAYER table");
    let lines: Vec<_> = table.lines().map(str::trim).collect();
    lines
        .windows(2)
        .find(|pair| pair[0] == "62")
        .and_then(|pair| pair[1].parse().ok())
        .expect("layer group 62")
}

#[test]
fn rgb_layer_writes_nearest_aci_and_true_color() {
    let document = document();
    let dxf = DxfWriter::new(&document).write_to_vec().unwrap();
    assert_eq!(layer_aci(&dxf), RGB.approximate_index());
    assert!(String::from_utf8_lossy(&dxf).contains("420\n13132850\n"));
}

#[test]
fn rgb_layer_keeps_nearest_aci_after_dwg_roundtrip() {
    let document = document();
    let dwg = DwgWriter::write_to_vec(&document).unwrap();
    let restored = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    let dxf = DxfWriter::new(&restored).write_to_vec().unwrap();
    assert_eq!(layer_aci(&dxf), RGB.approximate_index());
}
