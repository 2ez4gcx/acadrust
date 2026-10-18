# AutoCAD EXPLODE Oracle

This directory records **47 observed scenarios**, not a full conformance claim.
The library-wide audit is [Explode Compatibility Audit](../../src/docs/explode_audit.md).

## Provenance

- Captured on 2026-10-18 using installed AutoCAD 2027, English commands.
- Console build/product version: `26.0.118.0.0` (`X.118.0.0`, `ACADVER=26.0`).
- Console SHA-256: `44D10AF0C32FB54C52B3A34751A8E9B7C860448D02089A0F09AF68649F72B3E2`.
- The host selected its existing AutoCAD Architecture (Global) profile and
  `AEC Model (Global Ctb).dwt`. The native script restores `Standard` DIMSTYLE
  and explicitly sets the tested dimension variables and MLINE scale.
- Forty scenarios use AutoLISP and seven use the managed helper. Both invoke
  the actual `EXPLODE` command. The managed helper uses `Editor.Command`, not
  `Entity.Explode` or COM `Explode`.
- Scratch xrefs are created under `target/explode-oracle`. The console discards
  its unsaved drawing on exit; scratch files remain under `target`. No existing
  drawing is opened or saved.

## Run

Run from the repository root on Windows with licensed AutoCAD available:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tests/autocad/run-explode-oracle.ps1
```

`-AutoCADPath` selects another installed AutoCAD directory.
`-DotNetRuntimePath` selects the installed .NET runtime used for the managed
references. The captured host uses .NET 10.0.9 with an available .NET 8 SDK;
the helper project references those installed host runtime assemblies directly.
No SDK or runtime is installed by the runner. `-SkipManaged` runs only the
40 AutoLISP scenarios.

The runner launches hidden console processes with a 60-second deadline, requires
completion markers, rejects creation/runtime failures, and writes fresh logs
and `observed.json` under `target/explode-oracle`. It does not automatically
overwrite the checked-in observations. `SECURELOAD` is temporarily disabled
inside the isolated console to load the local helpers and restored before
exit. The native script also restores the profile variables it changes.

## Retained Observations

- [observed-2027.json](observed-2027.json): source survival and output DXF types
  for all 47 scenarios. These are observations, not library test expectations.
- [observed-2027-native.txt](observed-2027-native.txt): source/result `entget`
  records, output types, counts, and completion marker for the 40 native cases.
- [observed-2027-managed.txt](observed-2027-managed.txt): seven managed-created
  cases, including the anonymous block name for the nonuniform ACIS fallback.

`sourceSurvives=true` means AutoCAD refused the requested explosion and produced
no replacement objects. It does not authorize deleting the source. Entity
addresses, handles, modeler UUIDs, and anonymous block numbers in the raw
records identify this run and are not stable identifiers for comparison.

Important confirmed cases include:

| Input | AutoCAD result |
|---|---|
| CIRCLE, ELLIPSE, SPLINE, SOLID, 3DFACE | Source survives; no replacements |
| HELIX | One SPLINE |
| Square ANSI31 HATCH | Five clipped diagonal LINEs |
| Square solid HATCH | Source survives; no replacements |
| Standard two-vertex MLINE | Two offset LINEs |
| Straight/spline LEADER | SOLID + LWPOLYLINE / SOLID + SPLINE |
| 3x3 polygon mesh open / closed M / closed N / closed MN | 4 / 6 / 6 / 9 3DFACEs |
| One-/two-/three-/four-vertex polyface faces | POINT / LINE / 3DFACE / 3DFACE |
| Quad MESH | One 3DFACE |
| Box 3DSOLID / planar surface | Six REGIONs / one REGION |
| 2x2 TABLE | Three MTEXTs and seven LINEs |
| Rectangular WIPEOUT | Four LINEs |
| MINSERT / XREF | Source survives; no replacements |
| Uniform block CIRCLE / nonuniform block CIRCLE | CIRCLE / ELLIPSE |
| Nonuniform solid-only block | Source survives; no replacements |
| Nonuniform solid+line block | Anonymous `*E` INSERT and transformed LINE |
| Block with an attribute instance | ATTDEF with default value, plus LINE |
| `A{\C1;RED}B{\H2x;BIG}` MTEXT | Four TEXT runs: A, red RED, B, double-height BIG |

The formatted MTEXT result retains color index 1 on `RED` and height 5.0 on
`BIG`, while the other runs have height 2.5. The attribute result contains
`Default value`, not `Instance value`. These properties are in the raw records.

## Scope Limits

The oracle verifies the listed fixtures' refusal/output types and retains
native geometry records; it does not compare every output coordinate/property
with the Rust library. It does not cover every entity variant, dimension
subtype, text font/layout mode, hatch pattern/island/gradient, leader style,
MLINE style/cap/gap, fitted/smoothed mesh, ACIS curve/surface family, annotation
scale, dynamic block, proxy/object enabler, or file roundtrip. The full audit
marks those remaining obligations explicitly.

## Autodesk References

- [EXPLODE command](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-Core/files/GUID-E98BCEF4-DED6-48A6-87EB-10FE87188083.htm): output families, one grouping level, attributes, xrefs, MINSERT, and nonuniform ACIS fallback.
- [About Exploding Compound Objects](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-Core/files/GUID-91D100A0-36F0-463D-82AC-78592690D2DC.htm): dimension/hatch graphics and source replacement semantics.
- [ActiveX Explode](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-ActiveX-Reference/files/GUID-6D96780F-3EC6-447C-B341-0FE815BE9979.htm): API availability differs from a successful operation; inherited ExternalReference support has no effect.
- [Managed Entity.Explode](https://help.autodesk.com/cloudhelp/2026/ENU/OARX-ManagedRefGuide/files/OARX-ManagedRefGuide-Autodesk_AutoCAD_DatabaseServices_Entity_Explode_DBObjectCollection.html): returns detached objects and leaves database insertion and source erasure to its caller.
