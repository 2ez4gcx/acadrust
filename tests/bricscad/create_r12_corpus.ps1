param(
    [string]$BricsCad = 'D:\Bricsys\BricsCAD\bricscad.exe',
    [string]$OutputDirectory = 'target\ac1009-cad-created',
    [int]$TimeoutSeconds = 60,
    [string]$Filter = '*'
)
$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$output = [IO.Path]::GetFullPath((Join-Path $repository $OutputDirectory))
New-Item -ItemType Directory -Path $output -Force | Out-Null
$lisp = (Join-Path $PSScriptRoot 'create_r12_corpus.lsp').Replace('\', '/')
$template = Join-Path (Split-Path $BricsCad) 'UserDataCache\Templates\en_US\Default-mm.dwt'
$inputs = @()
foreach ($kind in @('native-geometry', 'native-viewports', 'native-header')) {
    if ($kind -notlike $Filter) { continue }
    $scriptPath = Join-Path $output ($kind + '.scr')
    $reportPath = Join-Path $output ($kind + '.generation.txt')
    if (Test-Path -LiteralPath $reportPath) { Remove-Item -LiteralPath $reportPath }
    $script = '(load "' + $lisp + '")' + "`r`n" + '(r12c:create "' + $output.Replace('\','/') + '" "' + $kind + '")' + "`r`n"
    Set-Content -LiteralPath $scriptPath -Value $script -Encoding ascii
    Write-Output "BricsCAD draws and saves: $kind"
    $process = Start-Process -FilePath $BricsCad -ArgumentList ('/nologo /t "' + $template + '" /b "' + $scriptPath + '"') -WorkingDirectory (Split-Path $BricsCad) -WindowStyle Hidden -PassThru
    $createdProcessId = $process.Id
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
        if ($process.WaitForExit(1000)) { break }
        $process.Refresh()
    }
    $timedOut = -not $process.HasExited
    if ($timedOut) { Stop-Process -Id $createdProcessId -Force }
    $report = if (Test-Path -LiteralPath $reportPath) { Get-Content -LiteralPath $reportPath -Raw } else { '' }
    if ($report -notmatch 'COMPLETE\s+T' -or $report -match 'ERROR\s' -or $timedOut) { throw "Native generation failed for $kind. See $reportPath" }
    foreach ($suffix in @('', '-edited')) {
        $name = $kind + $suffix + '.dwg'
        $path = Join-Path $output $name
        $magic = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($path), 0, 6)
        if ($magic -ne 'AC1009') { throw "$name was saved as $magic instead of AC1009" }
        $inputs += $name
        Write-Output "Created $name ($magic)"
    }
}
Set-Content -LiteralPath (Join-Path $output 'inputs.txt') -Value $inputs -Encoding ascii
