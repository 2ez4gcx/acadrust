use std::io::Cursor;

use acadrust::{objects::ObjectType, CadDocument, DwgReader, DwgWriter, DxfVersion, Handle};

#[test]
fn saved_self_reference_survives_read_without_losing_reverse_layout_link() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.ensure_model_layout();
    let model = doc
        .block_records
        .iter_mut()
        .find(|r| r.is_model_space())
        .unwrap();
    let model_handle = model.handle;
    let layout_handle = model.layout;
    assert_ne!(model_handle, layout_handle);
    model.layout = model_handle;

    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let model = loaded
        .block_records
        .iter()
        .find(|r| r.is_model_space())
        .unwrap();
    assert_eq!(model.layout, model.handle);
    let Some(ObjectType::Layout(layout)) = loaded.objects.get(&layout_handle) else {
        panic!("Missing reverse layout link")
    };
    assert_eq!(layout.block_record, model.handle);
}

#[test]
fn missing_back_link_is_still_recovered_from_existing_layout() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.ensure_model_layout();
    let model = doc
        .block_records
        .iter_mut()
        .find(|r| r.is_model_space())
        .unwrap();
    let layout_handle = model.layout;
    model.layout = Handle::NULL;
    doc.ensure_model_layout();
    assert_eq!(
        doc.block_records
            .iter()
            .find(|r| r.is_model_space())
            .unwrap()
            .layout,
        layout_handle
    );
}
