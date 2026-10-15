# Skills für die Oberfläche

Übernommen aus https://github.com/emilkowalski/skills (Commit `2d228630ba1446365f614a108cb8252f1feda202`, Lizenz MIT, Text in `LICENSE-emilkowalski-skills`). Die Dateien sind unverändert, auf Englisch und gehören dem Urheber. Unsere eigenen Regeln stehen in `CLAUDE.md` und `docs/SPEC.md`, bei Widerspruch gelten diese.

| Skill | Wann |
|---|---|
| `emil-design-eng` | Grundhaltung für Feinschliff, Animation und Komponenten. Bei jeder Arbeit an der Oberfläche |
| `animate` | Eine Bewegung neu bauen (Kurve, Dauer, Eigenschaften) |
| `review-animations` | Vorhandene Bewegung streng prüfen |
| `improve-animations` | Alle Bewegungen im Code prüfen und Pläne ableiten |
| `find-animation-opportunities` | Stellen finden, wo Bewegung etwas bringt, und solche, wo nicht |
| `animation-performance` | Ruckelnde Animation beheben |
| `animation-vocabulary` | Richtige Begriffe für Bewegung |
| `break-ui` | Oberfläche mit Extremdaten (lange Namen, leere Listen, große Zahlen) prüfen |

Bewusst nicht übernommen: `animate-expo`, `mobile-native`, `write-swift` (andere Plattformen), `apple-design`, `ask-sonner` (React-Bibliothek), `pick-ui-library` (würde Abhängigkeiten vorschlagen, die nach `CLAUDE.md` einen Vermerk in der Task-Datei brauchen), `prototype`.

## Grenzen für Lastwerk

- Keine neue Abhängigkeit nur wegen eines Skills. Bewegung zuerst mit CSS, wie `animate` es selbst verlangt.
- Keine Netzwerkzugriffe aus dem Frontend (SPEC 10), also keine Remote-Schriften, keine Beispielseiten, die nachladen.
- Die Skills nennen teils Bibliotheken für React. Lastwerk nutzt Svelte 5, das Prinzip gilt, der Code wird übersetzt.
- Aktualisieren: Dateien aus dem Quell-Repository neu laden, Commit-Hash oben nachtragen.
