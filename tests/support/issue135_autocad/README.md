# Issue 135 AutoCAD Oracle

Requires Windows, licensed AutoCAD 2027, a .NET SDK, and the .NET 10 runtime.
The harness compiles `Issue135Native.cs` against AutoCAD's public managed API,
then launches hidden AutoCAD Core Console processes one at a time. No plugin
installation or original user drawings are needed.

Run from the repository root:

```powershell
./tests/support/issue135_autocad/Run.ps1 -Mode Generate
./tests/support/issue135_autocad/Run.ps1 -Mode Audit -Drawings path/to/output.dwg,path/to/output.dxf
```

Use `-AutoCAD` and `-Template` to override local installation locations. Generated
files, scripts, inventories, audit transcripts, and compiler response files go to
the ignored `tests/autocad/issue135` directory, or the `-Output` directory.

Generation creates 13 independent groups covering horizontal, vertical, parallel,
perpendicular, collinear, equal-length, concentric, equal-radius, tangent, fixed
arc, fixed ellipse, fixed elliptical arc, and fixed spline constraints. AutoCAD
adds implicit endpoint/center relationships and datum geometry. The resulting
inventory is 60 constrained geometries and 47 constraints, saved as AC1024,
AC1027, and AC1032 DWG plus ASCII DXF.

Validation inventories every typed group and node, evaluates the associative
network, requires an unchanged inventory, saves separate AutoCAD DWG and DXF
copies, and runs `AUDIT` with repair disabled. An unexpected process exit,
missing inventory, changed evaluated graph, failed save, or nonzero audit count
fails the harness. Counts must match `13/60/47`; use `-ExpectedGroups`,
`-ExpectedGeometries`, and `-ExpectedConstraints` for another known fixture.
Use `-ReferenceInventory path/to/native.inventory.txt` to require every group,
node ID, geometry class, constraint class, activity flag, implied flag, degree
of freedom, and solution status to match a known native graph.
`-ContinueOnFailure` tests every supplied drawing before reporting failures.
It does not modify an input drawing.

For outputs from the generated corpus, add `-CheckHorizontalConstraint`. After
saving the comparison copies, the harness moves the first horizontal line's
endpoint from `(20, 0, 0)` to `(20, 5, 0)`, evaluates the network, and requires
the horizontal constraint to correct the edit. The input and saved comparison
files retain their original geometry.

Dimensional constraint creation is outside this generated geometric corpus.
`RESULTS.md` records the tested files and known coverage limits.

The portable Rust regressions use typed graph constructors and directly encoded
native streams in `tests/support/issue135_graphs.rs` and `issue135_stream.rs`.
They require no DWG/DXF files or CAD installation. Generated drawings remain
ignored local oracle material and are excluded from the commit.
