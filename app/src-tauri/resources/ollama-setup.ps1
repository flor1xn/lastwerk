# Lastwerk: Ollama erkennen, bei Bedarf installieren, starten und optional ein Modell laden.
# Task T29 Teil A, SPEC 9.2, 10.1, 10.2. Der Vertrag mit dem Hook (windows/hooks.nsh) und der App
# steht in docs/tasks/T29-ollama-einrichtung.md und ist bindend.
#
# Aufruf:
#   powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File ollama-setup.ps1 `
#     -Mode installer|app [-Model <name>] [-Decision yes|no] [-StatePath <datei>] [-DryRun]
#
# Ausgabe: je Schritt eine Zeile "LASTWERK:<phase>:<wert>" auf der Standardausgabe. Alle anderen
# Zeilen sind Hinweise für das Protokoll des Installers und werden von der App ignoriert.
# Exit-Code 0 Erfolg, 1 Fehler (letzte Zeile LASTWERK:error:<code>), 2 falsche Parameter.
#
# Läuft unter Windows PowerShell 5.1 (so ruft der NSIS-Hook es auf) und unter PowerShell 7.
# Deshalb keine Sprachmittel ab PowerShell 6 (kein ??, kein ternärer Operator, kein &&).
# Text aus dem Netz wird nie ausgeführt: kein Invoke-Expression, kein Skript aus einer Antwort.
# Ausgeführt wird nur der signaturgeprüfte Installer von Ollama oder winget.

# Kein [CmdletBinding()]: Ein einfaches Skript legt unbekannte Parameter in $args ab, statt mit
# einem Bindungsfehler (Exit-Code 1) abzubrechen. So bekommt jeder falsche Aufruf Exit-Code 2.
# Keine Pflichtparameter und kein ValidateSet aus demselben Grund, geprüft wird unten von Hand.
param(
    [string]$Mode = '',
    [string]$Model = '',
    [string]$Decision = '',
    [string]$StatePath = '',
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Der Fortschrittsbalken von Invoke-WebRequest bremst Windows PowerShell 5.1 stark aus.
$ProgressPreference = 'SilentlyContinue'

# Standardadresse von Ollama (SPEC 9.2), nur Loopback.
$ApiBase = 'http://127.0.0.1:11434'
# Wartezeit auf den Dienst nach dem Start (Faustregel, Vorgabe aus T29).
$StartTimeoutSec = 60
# Höchstdauer des stillen Installers von Ollama (Faustregel).
$InstallTimeoutSec = 900
# Höchste Pause zwischen zwei Fortschrittszeilen beim Laden eines Modells (Faustregel).
$PullIdleTimeoutMs = 600000
$WingetId = 'Ollama.Ollama'
$DownloadUrl = 'https://ollama.com/download/OllamaSetup.exe'
# Modellname nach Vertrag. \z statt $, damit ein angehängter Zeilenumbruch nicht durchrutscht.
# -cmatch, weil -match in PowerShell Groß- und Kleinschreibung nicht unterscheidet.
$ModelPattern = '^[a-z0-9][a-z0-9._-]*(:[a-z0-9._-]+)?\z'
# Längengrenze für den Modellnamen (Faustregel, Ollama-Namen sind deutlich kürzer).
$ModelMaxLength = 100
$UninstallRoots = @(
    'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
    'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'
)

# Zustandsdatei nach Vertrag. Keine Pfade, kein Text aus fremden Antworten.
$State = [ordered]@{
    schema_version = 1
    decision       = $null
    ollama_found   = $false
    installed_via  = $null
    model          = $null
    model_ready    = $false
    error_code     = $null
    updated_at     = ''
}
$script:Phase = 'args'
$script:StateFile = ''
$script:WriteState = $false

function Write-Step([string]$Phase, [string]$Value) {
    # ${...}, weil "$Phase:" sonst als Variable mit Laufwerksangabe gelesen würde.
    [Console]::Out.WriteLine("LASTWERK:${Phase}:${Value}")
    [Console]::Out.Flush()
}

function Write-Info([string]$Text) {
    # Nur ASCII, weil nsExec die Ausgabe in der Codepage der Konsole liest.
    [Console]::Out.WriteLine("ollama-setup: $Text")
    [Console]::Out.Flush()
}

function Save-SetupState {
    $State.updated_at = [DateTime]::UtcNow.ToString("yyyy-MM-dd'T'HH:mm:ss'Z'", [System.Globalization.CultureInfo]::InvariantCulture)
    # Kompakt in einer Zeile: Der Hook sucht darin nach "decision":"yes" (Vertrag).
    $json = ConvertTo-Json -InputObject $State -Compress -Depth 3
    $path = $script:StateFile
    $dir = [System.IO.Path]::GetDirectoryName($path)
    if (-not (Test-Path -LiteralPath $dir -PathType Container)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    # Atomar: erst eine Nachbardatei im selben Ordner schreiben, dann ersetzen. UTF-8 ohne BOM,
    # weil serde_json in der App eine BOM nicht annimmt.
    $tmp = Join-Path $dir ('.ollama-setup.' + [guid]::NewGuid().ToString('N') + '.tmp')
    $utf8 = New-Object System.Text.UTF8Encoding -ArgumentList $false
    [System.IO.File]::WriteAllText($tmp, $json + "`n", $utf8)
    try {
        if ([System.IO.File]::Exists($path)) {
            # [NullString]: PowerShell machte aus $null sonst einen leeren Text, und File.Replace
            # lehnt einen leeren Namen für die Sicherungskopie ab.
            [System.IO.File]::Replace($tmp, $path, [NullString]::Value)
        } else {
            [System.IO.File]::Move($tmp, $path)
        }
    } finally {
        if (Test-Path -LiteralPath $tmp) {
            Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue
        }
    }
}

function Save-StateQuietly {
    if (-not $script:WriteState) { return }
    try {
        Save-SetupState
    } catch {
        # Kein Abbruch: Der Zustand ist eine Auskunft, die App fragt Ollama ohnehin selbst.
        Write-Info ('could not write state file: ' + $_.Exception.Message)
    }
}

function Exit-WithError([string]$Code) {
    $State.error_code = $Code
    Save-StateQuietly
    Write-Step 'error' $Code
    exit 1
}

function Exit-BadArgs([string]$Reason) {
    # Bei falschen Parametern wird nichts geschrieben, auch nicht die Zustandsdatei.
    Write-Info "bad arguments: $Reason"
    Write-Info 'usage: ollama-setup.ps1 -Mode installer|app [-Model <name>] [-Decision yes|no] [-StatePath <file>] [-DryRun]'
    Write-Step 'error' 'bad_args'
    exit 2
}

function Read-SetupState {
    # Übernimmt die gemerkte Entscheidung und Angaben früherer Läufe. Eine kaputte Datei zählt
    # als nicht vorhanden. Übernommen werden nur Werte aus der festen Wertemenge des Vertrags.
    $path = $script:StateFile
    if ($path -eq '' -or -not (Test-Path -LiteralPath $path -PathType Leaf)) { return }
    try {
        $old = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
    } catch {
        Write-Info 'state file unreadable, starting fresh'
        return
    }
    if ($null -eq $old) { return }
    $p = $old.PSObject.Properties['decision']
    if ($null -ne $p -and ($p.Value -ceq 'yes' -or $p.Value -ceq 'no')) { $State.decision = [string]$p.Value }
    $p = $old.PSObject.Properties['installed_via']
    if ($null -ne $p -and ($p.Value -ceq 'winget' -or $p.Value -ceq 'download')) { $State.installed_via = [string]$p.Value }
    $p = $old.PSObject.Properties['model']
    if ($null -ne $p -and $p.Value -is [string] -and $p.Value -cmatch $ModelPattern) {
        $State.model = $p.Value
        $ready = $old.PSObject.Properties['model_ready']
        if ($null -ne $ready -and $ready.Value -is [bool]) { $State.model_ready = $ready.Value }
    }
}

function Find-OllamaExe {
    # 1. Im Suchpfad.
    $cmd = Get-Command -Name 'ollama' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -ne $cmd) { return $cmd.Path }
    # 2. Standardpfad des Installers von Ollama (Installation je Benutzer).
    if ($env:LOCALAPPDATA) {
        $local = Join-Path $env:LOCALAPPDATA 'Programs\Ollama\ollama.exe'
        if (Test-Path -LiteralPath $local -PathType Leaf) { return $local }
    }
    # 3. Deinstallationseintrag in der Registry (eigener Installationsordner).
    foreach ($root in $UninstallRoots) {
        foreach ($key in @(Get-ChildItem -LiteralPath $root -ErrorAction SilentlyContinue)) {
            $props = Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue
            if ($null -eq $props) { continue }
            $name = $props.PSObject.Properties['DisplayName']
            if ($null -eq $name -or $null -eq $name.Value) { continue }
            if (-not ([string]$name.Value).StartsWith('Ollama', [System.StringComparison]::OrdinalIgnoreCase)) { continue }
            $location = $props.PSObject.Properties['InstallLocation']
            if ($null -eq $location -or -not $location.Value) { continue }
            $dir = ([string]$location.Value).Trim().Trim('"')
            if ($dir -eq '') { continue }
            $candidate = Join-Path $dir 'ollama.exe'
            if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
        }
    }
    return $null
}

function Get-LoopbackRequest([string]$Path, [int]$TimeoutMs) {
    # Nur Loopback, ohne Proxy aus der Umgebung und ohne Weiterleitung (wie crates/coach).
    $request = [System.Net.WebRequest]::Create($ApiBase + $Path)
    $request.Proxy = $null
    $request.Timeout = $TimeoutMs
    $request.AllowAutoRedirect = $false
    $request.KeepAlive = $false
    return $request
}

function Test-OllamaApi {
    try {
        $request = Get-LoopbackRequest '/api/version' 2000
        $response = $request.GetResponse()
        try {
            return ([int]$response.StatusCode -eq 200)
        } finally {
            $response.Close()
        }
    } catch {
        return $false
    }
}

function Get-OllamaJson([string]$Path) {
    $request = Get-LoopbackRequest $Path 10000
    $response = $request.GetResponse()
    try {
        $reader = New-Object System.IO.StreamReader -ArgumentList $response.GetResponseStream(), ([System.Text.Encoding]::UTF8)
        try {
            $text = $reader.ReadToEnd()
        } finally {
            $reader.Dispose()
        }
    } finally {
        $response.Close()
    }
    return ($text | ConvertFrom-Json)
}

function Get-ModelKey([string]$Name) {
    # Ollama ergänzt ein fehlendes Tag zu ":latest".
    $key = $Name.ToLowerInvariant()
    if ($key.IndexOf(':') -lt 0) { $key = $key + ':latest' }
    return $key
}

function Test-ModelPresent([string]$Name) {
    # Gleiche Auskunft wie "ollama list", aber über die Schnittstelle (siehe Vermerk in T29).
    $want = Get-ModelKey $Name
    $tags = Get-OllamaJson '/api/tags'
    if ($null -eq $tags) { return $false }
    $list = $tags.PSObject.Properties['models']
    if ($null -eq $list -or $null -eq $list.Value) { return $false }
    foreach ($entry in @($list.Value)) {
        if ($null -eq $entry) { continue }
        foreach ($field in @('name', 'model')) {
            $prop = $entry.PSObject.Properties[$field]
            if ($null -ne $prop -and $prop.Value -is [string] -and (Get-ModelKey $prop.Value) -ceq $want) { return $true }
        }
    }
    return $false
}

function Invoke-ModelPull([string]$Name) {
    # POST /api/pull mit stream=true liefert je Zeile ein JSON-Objekt mit digest, total und
    # completed. Daraus entsteht ein Gesamtfortschritt über alle Schichten. Text aus der Antwort
    # (Feld error oder status) wird weder ausgegeben noch gespeichert.
    $body = ConvertTo-Json -Compress -InputObject ([ordered]@{ model = $Name; stream = $true })
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($body)
    $request = Get-LoopbackRequest '/api/pull' 30000
    $request.Method = 'POST'
    $request.ContentType = 'application/json'
    $request.ContentLength = $bytes.Length
    $request.ReadWriteTimeout = $PullIdleTimeoutMs
    $stream = $request.GetRequestStream()
    try {
        $stream.Write($bytes, 0, $bytes.Length)
    } finally {
        $stream.Dispose()
    }
    $response = $request.GetResponse()
    $totals = @{}
    $completed = @{}
    $last = -1
    $success = $false
    try {
        $reader = New-Object System.IO.StreamReader -ArgumentList $response.GetResponseStream(), ([System.Text.Encoding]::UTF8)
        try {
            for (;;) {
                $line = $reader.ReadLine()
                if ($null -eq $line) { break }
                if ($line.Trim() -eq '') { continue }
                $message = $line | ConvertFrom-Json
                if ($null -eq $message) { continue }
                if ($null -ne $message.PSObject.Properties['error']) {
                    Write-Info 'ollama reported an error while pulling'
                    return $false
                }
                $status = $message.PSObject.Properties['status']
                if ($null -ne $status -and $status.Value -ceq 'success') { $success = $true }
                $digest = $message.PSObject.Properties['digest']
                $total = $message.PSObject.Properties['total']
                if ($null -eq $digest -or $null -eq $total -or $null -eq $total.Value) { continue }
                $key = [string]$digest.Value
                $totals[$key] = [double]$total.Value
                $done = $message.PSObject.Properties['completed']
                if ($null -ne $done -and $null -ne $done.Value) { $completed[$key] = [double]$done.Value }
                $sumTotal = 0.0
                $sumDone = 0.0
                foreach ($k in @($totals.Keys)) {
                    $sumTotal += $totals[$k]
                    if ($completed.ContainsKey($k)) { $sumDone += [Math]::Min($completed[$k], $totals[$k]) }
                }
                if ($sumTotal -le 0) { continue }
                $percent = [int][Math]::Floor(100.0 * $sumDone / $sumTotal)
                if ($percent -gt 100) { $percent = 100 }
                # Nur steigend ausgeben: Spätere kleine Schichten vergrößern die Summe und
                # würden den Wert sonst kurz zurückspringen lassen.
                if ($percent -gt $last) {
                    $last = $percent
                    Write-Step 'pull' ([string]$percent)
                }
            }
        } finally {
            $reader.Dispose()
        }
    } finally {
        $response.Close()
    }
    return $success
}

function Install-ViaWinget {
    $winget = Get-Command -Name 'winget' -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $winget) {
        Write-Info 'winget not available, using the download from ollama.com'
        return $false
    }
    Write-Step 'install' 'winget'
    # Nur die Quelle winget (nicht msstore), still und ohne Rückfragen. Die Ausgabe von winget
    # enthält Fortschrittszeichen und geht nicht ins Protokoll.
    & $winget.Path install --id $WingetId --exact --source winget --silent --accept-package-agreements --accept-source-agreements --disable-interactivity | Out-Null
    $code = $LASTEXITCODE
    if ($code -ne 0) {
        Write-Info ('winget failed with exit code ' + $code + ', using the download from ollama.com')
        return $false
    }
    return $true
}

function Test-OllamaPublisher([string]$Subject) {
    # Vor dem ersten Livetest nur: Organisation (O=) beginnt mit dem Wort Ollama. Der genaue
    # Herausgeber wird beim Livetest in T29 vermerkt und dann hier fest geprüft.
    return ($Subject -cmatch '(^|,\s*)O="?Ollama\b')
}

function Install-ViaDownload {
    # Gibt '' bei Erfolg zurück, sonst den Fehlercode des Vertrags.
    Write-Step 'install' 'download'
    if ($PSVersionTable.PSVersion.Major -lt 6) {
        # Windows PowerShell 5.1 bietet TLS 1.2 nicht auf jedem System von selbst an.
        [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor [System.Net.SecurityProtocolType]::Tls12
    }
    # Eigener, neuer Ordner unter %TEMP%, damit keine fremde Datei gleichen Namens greift.
    $dir = Join-Path ([System.IO.Path]::GetTempPath()) ('lastwerk-ollama-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $dir | Out-Null
    $file = Join-Path $dir 'OllamaSetup.exe'
    try {
        try {
            Invoke-WebRequest -Uri $DownloadUrl -OutFile $file -UseBasicParsing -MaximumRedirection 5
        } catch {
            Write-Info ('download failed: ' + $_.Exception.Message)
            return 'download_failed'
        }
        $signature = Get-AuthenticodeSignature -FilePath $file
        $subject = ''
        if ($null -ne $signature.SignerCertificate) { $subject = [string]$signature.SignerCertificate.Subject }
        if ([string]$signature.Status -cne 'Valid' -or -not (Test-OllamaPublisher $subject)) {
            Write-Info ('signature rejected: status ' + [string]$signature.Status)
            Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
            return 'signature_invalid'
        }
        # Für den Vermerk im Livetest (T29, manueller Schritt 5).
        Write-Info ('publisher: ' + $subject)
        Write-Step 'install' 'verify_ok'
        $process = Start-Process -FilePath $file -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -PassThru
        # Handle sofort abfragen: Sonst liefert Windows PowerShell 5.1 nach dem Ende keinen ExitCode.
        $null = $process.Handle
        # WaitForExit statt Start-Process -Wait: -Wait wartet in 5.1 auch auf Kindprozesse, und der
        # Installer von Ollama startet am Ende die dauerhaft laufende App von Ollama.
        if (-not $process.WaitForExit($InstallTimeoutSec * 1000)) {
            Write-Info 'installer did not finish in time'
            return 'install_failed'
        }
        if ($process.ExitCode -ne 0) {
            Write-Info ('installer exit code ' + $process.ExitCode)
            return 'install_failed'
        }
        return ''
    } finally {
        Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-OllamaStart([string]$Exe) {
    if (Test-OllamaApi) { return $true }
    if ([string]::IsNullOrEmpty($Exe)) { return $false }
    # Bevorzugt die App von Ollama (Symbol im Infobereich, startet den Dienst selbst), sonst
    # "ollama serve" ohne Fenster im Hintergrund.
    $app = Join-Path (Split-Path -Parent $Exe) 'ollama app.exe'
    if (Test-Path -LiteralPath $app -PathType Leaf) {
        Write-Info 'starting the Ollama app'
        Start-Process -FilePath $app
    } else {
        Write-Info 'starting ollama serve in the background'
        Start-Process -FilePath $Exe -ArgumentList 'serve' -WindowStyle Hidden
    }
    $deadline = (Get-Date).AddSeconds($StartTimeoutSec)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 1
        if (Test-OllamaApi) { return $true }
    }
    return $false
}

function Invoke-DryRun {
    # Trockenlauf: kein Netz, kein Zugriff auf System, Registry oder Ollama. Gibt die Schritte
    # aus, die ein echter Lauf im ungünstigsten Fall (Ollama fehlt) nähme.
    Write-Info ('dry run, mode ' + $Mode + ': no network, no changes to the system')
    Write-Info ('would detect: Get-Command ollama, %LOCALAPPDATA%\Programs\Ollama\ollama.exe, uninstall keys, ' + $ApiBase + '/api/version')
    Write-Step 'detect' 'missing'
    if ($Decision -ceq 'no') {
        Write-Info 'decision no: would only record the decision'
        Write-Step 'install' 'skipped'
        Write-Step 'pull' 'skipped'
    } else {
        Write-Info ('would run: winget install --id ' + $WingetId + ' --exact --source winget --silent --accept-package-agreements --accept-source-agreements --disable-interactivity')
        Write-Step 'install' 'winget'
        Write-Info ('fallback: download ' + $DownloadUrl + ' into a new folder under %TEMP%, require a valid Authenticode signature with O=Ollama, run /VERYSILENT /SUPPRESSMSGBOXES /NORESTART')
        Write-Info ('would start Ollama and wait up to ' + $StartTimeoutSec + ' s for ' + $ApiBase + '/api/version')
        Write-Step 'start' 'ok'
        if ($Model -ne '') {
            Write-Info ('would pull model ' + $Model + ' via POST ' + $ApiBase + '/api/pull if /api/tags does not list it')
            Write-Step 'pull' 'start'
            Write-Step 'pull' 'done'
        } else {
            Write-Step 'pull' 'skipped'
        }
    }
    # Der Trockenlauf schreibt nur in eine ausdrücklich genannte Datei (-StatePath) und nur den
    # Stand ohne Wirkung: nichts gefunden, nichts installiert, kein Modell bereit.
    if ($Model -ne '') { $State.model = $Model; $State.model_ready = $false }
    $State.ollama_found = $false
    $State.installed_via = $null
    $State.error_code = $null
    if ($script:WriteState) {
        Save-StateQuietly
        Write-Info 'state file written (dry run)'
    } else {
        Write-Info 'state file not written (dry run without -StatePath)'
    }
    Write-Step 'done' 'ok'
    exit 0
}

function Invoke-Setup {
    $script:Phase = 'detect'
    $exe = Find-OllamaExe
    $running = Test-OllamaApi
    if ($running) {
        Write-Step 'detect' 'running'
    } elseif ($null -ne $exe) {
        Write-Step 'detect' 'found'
    } else {
        Write-Step 'detect' 'missing'
    }
    $found = ($running -or $null -ne $exe)
    $State.ollama_found = $found
    if ($found) {
        if ($null -eq $State.installed_via) { $State.installed_via = 'existing' }
    } else {
        # Angaben früherer Läufe gelten nicht mehr, wenn Ollama jetzt fehlt.
        $State.installed_via = $null
        $State.model_ready = $false
    }

    if ($Decision -ceq 'no') {
        # Antwort "Nein" im Installer: nur die Entscheidung und den Befund festhalten.
        Write-Step 'install' 'skipped'
        Write-Step 'pull' 'skipped'
        $State.error_code = $null
        Save-StateQuietly
        Write-Step 'done' 'ok'
        exit 0
    }

    $script:Phase = 'install'
    if ($found) {
        Write-Step 'install' 'skipped'
    } else {
        $via = ''
        if (Install-ViaWinget) {
            $exe = Find-OllamaExe
            if ($null -ne $exe -or (Test-OllamaApi)) {
                $via = 'winget'
            } else {
                Write-Info 'winget reported success but ollama.exe was not found, using the download from ollama.com'
            }
        }
        if ($via -eq '') {
            $result = Install-ViaDownload
            if ($result -ne '') { Exit-WithError $result }
            $exe = Find-OllamaExe
            if ($null -eq $exe -and -not (Test-OllamaApi)) {
                Write-Info 'ollama.exe not found after the installer finished'
                Exit-WithError 'install_failed'
            }
            $via = 'download'
        }
        $State.ollama_found = $true
        $State.installed_via = $via
    }

    $script:Phase = 'start'
    if (Invoke-OllamaStart $exe) {
        Write-Step 'start' 'ok'
    } else {
        Write-Step 'start' 'timeout'
        Exit-WithError 'start_timeout'
    }

    $script:Phase = 'pull'
    if ($Model -eq '') {
        Write-Step 'pull' 'skipped'
    } else {
        $State.model = $Model
        $State.model_ready = $false
        if (Test-ModelPresent $Model) {
            Write-Step 'pull' 'skipped'
        } else {
            Write-Step 'pull' 'start'
            $ok = $false
            try {
                $ok = Invoke-ModelPull $Model
            } catch {
                Write-Info ('pull failed: ' + $_.Exception.Message)
                $ok = $false
            }
            if (-not $ok -or -not (Test-ModelPresent $Model)) { Exit-WithError 'pull_failed' }
            Write-Step 'pull' 'done'
        }
        $State.model_ready = $true
    }

    $script:Phase = 'state'
    $State.error_code = $null
    Save-StateQuietly
    Write-Step 'done' 'ok'
    exit 0
}

# --- Parameter prüfen (vor jeder Wirkung) ---------------------------------------------------

if ($args.Count -gt 0) { Exit-BadArgs 'unknown or extra arguments' }
if ($Mode -cne 'installer' -and $Mode -cne 'app') { Exit-BadArgs '-Mode must be installer or app' }
if ($Decision -ne '' -and $Decision -cne 'yes' -and $Decision -cne 'no') { Exit-BadArgs '-Decision must be yes or no' }
if ($Model -ne '' -and ($Model.Length -gt $ModelMaxLength -or $Model -cnotmatch $ModelPattern)) { Exit-BadArgs '-Model does not match the allowed pattern' }
if ($Decision -ceq 'no' -and $Model -ne '') { Exit-BadArgs '-Decision no cannot be combined with -Model' }
if ($StatePath -ne '') {
    $rooted = $false
    try { $rooted = [System.IO.Path]::IsPathRooted($StatePath) } catch { $rooted = $false }
    if (-not $rooted) { Exit-BadArgs '-StatePath must be an absolute path' }
    $script:StateFile = $StatePath
} elseif ($env:LOCALAPPDATA) {
    $script:StateFile = Join-Path $env:LOCALAPPDATA 'Lastwerk\ollama-setup.json'
}
# Der Trockenlauf schreibt nie in die Standarddatei, nur in eine ausdrücklich genannte.
$script:WriteState = ($script:StateFile -ne '') -and ((-not $DryRun) -or $StatePath -ne '')

Read-SetupState
if ($Decision -ne '') { $State.decision = $Decision }
if ($Model -ne '') { $State.model = $Model; $State.model_ready = $false }

if ($DryRun) { Invoke-DryRun }

if ($env:OS -ne 'Windows_NT') {
    Write-Info 'Ollama setup is only available on Windows'
    Exit-WithError 'no_windows'
}

try {
    Invoke-Setup
} catch {
    # Unerwarteter Fehler: Code nach der Phase, in der er auftrat. Die Meldung geht nur ins
    # Protokoll, nie in die Zustandsdatei.
    Write-Info ('unexpected error: ' + $_.Exception.Message)
    $code = 'install_failed'
    if ($script:Phase -ceq 'start') { $code = 'start_timeout' }
    if ($script:Phase -ceq 'pull') { $code = 'pull_failed' }
    Exit-WithError $code
}
