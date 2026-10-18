# Baut den Windows-Installer (nsis, seit T30 ohne msi) lokal, ohne Netzwerkzugriff und ohne Codesigning.
# Aufruf im Wurzelverzeichnis: powershell -File scripts/build-installer.ps1 [-Version 1.0.2] (Standard: letzter Release-Tag)
#
# Mit gesetztem TAURI_SIGNING_PRIVATE_KEY (Pfad oder Inhalt) entstehen zusätzlich die
# Update-Artefakte (.sig). Ohne ihn wird der Installer trotzdem gebaut, ein späteres Update
# auf Knopfdruck braucht aber einen Release über den Workflow (T17).
param([string]$Version = '')
$ErrorActionPreference = 'Stop'

# Ohne Angabe die Version des letzten Release-Tags nehmen, damit lokale Builds nicht hinter
# dem veröffentlichten Stand zurückbleiben (die Version im Repository bleibt 0.0.0).
if (-not $Version) {
    $tag = git describe --tags --abbrev=0 --match 'v[0-9]*' 2>$null
    $Version = if ($tag) { $tag.TrimStart('v') } else { '0.0.0' }
}

$overlay = Get-Content 'app/src-tauri/tauri.release.conf.json' -Raw | ConvertFrom-Json
$overlay.PSObject.Properties.Remove('$schema')
$overlay | Add-Member -NotePropertyName version -NotePropertyValue $Version
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    Write-Warning 'Kein TAURI_SIGNING_PRIVATE_KEY: Installer ohne Update-Artefakte.'
    $overlay.bundle.PSObject.Properties.Remove('createUpdaterArtifacts')
}
$tmp = Join-Path $env:TEMP 'lastwerk-release.conf.json'
$overlay | ConvertTo-Json -Depth 10 | Set-Content -Path $tmp -Encoding utf8

pnpm --dir app/ui install --frozen-lockfile
pnpm --dir app/ui tauri build --config $tmp --bundles 'nsis'
if ($LASTEXITCODE -ne 0) { throw 'tauri build fehlgeschlagen' }

Get-ChildItem 'app/src-tauri/target/release/bundle' -Recurse -Include *.exe, *.sig |
    Select-Object -ExpandProperty FullName
