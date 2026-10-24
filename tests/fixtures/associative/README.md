# Synthetic associative fixtures

`r2010-v2-inline-group.dwg` is generated entirely from hand-encoded synthetic
data by `examples/generate_r2010_inline_constraint_fixture.rs`. It contains a
line and an R2010 version-2 constraint group with two inline nodes (IDs 1 and
4, next ID 9), status bytes, a connection in each direction and an active
constraint. It contains no data copied from a user or third-party drawing.

Regenerate from the repository root:

```sh
cargo run --example generate_r2010_inline_constraint_fixture
```

`dimassoc-perpendicular.dwg` is written by AutoCAD 2025 as AutoCAD 2013
(AC1027). It contains two lines (2C1: (0,0)–(100,0), 2C2: (0,50)–(100,80)), a
linear dimension (2C3), the dimension's extension dictionary (400) and its
DIMASSOC (401), and no data copied from a user or third-party drawing. The
DIMASSOC associates the second point (`90` = 2) through two chained
`AcDbOsnapPointRef` records: Perpendicular (`72` = 8) to 2C2 with subent type 2,
GS marker 4 and distance 0.5, then the `_End` last point (`72` = 1) to no
object, distance 1.0, point (0, 0, 2e50). The second record has no main
sub-entity, so AutoCAD stores no subent type, GS marker or xref paths for it,
in DWG and DXF alike. AutoCAD reopens the fixture with the association intact,
and AUDIT reports 0 errors.

AutoCAD Core Console cannot pick objects for an object snap, so the association
is added through DXF and AutoCAD writes the DWG:

1. In Core Console, starting from `acadiso.dwt`, `entmakex` the two lines, run
   `DIMLINEAR` from `100,0` to `91.74,77.52` with the dimension line at
   `150,40`, and `DXFOUT`.
2. In that DXF, add the `DIMASSOC` / `AcDbDimAssoc` class, a
   `{ACAD_XDICTIONARY` group (360 → 400) on the dimension and a `{ACAD_REACTORS`
   group (330 → 401) on 2C2. Append dictionary 400 (owner 2C3, entry
   `ACAD_DIMASSOC` → 401) and DIMASSOC 401 with the records above to OBJECTS,
   and raise `$HANDSEED` past 401.
3. Open the DXF in Core Console and `SAVEAS` `2013`.

The fixtures and generator are covered by the repository's MPL-2.0 license.
