# Mitmachen bei fw-ams

Danke, dass du helfen möchtest! Beiträge jeder Größe sind willkommen – vom Tippfehler über
Messdaten deines Laptops bis zu neuen Funktionen. *English contributions are welcome too.*

## Fehler melden und Ideen

- **Fehler:** [Issue anlegen](https://github.com/M-0-K/fw-ams/issues/new/choose) mit Modell,
  BIOS-Version, Betriebssystem und fw-ams-Version (Seite System). Für Kartenprobleme hilft die
  Ausgabe von `fw-ams cards` und `fw-ams usb`, für Tuning-Probleme der Reiter Erweitert →
  Ereignisse.
- **Sicherheitslücken** bitte nicht öffentlich melden, sondern wie in [SECURITY.md](SECURITY.md)
  beschrieben.
- **Größere Änderungen** vorher kurz in einem Issue besprechen, damit niemand umsonst arbeitet.

## Besonders hilfreich

- **Slot-Kalibrierung** für weitere Modelle (Laptop 12, 13 Intel/7040, 16, Desktop) –
  Anleitung in [docs/development.md](docs/development.md#slots-kalibrieren)
- **PM-Tabellen** weiterer Ryzen-CPUs (`fw-ams smu dump`), damit Werte pro Kern angezeigt werden
- **Linux-Portierung** der erweiterten Funktionen – Plan in [docs/linux-advanced.md](docs/linux-advanced.md)
- **Übersetzung** der Oberfläche (bisher nur Deutsch)

## Pull Requests

1. Fork anlegen, einen Branch vom aktuellen `main` erstellen.
2. Änderung umsetzen; Code und Kommentare auf Englisch, UI-Texte auf Deutsch.
3. Vor dem PR lokal prüfen:

   ```sh
   cargo test --workspace
   cargo clippy --workspace --all-targets
   cd app && npm run build
   ```

4. Kleine, in sich geschlossene Commits mit aussagekräftiger Nachricht.
5. Im PR beschreiben, was sich ändert und wie du es getestet hast (Modell, Betriebssystem, Mock).

Was beim Entwickeln zu beachten ist (Dark Mode, Zahlenformat, Bindings, Dienst-Sicherheit),
steht in [CLAUDE.md](CLAUDE.md) und [docs/development.md](docs/development.md).

### Regeln für Funktionen, die das System verändern

- Nur aktiv, wenn „Erweiterte Funktionen“ eingeschaltet sind.
- Den Originalwert speichern, bevor er geändert wird, und ihn beim Zurücksetzen und beim
  Deinstallieren wiederherstellen (`app/src-tauri/src/reset.rs`).
- Der Hilfsdienst nimmt nur typisierte, auf sichere Bereiche begrenzte Werte an – niemals rohe
  Hardware-Befehle über die Pipe.

## Lizenz deiner Beiträge

fw-ams steht unter der [GNU GPL v3 oder später](LICENSE). Mit einem Pull Request stimmst du zu,
dass dein Beitrag unter derselben Lizenz veröffentlicht wird.

## Verhaltenskodex

Für alle Beteiligten gilt der [Verhaltenskodex](CODE_OF_CONDUCT.md).
