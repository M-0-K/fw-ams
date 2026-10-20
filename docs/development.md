# Entwicklung

## Aufbau

| Pfad | Inhalt |
|---|---|
| `crates/fw-ams-core` | Hardware-Zugriff (`framework_lib`, `nusb`, sysfs), Datenmodell, Kartenerkennung und -gedächtnis, Benachrichtigungsregeln |
| `crates/fw-ams-tuner` | CPU-Tuning: SMU über PawnIO, Windows-Energieplan, Engine mit Statusmodell, Pipe-Protokoll |
| `crates/fw-ams-service` | Hilfsdienst (Windows, LocalSystem) für das CPU-Tuning |
| `crates/fw-ams-cli` | CLI `fw-ams` zum Debuggen und Kalibrieren |
| `app/` | Tauri-v2-App: Rust-Backend in `src-tauri/`, React-19-UI mit Tailwind v4 in `src/` |

Eine ausführlichere Karte der Dateien, Datenflüsse und Stolpersteine steht in
[`CLAUDE.md`](../CLAUDE.md) (auch für Menschen gedacht).

Konventionen: Oberfläche auf Deutsch, Code und Kommentare auf Englisch. Zahlen im UI immer über
`num()`/`signed()` (de-DE). Die TypeScript-Typen in `app/src/bindings/` erzeugt `ts-rs` – nie von
Hand ändern.

## Voraussetzungen

- Rust (stable) und Node.js 22
- Windows: WebView2 (in Windows 11 enthalten), Visual Studio Build Tools
- Linux: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libudev-dev libusb-1.0-0-dev patchelf`

## Bauen und testen

```sh
cargo test --workspace            # Tests, erzeugt auch die TS-Bindings neu
cargo clippy --workspace --all-targets

cd app
npm ci
npm run build                     # Typcheck + Vite-Build der Oberfläche
npm run tauri dev -- -- --mock    # App mit aufgezeichneten Daten, Hot-Reload
npm run tauri dev -- -- --mock --flyout   # Tray-Schnellansicht sofort offen
npx tauri build                   # Installer (NSIS / deb / rpm / AppImage)
npm run bundle:windows            # Windows-Installer inkl. Hilfsdienst und RyzenSMU.bin
```

App-Flags: `--mock` (oder Umgebungsvariable `FW_AMS_MOCK`), `--flyout`, `--minimized`,
`--restore-system` (ohne Oberfläche: Ladelimit zurück, Ladestrom-Limit weg; nutzt der
Deinstaller).

Die Mock-Daten stammen von einem Laptop 13 (Ryzen AI 300): USB-C links hinten mit Ladegerät,
USB-A links vorne, HDMI rechts vorne, rechts hinten leer. Der Mock-Tuner simuliert eine Firmware,
die STAPM alle 90 s zurücksetzt und PPT Fast auf 35 W deckelt.

## CLI

```sh
cargo run -p fw-ams-cli -- status     # Akku, Ladegerät, Thermik
cargo run -p fw-ams-cli -- cards      # Expansion Cards je Slot
cargo run -p fw-ams-cli -- usb        # USB-Geräte mit ACPI-Root-Port
cargo run -p fw-ams-cli -- dump       # Snapshot als JSON
cargo run -p fw-ams-cli -- smu info   # CPU, SMU-Version, PM-Tabelle (über Dienst oder als Admin)
cargo run -p fw-ams-cli -- smu dump   # PM-Tabelle roh, zum Zuordnen unbekannter Offsets
```

`--mock` nutzt auch hier aufgezeichnete Daten.

## Hilfsdienst (Windows)

```powershell
scripts\fetch-pawnio-module.ps1         # signiertes RyzenSMU.bin (PawnIO.Modules, LGPL-2.1)
cargo build -p fw-ams-service           # landet neben der App in target\debug
target\debug\fw-ams-service.exe console # im Vordergrund, in einer Admin-Konsole
```

Installiert liegt der Dienst in `%ProgramFiles%\fw-ams-tuner`, Profile und Protokoll in
`%ProgramData%\fw-ams`. `fw-ams-service uninstall --purge` entfernt alles und stellt den
Windows-Energieplan wieder her.

## Slots kalibrieren

Aktive Karten (HDMI, DP, SD, Speicher …) werden über den USB-Root-Port eindeutig einem Slot
zugeordnet. Die Tabelle steht in `crates/fw-ams-core/src/cards/slots.rs`.

1. Eine aktive Karte nacheinander in jeden Slot stecken.
2. Jeweils `fw-ams usb` ausführen und den Root-Port der Karte notieren (z. B. `XHC0.PRT2`).
3. Den Port beim Slot eintragen und als Pull Request (oder Issue mit der Ausgabe) einreichen.

## Release

1. Version in `Cargo.toml`, `app/package.json` und `app/src-tauri/tauri.conf.json` anheben,
   `CHANGELOG.md` ergänzen.
2. Auf `main` committen, Tag setzen und pushen: `git tag v0.2.0 && git push origin v0.2.0`.
3. Der Workflow [`release.yml`](../.github/workflows/release.yml) baut Windows- und
   Linux-Pakete, erzeugt `SHA256SUMS.txt` und veröffentlicht das Release mit dem Abschnitt aus
   dem Changelog.
