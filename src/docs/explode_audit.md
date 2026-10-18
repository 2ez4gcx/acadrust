# Explode Compatibility Audit

Date: 2026-10-18. Baseline: `303cc8b93654691c6557e82decbf8cb7d9c0ddee`.

## Verdict And Scope

Core EXPLODE behavior is now aligned with the captured AutoCAD 2027 matrix.
The API separates AutoCAD-style EXPLODE from the historical `flatten` helper
used by legacy format conversion. Full byte-for-byte equivalence still depends
on style tables, object enablers, exact font metrics, and curved modeler
topology, so the remaining limits below are explicit rather than hidden behind
an empty result.

The former urgent correctness issue—nested INSERT attribute identity reuse—is
resolved: nested attributes are detached, handles and SEQEND ownership are
remapped, extension graphs are cloned, and the DWG round-trip regression stays
in the test suite.

## Implemented Parity Slices

- Standalone CIRCLE, ARC, ELLIPSE, SPLINE, SOLID and 3DFACE refusal rules;
  HELIX to one SPLINE; and legacy flattening retained for R12 output.
- LWPOLYLINE/POLYLINE families, MLINE style offsets, patterned HATCH clipping
  including islands, dashes and double patterns, MLINE square/XY-round end
  caps, and WIPEOUT boundaries.
- Dimensions with extension lines, arrow solids and MTEXT; LEADER arrows and
  spline/polyline paths; TABLE grid/text graphics.
- Document-aware DIMSTYLE resolution for dimension EXPLODE: precision,
  measurement factors, prefixes/suffixes, text style/height, extension offsets,
  suppression flags, arrow size/width and blank-text handling.
- MLEADER straight path break intervals are applied as visible segment gaps;
  per-line arrow and landing output remains document-aware.
- Mesh per-face 3DFACEs, closed polygon-mesh seams, polyface POINT/LINE/face
  degeneracies, and per-face color retention.
- Document-aware INSERT expansion with MINSERT/xref refusal, base-point
  transforms, correct circle/arc similarity checks, nonuniform bulge handling,
  fresh nested handles and cloned extension dictionaries.
- ACIS planar face extraction to REGION entities for 3DSOLID/BODY/REGION and
  planar SURFACE inputs; REGION SAT loops retain straight, analytic ellipse,
  and NURBS edge curves, including coedge orientation and bounded spans.
- MTEXT formatted runs preserve explicit color, height, font and width data.
- MTEXT exploded TEXT positions honor tilted normals/DWG X directions and
  static or dynamic column flow with gutter spacing.

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

### P1: HATCH Pattern Fidelity Is Nearly Complete

`explode_hatch` now emits clipped visible pattern lines and points, including
pattern angle/scale, recorded origin, dash arrays, double patterns, island
styles, analytic line/arc/ellipse/spline boundaries, and OCS/WCS placement.
Solid and gradient hatches remain atomic as in AutoCAD. Remaining work is
native differential coverage for rational spline weights, fit tangents, and
unusual full-circle and self-intersecting boundary combinations.

### P1: Block Geometry Uses Exact Similarity And Fallbacks

The document-aware INSERT path now classifies source-plane similarity with the
full transform, preserves ARC thickness, converts nonuniform conics and bulged
segments to the appropriate ellipse geometry, and bakes nested shear or
nonuniform modeler members into anonymous `*E` blocks. Uniform circles remain
circles, and nested array spacing follows the composed transform. Remaining
coverage is limited to dynamic visibility and annotation-scale combinations.

### P1: Block Restrictions And Fallbacks Match The Native Contract

The document-aware API refuses MINSERT, xrefs, overlays, external definitions,
and non-explodable blocks. Unsupported BODY/3DSOLID/REGION members under a
nonuniform transform are retained in anonymous `*E` definitions, while a
solid-only definition leaves its original reference intact. The detached
`Insert::explode` helper still expands ordinary arrays for legacy callers, with
the documented one-million-cell work bound.

### P1: Exploded Clone Identity Is Remapped

Document-aware INSERT explosion allocates fresh outer and nested handles,
remaps ATTRIB/SEQEND ownership, clones extension-dictionary graphs, clears raw
record provenance, and preserves block ownership. The detached low-level helper
returns identity-free entities by contract; callers must allocate document
handles before insertion.
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

- `explode_mline` emits document-resolved element offsets, scale, colors,
  fills, square caps, and XY-plane round caps. Miter/joint trimming, segment
  gaps, tilted-plane round caps, inner-arc caps, and linetype-specific output
  still need native comparison; AutoCAD outputs individual visible LINEs and
  ARCs for those cases.
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
- `explode_multileader` omits some custom arrow/block semantics and ignores
  several path/style/color/weight overrides; straight break intervals are now
  honored, while spline break paths remain untrimmed. Arrow geometry is still
  built in XY rather than the content plane, and MTEXT style/attachment
  resolution needs broader native coverage.
- REGION, BODY, and 3DSOLID are now decomposed for planar modeler faces; the
  runtime box produces six REGIONs and standalone REGION SAT loops retain
  analytic boundary entities. Curved 3DSOLID/BODY faces still use polygonal
  wires when a face-specific curve model cannot be retained in the detached
  REGION/SURFACE representation.
- TABLE emits native-style grid LINEs and cell MTEXT, including title-row
  segmentation; merged-cell and row/column style variants still need coverage.
- Planar SURFACE and WIPEOUT now decompose to REGION/LINE output respectively;
  nonplanar surface kinds and nonrectangular wipeout boundaries remain limited.

### P2: MTEXT Layout Is Only An Approximation

`explode_mtext` preserves mixed color/height/font/width/oblique runs and emits
one TEXT per formatted run. It honors tilted normal axes and column gutters,
but still uses approximate font metrics and does not preserve
underline/overline/strikeout, stacked fractions, tabs, paragraph spacing,
exact/at-least spacing semantics, drawing direction, or background/frame
graphics. Complex dynamic column heights remain approximate.

Runtime example: `A{\C1;RED}B{\H2x;BIG}` becomes four native TEXT runs, with
color index 1 on RED and double height on BIG; the formatted-run path now
preserves those properties.

One TEXT per paragraph/wrapped line is insufficient for formatted runs.
Tests must compare text values, per-run properties, positions and bounds using
known fonts, not only line counts or stripped text.

### P2: Mesh Topology And Appearance Are Lost

`explode_mesh` now emits one 3DFACE per native mesh face and rejects invalid
indices instead of fabricating origin vertices. Subdivision level, creases,
and concave n-gon topology still need broader native coverage.

`explode_polyface_mesh` preserves per-face common properties and emits
POINT/LINE/3DFACE for one-, two-, and three-plus-vertex faces. Invalid or zero
indices are rejected; negative edge-visibility signs remain preserved for
ordinary faces.

`explode_polygon_mesh` honors CLOSED_M/CLOSED_N seams and checked signed vertex
counts. Fitting flags and smooth densities still require native differential
coverage.

### P2: Visual Property Inheritance Is Explicitly Split From Identity Cloning

`inherit_common` preserves visual handles, material, plot-style, shadow, visual
style, entity-mode fields, names, and ByBlock identities. Document-aware INSERT
resolution updates layer/linetype handles together with their names and keeps
raw bytes, graphics caches, reactors, dictionaries, and owners under the
separate clone policy. DXF/DWG readback remains the final validation for stale
writer precedence.

### P2: Failure Reporting And Resource Bounds Are Missing

An empty `Vec` conflates atomic objects, unsupported objects, forbidden
explosion, missing definitions, invalid data, and empty geometry. A caller
cannot reliably implement AutoCAD's refusal behavior, or decide when it is
valid to replace/delete the source. The API returns parts without changing the
document; this must stay explicit.

Direct MINSERT expansion now refuses arrays above one million cells before
materializing transforms or points. Smaller arrays still materialize all
rows/columns, so a streaming or cancellation-aware API remains preferable for
callers that need very large but valid arrays.

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
| Circle | Empty | Runtime: atomic, agrees; nonuniform INSERT members can become ELLIPSE |
| Arc | Empty | Runtime: atomic standalone; special conversion inside nonuniform blocks |
| Ellipse | Empty | Runtime: atomic, agrees |
| Polyline | LINE segments | Documented for 3D polylines; fitting flags need differential coverage |
| Polyline2D | LINE/ARC segments | Runtime ordinary 2D case agrees; elevation and fit/tangent variants need coverage |
| Polyline3D | LINE segments | Runtime ordinary 3D case agrees; fitting variants need coverage |
| LwPolyline | LINE/ARC segments | Runtime basic rectangle agrees; bulge/plane/closed/thickness cases need matrix |
| Text | Empty | Runtime: atomic, agrees |
| MText | Formatted TEXT runs | Runtime output family agrees; columns/metrics/tilted layout remain approximate |
| Spline | Empty | Runtime: atomic, agrees |
| Helix | One SPLINE | Runtime: one SPLINE, agrees |
| Dimension | Styled LINE/SOLID/MTEXT subset | Runtime/documented compound output; custom graphics/tolerances remain incomplete |
| Hatch | Clipped pattern LINE/POINTs; solid/gradient empty | Runtime patterned geometry agrees for captured matrix |
| Solid | Empty | Runtime: atomic, agrees; TRACE shares this variant |
| Face3D | Empty | Runtime: atomic, agrees |
| Insert | Context-aware block members; detached arrays retained | Runtime: refusal and fallback rules match document path |
| Block | Empty | Structural marker; not a selectable standalone object |
| BlockEnd | Empty | Structural marker |
| Ray | Empty | Runtime: atomic, agrees |
| XLine | Empty | Runtime: atomic, agrees |
| Viewport | Empty | Unverified; clipping/display state is outside the current implementation |
| AttributeDefinition | Empty | Atomic expected; restored after containing block explosion |
| AttributeEntity | Empty | Owned INSERT child; instance values correctly excluded from block explosion |
| Leader | SOLID plus LWPOLYLINE/SPLINE | Runtime straight/spline output families agree; hooks/styles remain incomplete |
| MultiLeader | LINE/LWPOLYLINE/MTEXT plus block content | Runtime simple case agrees; custom content, overrides and plane remain incomplete |
| MLine | Styled element LINEs, fills, square caps | Runtime standard/custom offsets agree; round/inner caps and joints remain incomplete |
| Mesh | One 3DFACE per face | Runtime quad contract agrees; fitting/crease variants remain incomplete |
| RasterImage | Empty | Atomic expected; native verification pending |
| Solid3D | One REGION/SURFACE per modeler face | Runtime box: six REGIONs, agrees for planar faces |
| Region | Boundary LINE/ELLIPSE/SPLINE entities | Runtime/documented curves; complex analytic faces need more coverage |
| Body | One REGION/SURFACE per modeler face | Documented compound output; curved face retention remains approximate |
| Surface | Planar SURFACE to REGION | Runtime planar surface agrees; other kinds need coverage |
| Table | Grid LINEs and cell MTEXT | Runtime 2x2 table count/type agrees; style/merge variants remain |
| Tolerance | Empty | Runtime: atomic for tested feature-control frame, agrees |
| PolyfaceMesh | POINT/LINE/3DFACE | Runtime low-vertex and ordinary face families agree; extended color variants remain |
| Wipeout | Boundary LINEs | Runtime rectangle becomes four LINEs, agrees |
| Shape | Empty | Native verification required with a known SHX fixture |
| Underlay | Empty | PDF/DWF/DGN-specific native verification required; import is a different command |
| Seqend | Empty | Structural marker; low-level INSERT path should filter standalone markers |
| Ole2Frame | Empty | Atomic expected; native verification pending |
| PolygonMesh | 3DFACEs with closed seams | Runtime closed seam counts agree; fitted surface handling remains incomplete |
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

The API keeps the detached `explode`/legacy `flatten` separation while the
document wrapper supplies tables and block graphs where AutoCAD requires them.
Narrow geometry defects were corrected with regressions:

- Straight LINEs extracted from LWPOLYLINE, 2D POLYLINE and SOLID now use WCS
  coordinates instead of copying OCS coordinates into a WCS entity.
- Linear/aligned dimension extension endpoints and their dimension line now
  project onto the offset dimension line rather than joining both measured
  points to one definition point.
- Clockwise elliptical hatch boundary conversion interprets mirrored stored
  parameters before creating ELLIPSE parameters.

These fixes do not establish fidelity for their entire entity families.
Focused regressions also cover DIMSTYLE formatting, analytic REGION curves,
MLINE caps, MLEADER breaks, and bounded MINSERT work.

## Validation

- `cargo test --all-targets` passed on the original baseline.
- Final `cargo test --all-targets --all-features --quiet`: latest run passed
  across all targets with no failures; ignored tests remain fixture-dependent.
- Focused `cargo test --all-features --lib explode --quiet`: **104 passed**,
  including DIMSTYLE, modeler, MLINE, polyface, formatted MTEXT, MLEADER
  break, and document-graph regressions.
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
