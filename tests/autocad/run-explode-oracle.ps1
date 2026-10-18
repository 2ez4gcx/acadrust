param(
    [string]$AutoCADPath = 'C:\Program Files\Autodesk\AutoCAD 2027',
    [string]$DotNetRuntimePath = 'C:\Program Files\dotnet\shared\Microsoft.NETCore.App\10.0.9',
    [switch]$SkipManaged
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$outputDirectory = Join-Path $repository 'target/explode-oracle'
$console = Join-Path $AutoCADPath 'accoreconsole.exe'
if (-not (Test-Path -LiteralPath $console)) { throw "AutoCAD console not found: $console" }
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

function Invoke-Oracle([string]$ScriptName, [string]$OutputName) {
    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $console
    $scriptPath = Join-Path $PSScriptRoot $ScriptName
    $startInfo.Arguments = "/s `"$scriptPath`" /l en-US"
    $startInfo.WorkingDirectory = $outputDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = [System.Text.Encoding]::Unicode
    $startInfo.EnvironmentVariables['ACADRUST_ORACLE_ROOT'] = $repository.Replace('\', '/')
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    try {
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(60000)) {
            $process.Kill()
            $process.WaitForExit()
            throw "AutoCAD oracle timed out: $ScriptName"
        }
        $log = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult()
        [IO.File]::WriteAllText((Join-Path $outputDirectory "$OutputName.console.log"), $log)
        if ($process.ExitCode -ne 0) { throw "AutoCAD exited with code $($process.ExitCode): $ScriptName" }
    }
    finally { $process.Dispose() }
}

$nativeLog = Join-Path $outputDirectory 'runtime.txt'
[IO.File]::WriteAllText($nativeLog, '')
Invoke-Oracle 'explode_oracle.scr' 'native'
if (-not ((Get-Content -LiteralPath $nativeLog) -contains 'DONE')) {
    throw 'The native oracle did not finish. Inspect target/explode-oracle/native.console.log.'
}

if (-not $SkipManaged) {
    $project = Join-Path $PSScriptRoot 'ExplodeOracle/ExplodeOracle.csproj'
    dotnet build $project --nologo --verbosity quiet /clp:ErrorsOnly "/p:AutoCADPath=$AutoCADPath" "/p:DotNetRuntimePath=$DotNetRuntimePath"
    if ($LASTEXITCODE -ne 0) { throw 'Managed oracle build failed.' }
    [IO.File]::WriteAllText((Join-Path $outputDirectory 'managed-runtime.txt'), '')
    Invoke-Oracle 'explode_extra_oracle.scr' 'managed'
    if (-not ((Get-Content -LiteralPath (Join-Path $outputDirectory 'managed-runtime.txt')) -contains 'DONE')) {
        throw 'The managed oracle did not finish. Inspect target/explode-oracle/managed.console.log.'
    }
}

$cases = [ordered]@{}
foreach ($line in (Get-Content -LiteralPath $nativeLog)) {
    if ($line -match '\|CREATE_FAILED(?:\||$)') { throw "Native oracle case failed: $line" }
    $parts = $line -split '\|', 3
    if ($parts.Count -ne 3) { continue }
    if ($parts[1] -eq 'CREATE_ERROR' -or $parts[1] -eq 'EXPLODE_ERROR') { throw "Native oracle case failed: $line" }
    if ($parts[1] -ne 'SOURCE_SURVIVES' -and $parts[1] -ne 'RESULT_TYPE') { continue }
    if (-not $cases.Contains($parts[0])) {
        $cases[$parts[0]] = [ordered]@{ sourceSurvives = $null; resultTypes = @() }
    }
    if ($parts[1] -eq 'SOURCE_SURVIVES') { $cases[$parts[0]].sourceSurvives = $parts[2] -eq 'YES' }
    if ($parts[1] -eq 'RESULT_TYPE') { $cases[$parts[0]].resultTypes += $parts[2] }
}
if (-not $SkipManaged) {
    foreach ($line in (Get-Content -LiteralPath (Join-Path $outputDirectory 'managed-runtime.txt'))) {
        $parts = $line -split '\|', 3
        if ($parts.Count -ne 3) { continue }
        if ($parts[1] -eq 'ERROR') { throw "Managed oracle case failed: $line" }
        if ($parts[1] -ne 'SOURCE_SURVIVES' -and $parts[1] -ne 'RESULT_TYPES' -and $parts[1] -ne 'RESULT_BLOCK') { continue }
        if (-not $cases.Contains($parts[0])) {
            $cases[$parts[0]] = [ordered]@{ sourceSurvives = $null; resultTypes = @() }
        }
        if ($parts[1] -eq 'SOURCE_SURVIVES') { $cases[$parts[0]].sourceSurvives = $parts[2] -eq 'YES' }
        if ($parts[1] -eq 'RESULT_TYPES' -and $parts[2]) { $cases[$parts[0]].resultTypes = @($parts[2] -split ',') }
        if ($parts[1] -eq 'RESULT_BLOCK') { $cases[$parts[0]].resultBlock = $parts[2] }
    }
}
$summaryPath = Join-Path $outputDirectory 'observed.json'
foreach ($name in $cases.Keys) {
    if ($null -eq $cases[$name].sourceSurvives) { throw "Oracle case lacks an outcome: $name" }
}
[IO.File]::WriteAllText($summaryPath, ($cases | ConvertTo-Json -Depth 6))
Write-Output "Observed $($cases.Count) cases. Results: $summaryPath"
