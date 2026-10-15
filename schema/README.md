# schema

JSON-Schemas (JSON Schema 2020-12). Die Entwürfe stehen in der Spezifikation und werden hier als Dateien abgelegt.

| Datei | Gegenstand | Entsteht in |
|---|---|---|
| plan.schema.json | Wochenplan, siehe [SPEC 7.1](../docs/SPEC.md) | M3 |
| plan-edit.schema.json | Änderungsvorschlag des lokalen Modells, [SPEC 7.2](../docs/SPEC.md) | M5 |
| plan-chat.schema.json | Antwort des lokalen Coachs im Gespräch (Text und optional ein Vorschlag), [SPEC 9.7](../docs/SPEC.md). Der Teil `plan_edit.edit` ist `plan-edit.schema.json` ohne Kopf, eingebettet statt per `$ref`, weil lokale Server Verweise unterschiedlich gut auflösen | T26 |
| recovery-day.schema.json | Erholungstag (HRV, Ruhepuls, Schlaf), [SPEC 7.3](../docs/SPEC.md), Feldnamen vorläufig bis L1 (O9) | M4d |
| archive-manifest.schema.json | Manifest-Eintrag des Archivs, [SPEC 5.5](../docs/SPEC.md) | M4a |
