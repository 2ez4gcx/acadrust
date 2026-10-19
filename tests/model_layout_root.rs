use std::io::Cursor;

use acadrust::{
    objects::{Dictionary, ObjectType},
    CadDocument, DxfReader, DxfWriter, Handle, Vector3,
};

fn read_minimal(objects: &str) -> CadDocument {
    let dxf = format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n30\n0\n11\n12\n21\n9\n31\n0\n0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n{objects}0\nENDSEC\n0\nEOF\n");
    DxfReader::from_reader(Cursor::new(dxf))
        .unwrap()
        .read()
        .unwrap()
}

fn assert_model_chain(doc: &CadDocument, check_cache: bool) {
    let Some(ObjectType::Dictionary(root)) = doc.objects.get(&doc.header.named_objects_dict_handle)
    else {
        panic!("missing named root")
    };
    assert_eq!(root.handle, doc.header.named_objects_dict_handle);
    assert!(!root.handle.is_null());
    assert!(root.owner.is_null());
    let targets = root
        .entries
        .iter()
        .filter(|(name, _)| name == "ACAD_LAYOUT")
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 1);
    if check_cache {
        assert_eq!(targets[0].1, doc.header.acad_layout_dict_handle);
    }
    let Some(ObjectType::Dictionary(dictionary)) = doc.objects.get(&targets[0].1) else {
        panic!("missing layout dictionary")
    };
    assert_eq!(dictionary.owner, root.handle);
    let model = doc
        .block_records
        .iter()
        .find(|b| b.is_model_space())
        .unwrap();
    let Some(ObjectType::Layout(layout)) = doc.objects.get(&model.layout) else {
        panic!("missing Model layout")
    };
    assert_eq!(layout.owner, dictionary.handle);
    assert_eq!(layout.block_record, model.handle);
    assert_eq!(
        dictionary
            .entries
            .iter()
            .filter(|(name, h)| name == "Model" && *h == layout.handle)
            .count(),
        1
    );
}

#[test]
fn a_null_dictionary_identity_is_not_reused_as_the_named_root() {
    let mut doc = CadDocument::new();
    remove_model_layout(&mut doc);
    doc.objects.clear();
    doc.objects
        .insert(Handle::NULL, ObjectType::Dictionary(Dictionary::new()));
    doc.header.named_objects_dict_handle = Handle::NULL;
    doc.ensure_model_layout();
    assert_model_chain(&doc, true);
}

#[test]
fn missing_or_handleless_empty_root_gets_a_consistent_model_scaffold() {
    for objects in ["", "0\nDICTIONARY\n100\nAcDbDictionary\n281\n1\n"] {
        let mut doc = read_minimal(objects);
        assert_model_chain(&doc, true);
        let objects_before = doc.objects.clone();
        doc.ensure_model_layout();
        assert_eq!(doc.objects, objects_before);
        let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
        let reopened = DxfReader::from_reader(Cursor::new(bytes))
            .unwrap()
            .read()
            .unwrap();
        // Existing-layout DXF cache synchronization is a separate fix. Check
        // the actual named graph here so this regression stands on its own.
        assert_model_chain(&reopened, false);
        let entities = reopened.model_space_entities().collect::<Vec<_>>();
        assert_eq!(entities.len(), 1);
        let acadrust::EntityType::Line(line) = entities[0] else {
            panic!()
        };
        assert_eq!(line.start, Vector3::ZERO);
        assert_eq!(line.end, Vector3::new(12., 9., 0.));
    }
}

fn remove_model_layout(doc: &mut CadDocument) {
    let model = doc
        .block_records
        .iter()
        .find(|b| b.is_model_space())
        .unwrap()
        .layout;
    doc.objects.remove(&model);
    let Some(ObjectType::Dictionary(dictionary)) =
        doc.objects.get_mut(&doc.header.acad_layout_dict_handle)
    else {
        panic!()
    };
    dictionary.entries.retain(|(_, h)| *h != model);
}

#[test]
fn model_creation_uses_named_dictionary_instead_of_unrelated_cached_metadata() {
    let mut doc = CadDocument::new();
    remove_model_layout(&mut doc);
    let target = doc.header.acad_layout_dict_handle;
    let decoy = doc.allocate_handle();
    let mut metadata = Dictionary::new();
    metadata.handle = decoy;
    metadata.owner = doc.header.named_objects_dict_handle;
    doc.objects
        .insert(decoy, ObjectType::Dictionary(metadata.clone()));
    doc.header.acad_layout_dict_handle = decoy;
    doc.ensure_model_layout();
    assert_eq!(doc.header.acad_layout_dict_handle, target);
    assert_model_chain(&doc, true);
    assert_eq!(
        doc.objects.get(&decoy),
        Some(&ObjectType::Dictionary(metadata))
    );
}

#[test]
fn model_creation_does_not_guess_between_roots_or_replace_named_conflicts() {
    for case in 0..3 {
        let mut doc = CadDocument::new();
        remove_model_layout(&mut doc);
        if case == 0 {
            doc.objects.clear();
            for _ in 0..2 {
                let mut d = Dictionary::new();
                d.handle = doc.allocate_handle();
                doc.objects.insert(d.handle, ObjectType::Dictionary(d));
            }
        } else {
            let target = doc.header.acad_layout_dict_handle;
            let Some(ObjectType::Dictionary(root)) =
                doc.objects.get_mut(&doc.header.named_objects_dict_handle)
            else {
                panic!()
            };
            if case == 1 {
                root.entries.push(("ACAD_LAYOUT".into(), target));
            } else {
                root.entries
                    .iter_mut()
                    .find(|(name, _)| name == "ACAD_LAYOUT")
                    .unwrap()
                    .1 = Handle::new(99999);
            }
        }
        let objects = doc.objects.clone();
        doc.ensure_model_layout();
        assert_eq!(doc.objects, objects);
        assert!(doc.notifications.iter().any(|notification| notification
            .message
            .contains("Model layout not synthesized")));
        assert!(!doc
            .objects
            .values()
            .any(|o| matches!(o, ObjectType::Layout(l) if l.name == "Model")));
    }
}
