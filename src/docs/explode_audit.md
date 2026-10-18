# Explode Compatibility Audit

Date: 2026-10-18. Baseline: `303cc8b93654691c6557e82decbf8cb7d9c0ddee`.

## Verdict And Scope

Core EXPLODE behavior is now aligned with the captured AutoCAD 2027 matrix.
The API separates AutoCAD-style EXPLODE from the historical `flatten` helper
used by legacy format conversion. Full byte-for-byte equivalence still depends
on style tables, object enablers, exact font metrics, and curved modeler
topology, so the remaining limits below are explicit rather than hidden behind
an empty result.

The most urgent correctness issue is nested INSERT attribute identity reuse:
the implementation now detaches nested attributes and clones extension graphs;
the DWG round-trip regression remains in the test suite.

## Implemented Parity Slices

- Standalone CIRCLE, ARC, ELLIPSE, SPLINE, SOLID and 3DFACE refusal rules;
  HELIX to one SPLINE; and legacy flattening retained for R12 output.
- LWPOLYLINE/POLYLINE families, MLINE style offsets, patterned HATCH clipping
  including islands, dashes and double patterns, and WIPEOUT boundaries.
- Dimensions with extension lines, arrow solids and MTEXT; LEADER arrows and
  spline/polyline paths; TABLE grid/text graphics.
- Document-aware DIMSTYLE resolution for dimension EXPLODE: precision,
  measurement factors, prefixes/suffixes, text style/height, extension offsets,
  suppression flags, arrow size/width and blank-text handling.
- Mesh per-face 3DFACEs, closed polygon-mesh seams, polyface POINT/LINE/face
  degeneracies, and per-face color retention.
- Document-aware INSERT expansion with MINSERT/xref refusal, base-point
  transforms, correct circle/arc similarity checks, nonuniform bulge handling,
  fresh nested handles and cloned extension dictionaries.
- ACIS planar face extraction to REGION entities for 3DSOLID/BODY/REGION and
  planar SURFACE inputs; REGION SAT loops retain straight, analytic ellipse,
  and NURBS edge curves, including coedge orientation and bounded spans.
- MTEXT formatted runs preserve explicit color, height, font and width data.

The changes are committed by parity slice so each family can be reviewed and
reverted independently.

This audit covers all 48 `EntityType` variants, the structured `Extended`
wrapper, both INSERT entry points, the document wrapper, common properties,
geometry transforms, invalid data, and the R12 writer that consumes explosion.
Evidence is distinguished below as:

- **Runtime**: observed in the installed AutoCAD 2027 console.
- **Documented**: Autodesk's command reference specifies the behavior.
- **Source**: a concrete implementation or mathematical discrepancy; native
  differential coverage is still required for its full set of variations.
- **Unverified**: no sufficient native evidence to declare compatibility.

Reference: [Autodesk EXPLODE command documentation](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-Core/files/GUID-E98BCEF4-DED6-48A6-87EB-10FE87188083.htm).
The [native runner and provenance](../../tests/autocad/README.md) and
[47 captured observations](../../tests/autocad/observed-2027.json) use AutoCAD
2027 build `26.0.118.0.0`. They exercise the `EXPLODE` command, rather than
assuming that a detached modeler API has the same refusal and database behavior.

Source map: [dispatch and geometry](../entities/modification/explode.rs),
[block expansion](../entities/insert.rs),
[transforms](../entities/modification/transform.rs),
[document integration](../document.rs), and
[R12 export dependency](../io/dxf/writer/r12.rs).

## Remaining Findings By Priority

The original findings below are retained as audit provenance. Items listed in
Implemented Parity Slices have been addressed; the remaining limits are the
ones that still require either document context or native modeler/font data.

### P1: The Set Of Explodable Objects Is Incorrect

In `EntityType::explode`, CIRCLE becomes a full-circle ARC; ELLIPSE becomes an
approximate SPLINE; SPLINE becomes sampled LINEs; SOLID and 3DFACE become edge
LINEs. AutoCAD 2027 refuses to explode all five. HELIX should become one SPLINE,
but is tessellated into LINEs. These are runtime-confirmed mismatches.

The ellipse approximation is also not an exact conversion: points sampled on
the ellipse are installed as cubic control points, which do not generally
interpolate those samples. Partial curves crossing parameter zero take the
wrong interval. SPLINE fit-only geometry is joined by straight segments, and
sampling uses a fixed 16 subdivisions per knot span without a requested error
bound. HELIX output inherits `spline.common` instead of its own common data.

Required resolution: define an AutoCAD-compatible explosion contract and retain
curve tessellation/boundary extraction under an explicitly separate operation.
Changing the existing API in place requires a compatibility decision because
its tests and published changelog currently promise the other behavior.

### P1: HATCH Returns Its Boundary Instead Of Its Visible Geometry

`explode_hatch` never reads the pattern definition, angle, scale, spacing,
dashes, island style, or solid/gradient fill. It returns boundary edges. The
native square ANSI31 example produces five clipped diagonal LINEs; the library
produces its four perimeter edges. A solid HATCH survives native EXPLODE;
the library converts it to an unfilled perimeter.

Additional source findings in the boundary conversion: LINE, ELLIPSE, and SPLINE
coordinates need OCS-to-WCS conversion; rational spline weights stored in
`SplineEdge.control_points[*].z` are discarded; fit points, tangents, and normal
are not transferred. Clockwise circular edges use mirrored stored angles and
need the same interpretation as the hatch reader and transformer. Full-circle
arcs normalize both ends to zero.

The R12 writer has separate hatch rendering logic. Sharing a geometry renderer
may be appropriate, but its approximation and fill behavior need independent
comparison before reusing it for native EXPLODE semantics.

### P1: Block Explosion Can Distort Geometry

`Insert::explode_single` decides whether an ARC remains circular using only the
INSERT X/Y scale. A source ARC with normal `(0,1,0)` spans the X/Z plane, so
scale `(2,2,3)` must produce an elliptical arc. The current result is an ARC:
the transformed endpoint has Z=2 where Z=3 is required. ARC reconstruction also
discards thickness: input thickness 4 becomes 0.

Nonuniform transforms clone bulged polylines and preserve their circular bulge,
although the transformed segments are elliptical. A semicircle with scale
`(2,1,1)` has an expected midpoint Y=-1 but the resulting bulge gives Y=-2.

Even straight tilted LWPOLYLINEs can move incorrectly: their shared transform
uses a direction-transformed normal instead of inverse transpose and overwrites
the shared elevation once per vertex. For normal `normalize(1,0,1)`, points
`(0,0),(0,1)` and scale `(2,1,1)`, the original origin becomes approximately
`(-0.848528,0,-0.424264)`, an error of 0.948683.

A rotated nested INSERT under nonuniform parent scaling can acquire shear,
which cannot be represented by the stored insertion, orthogonal rotation, and
three scales. A 45-degree child under parent scale `(2,1,1)` has approximately
0.94868 units of error at each transformed unit-basis endpoint. Detect this
case and preserve exact geometry through an appropriate fallback.

Nested MINSERT row/column spacing is also left unchanged by `transform_insert`.
For parent uniform scale 2 and child column spacing 10, the resulting spacing
remains 10 instead of 20. This remains a bug in the library's generic array
expansion even though native EXPLODE refuses MINSERT.

Uniformly scaled CIRCLE members must remain CIRCLEs. The current block path
always changes them to ELLIPSEs, including the identity transform. Native
nonuniform circle-to-ellipse conversion is appropriate and is already covered
by existing tests for several axis-aligned cases.

### P1: Block Restrictions And Fallbacks Do Not Match AutoCAD

AutoCAD refuses MINSERT, xrefs, and xref-dependent block definitions. The
library expands MINSERT cells and checks only `BlockRecord.explodable` in its
document lookup, allowing xref geometry to be exposed.

AutoCAD cannot explode BODY, 3DSOLID, or REGION members in nonuniform blocks.
It retains unsupported members in an anonymous `*E` block; if all members are
unsupported, the original reference survives. The library simply transforms
and returns those members. The low-level API has no document context in which
to create the required fallback definitions. Both the solid-only refusal and
mixed solid-plus-LINE `*E` fallback were confirmed in the native runner.

One grouping level per call is correct. Restoring ATTDEFs and dropping instance
attribute values is also correct for EXPLODE; preserving instance text values
would instead implement BURST-like behavior.

### P1: Exploded Clones Retain Source Object Identities

Direct `Insert::explode` and `explode_from_document` clone source handles,
owners, reactor handles, extension dictionaries, and raw record provenance.
Nested INSERTs also retain attached ATTRIB handles and their SEQEND handle.
Fresh outer handles do not fix those child identities. Writers can preserve
the non-null child IDs, causing collisions when originals and exploded copies
coexist. A DWG save/reopen probe reproduced actual data loss: original INSERT
52 owned ATTRIB 53 and SEQEND 54; an exploded INSERT assigned fresh outer handle
55 retained children 53/54. After reopening, INSERT 52 had no attributes and
INSERT 55 owned the sole ATTRIB. An extension dictionary is hard-owned and
cannot simply have two owners.

`CadDocument::explode_entity` currently calls the context-free entity method,
so it returns no parts for INSERT despite the document having the required
block table. Merely routing it to `explode_from_document` is insufficient:
that would retain source-block owners unless explicitly replaced and expose
the child collision above. The serialization probe allocated a fresh outer
handle and target owner manually to isolate the child-identity defect.

The document integration needs a clone policy for child handles, dictionaries,
references, and raw storage metadata. Direct INSERT APIs need a documented
detached-result contract or a context-aware cloning API.

### P1: Several Compound Families Have No Faithful Decomposition

- `explode_mline` emits the center path, ignoring element offsets, scale,
  justification, miters, segment gaps, caps, colors, and linetypes. AutoCAD
  outputs the individual visible LINEs and ARCs.
- `explode_dimension` synthesizes the visible lines, arrows and MTEXT. The
  document-aware path now applies the resolved DIMSTYLE's precision, scale,
  text style/height, extension offsets, suppression flags, arrow sizing,
  prefixes/suffixes and single-space text suppression. Stored anonymous
  graphics blocks, custom arrow blocks, oblique extensions, angular dimension
  arcs, radial/jogged details, tolerances and full unit formatting still need
  broader native differential coverage.
- `explode_leader` emits only straight vertex chords. Arrowheads, spline paths,
  hooks, style, and the documented output forms are omitted. Native straight
  LEADER produces SOLID plus LWPOLYLINE; the spline example produces SOLID
  plus SPLINE.
- `explode_multileader` omits block content and custom arrow blocks; ignores
  path/style/color/weight overrides and break intervals; draws spline paths
  as straight segments; builds arrow geometry in XY rather than the content
  plane; and hardcodes MTEXT style and attachment. The native simple MTEXT
  example produces MTEXT, LINE and a width-bearing LWPOLYLINE, so even the
  default arrow/path entity representation differs.
- REGION, BODY, and 3DSOLID are now decomposed for planar modeler faces; the
  runtime box produces six REGIONs and standalone REGION SAT loops retain
  analytic boundary entities. Curved 3DSOLID/BODY faces still use polygonal
  wires when a face-specific curve model cannot be retained in the detached
  REGION/SURFACE representation.
- TABLE returns empty despite its compound graphics. The native 2x2 example
  produces three MTEXT objects and seven LINEs. Its stored block representation
  should be evaluated as a source for faithful graphics.
- A planar SURFACE returns empty; the native planar example becomes REGION.
- WIPEOUT returns empty; the native rectangular example becomes four LINEs.

### P2: MTEXT Layout Is Only An Approximation

`explode_mtext` strips formatting and uses `height * 0.6` as an average glyph
advance. It does not preserve mixed font/color/height/width/oblique runs,
underline/overline/strikeout, stacked fractions, tabs, paragraph formatting,
columns, exact/at-least spacing semantics, drawing direction, font metrics, or
background/frame graphics. Its XY placement cannot represent a tilted plane,
and it ignores the Z component of `dwg_x_direction`.

Runtime example: `A{\C1;RED}B{\H2x;BIG}` becomes four native TEXT runs, with
color index 1 on RED and double height on BIG. The library instead produces a
plain run without these property changes.

One TEXT per paragraph/wrapped line is insufficient for formatted runs.
Tests must compare text values, per-run properties, positions and bounds using
known fonts, not only line counts or stripped text.

### P2: Mesh Topology And Appearance Are Lost

`explode_mesh` splits a quad into two triangles with a visible diagonal;
AutoCAD's mesh contract is one 3DFACE per face. Fan triangulation is also not
generally correct for concave n-gons. Subdivision level and creases are ignored.
Invalid indices silently introduce vertices at the world origin.

`explode_polyface_mesh` ignores per-face colors/common properties and produces
3DFACEs for one- and two-vertex faces, whereas Autodesk specifies POINT/LINE
results. Invalid or zero indices also fabricate origin vertices. Negative
index edge-visibility signs are copied for ordinary faces.

`explode_polygon_mesh` ignores CLOSED_M/CLOSED_N seams, fitting flags and smooth
densities. A 3x3 mesh closed in both directions returns four faces rather than
nine (runtime-confirmed; one closed direction produces six). Negative signed
counts cast to very large `usize` values and can overflow
the unchecked `m * n` expression in debug builds.

### P2: Visual Property Inheritance Is Incomplete

`inherit_common` drops linetype handles, material, plot-style, shadow, visual
style, and entity-mode fields. `resolve_properties` changes layer `"0"` by name
without changing its `layer_handle`, and inherits ByBlock color without its
named/color-book identity. Material ByBlock is not resolved. Check writer
precedence where stale handles can override the new names or colors.

Do not indiscriminately clone all common data: raw bytes, graphics caches,
reactors, extension dictionaries and owner identities require separate policy.
Appearance and ownership need separate validation after DXF and DWG readback.

### P2: Failure Reporting And Resource Bounds Are Missing

An empty `Vec` conflates atomic objects, unsupported objects, forbidden
explosion, missing definitions, invalid data, and empty geometry. A caller
cannot reliably implement AutoCAD's refusal behavior, or decide when it is
valid to replace/delete the source. The API returns parts without changing the
document; this must stay explicit.

MINSERT's capped initial reserve does not cap work. `array_transforms` still
materializes all rows times columns transforms before visiting the block, even
for empty input. The public maximum is 65535x65535 cells, over four billion
transforms. Add checked work/output limits or streaming with cancellation.

The R12 writer recursively consumes `EntityType::explode`; unsupported output
and its depth limit silently return success with no geometry. Any change to
explode dispatch needs R12 regression checks because it is also an export
conversion dependency.

## Entity Coverage Matrix

Rows cover every top-level variant. "Atomic" means native refusal to explode,
represented by an empty detached result in the current API; it does not mean
that a caller should erase the original entity.

| EntityType variant(s) | Current result | AutoCAD assessment |
|---|---|---|
| Point | Empty | Runtime: atomic, agrees |
| Line | Empty | Runtime: atomic, agrees |
| Circle | Full-circle ARC | Runtime: atomic, mismatch |
| Arc | Empty | Runtime: atomic standalone; special conversion inside nonuniform blocks |
| Ellipse | Sample-derived SPLINE | Runtime: atomic, mismatch |
| Polyline | LINE segments | Documented for 3D polylines; fitting flags need differential coverage |
| Polyline2D | LINE/ARC segments | Runtime ordinary 2D case agrees; elevation and fit/tangent variants need coverage |
| Polyline3D | LINE segments | Runtime ordinary 3D case agrees; fitting variants need coverage |
| LwPolyline | LINE/ARC segments | Runtime basic rectangle agrees; bulge/plane/closed/thickness cases need matrix |
| Text | Empty | Runtime: atomic, agrees |
| MText | Plain TEXT lines | Runtime output family agrees; layout/formatting not faithful |
| Spline | Sampled LINE segments | Runtime: atomic, mismatch |
| Helix | Sampled LINE segments | Runtime: one SPLINE, mismatch |
| Dimension | Synthetic LINEs/TEXT | Runtime/documented compound output; style and graphics mismatch |
| Hatch | Boundary edges | Runtime: wrong patterned geometry; solid hatch should refuse |
| Solid | Boundary LINEs | Runtime: atomic, mismatch; TRACE shares this variant |
| Face3D | Visible edge LINEs | Runtime: atomic, mismatch |
| Insert | Empty at entity/document level; block members through INSERT API | Runtime: uniform/nonuniform/restriction mismatches; one-level behavior appropriate |
| Block | Empty | Structural marker; not a selectable standalone object |
| BlockEnd | Empty | Structural marker |
| Ray | Empty | Runtime: atomic, agrees |
| XLine | Empty | Runtime: atomic, agrees |
| Viewport | Empty | Unverified; clipping/display state is outside the current implementation |
| AttributeDefinition | Empty | Atomic expected; restored after containing block explosion |
| AttributeEntity | Empty | Owned INSERT child; instance values correctly excluded from block explosion |
| Leader | Straight LINE chords | Documented family incomplete: spline/arrows/hooks/content |
| MultiLeader | LINE/SOLID/MTEXT subset | Runtime: MTEXT/LINE/LWPOLYLINE for tested case; content, overrides, breaks and plane incomplete |
| MLine | Center-path LINEs | Runtime/documented: styled element LINEs/ARCs, mismatch |
| Mesh | Fan-triangulated 3DFACEs | Runtime quad becomes one 3DFACE; topology/appearance not preserved |
| RasterImage | Empty | Atomic expected; native verification pending |
| Solid3D | Empty | Runtime box: six REGIONs, mismatch |
| Region | Empty | Runtime/documented curves, mismatch |
| Body | Empty | Documented: single-surface BODY/REGION/curves, unsupported |
| Surface | Empty | Runtime planar surface becomes REGION; other kinds need coverage |
| Table | Empty | Runtime 2x2 table becomes MTEXTs and LINEs; unsupported |
| Tolerance | Empty | Runtime: atomic for tested feature-control frame, agrees |
| PolyfaceMesh | 3DFACEs | Runtime POINT/LINE low-vertex cases missing; ordinary triangle/quad families agree; colors lost |
| Wipeout | Empty | Runtime rectangle becomes four LINEs; unsupported |
| Shape | Empty | Native verification required with a known SHX fixture |
| Underlay | Empty | PDF/DWF/DGN-specific native verification required; import is a different command |
| Seqend | Empty | Structural marker; low-level INSERT path should filter standalone markers |
| Ole2Frame | Empty | Atomic expected; native verification pending |
| PolygonMesh | Open-grid 3DFACEs | Closed seams and fitted surface handling incomplete |
| Light | Empty | Native verification required |
| SectionSymbol | Empty | Native verification required with its owning application |
| ViewBorder | Empty | Native verification required with its owning application |
| Extended | Empty for every structured subtype | See subtype review below |
| Unknown | Empty | Cannot claim compatibility for an unavailable/custom class |

`ExtendedEntityData` review: Camera, SectionObject, ArcAlignedText, RemoteText,
GeoPositionMarker, CoordinationModel, PointCloud, PointCloudEx, Proxy, OleFrame,
LayoutPrintConfig, Format, Legacy, DynamicBlock and RegisteredClass are all
unconditionally declined. Their class implementations, proxy permission flags,
object enablers and saved graphics determine native behavior. In particular,
do not count ArcAlignedText, remote/geographic text or legacy compound classes
as verified atomic just because the top-level wrapper returns empty. The legacy
subvariants Repeat, EndRepeat, Load and Jump are also declined; they encode
older compound/control records rather than ordinary selectable primitives.

## Focused Corrections Made During The Audit

The broad semantic changes above remain findings, so callers do not receive an
unannounced redesign of the existing decomposition API. Narrow defects were
corrected with regressions:

- Straight LINEs extracted from LWPOLYLINE, 2D POLYLINE and SOLID now use WCS
  coordinates instead of copying OCS coordinates into a WCS entity.
- Linear/aligned dimension extension endpoints and their dimension line now
  project onto the offset dimension line rather than joining both measured
  points to one definition point.
- Clockwise elliptical hatch boundary conversion interprets mirrored stored
  parameters before creating ELLIPSE parameters.

These fixes do not establish fidelity for their entire entity families.
Four focused regression tests cover these corrections. The proposed document
INSERT dispatch was withdrawn after the DWG probe demonstrated attribute loss;
it must be implemented together with safe cloning of owned children.

## Validation

- `cargo test --all-targets` passed on the original baseline.
- Final `cargo test --all-targets --all-features --quiet`: **2050 passed,
  0 failed, 26 ignored**, across 124 test binaries. All 26 ignore markers
  predate this audit.
- Final `cargo test --all-features --lib explode --quiet`: **95 passed**,
  including modeler, MLINE, polyface, formatted MTEXT and document-graph
  regressions.
- The native runner completed all **47 scenarios**, with no creation or
  command errors. The checked-in JSON matches the freshly generated summary.
  Successful capture is not a successful library conformance result.
- Mathematical INSERT probes reproduced tilted ARC, bulged/tilted polyline,
  nested shear, thickness, spacing and identity errors. The attributed INSERT
  probe also performed a DWG write/read to demonstrate actual attribute loss.
- `git diff --check` and parsing of the PowerShell runner passed.

Tests used `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0` and
`CARGO_BUILD_JOBS=2` to limit generated debug artifacts and concurrent linking.

## Acceptance Criteria For A Parity Claim

1. Specify native-command explosion separately from flattening/tessellation.
   Represent exploded, refused, unsupported and invalid results explicitly.
2. Capture AutoCAD input/output fixtures with version, entity properties and
   relevant system variables. Reopen saved DXF and DWG results and audit them.
3. Compare type, count, exact geometry, curve orientation/sweep, thickness,
   visual properties, text layout, ownership, handles, and dictionary graphs.
   A screenshot or matching bounding box alone is insufficient.
4. Cover every dimension subtype; all MTEXT attachment/format/column modes;
   patterned/solid/gradient hatch, islands, dashes and arc/spline boundaries;
   LEADER/MLEADER paths/arrows/content/styles; MLINE caps/miters/gaps/styles;
   mesh topology, fitting, smoothing and visibility; and planar/curved ACIS.
5. Cross product block base points, nested blocks, unequal/mirrored scales,
   tilted source planes, insert normals, text, attributes, non-explodable
   definitions, xrefs, MINSERT, dynamic visibility and annotation scales.
   Use transformed sample-point comparisons for conics and sheared nesting.
6. Test detached source identity, child handle allocation, fresh dictionaries,
   ByLayer/ByBlock appearance, layer/name/handle consistency, paper-space
   ownership, repeated insertion, and source immutability.
7. Reject invalid indices/counts and non-finite geometry without fabricated
   points or panics; bound output size and work; verify empty/missing blocks.
8. Keep the R12 export tests passing through any dispatch/API separation.

Full source coverage is not full native conformance. Any rows marked unverified
remain explicit limits of this audit, not successful parity results.
