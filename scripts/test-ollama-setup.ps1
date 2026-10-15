# Prüft app/src-tauri/resources/ollama-setup.ps1 gegen den Vertrag aus Task T29: nur
# Trockenläufe und falsche Parameter, also kein Netz, keine Installation, kein Modell.
#
# Aufruf im Wurzelverzeichnis (CI-Job "Ollama-Einrichtung"):
#   pwsh -File scripts/test-ollama-setup.ps1 -Engine powershell   # Windows PowerShell 5.1
#   pwsh -File scripts/test-ollama-setup.ps1 -Engine pwsh         # PowerShell 7
param([string]$Engine = 'powershell')

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$setup = (Resolve-Path (Join-Path $PSScriptRoot '../app/src-tauri/resources/ollama-setup.ps1')).Path
$work = Join-Path ([System.IO.Path]::GetTempPath()) ('lastwerk-setup-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$failures = New-Object System.Collections.Generic.List[string]
$stateKeys = 'schema_version,decision,ollama_found,installed_via,model,model_ready,error_code,updated_at'

function Invoke-SetupScript([string[]]$Arguments) {
    $out = & $Engine -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $setup @Arguments
    $code = $LASTEXITCODE
    $lines = @($out | ForEach-Object { [string]$_ })
    foreach ($line in $lines) { Write-Host "    | $line" }
    return [pscustomobject]@{
        Code  = $code
        Steps = @($lines | Where-Object { $_.StartsWith('LASTWERK:') })
    }
}

function Assert-True([string]$Name, [bool]$Condition, [string]$Detail) {
    if ($Condition) {
        Write-Host "ok    $Name"
    } else {
        Write-Host "FAIL  $Name ($Detail)"
        $failures.Add("$Name ($Detail)")
    }
}

function Assert-Run([string]$Name, [string[]]$Arguments, [int]$Code, [string[]]$Steps) {
    Write-Host "--- $Name"
    $result = Invoke-SetupScript $Arguments
    Assert-True "$Name exit code" ($result.Code -eq $Code) "expected $Code, got $($result.Code)"
    $expected = $Steps -join ' '
    $actual = $result.Steps -join ' '
    Assert-True "$Name steps" ($expected -ceq $actual) "expected '$expected', got '$actual'"
}

function Assert-BadArgs([string]$Name, [string[]]$Arguments) {
    $state = Join-Path $work ('bad-' + [guid]::NewGuid().ToString('N') + '.json')
    Assert-Run $Name ($Arguments + @('-StatePath', $state)) 2 @('LASTWERK:error:bad_args')
    Assert-True "$Name writes no state" (-not (Test-Path -LiteralPath $state)) $state
}

function Assert-State([string]$Name, [string]$Path, [hashtable]$Expected) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Assert-True "$Name state file exists" $false $Path
        return
    }
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $bom = ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)
    Assert-True "$Name state without BOM" (-not $bom) 'UTF-8 BOM found'
    $text = [System.Text.Encoding]::UTF8.GetString($bytes)
    # Der Hook liest nur die erste Zeile und sucht darin "decision":"yes" oder "no".
    Assert-True "$Name state on one line" (-not $text.TrimEnd("`n").Contains("`n")) $text
    Assert-True "$Name updated_at" ($text -cmatch '"updated_at":"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z"') $text
    $state = $text | ConvertFrom-Json
    $keys = $state.PSObject.Properties.Name -join ','
    Assert-True "$Name state keys" ($keys -ceq $stateKeys) $keys
    foreach ($key in $Expected.Keys) {
        $prop = $state.PSObject.Properties[$key]
        $actual = $null
        if ($null -ne $prop) { $actual = $prop.Value }
        $want = $Expected[$key]
        if ($null -eq $want) {
            $same = ($null -eq $actual)
        } elseif ($want -is [int]) {
            # PowerShell 7 liest ganze Zahlen aus JSON als Int64, 5.1 als Int32.
            $same = (($actual -is [int] -or $actual -is [long]) -and [long]$actual -eq $want)
        } else {
            $same = ($null -ne $actual -and $actual.GetType() -eq $want.GetType() -and $actual -ceq $want)
        }
        Assert-True "$Name state $key" $same "expected '$want', got '$actual'"
    }
    # Keine Pfade in der Zustandsdatei (Vertrag).
    Assert-True "$Name state without paths" (-not ($text -match '[A-Za-z]:\\\\|/tmp/|\\\\Users')) $text
}

try {
    Write-Host "engine: $Engine, script: $setup"

    # Ohne Parameter: falscher Aufruf.
    Assert-Run 'no arguments' @() 2 @('LASTWERK:error:bad_args')

    # Einrichtung aus der App mit Startmodell.
    $appState = Join-Path $work 'app.json'
    Assert-Run 'app with model' @('-Mode', 'app', '-Model', 'qwen3:4b', '-DryRun', '-StatePath', $appState) 0 @(
        'LASTWERK:detect:missing', 'LASTWERK:install:winget', 'LASTWERK:start:ok',
        'LASTWERK:pull:start', 'LASTWERK:pull:done', 'LASTWERK:done:ok')
    Assert-State 'app with model' $appState @{
        schema_version = 1; decision = $null; ollama_found = $false; installed_via = $null
        model = 'qwen3:4b'; model_ready = $false; error_code = $null
    }

    # Antwort "Nein" im Installer: nur merken.
    $noState = Join-Path $work 'no.json'
    Assert-Run 'installer decision no' @('-Mode', 'installer', '-Decision', 'no', '-DryRun', '-StatePath', $noState) 0 @(
        'LASTWERK:detect:missing', 'LASTWERK:install:skipped', 'LASTWERK:pull:skipped', 'LASTWERK:done:ok')
    Assert-State 'installer decision no' $noState @{
        schema_version = 1; decision = 'no'; ollama_found = $false; installed_via = $null
        model = $null; model_ready = $false; error_code = $null
    }
    Assert-True 'hook finds decision no' ((Get-Content -LiteralPath $noState -Raw).Contains('"decision":"no"')) 'search text missing'

    # Ein späterer Lauf ohne -Decision behält die gemerkte Entscheidung.
    Assert-Run 'app keeps decision' @('-Mode', 'app', '-DryRun', '-StatePath', $noState) 0 @(
        'LASTWERK:detect:missing', 'LASTWERK:install:winget', 'LASTWERK:start:ok',
        'LASTWERK:pull:skipped', 'LASTWERK:done:ok')
    Assert-State 'app keeps decision' $noState @{ decision = 'no'; model = $null }

    # Antwort "Ja" im Installer.
    $yesState = Join-Path $work 'yes.json'
    Assert-Run 'installer decision yes' @('-Mode', 'installer', '-Decision', 'yes', '-Model', 'qwen3:4b', '-DryRun', '-StatePath', $yesState) 0 @(
        'LASTWERK:detect:missing', 'LASTWERK:install:winget', 'LASTWERK:start:ok',
        'LASTWERK:pull:start', 'LASTWERK:pull:done', 'LASTWERK:done:ok')
    Assert-State 'installer decision yes' $yesState @{ decision = 'yes'; model = 'qwen3:4b' }
    Assert-True 'hook finds decision yes' ((Get-Content -LiteralPath $yesState -Raw).Contains('"decision":"yes"')) 'search text missing'

    # Trockenlauf ohne -StatePath schreibt nie in die Standarddatei.
    $fakeLocal = Join-Path $work 'localappdata'
    New-Item -ItemType Directory -Path $fakeLocal | Out-Null
    $savedLocal = $env:LOCALAPPDATA
    try {
        $env:LOCALAPPDATA = $fakeLocal
        Assert-Run 'dry run without state path' @('-Mode', 'app', '-DryRun') 0 @(
            'LASTWERK:detect:missing', 'LASTWERK:install:winget', 'LASTWERK:start:ok',
            'LASTWERK:pull:skipped', 'LASTWERK:done:ok')
    } finally {
        $env:LOCALAPPDATA = $savedLocal
    }
    Assert-True 'dry run leaves default state alone' (-not (Test-Path -LiteralPath (Join-Path $fakeLocal 'Lastwerk'))) $fakeLocal

    # Bösartige oder unzulässige Modellnamen und Parameter.
    Assert-BadArgs 'model with semicolon' @('-Mode', 'app', '-DryRun', '-Model', 'qwen3:4b;calc')
    Assert-BadArgs 'model with ampersand' @('-Mode', 'app', '-DryRun', '-Model', 'qwen3:4b&calc')
    Assert-BadArgs 'model with pipe' @('-Mode', 'app', '-DryRun', '-Model', 'qwen3|calc')
    Assert-BadArgs 'model with subexpression' @('-Mode', 'app', '-DryRun', '-Model', '$(calc)')
    Assert-BadArgs 'model with space' @('-Mode', 'app', '-DryRun', '-Model', 'qwen3 4b')
    Assert-BadArgs 'model with slash' @('-Mode', 'app', '-DryRun', '-Model', '../qwen3')
    Assert-BadArgs 'model with upper case' @('-Mode', 'app', '-DryRun', '-Model', 'Qwen3:4b')
    Assert-BadArgs 'model with two tags' @('-Mode', 'app', '-DryRun', '-Model', 'qwen3:4b:x')
    Assert-BadArgs 'model too long' @('-Mode', 'app', '-DryRun', '-Model', ('q' * 101))
    Assert-BadArgs 'unknown mode' @('-Mode', 'server', '-DryRun')
    Assert-BadArgs 'mode in upper case' @('-Mode', 'App', '-DryRun')
    Assert-BadArgs 'unknown decision' @('-Mode', 'installer', '-Decision', 'maybe', '-DryRun')
    Assert-BadArgs 'decision no with model' @('-Mode', 'installer', '-Decision', 'no', '-Model', 'qwen3:4b', '-DryRun')
    Assert-BadArgs 'unknown parameter' @('-Mode', 'app', '-DryRun', '-Bogus', 'x')
    Assert-Run 'relative state path' @('-Mode', 'app', '-DryRun', '-StatePath', 'relative.json') 2 @('LASTWERK:error:bad_args')

    if ($env:OS -ne 'Windows_NT') {
        # Außerhalb von Windows bricht der echte Lauf sofort ab, ohne etwas zu versuchen.
        $linuxState = Join-Path $work 'linux.json'
        Assert-Run 'real run outside windows' @('-Mode', 'app', '-StatePath', $linuxState) 1 @('LASTWERK:error:no_windows')
        Assert-State 'real run outside windows' $linuxState @{ error_code = 'no_windows'; ollama_found = $false }
    }
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failures.Count -gt 0) {
    Write-Host ''
    Write-Host "$($failures.Count) check(s) failed:"
    foreach ($f in $failures) { Write-Host "  $f" }
    exit 1
}
Write-Host ''
Write-Host 'all checks passed'
exit 0
