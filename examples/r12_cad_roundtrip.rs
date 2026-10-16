//! Inspect and resave the native AC1009 corpus created by the CAD scripts.
use acadrust::io::dwg::r12::header::read_header;
use acadrust::{DwgReader, DwgWriter, EntityType};
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/ac1009-cad-created"));
    let mut inputs = Vec::new();
    for name in ["native-geometry", "native-viewports", "native-header"] {
        for suffix in ["", "-edited", "-incremental"] {
            let filename = format!("{name}{suffix}.dwg");
            let path = output.join(&filename);
            if !path.exists() {
                continue;
            }
            let bytes = fs::read(&path)?;
            let header = read_header(&bytes)?;
            let mut types = BTreeMap::new();
            for (label, start, size) in [
                (
                    "main",
                    header.layout.entities_start,
                    header.layout.entities_end - header.layout.entities_start,
                ),
                (
                    "blocks",
                    header.layout.blocks_start,
                    header.layout.blocks_size,
                ),
                (
                    "extras",
                    header.layout.extras_start,
                    header.layout.extras_size,
                ),
            ] {
                let mut offset = start as usize;
                while offset < (start + size) as usize {
                    *types.entry((label, bytes[offset] & 127)).or_insert(0) += 1;
                    offset +=
                        u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into()?) as usize;
                }
            }
            println!(
                "{filename}: header={} sections={types:?} VX={}",
                header.num_header_variables, header.tables[9].count
            );
            let document = DwgReader::from_stream(Cursor::new(bytes))
                .read()
                .map_err(|error| anyhow::anyhow!("{filename}: {error}"))?;
            let mut kinds = BTreeMap::new();
            for entity in document.entities() {
                let kind = match entity {
                    EntityType::Polyline2D(_) => "POLYLINE2D",
                    EntityType::Polyline3D(_) => "POLYLINE3D",
                    EntityType::PolygonMesh(_) => "MESH",
                    EntityType::PolyfaceMesh(_) => "POLYFACE",
                    EntityType::Insert(_) => "INSERT",
                    EntityType::AttributeDefinition(_) => "ATTDEF",
                    EntityType::Line(_) => "LINE",
                    EntityType::Text(_) => "TEXT",
                    EntityType::Circle(_) => "CIRCLE",
                    EntityType::Viewport(viewport) => {
                        println!(" viewport {} center={:?} size={}x{} target={:?} direction={:?} twist={} snap={:?}/{:?} frozen={:?}", viewport.id, viewport.center, viewport.width,viewport.height,viewport.view_target, viewport.view_direction, viewport.twist_angle, viewport.snap_base, viewport.snap_spacing,viewport.frozen_layers);
                        "VIEWPORT"
                    }
                    _ => "OTHER",
                };
                *kinds.entry(kind).or_insert(0) += 1;
            }
            println!(" decoded={kinds:?}");
            fs::write(
                output.join(format!("{name}{suffix}-rust.dxf")),
                acadrust::DxfWriter::new(&document).write_to_vec()?,
            )?;
            let saved = DwgWriter::write_to_vec(&document)?;
            anyhow::ensure!(
                saved == DwgWriter::write_to_vec(&document)?,
                "nondeterministic native save"
            );
            let saved_name = format!("{name}{suffix}-saved.dwg");
            fs::write(output.join(&saved_name), saved)?;
            inputs.extend([filename, saved_name]);
        }
    }
    fs::write(output.join("inputs.txt"), inputs.join("\n") + "\n")?;
    Ok(())
}
