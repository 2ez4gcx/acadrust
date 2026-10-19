use std::io::Cursor;

use acadrust::{
    objects::{Dictionary, ObjectType},
    CadDocument, DxfReader, DxfWriter, Handle,
};

fn relocated_layouts(decoy: bool) -> (CadDocument, Handle) {
    let mut doc = CadDocument::new();
    let old = doc.header.acad_layout_dict_handle;
    let new = doc.allocate_handle();
    let Some(ObjectType::Dictionary(mut layouts)) = doc.objects.remove(&old) else {
        panic!()
    };
    layouts.handle = new;
    let Some(ObjectType::Dictionary(root)) =
        doc.objects.get_mut(&doc.header.named_objects_dict_handle)
    else {
        panic!()
    };
    root.entries
        .iter_mut()
        .find(|(name, _)| name == "ACAD_LAYOUT")
        .unwrap()
        .1 = new;
    if decoy {
        root.entries.push(("EDITOR_METADATA".into(), old));
        let mut metadata = Dictionary::new();
        metadata.handle = old;
        metadata.owner = doc.header.named_objects_dict_handle;
        doc.objects.insert(old, ObjectType::Dictionary(metadata));
    }
    for object in doc.objects.values_mut() {
        if let ObjectType::Layout(layout) = object {
            layout.owner = new;
        }
    }
    doc.objects.insert(new, ObjectType::Dictionary(layouts));
    doc.header.acad_layout_dict_handle = new;
    (doc, new)
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

#[test]
fn relocated_named_dictionary_updates_cache_even_when_old_handle_is_metadata() {
    for decoy in [false, true] {
        let (doc, expected) = relocated_layouts(decoy);
        let read = roundtrip(&doc);
        assert_eq!(read.header.acad_layout_dict_handle, expected);
        let Some(ObjectType::Dictionary(layouts)) = read.objects.get(&expected) else {
            panic!()
        };
        assert_eq!(layouts.owner, read.header.named_objects_dict_handle);
        for (name, h) in &layouts.entries {
            let Some(ObjectType::Layout(layout)) = read.objects.get(h) else {
                panic!()
            };
            assert_eq!(&layout.name, name);
            assert_eq!(layout.owner, expected);
        }
        assert_eq!(roundtrip(&read).header.acad_layout_dict_handle, expected);
    }
}

#[test]
fn ambiguous_or_invalid_named_targets_do_not_select_a_layout_cache() {
    for case in 0..4 {
        let (mut doc, target) = relocated_layouts(true);
        let Some(ObjectType::Dictionary(root)) =
            doc.objects.get_mut(&doc.header.named_objects_dict_handle)
        else {
            panic!()
        };
        match case {
            0 => root.entries.push(("ACAD_LAYOUT".into(), target)),
            1..=3 => {}
            _ => unreachable!(),
        }
        let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
        if case != 0 {
            let invalid = match case {
                1 => Handle::new(99999),
                2 => doc.header.model_space_block_handle,
                3 => doc.header.named_objects_dict_handle,
                _ => unreachable!(),
            };
            // Inject the invalid pointer after writing: the writer deliberately
            // omits dangling dictionary entries, so it cannot author this input.
            let index = (0..lines.len())
                .step_by(2)
                .find(|i| lines[*i].trim() == "3" && lines[*i + 1] == "ACAD_LAYOUT")
                .unwrap();
            assert!(matches!(lines[index + 2].trim(), "350" | "360"));
            lines[index + 3] = format!("{:X}", invalid.value());
        }
        let read = DxfReader::from_reader(Cursor::new(lines.join("\n")))
            .unwrap()
            .read()
            .unwrap();
        assert!(read.header.acad_layout_dict_handle.is_null());
        let Some(ObjectType::Dictionary(root)) =
            read.objects.get(&read.header.named_objects_dict_handle)
        else {
            panic!()
        };
        assert_eq!(
            root.entries
                .iter()
                .filter(|(name, _)| name == "ACAD_LAYOUT")
                .count(),
            if case == 0 { 2 } else { 1 }
        );
    }
}
