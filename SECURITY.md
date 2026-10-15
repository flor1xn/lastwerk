# Sicherheit

## Eine Schwachstelle melden

Bitte nicht als öffentliches Issue. Der Meldeweg ist die **private Meldung von Schwachstellen auf GitHub**: im Repository auf den Reiter "Security", dann "Report a vulnerability". Die Meldung sehen nur die Maintainer, eine private Mailadresse braucht es nicht.

Hilfreich sind: betroffene Version (steht in den Einstellungen der App), Betriebssystem, Schritte zum Nachstellen und die erwartete Wirkung. Bitte keine echten Trainingsdaten, Garmin-Zugangsdaten oder Token anhängen. Ein synthetisches Beispiel genügt.

Lastwerk wird in der Freizeit gepflegt. Es gibt keine Reaktionszeit und kein Kopfgeld. Eine Meldung wird gelesen, eingeordnet und, wenn sie zutrifft, in einem neuen Release behoben und im Changelog (`catalog/changelog.json`) genannt.

## Unterstützte Versionen

Nur der neueste Release. Die App meldet neue Versionen über "Nach Update suchen" (Windows). Ältere Stände bekommen keine Korrekturen.

## Was als Schwachstelle zählt

Ziel von Lastwerk ist, dass Daten den Rechner nicht verlassen (SPEC 10). Besonders interessant sind deshalb:

- Garmin-Token oder Passwörter außerhalb des Schlüsselbunds des Betriebssystems (in Datenbank, Backup, Log, Fehlertext oder Oberfläche).
- Netzwerkzugriffe aus der Oberfläche oder an andere Ziele als Garmin Connect und das lokale Sprachmodell.
- Der MCP-Server liefert mehr als die dokumentierten Kennzahlen (zum Beispiel Koordinaten, Dateipfade, Notizen).
- Ein manipuliertes Update lässt sich installieren (Signaturprüfung des Updaters, Release-Workflow).
- Eine präparierte FIT-Datei, ein Plan, ein Backup oder eine Antwort des Sprachmodells bringt die App zum Absturz oder führt Code aus.

Nicht als Schwachstelle gilt, dass der Garmin-Zugang inoffiziell ist und sich ändern kann, dass die Installer nicht signiert sind (Windows SmartScreen warnt dann) oder dass jemand mit Zugriff auf das eigene Benutzerkonto die lokale Datenbank lesen kann.

## Für Maintainer

Die Prüfungen vor einer Veröffentlichung stehen in [docs/tasks/T38-oeffentlich.md](docs/tasks/T38-oeffentlich.md). `sh scripts/check-history.sh` prüft einen Spiegelklon auf Geheimnisse, Binärreste und Personenbezug im ganzen Verlauf.

---

## Reporting a vulnerability (English)

Please do not open a public issue. Use GitHub's private vulnerability reporting: open the "Security" tab of the repository and choose "Report a vulnerability". Do not attach real training data, Garmin credentials or tokens. Lastwerk is maintained in spare time, without a response-time guarantee. Only the latest release is supported.
