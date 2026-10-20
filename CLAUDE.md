# fw-ams: Hinweise für KI-Assistenten

Monitoring für Framework-Laptops (Akku, USB-C PD, Thermik, Expansion Cards), Windows + Linux.
Rust-Kern + Tauri-v2-App (React 19, Tailwind v4). **UI-Texte Deutsch, Code und Kommentare Englisch.**
Mehr Nutzerdoku: README.md. Hier nur, was man zum schnellen Weiterentwickeln braucht.

## Karte
| Pfad | Aufgabe |
|---|---|
| `crates/fw-ams-core/src/model.rs` | Datenmodell (`Snapshot`, `Slot`, `PdPort`, ...), per ts-rs nach TS exportiert |
| `.../collector.rs` | baut den `Snapshot` aus EC/USB/sysfs/OS-Akku (`Collector::snapshot`) |
| `.../source/{ec,usb,linux,os_battery}.rs` | Hardware-Zugriff (framework_lib, nusb, sysfs) |
| `.../cards/slots.rs` | Slot-Layout je Modell, Fähigkeiten, kalibrierte USB-Root-Ports |
| `.../cards/{detect,catalog}.rs` | Kartenerkennung (USB-IDs, PD-Heuristik) |
| `.../cards/memory.rs` | Kartengedächtnis: merkt sichere Erkennungen je Slot, füllt stille passive Karten auf (`Confidence::Remembered`) |
| `.../alerts.rs` | Benachrichtigungsregeln mit Hysterese + `AlertSettings` |
| `.../mock.rs` | Aufgezeichnete Testdaten (`--mock`, Testfixture) |
| `.../saver.rs` | Zustandsmaschine Auto-Sparmodus (niedriger Akku) |
| `crates/fw-ams-tuner/src/engine.rs` | CPU-Tuner: Profil anwenden, rücklesen, Firmware-Override erkennen/wiederherstellen, Energieplan-Originale (`plan_originals.json`), Ereignislog |
| `.../{family,pm_table}.rs` | SMU-Message-IDs, Mailbox, PM-Tabellen-Offsets je CPU (aus RyzenAdj) |
| `.../smu/{pawnio,mock}.rs` | SMU-Backends: PawnIO + RyzenSMU-Modul (Windows), Simulation |
| `.../{winpower,oscpu}.rs` | Windows-Energieplan/-modus, PDH-Takt/-Last |
| `.../{protocol,client}.rs` | Named Pipe App ↔ Dienst (JSON-Zeilen), Dienststatus, UAC-Start |
| `crates/fw-ams-service` | Windows-Dienst (LocalSystem), Pipe-Server, install/uninstall/console |
| `crates/fw-ams-cli` | CLI zum Debuggen (`status`, `cards`, `usb`, `dump`, `--mock`) |
| `app/src-tauri/src/main.rs` | Setup, Fenster-Events, Exit (Sparmodus beenden) |
| `.../state.rs`, `commands.rs`, `poller.rs` | `AppState` (+ Lock-Reihenfolge, `commit_settings`, `Wake`); alle Commands; Poller- und Tuner-Thread |
| `.../charge.rs`, `tuner.rs`, `saver.rs` | Ladelimit/-strom mit Statusmodell und gemerktem Original; Tuner-Anbindung, Verlauf (10 min); Sparmodus |
| `.../reset.rs`, `store.rs` | Zusatzfunktionen aus / Werksreset mit Bericht je Schritt; JSON-Dateien im App-Config-Dir |
| `.../tray.rs`, `flyout.rs` | Tray-Icon, Hover-Flyout (Breite `WIDTH` hier **und** in `Flyout.tsx`) |
| `.../settings.rs` | `Settings` (+ `Theme`), JSON in App-Config-Dir, `sanitized()` klemmt Werte |
| `app/src/api.ts` | `useSnapshot`, `useSettings`, Settings-Commands |
| `app/src/slots.ts` | `describe`, `flow`, `slotRows`: gemeinsam für Flyout, Diagramm, Ports |
| `app/src/theme.ts` | Dark Mode (`.dark` auf `<html>`, Titelleiste, localStorage-Cache) |
| `app/src/format.ts` | Zahlenformat (`num`, `signed`), Labels, `cardStyle` |
| `app/src/pages/*`, `pages/advanced/*` | Seiten (Übersicht, Ports, Thermik, System, Erweitert, Einstellungen); Erweitert ist in Live, Tuning, BatteryCare, Events, Setup zerlegt |
| `app/src/components/*` | `ui` (Card, Button, ConfirmButton, Notice, Toast), `controls` (Row, Switch, NumberInput), `ResetPanel`, `LaptopDiagram`, `BatteryRing`, `Stat`, `LineChart`, `TuneTable`, `PowerBreakdown` |
| `app/src/bindings/` | **generiert**, nie von Hand ändern |

## Datenfluss
`Collector` → Poller-Thread (`poller.rs`: Kartengedächtnis, Ladesteuerung, Alerts, Sparmodus) → Tray + Event `snapshot` → `useSnapshot()`.
Weitere Events: `settings` (nach `set_settings`, für Live-Theme), `navigate` (Slotnummer, vom Flyout),
`charge` (Ladesteuerung), `tuner` (1 Hz: `TunerView` + `HistorySample`).
Commands: `get_snapshot`, `get_settings`, `set_settings`, `show_main(slot?)`, `test_notification`,
`forget_cards(slot?)`, `reset_system({factory, removeService})`, `get_charge_control`, `charge_full_once`,
`get_tuner`, `get_setup`, `get_history`, `tuner_set_config`, `tuner_reset`, `tuner_install`, `tuner_uninstall`,
`set_power_mode`, `open_pawnio_site`.
Tuner: App → Named Pipe `\\.\pipe\fw-ams-tuner` → `fw-ams-service` → `Engine` → PawnIO. Mit `--mock` läuft die Engine simuliert in der App.

## Befehle
- `cargo test --workspace`: Tests **und** TS-Bindings neu erzeugen (`.cargo/config.toml` setzt `TS_RS_EXPORT_DIR`)
- `cd app && npm run build`: tsc + vite (schneller Typcheck der UI)
- `cd app && npm run tauri dev -- -- --mock [--flyout]`: App mit Mockdaten, Flyout sofort offen
- App-Flags: `--mock` / `FW_AMS_MOCK`, `--flyout`, `--minimized`, `--restore-system` (ohne UI: Ladelimit zurück, Ladestrom-Limit weg; ruft der Deinstaller auf)
- Mock ist ein FW13 AI 300: USB-C links hinten (Ladegerät), USB-A links vorne, HDMI rechts vorne, rechts hinten leer
- Mock-Tuner: Strix-PM-Tabelle, Firmware setzt STAPM alle 90 s zurück, Fast-Limit auf 35 W gedeckelt (testet „wiederhergestellt“/„begrenzt“)
- `cargo build -p fw-ams-service` + `scripts/fetch-pawnio-module.ps1`: Dienst und RyzenSMU-Modul für die Entwicklung
- `fw-ams smu info|dump`: SMU/PM-Tabelle prüfen; `target\debug\fw-ams-service.exe console` (Admin) zum Debuggen
- `cd app && npm run bundle:windows`: Installer inkl. Dienst (Sidecar) und RyzenSMU.bin

## Rezepte
- **Neue Einstellung:** Feld in `settings.rs` (bzw. `AlertSettings`) + Default + ggf. `sanitized()` → `cargo test` → UI in `pages/Settings.tsx` (`Row` + `Switch`/`NumberInput`; speichert automatisch).
- **Neue Funktion, die das System verändert:** nur bei `settings.advanced_features`, Originalwert vor dem ersten Schreiben merken und in `reset.rs` zurückschreiben (Schritt im Bericht).
- **Neuer Kartentyp:** `CardKind` (model.rs) + `cards/catalog.rs` + `cardStyle` in `format.ts`.
- **Slot kalibrieren:** `fw-ams usb` ausführen, Root-Port in `cards/slots.rs` eintragen.
- **Neuer Tuning-Parameter:** `ParamId` + `TuneProfile` (types.rs) + `family::specs` + `Engine::read_actual`/`write` → UI-Text in `paramMeta` (format.ts).
- **Neue CPU-Familie:** `family.rs` (Message-IDs, Codename) + `pm_table.rs` (Layout je Tabellenversion, mit `fw-ams smu dump` prüfen).
- **Neue Fenster-API im Frontend:** Permission in `app/src-tauri/capabilities/default.json` ergänzen, sonst schlägt der Aufruf still fehl.

## Stolpersteine
- Dark Mode läuft über die Klasse `.dark` (`@custom-variant dark` in styles.css), nicht über die Media-Query. Neue Farben immer mit `dark:`-Variante.
- CSP verbietet Inline-Scripts. Theme-Flash wird deshalb in `main.tsx` über `applyCachedTheme()` verhindert.
- Flyout: Fensterhöhe folgt dem Inhalt (ResizeObserver). Linux hat keine Tray-Hover-Events, dort öffnet das Menü die Schnellübersicht.
- React 19: `useRef` braucht ein Argument (`useRef<T | undefined>(undefined)`).
- Zahlen im UI immer über `num()`/`signed()` (de-DE, `n/v` bei fehlenden Werten).
- PD-Port-Index ist nicht die Slotnummer: Zuordnung steht in `cards/slots.rs`.
- Dienst nimmt nur typisierte, geklemmte Werte an (`Engine::sanitize`), nie rohe SMU-Befehle. Pipe-DACL: SYSTEM, Admins, installierender Nutzer.
- Gerät des Entwicklers ist Krackan Point (Ryzen AI 7 350, PM-Tabelle 0x650005): Kern-Offsets unbekannt, daher Kernwerte aus PDH.
- Curve Optimizer ist nicht rücklesbar (Status „gesendet, nicht rücklesbar“); nach Absturz im Testlauf nie automatisch anwenden.
- Windows-Energieplanwerte (EPP, Boost, max. Takt) sind dauerhaft, SMU-Werte nicht. Die Engine merkt sich Plan-Originale je AC/DC; `fw-ams-service uninstall` stellt sie wieder her, `--purge` löscht zusätzlich Profile und Log.
- Ladelimit: Original steht in `charge-restore.json`, wird beim Ausschalten der Verwaltung zurückgeschrieben.
- Kartengedächtnis (`cards.json`) wird mit `--mock` nicht gespeichert. Leerer Slot zeigt nur USB-A/USB-C aus dem Gedächtnis, aktive Karten nie (die brauchen Strom, leer = wirklich leer).
- Poller liest Settings unter der Collector-Sperre; `reset.rs` hält sie beim Ändern, sonst schreibt ein laufender Poll alte Werte zurück.
- Bestätigungen über `ConfirmButton`, nicht `window.confirm`.
- Pipe-Anfragen haben 5 s Timeout (`client::request`, Worker-Thread + `CancelSynchronousIo`), danach 15 s Fail-fast. Der Dienst bedient jeden Client in einem eigenen Thread.
- Deinstallation der App: `app/src-tauri/windows/hooks.nsh` (nicht bei Updates) schließt die App, ruft `--restore-system`, `fw-ams-service uninstall --purge` (UAC, nur wenn installiert) und löscht den Autostart-Eintrag. Datei mit UTF-8-BOM speichern (Umlaute).
- Commits: auf Feature-Branch, je Phase/kleinem Schritt (Wunsch des Nutzers).
