//! AutoCAD Architecture classes.
//!
//! Walls, doors, stairs, spaces, slabs, curtain walls, structural members, and
//! the style and display objects that belong to them are registered AEC
//! classes. Their DWG records stay in the original class layout so Architecture
//! can open them. Profiles decoded from those records are editable: moving or
//! reshaping a wall rewrites every stored copy of its profile, including the
//! proxy-graphics preview.

mod entity;
mod geometry;
mod object;

pub use entity::{reshape_rectangle, ArchEntityData, ArchKind};
pub use geometry::{decode_profiles, map_profile_points, polyline_from_type6};
pub use object::ArchObject;

/// DXF names of the Architecture entities found in the Architecture sample
/// projects, with the C++ class AutoCAD registers for each.
pub const ENTITY_CLASSES: &[(&str, &str, ArchKind)] = &[
    ("AEC_WALL", "AecDbWall", ArchKind::Wall),
    ("AEC_DOOR", "AecDbDoor", ArchKind::Door),
    ("AEC_WINDOW", "AecDbWindow", ArchKind::Window),
    ("AEC_OPENING", "AecDbOpening", ArchKind::Opening),
    ("AEC_STAIR", "AecDbStair", ArchKind::Stair),
    ("AEC_RAILING", "AecDbRailing", ArchKind::Railing),
    ("AEC_SLAB", "AecDbSlab", ArchKind::Slab),
    ("AEC_ROOFSLAB", "AecDbRoofSlab", ArchKind::RoofSlab),
    ("AEC_ROOF", "AecDbRoof", ArchKind::Roof),
    ("AEC_SPACE", "AecDbSpace", ArchKind::Space),
    ("AEC_CURTAIN_WALL_LAYOUT", "AecDbCurtainWallLayout", ArchKind::CurtainWall),
    ("AEC_CURTAIN_WALL_UNIT", "AecDbCurtainWallUnit", ArchKind::CurtainWallUnit),
    ("AEC_WINDOW_ASSEMBLY", "AecDbWindowAssembly", ArchKind::WindowAssembly),
    ("AEC_COLUMN_GRID", "AecDbColumnGrid", ArchKind::ColumnGrid),
    ("AEC_CEILING_GRID", "AecDbCeilingGrid", ArchKind::CeilingGrid),
    ("AEC_POLYGON", "AecDbPolygon", ArchKind::Polygon),
    ("AEC_MASS_ELEM", "AecDbMassElem", ArchKind::MassElement),
    ("AEC_MASS_GROUP", "AecDbMassGroup", ArchKind::MassGroup),
    ("AEC_MVBLOCK_REF", "AecDbMvBlockRef", ArchKind::MultiViewBlock),
    ("AEC_MASKBLOCK_REF", "AecDbMaskBlockRef", ArchKind::MaskBlock),
    ("AEC_2D_SECTION", "AecDb2dSection", ArchKind::Section),
    ("AEC_BDG_SECTIONLINE", "AecDbBdgSectionLine", ArchKind::SectionLine),
    ("AEC_BDG_ELEVLINE", "AecDbBdgElevLine", ArchKind::ElevationLine),
    ("AEC_DIMENSION_GROUP", "AecDbDimensionGroup", ArchKind::Dimension),
    ("AEC_SCHEDULE_TABLE", "AecDbScheduleTable", ArchKind::Schedule),
    ("AEC_LAYOUT_CURVE", "AecDbLayoutCurve", ArchKind::LayoutCurve),
    ("AEC_LAYOUT_GRID2D", "AecDbLayoutGrid2d", ArchKind::LayoutGrid),
    ("AEC_LAYOUT_GRID3D", "AecDbLayoutGrid3d", ArchKind::LayoutGrid),
    ("AEC_ZONE", "AecDbZone", ArchKind::Zone),
    ("AEC_AREA", "AecDbArea", ArchKind::Area),
    ("AEC_AREAGROUP", "AecDbAreaGroup", ArchKind::Area),
    ("AEC_CAMERA", "AecDbCamera", ArchKind::Camera),
    ("AEC_SLICE", "AecDbSlice", ArchKind::Slice),
    ("AEC_ENT_REF", "AecDbEntRef", ArchKind::EntityReference),
    ("AEC_POINT", "AecDbPoint", ArchKind::Point),
    ("AEC_DISPLAYTHEME", "AecDbDisplayTheme", ArchKind::DisplayTheme),
    ("AECS_MEMBER", "AecsDbMember", ArchKind::StructuralMember),
];

/// Whether `dxf_name` is an AutoCAD Architecture / MEP class.
///
/// Matches the AEC, structural, and building-systems classes stored in the
/// Architecture sample drawings. Third-party classes are left untouched.
pub fn is_architecture_name(dxf_name: &str) -> bool {
    let name = dxf_name.trim();
    let upper = name.to_ascii_uppercase();
    upper.starts_with("AEC") || upper.starts_with("AAEC") || upper.starts_with("ABST_")
}

/// Entity kind for a known Architecture DXF name.
pub fn kind_of(dxf_name: &str) -> Option<ArchKind> {
    let upper = dxf_name.trim().to_ascii_uppercase();
    ENTITY_CLASSES
        .iter()
        .find(|(name, _, _)| *name == upper)
        .map(|(_, _, kind)| *kind)
}

/// C++ class name registered for a known Architecture entity, when this crate
/// knows it. Callers that have the drawing's CLASSES section should prefer
/// that entry.
pub fn cpp_class_name(dxf_name: &str) -> Option<&'static str> {
    let upper = dxf_name.trim().to_ascii_uppercase();
    ENTITY_CLASSES
        .iter()
        .find(|(name, _, _)| *name == upper)
        .map(|(_, cpp, _)| *cpp)
}
