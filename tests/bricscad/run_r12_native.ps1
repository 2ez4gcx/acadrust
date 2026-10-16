param(
    [string]$CoreConsole = 'C:\Program Files\Autodesk\AutoCAD 2027\accoreconsole.exe',
    [string]$OutputDirectory = 'target\ac1009-native',
    [string]$Filter = '*',
    [int]$TimeoutSeconds = 60,
    [switch]$SkipGenerate
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if (-not (Test-Path -LiteralPath $CoreConsole)) { throw "Missing AutoCAD Core Console: $CoreConsole" }
if (-not $SkipGenerate) {
    & cargo run --example r12_native_fixtures -- $OutputDirectory
    if ($LASTEXITCODE -ne 0) { throw 'Failed to generate R12 fixtures.' }
}
$output = (Resolve-Path (Join-Path $repository $OutputDirectory)).Path
$lisp = (Join-Path $PSScriptRoot 'r12_native.lsp').Replace('\', '/')
$results = @()
foreach ($name in (Get-Content -LiteralPath (Join-Path $output 'inputs.txt'))) {
    if ($name -notlike $Filter) { continue }
    $inputPath = Join-Path $output $name
    $runDirectory = Join-Path $output ('autocad-' + $name)
    New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
    $runInput = Join-Path $runDirectory $name
    Copy-Item -LiteralPath $inputPath -Destination $runInput -Force
    $reportPath = Join-Path $output ($name + '.report.txt')
    $stdoutPath = Join-Path $output ($name + '.console.log')
    $stderrPath = Join-Path $output ($name + '.stderr.log')
    $scriptPath = Join-Path $output ($name + '.scr')
    if (Test-Path -LiteralPath $reportPath) { Remove-Item -LiteralPath $reportPath }
    $script = '(setq ar12:secureload (getvar "SECURELOAD"))' + "`r`n" + '(setvar "SECURELOAD" 0)' + "`r`n" +
        '(load "' + $lisp + '")' + "`r`n" +
        '(setvar "SECURELOAD" ar12:secureload)' + "`r`n" +
        '(ar12:run "' + $reportPath.Replace('\', '/') + '" "' + $name + '")' + "`r`n"
    Set-Content -LiteralPath $scriptPath -Value $script -Encoding ascii
    Write-Output "AutoCAD R12 validation: $name"
    $process = Start-Process -FilePath $CoreConsole -ArgumentList ('/i "' + $runInput + '" /s "' + $scriptPath + '" /l en-US') -WorkingDirectory $runDirectory -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
    $createdProcessId = $process.Id
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        if ($process.WaitForExit(1000)) { break }
        $process.Refresh()
    }
    $timedOut = -not $process.HasExited
    if ($timedOut) { Stop-Process -Id $createdProcessId -Force }
    $report = if (Test-Path -LiteralPath $reportPath) { Get-Content -LiteralPath $reportPath -Raw } else { '' }
    $console = (Get-Content -LiteralPath $stdoutPath -Raw) -replace "`0", ''
    $auditPassed = $console -match 'Total errors found\s+0\s+fixed\s+0'
    $importClean = $console -notmatch 'Objects recovered|Bad entity header|Unterminated Block|INTERNAL ERROR|Error Aborting'
    $exitCode = if ($timedOut) { $null } else { $process.ExitCode }
    $status = if (-not $timedOut -and $exitCode -eq 0 -and $importClean -and $report -match 'COMPLETE\s+T' -and $report -notmatch '"FAIL"|ERROR\s' -and $auditPassed) { 'PASS' } else { 'FAIL' }
    $results += [pscustomobject]@{ Input=$name; Status=$status; AuditPassed=$auditPassed; ImportClean=$importClean; ExitCode=$exitCode; TimedOut=$timedOut; Report=$reportPath; Console=$stdoutPath }
    Write-Output "$status $name"
}
$results | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $output 'autocad-results.json') -Encoding utf8
$results | Format-Table Input,Status,AuditPassed,TimedOut -AutoSize
if ($results.Count -eq 0 -or $results.Status -contains 'FAIL') { exit 1 }
