# fw-ams

Framework Advanced Monitoring System – Akku, USB-C Power Delivery, Thermik und
Expansion-Card-Erkennung für Framework Laptops, unter Windows und Linux.

## Aufbau

| Pfad | Inhalt |
|---|---|
| `crates/fw-ams-core` | Hardware-Zugriff (`framework_lib`, `nusb`, sysfs) und Datenmodell |
| `crates/fw-ams-tuner` | CPU-Tuning: SMU über PawnIO, Windows-Energieplan, Engine mit Statusmodell |
| `crates/fw-ams-service` | Hilfsdienst (Windows, LocalSystem) für das CPU-Tuning |
| `crates/fw-ams-cli`  | CLI `fw-ams` zum Debuggen und Kalibrieren |
| `app/`               | Tauri-v2-App (Tray, UI) |

## CLI

```
cargo run -p fw-ams-cli -- status   # Akku, Ladegerät, Thermik
cargo run -p fw-ams-cli -- cards    # Expansion Cards je Slot
cargo run -p fw-ams-cli -- usb      # USB-Geräte mit ACPI-Root-Port
cargo run -p fw-ams-cli -- dump     # Snapshot als JSON
cargo run -p fw-ams-cli -- smu info # CPU, SMU-Version, PM-Tabelle (über Dienst oder als Admin)
cargo run -p fw-ams-cli -- smu dump # PM-Tabelle roh, zum Zuordnen unbekannter Offsets
```

`--mock` nutzt aufgezeichnete Daten statt Hardware.

## App

```
cd app
npm install
npm run tauri dev               # Entwicklung mit Hot-Reload
npx tauri build                 # Installer (NSIS / deb / rpm / AppImage)
npx tauri build --no-bundle     # nur target/release/fw-ams-app.exe
```

`fw-ams-app --mock` startet mit aufgezeichneten Daten. Fenster schließen minimiert in den Tray,
beenden über Tray → „Beenden“. Einstellungen liegen in `%APPDATA%\io.github.fw-ams\settings.json`
bzw. `~/.config/io.github.fw-ams/settings.json`.

Linux-Build-Abhängigkeiten: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
libudev-dev libusb-1.0-0-dev`.

## EC-Zugriff

- **Windows:** Framework-EC-Treiber (im Framework Driver Bundle, aktuelles BIOS). Keine Admin-Rechte nötig.
- **Linux:** Kernel-Modul `cros_ec_lpcs`, Zugriff auf `/dev/cros_ec` über `packaging/linux/99-fw-ams.rules`.
  Ohne EC-Zugriff werden sysfs/hwmon-Werte verwendet.

## Erweitert: CPU-Tuning und Akkupflege (Windows)

Die Seite „Erweitert“ zeigt live Leistungsaufnahme, Temperatur und Takt und stellt ein:
Powerlimits (STAPM, PPT Fast/Slow), Temperaturgrenze (Tctl), Curve Optimizer, max. Takt,
Boost, EPP, Ladelimit, Ladestrom. Jeder Wert zeigt **Standard, Eingestellt, Gesendet, Ist
(Hardware)** und einen Status (übernommen, begrenzt, abgelehnt, von Firmware überschrieben und
wiederhergestellt …). Ohne Treiber funktionieren Windows-Energiemodus, Live-Takt/-Last und
Akkupflege.

Für Powerlimits, Tctl und Curve Optimizer:
1. **PawnIO** installieren (https://pawnio.eu, signierter Nachfolger von WinRing0).
2. In der App unter Erweitert „Hilfsdienst installieren“ (eine UAC-Abfrage). Der Dienst liegt in
   `%ProgramFiles%\fw-ams-tuner`, Konfiguration und Log in `%ProgramData%\fw-ams`.

Entwicklung: `scripts/fetch-pawnio-module.ps1` lädt das signierte `RyzenSMU.bin`
(PawnIO.Modules, LGPL-2.1), `cargo build -p fw-ams-service` baut den Dienst neben die App.
Installer mit Dienst: `cd app && npm run bundle:windows`. Zum Debuggen in einer Admin-Konsole:
`target\debug\fw-ams-service.exe console`.

Unterstützt: Ryzen AI 300 (Strix Point, Krackan Point), Ryzen AI Max (Strix Halo). Message-IDs
und Tabellen-Offsets stammen aus RyzenAdj. Für Krackan Point (z. B. Ryzen AI 7 350) sind Limits
und Tctl bekannt, Werte pro Kern kommen aus den Windows-Leistungsindikatoren. Alle SMU-Werte
sind flüchtig: ein Neustart setzt sie zurück. Linux: siehe `docs/linux-advanced.md`.

## Expansion Cards merken

Passive Karten melden sich nicht immer: Eine USB-C-Karte ist ohne angeschlossenes Gerät
unsichtbar, eine USB-A-Karte nach dem Standby oft auch. fw-ams merkt sich deshalb, welche Karte
zuletzt sicher in welchem Slot erkannt wurde, und zeigt sie weiter an (blass, gestrichelt, „↺“).
Nach dem Umstecken: Ports & Karten → „Slot vergessen“ oder Einstellungen → „Alle vergessen“.
Gespeichert in `cards.json` neben `settings.json`.

## Zurücksetzen

Einstellungen → „Erweiterte Funktionen“ ausschalten macht alles rückgängig, was fw-ams am System
verändert hat, und lässt nur die Überwachung laufen: CPU-Profile leeren, Powerlimits, Curve
Optimizer und Windows-Energieplan auf die Werte von vorher, ursprüngliches Ladelimit zurück in den
EC, Ladestrom ohne Limit, Sparmodus beenden. Optional wird der Hilfsdienst samt Profilen
deinstalliert. „Werkseinstellungen“ setzt zusätzlich alle Einstellungen zurück, entfernt den
Autostart-Eintrag und löscht die gemerkten Karten. Jeder Schritt wird mit Ergebnis angezeigt.
Den separat installierten PawnIO-Treiber entfernt fw-ams nicht.

Beim Deinstallieren der App (Windows) passiert dasselbe automatisch: Der Deinstaller schließt
fw-ams, gibt dem EC sein Ladelimit zurück, entfernt den Hilfsdienst samt Profilen (eine
Admin-Abfrage, nur wenn er installiert ist) und den Autostart-Eintrag. Bei Updates bleibt alles.

## Kalibrierung der Slots

Die Zuordnung USB-Root-Port → Slot steht in `crates/fw-ams-core/src/cards/slots.rs`.
Eine aktive Karte (HDMI/DP/SD/…) nacheinander in jeden Slot stecken, `fw-ams usb` ausführen
und den Root-Port (z. B. `XHC0.PRT2`) beim Slot eintragen.
