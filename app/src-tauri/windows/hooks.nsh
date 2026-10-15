; Lastwerk: Einrichtung von Ollama im NSIS-Installer (Task T29 Teil A, SPEC 9.2, 10.1, 10.2).
;
; Eingebunden über bundle.windows.nsis.installerHooks in tauri.release.conf.json. Tauri fügt das
; Makro NSIS_HOOK_POSTINSTALL am Ende des Abschnitts "Install" ein, nachdem alle Dateien kopiert
; sind (tauri-bundler 2.10.2, installer.nsi). Die Ressource liegt dann unter
; $INSTDIR\resources\ollama-setup.ps1.
;
; Ablauf: Gemerkte Entscheidung aus der Zustandsdatei lesen. Im stillen oder passiven Modus und
; beim Update nie fragen. Sonst einmal fragen und die Antwort über das Skript merken. Ein Fehler
; des Skripts bricht die Installation von Lastwerk nie ab, er steht nur im Protokoll.

; Startmodell (Faustregel, bis O13 gemessen ist). Muss gleich dem Eintrag mit "starter": true in
; catalog/models.json sein, ein Test prüft das nach dem Zusammenführen von Teil A und Teil B.
!define LASTWERK_OLLAMA_STARTER_MODEL "qwen3:4b"

!macro NSIS_HOOK_POSTINSTALL
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4

  ; $R0 Zustandsdatei (Standard des Skripts, hier ausdrücklich übergeben)
  ; $R1 Entscheidung: "", "yes" oder "no"
  ; $R2 Hilfswert (gelesene Zeile, Text der Frage, Pfad von PowerShell)
  ; $R3 Parameter für das Skript, leer heißt: Skript nicht aufrufen
  ; $R4 Hilfswert (Dateigriff, Fundstelle, Rückgabe von nsExec)
  StrCpy $R0 "$LOCALAPPDATA\Lastwerk\ollama-setup.json"
  StrCpy $R1 ""
  StrCpy $R3 ""

  ; Das Skript schreibt das JSON kompakt in eine Zeile, deshalb genügt eine Textsuche.
  ClearErrors
  FileOpen $R4 "$R0" r
  ${IfNot} ${Errors}
    FileRead $R4 $R2
    FileClose $R4
    ${StrLoc} $R4 "$R2" '"decision":"yes"' ">"
    ${If} $R4 != ""
      StrCpy $R1 "yes"
    ${Else}
      ${StrLoc} $R4 "$R2" '"decision":"no"' ">"
      ${If} $R4 != ""
        StrCpy $R1 "no"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  ClearErrors

  ${If} ${Silent}
  ${OrIf} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
    ; Still, passiv oder Update (das Update von Tauri startet den Installer passiv): nie fragen.
    ; Nur nach einem gemerkten Ja wird Ollama geprüft und das Startmodell geladen, sonst nichts.
    ${If} $R1 == "yes"
      StrCpy $R3 '-Model "${LASTWERK_OLLAMA_STARTER_MODEL}"'
    ${EndIf}
  ${ElseIf} $R1 == ""
    ; 1031 ist Deutsch (Deutschland). NSIS wählt bei mehreren Installersprachen die Sprache des
    ; Systems, Varianten wie de-AT landen ebenfalls bei 1031.
    ${If} $LANGUAGE = 1031
      StrCpy $R2 "Lokalen Coach (Ollama) einrichten? Lädt etwa 2,5 GB."
    ${Else}
      StrCpy $R2 "Set up the local coach (Ollama)? Downloads about 2.5 GB."
    ${EndIf}
    ${If} ${Cmd} `MessageBox MB_YESNO|MB_ICONQUESTION "$R2" /SD IDNO IDYES`
      StrCpy $R3 '-Decision yes -Model "${LASTWERK_OLLAMA_STARTER_MODEL}"'
    ${Else}
      ; Auch das Nein wird gemerkt, damit der nächste Installer nicht erneut fragt.
      StrCpy $R3 "-Decision no"
    ${EndIf}
  ${ElseIf} $R1 == "yes"
    ; Erneute Installation nach einem Ja: ohne Frage prüfen, im Normalfall nach Sekunden fertig.
    StrCpy $R3 '-Model "${LASTWERK_OLLAMA_STARTER_MODEL}"'
  ${EndIf}

  ${If} $R3 != ""
    ; Der Installer ist ein 32-Bit-Programm. Über Sysnative startet die 64-Bit-PowerShell, damit
    ; Registry, Programmordner und winget dieselbe Sicht wie die App haben.
    StrCpy $R2 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
    ${If} ${FileExists} "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
      StrCpy $R2 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
    ${EndIf}
    ${If} $LANGUAGE = 1031
      DetailPrint "Lastwerk: Ollama wird eingerichtet ..."
    ${Else}
      DetailPrint "Lastwerk: setting up Ollama ..."
    ${EndIf}
    ; -ExecutionPolicy Bypass gilt nur für diesen einen Prozess.
    nsExec::ExecToLog '"$R2" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\resources\ollama-setup.ps1" -Mode installer -StatePath "$R0" $R3'
    Pop $R4
    ${If} $R4 != "0"
      ${If} $LANGUAGE = 1031
        DetailPrint "Lastwerk: Einrichtung von Ollama nicht abgeschlossen ($R4). In der App unter Coach erneut möglich."
      ${Else}
        DetailPrint "Lastwerk: Ollama setup did not finish ($R4). You can retry in the app under Coach."
      ${EndIf}
    ${EndIf}
  ${EndIf}
  ClearErrors

  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
!macroend
