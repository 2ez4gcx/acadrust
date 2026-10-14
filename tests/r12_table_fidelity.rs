use std::io::Cursor;

use acadrust::io::dwg::r12::header::read_header;
use acadrust::tables::{
    AppId, BlockRecord, DimStyle, Layer, LineType, LineTypeElement, TextStyle, Ucs, VPort, View,
    VxTableRecord,
};
use acadrust::{CadDocument, Color, DwgReader, DwgWriter, DxfVersion, Vector2, Vector3};

fn round_trip(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn ac1009_preserves_native_symbol_table_properties() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    let mut linetype = LineType::new("DASHED");
    linetype.handle = document.allocate_handle();
    linetype.description = "Long dash, short gap".into();
    linetype.elements = vec![LineTypeElement::dash(2.5), LineTypeElement::space(0.75)];
    linetype.pattern_length = 3.25;
    document.line_types.add(linetype.clone()).unwrap();

    let mut layer = Layer::new("CUT");
    layer.handle = document.allocate_handle();
    layer.color = Color::Index(4);
    layer.line_type = linetype.name.clone();
    layer.flags.off = true;
    layer.flags.frozen = true;
    layer.flags.locked = true;
    layer.flags.frozen_in_new_viewport = true;
    document.layers.add(layer.clone()).unwrap();

    let mut style = TextStyle::new("LABELS");
    style.handle = document.allocate_handle();
    style.font_file = "romans.shx".into();
    style.big_font_file = "bigfont.shx".into();
    style.height = 3.5;
    style.last_height = 4.5;
    style.width_factor = 0.8;
    style.oblique_angle = 15.0_f64.to_radians();
    style.flags.backward = true;
    style.is_vertical = true;
    document.text_styles.add(style.clone()).unwrap();

    let mut dimensions = DimStyle::new("DETAIL");
    dimensions.handle = document.allocate_handle();
    dimensions.dimscale = 2.0;
    dimensions.dimasz = 0.65;
    dimensions.dimtxt = 3.25;
    dimensions.dimgap = 0.45;
    dimensions.dimtol = true;
    dimensions.dimse1 = true;
    dimensions.dimtad = 1;
    dimensions.dimclrd = 3;
    dimensions.dimclre = 4;
    dimensions.dimclrt = 5;
    dimensions.dimupt = true;
    dimensions.dimpost = "<> mm".into();
    dimensions.dimblk_name = "ARROW".into();
    dimensions.dimblk1_name = "ARROW1".into();
    dimensions.dimblk2_name = "ARROW2".into();
    document.dim_styles.add(dimensions.clone()).unwrap();
    document.app_ids.add(AppId::new("CUSTOM_APP")).unwrap();
    let mut first_block = BlockRecord::new("FIRST");
    first_block.handle = document.allocate_handle();
    first_block.block_entity_handle = document.allocate_handle();
    first_block.block_end_handle = document.allocate_handle();
    document.block_records.add(first_block).unwrap();

    let mut block = BlockRecord::new("MARKER");
    block.handle = document.allocate_handle();
    block.block_entity_handle = document.allocate_handle();
    block.block_end_handle = document.allocate_handle();
    block.units = 4;
    block.flags.has_attributes = true;
    document.block_records.add(block).unwrap();

    document.header.current_layer_handle = layer.handle;
    document.header.current_text_style_handle = style.handle;
    document.header.current_linetype_handle = linetype.handle;

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let header = read_header(&bytes).unwrap();
    let block_table = header.tables[0];
    let marker_index = document
        .block_records
        .iter()
        .filter(|block| !block.is_model_space() && !block.name.eq_ignore_ascii_case("*Paper_Space"))
        .position(|block| block.name == "MARKER")
        .unwrap();
    assert_eq!(
        bytes[block_table.address as usize + marker_index * block_table.record_size as usize],
        2,
        "R12 BLOCK flags must come from BLOCK, not BLOCK_RECORD insertion units"
    );
    let marker_address =
        block_table.address as usize + marker_index * block_table.record_size as usize;
    let marker_offset = u32::from_le_bytes(
        bytes[marker_address + 35..marker_address + 39]
            .try_into()
            .unwrap(),
    );
    assert_eq!(marker_offset & 0x40000000, 0x40000000);
    assert_ne!(marker_offset & 0x3fffffff, 0);
    assert_eq!(
        bytes[header.layout.blocks_start as usize + (marker_offset & 0x3fffffff) as usize],
        12,
        "R12 BLOCK table offsets must address their own BLOCK marker"
    );
    let restored = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

    let actual = restored.layers.get("CUT").unwrap();
    assert_eq!(actual.color, layer.color);
    assert_eq!(actual.line_type, layer.line_type);
    assert_eq!(actual.flags, layer.flags);
    assert_eq!(restored.header.current_layer_handle, actual.handle);

    let actual = restored.line_types.get("DASHED").unwrap();
    assert_eq!(actual.description, linetype.description);
    assert_eq!(actual.elements, linetype.elements);
    assert_eq!(actual.pattern_length, linetype.pattern_length);
    assert_eq!(restored.header.current_linetype_handle, actual.handle);

    let actual = restored.text_styles.get("LABELS").unwrap();
    assert_eq!(actual.font_file, style.font_file);
    assert_eq!(actual.big_font_file, style.big_font_file);
    assert_eq!(actual.height, style.height);
    assert_eq!(actual.last_height, style.last_height);
    assert_eq!(actual.width_factor, style.width_factor);
    assert!((actual.oblique_angle - style.oblique_angle).abs() < 1e-12);
    assert_eq!(actual.flags, style.flags);
    assert_eq!(actual.is_vertical, style.is_vertical);
    assert_eq!(restored.header.current_text_style_handle, actual.handle);

    let actual = restored.dim_styles.get("DETAIL").unwrap();
    assert_eq!(actual.dimscale, dimensions.dimscale);
    assert_eq!(actual.dimasz, dimensions.dimasz);
    assert_eq!(actual.dimtxt, dimensions.dimtxt);
    assert_eq!(actual.dimgap, dimensions.dimgap);
    assert_eq!(actual.dimtol, dimensions.dimtol);
    assert_eq!(actual.dimse1, dimensions.dimse1);
    assert_eq!(actual.dimtad, dimensions.dimtad);
    assert_eq!(actual.dimclrd, dimensions.dimclrd);
    assert_eq!(actual.dimclre, dimensions.dimclre);
    assert_eq!(actual.dimclrt, dimensions.dimclrt);
    assert_eq!(actual.dimupt, dimensions.dimupt);
    assert_eq!(actual.dimpost, dimensions.dimpost);
    assert_eq!(actual.dimblk_name, dimensions.dimblk_name);
    assert_eq!(actual.dimblk1_name, dimensions.dimblk1_name);
    assert_eq!(actual.dimblk2_name, dimensions.dimblk2_name);
    assert!(restored.app_ids.get("CUSTOM_APP").is_some());
    assert!(
        restored
            .block_records
            .get("MARKER")
            .unwrap()
            .flags
            .has_attributes
    );
}

#[test]
fn ac1009_preserves_view_ucs_viewport_and_vx_tables() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    let mut view = View::new("DETAIL_VIEW");
    view.height = 20.0;
    view.width = 35.0;
    view.center = Vector3::new(2.0, 4.0, 0.0);
    view.direction = Vector3::new(1.0, 2.0, 3.0);
    view.target = Vector3::new(5.0, 6.0, 7.0);
    view.lens_length = 70.0;
    view.twist_angle = 30.0_f64.to_radians();
    view.front_clip = 2.0;
    view.back_clip = 80.0;
    view.perspective = true;
    view.front_clipping = true;
    document.views.add(view.clone()).unwrap();

    document.ucss.add(Ucs::new("FIRST_UCS")).unwrap();
    let ucs = Ucs::from_origin_axes(
        "DETAIL_UCS",
        Vector3::new(4.0, 5.0, 6.0),
        Vector3::UNIT_Y,
        -Vector3::UNIT_X,
    );
    document.ucss.add(ucs.clone()).unwrap();
    document.header.model_space_ucs_name = ucs.name.clone();
    document.header.paper_space_ucs_name = "FIRST_UCS".into();

    let mut viewport = VPort::new("DETAIL_VPORT");
    viewport.lower_left = Vector2::new(0.1, 0.2);
    viewport.upper_right = Vector2::new(0.8, 0.9);
    viewport.view_center = Vector2::new(3.0, 4.0);
    viewport.view_height = 40.0;
    viewport.aspect_ratio = 1.5;
    viewport.view_target = Vector3::new(5.0, 6.0, 7.0);
    viewport.view_direction = Vector3::new(1.0, 2.0, 3.0);
    viewport.view_twist = 20.0_f64.to_radians();
    viewport.snap_rotation = 10.0_f64.to_radians();
    viewport.snap_base = Vector2::new(2.0, 3.0);
    viewport.snap_spacing = Vector2::new(0.25, 0.75);
    viewport.grid_spacing = Vector2::new(5.0, 10.0);
    viewport.grid_on = true;
    viewport.snap_on = true;
    viewport.snap_style = true;
    viewport.snap_isopair = 2;
    viewport.ucsicon_lower = false;
    document.vports.add(viewport.clone()).unwrap();

    let mut vx = VxTableRecord::new("DETAIL_VX");
    vx.is_on = true;
    vx.legacy_viewport_entity_address = 1755;
    vx.legacy_viewport_index = 1;
    vx.legacy_previous_entry_index = -1;
    document.vx_table.add(vx.clone()).unwrap();

    let restored = round_trip(&document);
    let actual = restored.views.get(&view.name).unwrap();
    assert_eq!(actual.height, view.height);
    assert_eq!(actual.width, view.width);
    assert_eq!(actual.center, view.center);
    assert_eq!(actual.direction, view.direction);
    assert_eq!(actual.target, view.target);
    assert_eq!(actual.lens_length, view.lens_length);
    assert!((actual.twist_angle - view.twist_angle).abs() < 1e-12);
    assert_eq!(actual.front_clip, view.front_clip);
    assert_eq!(actual.back_clip, view.back_clip);
    assert_eq!(actual.perspective, view.perspective);
    assert_eq!(actual.front_clipping, view.front_clipping);

    let actual = restored.ucss.get(&ucs.name).unwrap();
    assert_eq!(actual.origin, ucs.origin);
    assert_eq!(actual.x_axis, ucs.x_axis);
    assert_eq!(actual.y_axis, ucs.y_axis);
    assert_eq!(restored.header.model_space_ucs_name, ucs.name);
    assert_eq!(restored.header.paper_space_ucs_name, "FIRST_UCS");

    let actual = restored.vports.get(&viewport.name).unwrap();
    assert_eq!(actual.lower_left, viewport.lower_left);
    assert_eq!(actual.upper_right, viewport.upper_right);
    assert_eq!(actual.view_center, viewport.view_center);
    assert_eq!(actual.view_height, viewport.view_height);
    assert_eq!(actual.aspect_ratio, viewport.aspect_ratio);
    assert_eq!(actual.view_target, viewport.view_target);
    assert_eq!(actual.view_direction, viewport.view_direction);
    assert!((actual.view_twist - viewport.view_twist).abs() < 1e-12);
    assert!((actual.snap_rotation - viewport.snap_rotation).abs() < 1e-12);
    assert_eq!(actual.snap_base, viewport.snap_base);
    assert_eq!(actual.snap_spacing, viewport.snap_spacing);
    assert_eq!(actual.grid_spacing, viewport.grid_spacing);
    assert_eq!(actual.grid_on, viewport.grid_on);
    assert_eq!(actual.snap_on, viewport.snap_on);
    assert_eq!(actual.snap_style, viewport.snap_style);
    assert_eq!(actual.snap_isopair, viewport.snap_isopair);
    assert_eq!(actual.ucsicon_lower, viewport.ucsicon_lower);
    assert_eq!(actual.ucsicon_origin, viewport.ucsicon_origin);

    let actual = restored.vx_table.get(&vx.name).unwrap();
    assert_eq!(actual.is_on, vx.is_on);
    assert_eq!(
        actual.legacy_viewport_entity_address,
        vx.legacy_viewport_entity_address
    );
    assert_eq!(actual.legacy_viewport_index, vx.legacy_viewport_index);
    assert_eq!(
        actual.legacy_previous_entry_index,
        vx.legacy_previous_entry_index
    );
    assert!(actual.handle.is_valid());
}

#[test]
fn ac1009_preserves_special_current_linetypes_and_world_ucs() {
    for (name, index) in [("ByLayer", 32767), ("ByBlock", 32766)] {
        let mut document = CadDocument::with_version(DxfVersion::AC1009);
        document.header.current_linetype_handle = document.line_types.get(name).unwrap().handle;
        document.ucss.add(Ucs::new("UNSELECTED_UCS")).unwrap();
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let header = read_header(&bytes).unwrap();
        assert_eq!(header.references.linetype_index, index);
        assert_eq!(header.references.ucs_index, -1);
        assert_eq!(header.references.paper_ucs_index, -1);
        let restored = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(
            restored.header.current_linetype_handle,
            restored.line_types.get(name).unwrap().handle
        );
        assert!(restored.header.model_space_ucs_name.is_empty());
        assert!(restored.header.paper_space_ucs_name.is_empty());
    }
}

#[test]
fn ac1009_resolves_escaped_table_names_and_preserves_text_spaces() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document.header.code_page = "ANSI_1252".into();
    let mut layer = Layer::new("\u{5c42}");
    layer.handle = document.allocate_handle();
    let mut style = TextStyle::new("\u{5b57}");
    style.handle = document.allocate_handle();
    let mut linetype = LineType::new("\u{7ebf}");
    linetype.handle = document.allocate_handle();
    linetype.description = "  spaced description  ".into();
    layer.line_type = linetype.name.clone();
    document.header.current_layer_handle = layer.handle;
    document.header.current_text_style_handle = style.handle;
    document.header.current_linetype_handle = linetype.handle;
    document.header.model_space_ucs_name = "\u{5750}".into();
    document.ucss.add(Ucs::new("\u{5750}")).unwrap();
    document.layers.add(layer.clone()).unwrap();
    document.text_styles.add(style.clone()).unwrap();
    document.line_types.add(linetype.clone()).unwrap();
    let mut text = acadrust::Text::with_value("  label  ", Vector3::ZERO);
    text.common.layer = layer.name.clone();
    text.style = style.name.clone();
    document
        .add_entity(acadrust::EntityType::Text(text))
        .unwrap();

    let restored = round_trip(&document);
    assert_eq!(
        restored.header.current_layer_handle,
        restored.layers.get(&layer.name).unwrap().handle
    );
    assert_eq!(
        restored.header.current_text_style_handle,
        restored.text_styles.get(&style.name).unwrap().handle
    );
    assert_eq!(
        restored.header.current_linetype_handle,
        restored.line_types.get(&linetype.name).unwrap().handle
    );
    assert_eq!(restored.header.model_space_ucs_name, "\u{5750}");
    assert_eq!(
        restored.layers.get(&layer.name).unwrap().line_type,
        linetype.name
    );
    assert_eq!(
        restored.line_types.get(&linetype.name).unwrap().description,
        linetype.description
    );
    let acadrust::EntityType::Text(text) = restored.entities().next().unwrap() else {
        panic!("expected TEXT");
    };
    assert_eq!(text.value, "  label  ");
    assert_eq!(text.style, style.name);
}

#[test]
fn ac1009_preserves_duplicate_unnamed_vx_rows() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    for index in 0..3 {
        let mut vx = VxTableRecord::new("");
        vx.legacy_viewport_entity_address = 2000 + index as u16;
        vx.legacy_viewport_index = index;
        vx.legacy_previous_entry_index = index - 1;
        document.vx_table.add_allow_duplicate(vx);
    }
    let restored = round_trip(&document);
    assert_eq!(restored.vx_table.len(), 3);
    for (index, vx) in restored.vx_table.iter().enumerate() {
        assert_eq!(vx.name, "");
        assert_eq!(vx.legacy_viewport_entity_address, 2000 + index as u16);
        assert_eq!(vx.legacy_viewport_index, index as i16);
        assert_eq!(vx.legacy_previous_entry_index, index as i16 - 1);
        assert!(vx.handle.is_valid());
    }
}
