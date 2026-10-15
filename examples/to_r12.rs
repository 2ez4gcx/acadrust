//! Converts a DXF/DWG file to genuine DXF R12 (AC1009).
//!
//!   cargo run --example to_r12 -- input.dwg output.dxf
use acadrust::types::DxfVersion;
use acadrust::{DwgReader, DxfReader, DxfWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let document = if input.to_ascii_lowercase().ends_with(".dwg") {
        DwgReader::from_file(input)?.read()?
    } else {
        DxfReader::from_file(input)?.read()?
    };
    DxfWriter::new(&document)
        .with_version(DxfVersion::AC1009)
        .write_to_file(output)?;
    println!(
        "{input} -> {output} ({} entities)",
        document.entities().count()
    );
    Ok(())
}
