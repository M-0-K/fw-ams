# Komponenten Dritter

fw-ams selbst steht unter der [GPL-3.0-or-later](LICENSE). Es nutzt und dankt diesen Projekten:

## Mitgelieferte Dateien

| Komponente | Lizenz | Verwendung |
|---|---|---|
| [PawnIO.Modules](https://github.com/namazso/PawnIO.Modules) – `RyzenSMU.bin` | LGPL-2.1 | Signiertes Modul für den PawnIO-Treiber, über das der Hilfsdienst die SMU anspricht. Unverändert mitgeliefert, Lizenztext liegt als `resources/PawnIO.Modules-COPYING.txt` im Installationsordner. |

Der PawnIO-Treiber selbst ([pawnio.eu](https://pawnio.eu)) wird nicht mitgeliefert, sondern von
dir separat installiert.

## Wichtige Bibliotheken

| Projekt | Lizenz | Wofür |
|---|---|---|
| [framework-system / framework_lib](https://github.com/FrameworkComputer/framework-system) | BSD-3-Clause | Zugriff auf den Embedded Controller, SMBIOS |
| [Tauri](https://tauri.app) | MIT oder Apache-2.0 | App-Rahmen, Tray, Fenster |
| [nusb](https://github.com/kevinmehall/nusb) | MIT oder Apache-2.0 | USB-Geräteliste für die Kartenerkennung |
| [starship-battery](https://github.com/starship/rust-battery) | ISC | Akkuwerte des Betriebssystems ohne EC |
| [windows-rs / windows-sys](https://github.com/microsoft/windows-rs), [windows-service](https://github.com/mullvad/windows-service-rs) | MIT oder Apache-2.0 | Windows-APIs, Dienst |
| [ts-rs](https://github.com/Aleph-Alpha/ts-rs), [serde](https://serde.rs) | MIT (oder Apache-2.0) | Datenmodell Rust ↔ TypeScript |
| [React](https://react.dev), [Tailwind CSS](https://tailwindcss.com), [Vite](https://vite.dev) | MIT | Oberfläche |

Alle weiteren Rust-Abhängigkeiten stehen unter permissiven Lizenzen (MIT, Apache-2.0, BSD, ISC,
Zlib, Unicode-3.0) oder der MPL-2.0; die vollständige Liste liefert `cargo metadata` bzw.
`Cargo.lock` und `app/package-lock.json`.

## Wissen aus anderen Projekten

- [RyzenAdj](https://github.com/FlyGoat/RyzenAdj) (LGPL-3.0): SMU-Message-IDs, Mailbox-Adressen
  und PM-Tabellen-Offsets je CPU-Familie dienten als Referenz für `crates/fw-ams-tuner`.
- Framework Knowledge Base, Arch- und Gentoo-Wiki: USB-IDs der Expansion Cards, Slot-Fähigkeiten
  und Standby-Verbrauch je Slot.

„Framework“ ist eine Marke von Framework Computer Inc. fw-ams ist ein unabhängiges
Community-Projekt.
