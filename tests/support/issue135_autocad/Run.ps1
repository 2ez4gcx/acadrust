param(
    [ValidateSet('Generate', 'Audit')][string]$Mode = 'Generate',
    [string[]]$Drawings = @(),
    [string]$AutoCAD = 'C:\Program Files\Autodesk\AutoCAD 2027',
    [string]$Template = "$env:LOCALAPPDATA\Autodesk\ACA 2027\enu\Template\acadiso.dwt",
    [string]$Output = '',
    [int]$ExpectedGroups = 13,
    [int]$ExpectedGeometries = 60,
    [int]$ExpectedConstraints = 47,
    [string]$ReferenceInventory = '',
    [switch]$CheckHorizontalConstraint,
    [switch]$ContinueOnFailure,
    [int]$TimeoutSeconds = 90
)
$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
if (!$Output) { $Output = Join-Path $repository 'tests\autocad\issue135' }
$Output = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Path $Output -Force | Out-Null
$console = Join-Path $AutoCAD 'accoreconsole.exe'
if (!(Test-Path -LiteralPath $console)) { throw "AutoCAD Core Console missing: $console" }
if (Get-Process accoreconsole -ErrorAction SilentlyContinue) { throw 'Another AutoCAD Core Console is running.' }
if ($ReferenceInventory -and !(Test-Path -LiteralPath $ReferenceInventory)) {
    throw "Reference inventory missing: $ReferenceInventory"
}
$previousOutput = $env:ISSUE135_OUTPUT
$previousTag = $env:ISSUE135_TAG
$previousFunctional = $env:ISSUE135_FUNCTIONAL

# AutoCAD 2027 loads .NET 10; use its installed runtime as the reference set.
$runtime = Get-ChildItem 'C:\Program Files\dotnet\shared\Microsoft.NETCore.App' -Directory |
    Where-Object { $_.Name -match '^10\.' } | Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
$sdk = (& dotnet --list-sdks | Select-Object -Last 1) -replace '^(\S+) \[([^\]]+)\].*$', '$2\$1'
$compiler = Join-Path $sdk 'Roslyn\bincore\csc.dll'
if (!$runtime -or !(Test-Path -LiteralPath $compiler)) { throw 'A .NET SDK and the .NET 10 runtime are required.' }
$dll = Join-Path $Output 'Issue135Native.dll'
$references = Get-ChildItem $runtime.FullName -Filter '*.dll' |
    Where-Object { $_.Name -match '^(System\..*|mscorlib|netstandard|Microsoft.CSharp)\.dll$' -and $_.Name -notmatch 'Native' }
$response = @('/nostdlib+', '/target:library', "/out:`"$dll`"")
$response += $references | ForEach-Object { "/r:`"$($_.FullName)`"" }
$response += 'AcCoreMgd', 'AcDbMgd', 'AcMgd' | ForEach-Object { "/r:`"$(Join-Path $AutoCAD "$_.dll")`"" }
$response += "`"$(Join-Path $PSScriptRoot 'Issue135Native.cs')`""
$responseFile = Join-Path $Output 'compile.rsp'
$response | Set-Content -LiteralPath $responseFile
& dotnet $compiler "@$responseFile"
if ($LASTEXITCODE) { throw "Compiler exited $LASTEXITCODE" }

function Invoke-Oracle([string]$InputDrawing, [string]$Tag, [string]$Command) {
    $script = Join-Path $Output "$Tag.scr"
    @('(setq issue135_secureload (getvar "SECURELOAD"))', '(setq issue135_filedia (getvar "FILEDIA"))',
      '(setvar "SECURELOAD" 0)', '(setvar "FILEDIA" 0)', '_.NETLOAD', ($dll -replace '\\', '/'), $Command,
      '_.AUDIT', '_N', '(setvar "SECURELOAD" issue135_secureload)', '(setvar "FILEDIA" issue135_filedia)',
      '_.QUIT', '_Y') | Set-Content -LiteralPath $script
    $env:ISSUE135_OUTPUT = $Output
    $env:ISSUE135_TAG = $Tag
    $env:ISSUE135_FUNCTIONAL = if ($CheckHorizontalConstraint) { '1' } else { '0' }
    $log = Join-Path $Output "$Tag.log"
    $process = Start-Process -FilePath $console -ArgumentList "/i `"$InputDrawing`" /s `"$script`" /l en-US" `
        -WorkingDirectory $Output -WindowStyle Hidden -PassThru -RedirectStandardOutput $log `
        -RedirectStandardError (Join-Path $Output "$Tag.err")
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
        $process.Kill()
        $process.WaitForExit()
        throw "AutoCAD timed out: $Tag"
    }
    $transcript = Get-Content -LiteralPath $log -Encoding Unicode -Raw
    $summary = [regex]::Matches($transcript, 'ISSUE135_SUMMARY groups=\d+ geometries=\d+ constraints=\d+') | Select-Object -Last 1
    $audit = [regex]::Matches($transcript, 'Total errors found \d+ fixed \d+') | Select-Object -Last 1
    Write-Output "$Tag exit=$($process.ExitCode) $summary $audit"
    if ($process.ExitCode -ne 0 -or !$summary -or !$audit -or $audit.Value -ne 'Total errors found 0 fixed 0') {
        throw "AutoCAD validation failed: $Tag (see $log)"
    }
    if ($Command -eq 'ISSUE135VALIDATE' -and !$transcript.Contains('ISSUE135_EVALUATE_UNCHANGED')) {
        throw "Constraint evaluation or save failed: $Tag (see $log)"
    }
    if ($Command -eq 'ISSUE135VALIDATE' -and $CheckHorizontalConstraint -and !$transcript.Contains('ISSUE135_FUNCTIONAL_PASS')) {
        throw "Horizontal constraint did not respond to an edit: $Tag (see $log)"
    }
    $counts = [regex]::Match($summary.Value, 'groups=(\d+) geometries=(\d+) constraints=(\d+)')
    if ([int]$counts.Groups[1].Value -ne $ExpectedGroups -or [int]$counts.Groups[2].Value -ne $ExpectedGeometries -or [int]$counts.Groups[3].Value -ne $ExpectedConstraints) {
        throw "Unexpected constraint inventory: $Tag expected=$ExpectedGroups/$ExpectedGeometries/$ExpectedConstraints actual=$summary"
    }
    if ($ReferenceInventory) {
        $expected = (Get-Content -LiteralPath $ReferenceInventory) -join "`n"
        $actual = (Get-Content -LiteralPath (Join-Path $Output "$Tag.inventory.txt")) -join "`n"
        if ($expected -cne $actual) { throw "Constraint inventory differs from reference: $Tag" }
    }
}

try {
    if ($Mode -eq 'Generate') {
        if (!(Test-Path -LiteralPath $Template)) { throw "Template missing: $Template" }
        Invoke-Oracle $Template 'generate' 'ISSUE135GENERATE'
        $Drawings = '2010', '2013', '2018' | ForEach-Object { Join-Path $Output "native_$_.dwg" }
    }
    if (!$Drawings) { throw 'Audit requires at least one drawing path.' }
    $validationFailures = @()
    foreach ($drawing in $Drawings) {
        $drawing = (Resolve-Path -LiteralPath $drawing).Path
        try {
            Invoke-Oracle $drawing ([IO.Path]::GetFileName($drawing) -replace '\.', '_') 'ISSUE135VALIDATE'
        } catch {
            if (!$ContinueOnFailure) { throw }
            $validationFailures += $_.Exception.Message
            Write-Warning $_.Exception.Message
        }
    }
    if ($validationFailures) { throw ($validationFailures -join [Environment]::NewLine) }
} finally {
    $env:ISSUE135_OUTPUT = $previousOutput
    $env:ISSUE135_TAG = $previousTag
    $env:ISSUE135_FUNCTIONAL = $previousFunctional
}
