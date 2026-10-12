//! Erased anonymous dimension blocks may leave entity records in a DWG.
//! Explicit orphan ownership must never turn that picture into model geometry.

use std::collections::{BTreeSet, HashMap};
use std::io::Cursor;

use acadrust::entities::{Dimension, DimensionLinear, EntityType, Line, MText, Solid};
use acadrust::io::dwg::dwg_document_builder::DwgDocumentBuilder;
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const VERSIONS: [DxfVersion; 8] = [
    DxfVersion::AC1012,
    DxfVersion::AC1014,
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

struct Fixture {
    bytes: Vec<u8>,
    offsets: HashMap<u64, i64>,
    version: DxfVersion,
    orphan_owner: Handle,
    live_owner: Handle,
    orphans: BTreeSet<Handle>,
    model: BTreeSet<Handle>,
    paper: BTreeSet<Handle>,
    dimension_block: BTreeSet<Handle>,
}

impl Fixture {
    fn objects(&self) -> DwgObjectReader {
        DwgObjectReader::new(self.bytes.clone(), self.version, self.offsets.clone()).unwrap()
    }
}

fn handles(document: &CadDocument, block: &str) -> BTreeSet<Handle> {
    document
        .entities_in_block(block)
        .map(|entity| entity.common().handle)
        .collect()
}

fn add_block(document: &mut CadDocument, name: &str) -> BlockRecord {
    let mut block = BlockRecord::new(name);
    block.handle = document.allocate_handle();
    block.block_entity_handle = document.allocate_handle();
    block.block_end_handle = document.allocate_handle();
    block.flags.anonymous = true;
    document.block_records.add(block.clone()).unwrap();
    block
}

fn fixture(version: DxfVersion) -> Fixture {
    let mut source = CadDocument::with_version(version);
    let erased = add_block(&mut source, "*D0");
    let live = add_block(&mut source, "*D1");
    source
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 0.0, 0.0,
        )))
        .unwrap();
    let mut dimension = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    dimension.base.block_name = live.name.clone();
    source
        .add_entity(EntityType::Dimension(Box::new(Dimension::Linear(
            dimension,
        ))))
        .unwrap();
    for owner in [source.header.paper_space_block_handle, live.handle] {
        let mut line = Line::from_coords(0.0, 1.0, 0.0, 10.0, 1.0, 0.0);
        line.common.owner_handle = owner;
        source.add_entity(EntityType::Line(line)).unwrap();
    }

    let mut label = MText::new();
    label.value = "10".into();
    let mut orphans = BTreeSet::new();
    for mut entity in [
        EntityType::Line(Line::from_coords(0.0, 2.0, 0.0, 10.0, 2.0, 0.0)),
        EntityType::Solid(Solid::triangle(
            Vector3::ZERO,
            Vector3::new(1.0, 0.5, 0.0),
            Vector3::new(1.0, -0.5, 0.0),
        )),
        EntityType::MText(Box::new(label)),
    ] {
        entity.common_mut().owner_handle = erased.handle;
        orphans.insert(source.add_entity(entity).unwrap());
    }

    let model = handles(&source, "*Model_Space");
    let paper = handles(&source, "*Paper_Space");
    let dimension_block = handles(&source, "*D1");
    let (bytes, records, _, _) = DwgObjectWriter::new(&source).unwrap().write();
    // Remove the erased block's records from the DWG handle map while retaining
    // its entities, reproducing the dangling owners present in real drawings.
    let erased_records = [
        erased.handle,
        erased.block_entity_handle,
        erased.block_end_handle,
    ];
    let offsets = records
        .into_iter()
        .filter(|(handle, _)| !erased_records.contains(&Handle::new(*handle)))
        .map(|(handle, offset)| (handle, offset as i64))
        .collect();
    Fixture {
        bytes,
        offsets,
        version,
        orphan_owner: erased.handle,
        live_owner: live.handle,
        orphans,
        model,
        paper,
        dimension_block,
    }
}

fn assert_membership(document: &CadDocument, fixture: &Fixture) {
    assert_eq!(handles(document, "*Model_Space"), fixture.model);
    assert_eq!(handles(document, "*Paper_Space"), fixture.paper);
    let live_block = document
        .block_records
        .iter()
        .find(|block| block.handle == fixture.live_owner)
        .unwrap();
    assert_eq!(
        live_block
            .entity_handles
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        fixture.dimension_block
    );
    assert!(document
        .block_records
        .iter()
        .all(|block| block.handle != fixture.orphan_owner));
    for block in document.block_records.iter() {
        assert!(block
            .entity_handles
            .iter()
            .all(|handle| !fixture.orphans.contains(handle)));
    }
}

#[test]
fn orphan_dimension_picture_is_retained_without_drawable_membership_across_dwg_versions() {
    for version in VERSIONS {
        let fixture = fixture(version);
        let mut document = CadDocument::with_version(version);
        let outcome = DwgDocumentBuilder::new(fixture.objects()).build_with_stats(&mut document);
        assert_eq!(outcome.skipped_records, 0, "{version:?}");
        assert_membership(&document, &fixture);
        for handle in &fixture.orphans {
            assert_eq!(
                document.get_entity(*handle).unwrap().common().owner_handle,
                fixture.orphan_owner,
                "{version:?} {handle}"
            );
        }
    }
}

#[test]
fn orphan_dimension_picture_is_not_exported_as_live_dwg_or_dxf_geometry() {
    for version in VERSIONS {
        let fixture = fixture(version);
        let mut document = CadDocument::with_version(version);
        DwgDocumentBuilder::new(fixture.objects()).build(&mut document);
        for _ in 0..2 {
            let bytes = DwgWriter::write_to_vec(&document).unwrap();
            document = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
            assert_membership(&document, &fixture);
            for handle in &fixture.orphans {
                assert!(
                    document.get_entity(*handle).is_none(),
                    "{version:?} {handle}"
                );
            }
        }

        let mut original = CadDocument::with_version(version);
        DwgDocumentBuilder::new(fixture.objects()).build(&mut original);
        for binary in [false, true] {
            let mut writer = DxfWriter::new(&original);
            writer.set_binary(binary);
            let bytes = writer.write_to_vec().unwrap();
            let exported = DxfReader::from_reader(Cursor::new(bytes))
                .unwrap()
                .read()
                .unwrap();
            assert_membership(&exported, &fixture);
            for handle in &fixture.orphans {
                assert!(
                    exported.get_entity(*handle).is_none(),
                    "{version:?} binary={binary} {handle}"
                );
            }
        }
    }
}

#[test]
fn visitors_receive_orphans_without_rehoming_kept_or_dropped_records() {
    for version in VERSIONS {
        let fixture = fixture(version);
        let dropped = *fixture.orphans.first().unwrap();
        let mut seen = BTreeSet::new();
        let mut document = CadDocument::with_version(version);
        let outcome = DwgDocumentBuilder::new(fixture.objects()).build_with_visitor_stats(
            &mut document,
            &mut |_, entity| {
                let handle = entity.common().handle;
                if fixture.orphans.contains(&handle) {
                    assert_eq!(entity.common().owner_handle, fixture.orphan_owner);
                    seen.insert(handle);
                }
                (handle != dropped).then_some(entity)
            },
        );
        assert_eq!(outcome.skipped_records, 0, "{:?}", fixture.version);
        assert_eq!(seen, fixture.orphans);
        assert_membership(&document, &fixture);
        assert!(document.get_entity(dropped).is_none());
        for handle in fixture.orphans.iter().filter(|handle| **handle != dropped) {
            assert_eq!(
                document.get_entity(*handle).unwrap().common().owner_handle,
                fixture.orphan_owner
            );
        }
    }
}
