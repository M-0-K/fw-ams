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
| `.../alerts.rs` | Benachrichtigungsregeln mit Hysterese + `AlertSettings` |
| `.../mock.rs` | Aufgezeichnete Testdaten (`--mock`, Testfixture) |
| `crates/fw-ams-cli` | CLI zum Debuggen (`status`, `cards`, `usb`, `dump`, `--mock`) |
| `app/src-tauri/src/main.rs` | Commands, Poller-Thread, Fenster-Events |
| `.../tray.rs`, `flyout.rs` | Tray-Icon, Hover-Flyout (Breite `WIDTH` hier **und** in `Flyout.tsx`) |
| `.../settings.rs` | `Settings` (+ `Theme`), JSON in App-Config-Dir, `sanitized()` klemmt Werte |
| `app/src/api.ts` | `useSnapshot`, `useSettings`, Settings-Commands |
| `app/src/slots.ts` | `describe`, `flow`, `slotRows`: gemeinsam für Flyout, Diagramm, Ports |
| `app/src/theme.ts` | Dark Mode (`.dark` auf `<html>`, Titelleiste, localStorage-Cache) |
| `app/src/format.ts` | Zahlenformat (`num`, `signed`), Labels, `cardStyle` |
| `app/src/pages/*`, `components/*` | Seiten (Übersicht, Ports, Thermik, System, Einstellungen), `LaptopDiagram`, `BatteryRing`, `Stat` |
| `app/src/bindings/` | **generiert**, nie von Hand ändern |

## Datenfluss
`Collector` → Poller-Thread (`main.rs`) → Tray + Alerts + Event `snapshot` → `useSnapshot()`.
Weitere Events: `settings` (nach `set_settings`, für Live-Theme), `navigate` (Slotnummer, vom Flyout).
Commands: `get_snapshot`, `get_settings`, `set_settings`, `show_main(slot?)`, `test_notification`.

## Befehle
- `cargo test --workspace`: Tests **und** TS-Bindings neu erzeugen (`.cargo/config.toml` setzt `TS_RS_EXPORT_DIR`)
- `cd app && npm run build`: tsc + vite (schneller Typcheck der UI)
- `cd app && npm run tauri dev -- -- --mock [--flyout]`: App mit Mockdaten, Flyout sofort offen
- App-Flags: `--mock` / `FW_AMS_MOCK`, `--flyout`, `--minimized`
- Mock ist ein FW13 AI 300: USB-C links hinten (Ladegerät), USB-A links vorne, HDMI rechts vorne, rechts hinten leer

## Rezepte
- **Neue Einstellung:** Feld in `settings.rs` (bzw. `AlertSettings`) + Default + ggf. `sanitized()` → `cargo test` → UI in `pages/Settings.tsx` (`Row` + `Switch`/`NumberInput`; speichert automatisch).
- **Neuer Kartentyp:** `CardKind` (model.rs) + `cards/catalog.rs` + `cardStyle` in `format.ts`.
- **Slot kalibrieren:** `fw-ams usb` ausführen, Root-Port in `cards/slots.rs` eintragen.
- **Neue Fenster-API im Frontend:** Permission in `app/src-tauri/capabilities/default.json` ergänzen, sonst schlägt der Aufruf still fehl.

## Stolpersteine
- Dark Mode läuft über die Klasse `.dark` (`@custom-variant dark` in styles.css), nicht über die Media-Query. Neue Farben immer mit `dark:`-Variante.
- CSP verbietet Inline-Scripts. Theme-Flash wird deshalb in `main.tsx` über `applyCachedTheme()` verhindert.
- Flyout: Fensterhöhe folgt dem Inhalt (ResizeObserver). Linux hat keine Tray-Hover-Events, dort öffnet das Menü die Schnellübersicht.
- React 19: `useRef` braucht ein Argument (`useRef<T | undefined>(undefined)`).
- Zahlen im UI immer über `num()`/`signed()` (de-DE, `n/v` bei fehlenden Werten).
- PD-Port-Index ist nicht die Slotnummer: Zuordnung steht in `cards/slots.rs`.
- Commits nur auf Wunsch des Nutzers.
