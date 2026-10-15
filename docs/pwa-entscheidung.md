# PWA für das Handy: Entscheidung (O5)

Stand: 13. Oktober 2026 (Task T16). Status: **Empfehlung des Tasks, Entscheidung des Projekts offen.** Es wird nichts implementiert. F31 (PWA für Handy) bleibt "Kann" (SPEC 4.3).

## Frage

Lastwerk ist eine lokale Desktop-App ohne Konto und ohne Server (SPEC 10.1). Ein Handy hat keine Adresse, unter der es die App erreichen könnte. Wie kommt eine Ansicht des Wochenplans (und gegebenenfalls der Form) auf das Handy, ohne Daten an Dritte zu geben (O5)?

## Was das Handy heute schon bekommt

Bevor eine PWA gebaut wird, ist festzuhalten, was ohne sie bereits geht:

- **Kalender.** `export-ics` (T08) schreibt eine Datei mit Titel, Zeit und Dauer je Einheit, ohne Orte und ohne Schwellenwerte (SPEC 10.2). Ein Kalenderprogramm auf dem Handy zeigt den Plan an, sobald die Datei dort importiert ist.
- **Uhr und Edge.** `push` (T16) legt Workouts in Garmin Connect an und plant sie, nach der manuellen Freigabe (O18). Die Garmin-Connect-App auf dem Handy zeigt sie dann ohnehin an, und das Gerät synchronisiert sie.

Eine PWA lohnt sich also nur für das, was beide nicht liefern: die Wochenansicht von Lastwerk selbst mit Belastung, Fitness, Ermüdung und Form, und die Begründungen der Planregeln.

## Voraussetzungen jeder Variante

- **Nur lesend.** Die PWA zeigt Wochenplan, Form und Begründungen. Sie schreibt nichts in die Datenbank, ruft keinen Import und keinen Garmin-Zugriff auf.
- **Ausdrückliche Handlung.** Jede Verbindung nach außen braucht eine ausdrückliche Handlung und ist in den Einstellungen sichtbar (SPEC 10.1). Standard: aus.
- **Gefilterte Ausgabe.** Die PWA bekommt dieselbe Art fester Ausgabetypen wie `lastwerk-mcp` (SPEC 9.6, 10.2): keine Koordinaten, keine Freitexte, keine Gerätekennungen, keine Rohdaten.
- **Sicherer Kontext.** Nach dem Stand des Wissens über die Webplattform verlangen Service Worker und die Installation einer PWA einen sicheren Kontext (HTTPS, oder die Adresse `localhost`). Eine Adresse im Heimnetz ohne Zertifikat erfüllt das nicht. Diese Aussage ist hier nicht gegen die aktuelle Browserdokumentation geprüft und ist vor einer Umsetzung zu prüfen. Ohne Service Worker bleibt die Seite eine normale Webseite, die sich als Lesezeichen auf den Startbildschirm legen lässt, aber nicht offline läuft.

## Varianten

### A: Server im Heimnetz (LAN-Server aus der Desktop-App)

Das Rust-Backend der App öffnet, wenn der Nutzer es einschaltet, einen HTTP-Server im lokalen Netz. Das Handy ruft im selben WLAN eine Adresse mit Port auf.

| Punkt | Bewertung |
|---|---|
| Erreichbarkeit vom Handy | Nur im selben Netz, solange der Rechner läuft und die App den Server offen hat. Von unterwegs nicht, außer über Variante B. Die Adresse des Rechners kann wechseln (dynamische Vergabe im Heimnetz), stabil wird sie über eine feste Zuordnung im Router oder einen lokalen Namensdienst. Ein Gast-WLAN mit Geräteisolation verhindert den Zugriff. Auf einem Windows-Rechner braucht der Port eine Freigabe in der Firewall |
| Datenschutzfolge | Die Daten verlassen das Heimnetz nicht. Wer im selben Netz sitzt, kann den Server erreichen und, ohne Zugangsschutz, die Ansicht abrufen. Ohne TLS ist der Verkehr im WLAN mitlesbar. Es sind also nötig: Kopplung mit einem Token (zum Beispiel per QR-Code angezeigt, nur einmal gültig), TLS mit einem Zertifikat, dem das Handy vertraut, und Beschränkung auf lesende Ausgabetypen. Kein Dritter sieht Inhalte |
| Aufwand | Mittel bis hoch: HTTP-Server im Backend, Kopplung und Token-Verwaltung, TLS samt Zertifikatsverteilung auf das Handy, schmale Lese-Schnittstelle, eigene mobile Oberfläche, Einstellung zum Ein- und Ausschalten, Tests |
| Risiken | Ein Listener in einer Desktop-App ist neue Angriffsfläche und widerspricht dem Satz "es gibt keinen Server" (SPEC 10.1), wenn er nicht klar abgegrenzt und standardmäßig aus ist. Er verlangt eine Änderung von SPEC 10.1 und 10.4 (Netzwerkzugriffe liegen weiterhin nur im Backend, das Frontend bekommt keine HTTP-Berechtigung). Zertifikate auf dem Handy zu installieren ist für den Nutzer lästig. Der Rechner muss laufen |

### B: Privater Tunnel

Das Handy erreicht den Rechner zu Hause über einen verschlüsselten Tunnel, der nur den eigenen Geräten offensteht. Gemeint ist ein privates Netz zwischen den eigenen Geräten (Mechanismus: virtuelles privates Netz zwischen Endgeräten, Verbindung vom Handy zum Heimrechner, bei Bedarf über ein Relais), oder ein selbst betriebener Zugang zum Heimnetz. Gemeint ist nicht ein Tunnel, der eine öffentliche Adresse bereitstellt, die jeder aufrufen kann. Dort terminiert der Betreiber des Tunnels die Verschlüsselung und sieht den Inhalt, das widerspricht dem Ziel "privat".

| Punkt | Bewertung |
|---|---|
| Erreichbarkeit vom Handy | Auch unterwegs, solange der Rechner läuft und der Server der Variante A offen ist. Aus Sicht von Lastwerk ist das dieselbe Schnittstelle wie A, nur der Weg dorthin ist ein anderer |
| Datenschutzfolge | Bei Ende-zu-Ende-Verschlüsselung zwischen den eigenen Geräten sieht ein Vermittler höchstens Metadaten (welche Geräte wann verbunden sind, Adressen, Datenmenge), nicht den Inhalt. Das hängt von der Bauart des Tunnels ab und ist für den gewählten Anbieter zu prüfen, bevor er genutzt wird. Selbst betrieben liegt der Schlüssel allein beim Nutzer, dafür trägt er den Betrieb. Beim Tunnel mit öffentlicher Adresse sieht der Betreiber Klartext: ausgeschlossen |
| Aufwand | Für Lastwerk: kein zusätzlicher Code über A hinaus, dazu ein Abschnitt im README. Für den Nutzer: Software auf Rechner und Handy einrichten, gegebenenfalls ein Konto beim Anbieter oder ein eigener Server |
| Risiken | Zusätzlicher Anbieter oder eigener Betrieb, Abhängigkeit von dessen Verfügbarkeit und Bedingungen, Fehlkonfiguration kann den Server ins Internet öffnen. Der Rechner muss laufen |

### C: Statische Veröffentlichung

Lastwerk exportiert eine fertige Wochenansicht als statische Dateien, der Nutzer legt sie auf einen Webspace oder in einen Speicher mit Weblink. Das Handy ruft die Adresse auf.

| Punkt | Bewertung |
|---|---|
| Erreichbarkeit vom Handy | Überall, auch bei ausgeschaltetem Rechner. Die Ansicht ist aber ein Stand zum Zeitpunkt des Exports und muss bei jeder Änderung neu veröffentlicht werden |
| Datenschutzfolge | Der Inhalt liegt bei einem Dritten. Der Betreiber sieht Inhalt und Abrufe (wer, wann, von welcher Adresse), und jeder mit dem Link kann ihn aufrufen. Das ist die Weitergabe von Trainingsdaten und widerspricht "Daten bleiben lokal" (SPEC 10.1), wenn es nicht ausdrücklich gewollt ist. Mildern lässt sich das durch Verschlüsselung vor der Veröffentlichung (Chiffretext auf dem Webspace, Entschlüsselung im Browser des Handys mit einem Schlüssel, der nicht auf den Server gelangt, zum Beispiel im Fragment der Adresse oder als Passwort). Dann sieht der Betreiber nur Chiffretext und Abrufmetadaten. Der Schlüssel im Fragment landet im Browserverlauf des Handys |
| Aufwand | Gering bis mittel: Export der Ansicht, Verschlüsselung und kleine Entschlüsselungsseite, keine Serverlogik, keine Kopplung. Mehr Arbeit, wenn die Ansicht dieselbe Oberfläche wie die App zeigen soll |
| Risiken | Veralteter Stand, Verbleib von Kopien (Zwischenspeicher, Sicherungen beim Betreiber) auch nach dem Löschen, ein verlorener oder weitergegebener Link, Metadaten der Abrufe. Aussagen über konkrete Anbieter sind hier bewusst nicht getroffen |

## Gegenüberstellung

| | A: Heimnetz | B: privater Tunnel | C: statisch |
|---|---|---|---|
| Handy im Heimnetz | ja | ja | ja |
| Handy unterwegs | nein | ja | ja |
| Rechner muss laufen | ja | ja | nein |
| Aktueller Stand | immer | immer | nur nach Export |
| Dritter sieht Inhalt | nein | nein (bei Ende-zu-Ende-Verschlüsselung) | ja, ohne Verschlüsselung; sonst nur Chiffretext |
| Dritter sieht Metadaten | nein | ggf. der Vermittler | der Betreiber des Webspace |
| Neuer Server in der App | ja | ja (wie A) | nein |
| Änderung an SPEC 10.1 und 10.4 | ja | ja | nein |
| Aufwand für Lastwerk | mittel bis hoch | wie A, plus Anleitung | gering bis mittel |

## Empfehlung

1. **Die PWA jetzt nicht bauen.** F31 bleibt "Kann". Kalender (ICS) und Garmin-Connect-App decken die Hauptnutzung auf dem Handy ab: den Plan ansehen und die Einheiten auf dem Gerät folgen. Die offene Frage ist, ob die Lastwerk-eigene Ansicht (Form, Belastung, Begründungen) auf dem Handy den Aufwand und die neue Angriffsfläche wert ist. Das entscheidet der Nutzer nach einigen Wochen mit ICS und `push`.
2. **Wenn gebaut wird: Variante A als Grundlage, B als Betriebsoption.** Der Server ist ausschließlich lesend, standardmäßig aus, in den Einstellungen sichtbar, über Token gekoppelt und mit TLS. Den Zugang von unterwegs löst der Nutzer mit einem privaten Tunnel nach Variante B, ohne dass Lastwerk davon etwas wissen muss. Damit gibt es keine neue Datenübergabe an Dritte, und die Datenschutzgrenze bleibt das eigene Netz.
3. **Variante C nicht für Version 1.** Sie ist die einzige, bei der Inhalte bei einem Dritten liegen. Sie kommt höchstens später als ausdrücklicher, verschlüsselter Export in Frage.
4. **Vorab zu klären**, bevor Code entsteht: Sicherer Kontext und Service Worker im Heimnetz gegen die Browserdokumentation prüfen, Zertifikatsweg für das Handy wählen, Umfang der Lese-Schnittstelle als eigene Ausgabetypen festlegen (Wiederverwendung der Typen aus `lastwerk-mcp`), SPEC 10.1 und 10.4 anpassen.

## Offen

- Entscheidung des Projekts über diese Empfehlung (O5).
- Prüfung der Aussage zum sicheren Kontext und die Wahl des Tunnelwegs, falls B genutzt wird.
- Ob die Ansicht auf dem Handy überhaupt gebraucht wird, siehe Empfehlung 1.
