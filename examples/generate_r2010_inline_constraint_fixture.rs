//! Generate a synthetic DWG with a hand-encoded R2010 version-2 inline group.
//! No external drawings or CAD installation are required.
use acadrust::io::dwg::{
    dwg_reference_type::DwgReferenceType, dwg_stream_writers::merged_writer::DwgMergedWriter,
    DwgVersion,
};
use acadrust::{
    classes::DxfClass,
    entities::Line,
    objects::ObjectType,
    types::{DxfVersion, Vector3},
    CadDocument, DwgWriter, EntityType,
};

fn main() {
    let version = DxfVersion::AC1024;
    let mut document = CadDocument::with_version(version);
    document.dwg_source_version = Some(version);
    document.classes.clear();
    let mut class = DxfClass::new("ACDBASSOC2DCONSTRAINTGROUP", "AcDbAssoc2dConstraintGroup");
    class.class_number = 500;
    document.classes.push_preserving(class);
    document
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::UNIT_X,
        )))
        .unwrap();
    let owner = document.header.named_objects_dict_handle;
    let group = document.allocate_handle();
    let mut writer = DwgMergedWriter::new(DwgVersion::from_dxf_version(version).unwrap(), version);
    writer.write_object_type(500);
    writer.main_mut().write_handle_undefined(group.value());
    writer.write_bit_short(0); // no EED
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    writer.write_bit_long(0); // no reactors
    writer.write_bit(true); // no extension dictionary
    writer.write_bit_short(2);
    writer.write_bit_long(0); // action version/status
    writer.write_handle(DwgReferenceType::SoftPointer, 0); // network
    writer.write_handle(DwgReferenceType::HardOwnership, 0); // action body
    for _ in 0..3 {
        writer.write_bit_long(0);
    } // index, max dependency, dependencies
    for _ in 0..2 {
        writer.write_bit_short(0);
        writer.write_bit_long(0);
    } // params/values
    writer.write_bit_long(2);
    writer.write_bit(false); // group version/flag
    for point in [Vector3::ZERO, Vector3::UNIT_X, Vector3::UNIT_Y] {
        writer.write_3bit_double(point);
    }
    writer.write_handle(DwgReferenceType::HardOwnership, 0); // group dependency
    writer.write_bit_long(0); // no actions
    writer.write_bit_long(9); // next ID, distinct from count and last assigned ID
    writer.write_bit_long(2); // native inline count; no root/class registry
    writer.write_variable_text("AcConstrainedBoundedLine");
    writer.write_bit_long(1);
    writer.write_byte(3);
    writer.write_bit_long(1);
    writer.write_bit_long(4);
    writer.write_handle(DwgReferenceType::SoftPointer, 0);
    writer.write_bit_long(0);
    writer.write_3bit_double(Vector3::ZERO);
    writer.write_3bit_double(Vector3::UNIT_X);
    writer.write_bit(false);
    writer.write_3bit_double(Vector3::ZERO);
    writer.write_3bit_double(Vector3::UNIT_X);
    writer.write_variable_text("AcFixedConstraint");
    writer.write_bit_long(4);
    writer.write_byte(7);
    writer.write_bit_long(1);
    writer.write_bit_long(1);
    writer.write_bit_long(1);
    writer.write_bit(false);
    writer.write_bit(true);
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    document.objects.insert(
        group,
        ObjectType::Unknown {
            type_name: "ACDBASSOC2DCONSTRAINTGROUP".into(),
            handle: group,
            owner,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(version),
        },
    );
    if let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&owner) {
        dictionary.add_entry("SyntheticInlineConstraintGroup", group);
    }
    let output = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "tests/fixtures/associative/r2010-v2-inline-group.dwg".into());
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&output, DwgWriter::write_to_vec(&document).unwrap()).unwrap();
    println!("Generated {}", output.display());
}
