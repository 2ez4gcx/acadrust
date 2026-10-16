# Native Validation

AC1009 fixtures have a separate AutoCAD Core Console harness:

```powershell
cargo run --example r12_native_fixtures
pwsh -NoProfile -File tests/bricscad/run_r12_native.ps1 -SkipGenerate
```

Override `-CoreConsole` for another AutoCAD installation. Valid cases are
written to `target/ac1009-native`; malformed parser cases are not exported.
The harness validates copies, runs AUDIT without repairs, compares geometry,
and rejects import recovery. Results include per-file logs and
`autocad-results.json`. See [the R12 status](../../src/docs/ac1009_support.md)
for tested behavior and remaining limits.

To draw the extended corpus inside BricsCAD and save real R12 DWG/DXF sources:

```powershell
pwsh -NoProfile -File tests/bricscad/create_r12_corpus.ps1
cargo run --example r12_cad_roundtrip
pwsh -NoProfile -File tests/bricscad/run_r12_native.ps1 -SkipGenerate -OutputDirectory target/ac1009-cad-created
```

The [native corpus instructions](../fixtures/r12/cad-created/README.md)
describe how to regenerate the DWGs when CAD validation is needed. Native DWG
binaries are intentionally omitted from the repository; the checked-in DXF
companions and synthetic stream fixtures keep ordinary Rust tests independent
of an installed CAD application.

Run `pwsh -File tests/bricscad/run_issue_regressions.ps1` from the repository root
on a Windows machine with BricsCAD. The example generates synthetic DWG, ASCII
DXF, binary DXF, and DWG-to-DXF conversion inputs under `target/`.

The harness starts a separate hidden BricsCAD process for each input, opens the
file through a command script, runs AUDIT without repairs, checks the relevant
fields, and writes a report plus `native-results.json`. It stops only the process
ID it created on timeout. Existing BricsCAD sessions are never addressed.

Use `-Filter modern2010.dwg -SkipGenerate` to run one existing input or
`-WriteNativeFixtures` to save BricsCAD's interpretation alongside the source.
Native fixture export does not repair failed checks. Native BricsCAD validation
can reveal interoperability problems beyond the library's internal round trips;
a FAIL report remains a failure even when AUDIT finds no errors.

ELEVATION is checked through the active viewport as well as the Rust header
regression for #127. Wire-cache color for #129 is not exposed by AutoLISP;
the harness checks solid count and volume while the Rust tests verify wire
colors and binary record alignment.

BricsCAD V20 retains TABLESTYLE title/header suppression when importing DXF but
drops both values even when saving its own suppression-enabled style to a 2010
DWG and reopening it. The native DWG suppression checks remain strict and fail
on this behavior. The Rust tests separately check the values in the public
model and DWG/DXF conversion. `create_style_fixture.lsp` is a minimal native
reproduction, accepting an output basename through `ar:create-style-fixture`.
