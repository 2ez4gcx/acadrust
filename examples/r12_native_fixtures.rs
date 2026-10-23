//! Export the exact valid stream fixtures used by the Rust regressions.

#[allow(dead_code)]
#[path = "../tests/r12_entity_stream.rs"]
mod stream_fixtures;

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use acadrust::entities::Shape;
use acadrust::tables::TextStyle;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, EntityType, Vector3};

fn main() -> anyhow::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/ac1009-native"));
    fs::create_dir_all(&output)?;
    let fixtures = [
        ("model-polyline", stream_fixtures::model_polyline_fixture()),
        ("block-polyline", stream_fixtures::block_polyline_fixture()),
        (
            "insert-attributes",
            stream_fixtures::insert_attributes_fixture(),
        ),
    ];
    let mut inputs = Vec::new();
    for (name, bytes) in fixtures {
        let raw_name = format!("{name}.dwg");
        fs::write(output.join(&raw_name), &bytes)?;
        inputs.push(raw_name);
        let document = DwgReader::from_stream(Cursor::new(bytes)).read()?;
        let native_name = format!("{name}-saved.dwg");
        let saved = DwgWriter::write_to_vec(&document)?;
        anyhow::ensure!(
            saved == DwgWriter::write_to_vec(&document)?,
            "nondeterministic {name} output"
        );
        fs::write(output.join(&native_name), saved)?;
        inputs.push(native_name);
    }
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    let mut style = TextStyle::new("R12_SHAPES");
    style.handle = document.allocate_handle();
    style.is_shape_file = true;
    style.font_file = "ltypeshp.shx".into();
    let style_handle = style.handle;
    document
        .text_styles
        .add(style)
        .map_err(anyhow::Error::msg)?;
    // BOX is shape 132 in the standard AutoCAD ltypeshp.shx installation.
    let mut shape = Shape::with_number(Vector3::ZERO, 132, 2.0);
    shape.style_handle = Some(style_handle);
    shape.style_name = "R12_SHAPES".into();
    document.add_entity(EntityType::Shape(shape))?;
    document.header.menu_name = "C:\\R12Fixtures\\Menus\\acad".into();
    fs::write(
        output.join("shape-menu.dwg"),
        DwgWriter::write_to_vec(&document)?,
    )?;
    inputs.push("shape-menu.dwg".into());
    fs::write(output.join("inputs.txt"), inputs.join("\n") + "\n")?;
    println!(
        "Exported {} AC1009 fixtures to {}",
        inputs.len(),
        output.display()
    );
    Ok(())
}
