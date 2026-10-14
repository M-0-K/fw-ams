# fw-ams

Framework Advanced Monitoring System – Akku, USB-C Power Delivery, Thermik und
Expansion-Card-Erkennung für Framework Laptops, unter Windows und Linux.

## Aufbau

| Pfad | Inhalt |
|---|---|
| `crates/fw-ams-core` | Hardware-Zugriff (`framework_lib`, `nusb`, sysfs) und Datenmodell |
| `crates/fw-ams-cli`  | CLI `fw-ams` zum Debuggen und Kalibrieren |
| `app/`               | Tauri-v2-App (Tray, UI) |

## CLI

```
cargo run -p fw-ams-cli -- status   # Akku, Ladegerät, Thermik
cargo run -p fw-ams-cli -- cards    # Expansion Cards je Slot
cargo run -p fw-ams-cli -- usb      # USB-Geräte mit ACPI-Root-Port
cargo run -p fw-ams-cli -- dump     # Snapshot als JSON
```

`--mock` nutzt aufgezeichnete Daten statt Hardware.

## EC-Zugriff

- **Windows:** Framework-EC-Treiber (im Framework Driver Bundle, aktuelles BIOS). Keine Admin-Rechte nötig.
- **Linux:** Kernel-Modul `cros_ec_lpcs`, Zugriff auf `/dev/cros_ec` über `packaging/linux/99-fw-ams.rules`.
  Ohne EC-Zugriff werden sysfs/hwmon-Werte verwendet.

## Kalibrierung der Slots

Die Zuordnung USB-Root-Port → Slot steht in `crates/fw-ams-core/src/cards/slots.rs`.
Eine aktive Karte (HDMI/DP/SD/…) nacheinander in jeden Slot stecken, `fw-ams usb` ausführen
und den Root-Port (z. B. `XHC0.PRT2`) beim Slot eintragen.
