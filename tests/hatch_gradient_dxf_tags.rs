//! Literal DXF gradient blocks: a stop's 421 (RGB) wins over its 63 (ACI)
//! whichever comes first. The codec writer emits 63 before 421, so its own
//! round trip cannot catch the other order.
use std::io::Cursor;

use acadrust::{CadDocument, Color, DxfReader, EntityType};

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn gradient_hatch(stop: &str) -> Vec<u8> {
    format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
         0\nHATCH\n5\n20\n100\nAcDbEntity\n8\n0\n100\nAcDbHatch\n10\n0\n20\n0\n30\n0\n\
         210\n0\n220\n0\n230\n1\n2\nSOLID\n70\n1\n71\n0\n91\n1\n92\n3\n72\n0\n73\n1\n93\n4\n\
         10\n0\n20\n0\n10\n1\n20\n0\n10\n1\n20\n1\n10\n0\n20\n1\n97\n0\n75\n0\n76\n1\n98\n0\n\
         450\n1\n451\n0\n452\n0\n453\n2\n463\n0\n63\n5\n421\n255\n{stop}\
         470\nLINEAR\n0\nENDSEC\n0\nEOF\n"
    )
    .into_bytes()
}

fn second_stop_color(doc: &CadDocument) -> Color {
    doc.entities()
        .find_map(|entity| match entity {
            EntityType::Hatch(hatch) => Some(hatch.gradient_color.colors[1].color.clone()),
            _ => None,
        })
        .expect("hatch")
}

#[test]
fn rgb_wins_when_aci_comes_after_it() {
    let doc = read_dxf(gradient_hatch("463\n1\n421\n1193046\n63\n7\n"));
    assert_eq!(
        second_stop_color(&doc),
        Color::Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56
        }
    );
}

#[test]
fn rgb_wins_when_aci_comes_before_it() {
    let doc = read_dxf(gradient_hatch("463\n1\n63\n7\n421\n1193046\n"));
    assert_eq!(
        second_stop_color(&doc),
        Color::Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56
        }
    );
}

#[test]
fn aci_alone_is_kept() {
    let doc = read_dxf(gradient_hatch("463\n1\n63\n7\n"));
    assert_eq!(second_stop_color(&doc), Color::from_index(7));
}

#[test]
fn dwg_keeps_gradient_color_book_names() {
    use acadrust::entities::hatch::GradientColorEntry;
    use acadrust::entities::{BoundaryEdge, BoundaryPath, Hatch, LineEdge};
    use acadrust::{DwgReadOptions, DwgReader, DwgWriter, Vector2};
    let mut hatch = Hatch::new();
    let mut path = BoundaryPath::new();
    for (a, b) in [
        ((0.0, 0.0), (1.0, 0.0)),
        ((1.0, 0.0), (1.0, 1.0)),
        ((1.0, 1.0), (0.0, 0.0)),
    ] {
        path.edges.push(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(a.0, a.1),
            end: Vector2::new(b.0, b.1),
        }));
    }
    hatch.paths.push(path);
    hatch.gradient_color.enabled = true;
    hatch.gradient_color.name = "LINEAR".into();
    hatch.gradient_color.colors = vec![
        GradientColorEntry {
            value: 0.0,
            color: Color::Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            },
            color_name: Some("PANTONE 2945 C".into()),
            book_name: Some("PANTONE+ Solid Coated".into()),
        },
        GradientColorEntry {
            value: 1.0,
            color: Color::from_index(7),
            ..Default::default()
        },
    ];
    let mut doc = CadDocument::new();
    doc.add_entity(EntityType::Hatch(Box::new(hatch.clone())))
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let back = DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
        .read()
        .unwrap();
    let read = back
        .entities()
        .find_map(|entity| match entity {
            EntityType::Hatch(hatch) => Some(hatch.gradient_color.colors.clone()),
            _ => None,
        })
        .expect("hatch");
    assert_eq!(read, hatch.gradient_color.colors);
}
