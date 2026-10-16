use std::io::Cursor;

use opencadcodec::objects::{
    AssocDependency, AssocGeomDependency, AssocPersistentSubentId, AssociativeData,
    AssociativeObject, ObjectType,
};
use opencadcodec::types::DxfVersion;
use opencadcodec::{CadDocument, DwgReader, DwgWriter};

fn document_with_geom_dependency(version: DxfVersion) -> CadDocument {
    let mut document = CadDocument::with_version(version);
    let handle = document.allocate_handle();
    let owner = document.header.named_objects_dict_handle;
    document.objects.insert(
        handle,
        ObjectType::Associative(AssociativeObject {
            handle,
            owner,
            dxf_name: "ASSOCGEOMDEPENDENCY".to_string(),
            cpp_class_name: "AcDbAssocGeomDependency".to_string(),
            data: AssociativeData::GeomDependency(AssocGeomDependency {
                dependency: AssocDependency {
                    name: Some("dependency".to_string()),
                    ..Default::default()
                },
                persistent_subent: AssocPersistentSubentId {
                    class_name: "AcDbAssocSingleEdgePersSubentId".to_string(),
                    dependent_on_compound_object: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        }),
    );
    document
}

fn read_geom_dependency(bytes: Vec<u8>) -> AssocGeomDependency {
    let document = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG should read without a truncated persistent subentity id");
    document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::GeomDependency(value),
                ..
            }) => Some(value.clone()),
            _ => None,
        })
        .expect("geometry dependency should remain typed")
}

#[test]
fn r2010_geom_dependency_uses_the_text_stream_for_its_class_name() {
    let source = document_with_geom_dependency(DxfVersion::AC1024);
    let expected = match source
        .objects
        .values()
        .find(|object| matches!(object, ObjectType::Associative(_)))
        .unwrap()
    {
        ObjectType::Associative(AssociativeObject {
            data: AssociativeData::GeomDependency(value),
            ..
        }) => value.clone(),
        _ => unreachable!(),
    };

    let actual = read_geom_dependency(DwgWriter::write_to_vec(&source).unwrap());
    assert_eq!(actual.class_version, expected.class_version);
    assert_eq!(actual.enabled, expected.enabled);
    assert_eq!(actual.dependency, expected.dependency);
    assert_eq!(
        actual.persistent_subent.class_name,
        expected.persistent_subent.class_name
    );
    assert_eq!(actual.persistent_subent.class_code, 1);
    assert!(actual.persistent_subent.values.is_empty());
    assert_eq!(
        actual.persistent_subent.dependent_on_compound_object,
        expected.persistent_subent.dependent_on_compound_object
    );
}

#[test]
fn r2013_compact_geom_dependency_layout_still_round_trips() {
    let source = document_with_geom_dependency(DxfVersion::AC1027);
    let actual = read_geom_dependency(DwgWriter::write_to_vec(&source).unwrap());
    assert_eq!(
        actual.persistent_subent.class_name,
        "AcDbAssocSingleEdgePersSubentId"
    );
    assert_eq!(actual.persistent_subent.class_code, 1);
    assert!(actual.persistent_subent.values.is_empty());
    assert!(actual.persistent_subent.dependent_on_compound_object);
}
