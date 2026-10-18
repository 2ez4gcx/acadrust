# CAD-Created AC1009 Corpus

These drawings were constructed inside BricsCAD V20.1.07 and saved using its
native `acR12_dwg` export (`SaveAs` type -3). Companion DXFs were independently
exported using `acR12_dxf` (type 1). The native DWG binaries are intentionally
not checked in; the scripts regenerate them under `target/` when CAD validation
is needed.

```powershell
pwsh -NoProfile -File tests/bcad/create_r12_corpus.ps1
cargo run --example r12_cad_roundtrip
pwsh -NoProfile -File tests/bcad/run_r12_native.ps1 -SkipGenerate -OutputDirectory target/ac1009-cad-created
```

- `native-geometry`: named compound block, attribute, closed 2D and 3D
  polylines, a 2-by-3 polygon mesh and a quad polyface mesh.
- `native-viewports`: two configured paper-space viewports plus the template's
  overall/default viewports, with distinct target/direction, twist, lens,
  snap and grid values. VX records reference their real native entity addresses.
- `native-header`: named UCS/view and nondefault drafting settings.
- `-edited` files change the block/attribute, the second viewport's twist, or
  header spacing, respectively.
- `native-geometry-incremental`: six native `Save` operations with
  `ISAVEPERCENT=100`. BricsCAD still wrote a compact AC1009 file with zero extra
  entity bytes; it did not produce a historical JUMP/change-subfile layout.

The empty template supplies ordinary installed fonts and page defaults; no
proprietary font or external drawing is embedded. The DXF companions remain
available for Rust tests. The native DWG tests are ignored by default; copy the
generated `native-*.dwg` files from `target/ac1009-cad-created` into this
folder (git ignores them there) and run
`cargo test --all-features --test r12_native_state --test r12_native_mesh -- --ignored`.

Seven source DWGs and their seven acadrust resaves passed AutoCAD 2027 AUDIT
without repairs and the geometry/state checks. BricsCAD's R12 DWG export clears
UNITMODE while the independently exported DXF retains 1; its active VPORT
stores VIEWRES=1200 although the old global circle-zoom slot is zero. These
exporter distinctions are recorded in the tests rather than conflated.
