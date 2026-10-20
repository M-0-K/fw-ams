<div align="center">

# fw-ams

**Framework Advanced Monitoring System**

Akku, USB-C Power Delivery, Thermik und Expansion Cards deines Framework Laptops auf einen Blick –
mit Tray-Schnellansicht, Benachrichtigungen, Akkupflege und optionalem CPU-Tuning.

[![Build](https://github.com/M-0-K/fw-ams/actions/workflows/build.yml/badge.svg)](https://github.com/M-0-K/fw-ams/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/M-0-K/fw-ams?include_prereleases&label=Release)](https://github.com/M-0-K/fw-ams/releases/latest)
[![Lizenz: GPL v3](https://img.shields.io/badge/Lizenz-GPLv3-blue.svg)](LICENSE)
![Windows | Linux](https://img.shields.io/badge/Plattform-Windows%20%7C%20Linux-lightgrey)

[**Download**](https://github.com/M-0-K/fw-ams/releases/latest) ·
[Funktionen](#funktionen) ·
[Installation](#installation) ·
[Zurücksetzen](#zurücksetzen-und-deinstallieren) ·
[Mitmachen](CONTRIBUTING.md)

</div>

> **English summary:** fw-ams is a free, open source (GPL-3.0) monitoring app for Framework laptops
> on Windows and Linux. It shows battery health and charge state, USB-C PD contracts per port,
> temperatures and fans, and which expansion card sits in which slot. Optional extras (Windows):
> charge limit and charge current, a low-battery power saver, and AMD Ryzen AI power limits, Tctl
> and Curve Optimizer through a small helper service. Everything can be reverted with one click.
> The user interface is in German.

> [!NOTE]
> fw-ams ist ein Community-Projekt und steht in keiner Verbindung zu Framework Computer Inc.
> „Framework“ ist eine Marke von Framework Computer Inc.

## Funktionen

### Überwachung (ohne Adminrechte, Windows und Linux)

| | |
|---|---|
| **Akku** | Ladestand, Lade-/Entladeleistung, Restlaufzeit bzw. Zeit bis zum Ladelimit, Gesundheit (Kapazität zu Neuzustand), Ladezyklen, Temperatur |
| **USB-C Power Delivery** | Je Port: Ladegerät oder Verbraucher, ausgehandelte Spannung, Strom und Leistung, EPR, DisplayPort Alt Mode, VCONN, Datenrolle |
| **Expansion Cards** | Welche Karte in welchem Slot steckt – als Draufsicht auf den Laptop, mit den Fähigkeiten des Slots (USB4, Display, Laden) und Hinweisen wie „USB-A hinten erhöht den Standby-Verbrauch“ |
| **Thermik** | Alle EC-Temperatursensoren, Lüfterdrehzahl, Drosselung (Leistungslimit, PROCHOT) |
| **Tray-Schnellansicht** | Mit der Maus über das Tray-Symbol fahren: Ladestand, Leistung je Port und alle Slots auf einen Blick |
| **Benachrichtigungen** | Akku fast leer / kritisch, Ladelimit erreicht, schwaches Ladegerät, Drosselung, hohe Temperatur, Karte gesteckt/entfernt |

**Karten merken:** Passive Karten melden sich nicht immer – eine USB-C-Karte ist ohne Gerät
unsichtbar, eine USB-A-Karte nach dem Standby oft auch. fw-ams merkt sich, was zuletzt sicher in
welchem Slot erkannt wurde, und zeigt es weiter an (blass mit „↺“).

### Erweiterte Funktionen (optional, Windows)

Diese Funktionen verändern das System und lassen sich unter **Einstellungen → Erweiterte
Funktionen** komplett abschalten.

- **Akkupflege:** Ladelimit (z. B. 80 %) und schonender Ladestrom über den Embedded Controller,
  „einmalig voll laden“ vor einer Reise
- **Energiesparmodus:** Unter einem einstellbaren Akkustand automatisch „Beste Energieeffizienz“
  und ein Sparprofil, danach zurück zum vorherigen Modus
- **CPU-Tuning** (AMD Ryzen AI 300 „Strix/Krackan Point“, Ryzen AI Max „Strix Halo“): Powerlimits
  (STAPM, PPT Fast/Slow), Temperaturgrenze, Curve Optimizer, max. Takt, Boost, EPP – getrennt
  für Netzteil, Akku und Sparmodus
- **Live-Analyse:** Leistungsaufnahme, Temperatur und Takt der letzten 10 Minuten, welches Limit
  gerade bremst, wohin die Leistung geht

Jeder Wert zeigt transparent **Standard, Eingestellt, Gesendet und Ist (Hardware)** und einen
Status wie „übernommen“, „von der Firmware begrenzt“ oder „überschrieben und wiederhergestellt“.

## Unterstützte Geräte

| Gerät | Überwachung | Kartenerkennung | CPU-Tuning |
|---|---|---|---|
| Laptop 13 (AMD Ryzen AI 300) | ✓ | ✓ kalibriert | ✓ Windows |
| Laptop 13 (AMD Ryzen 7040) | ✓ | ✓ | – |
| Laptop 13 (Intel 11.–13. Gen, Core Ultra) | ✓ | ✓ | – |
| Laptop 16 (AMD 7040, AI 300) | ✓ | ✓ | ✓ AI 300, Windows |
| Laptop 12 | ✓ | ✓ | – |
| Desktop (Ryzen AI Max) | ✓ | – | ✓ Windows |

„Kalibriert“ heißt: Aktive Karten werden über ihren USB-Port eindeutig einem Slot zugeordnet.
Auf anderen Modellen geschieht das über den USB-C-Port-Status – hilf gern beim
[Kalibrieren](docs/development.md#slots-kalibrieren)! Andere Geräte zeigen die allgemeinen
Akkuwerte des Betriebssystems.

## Installation

### Windows 10/11

1. **Framework Driver Bundle** und ein aktuelles BIOS installieren (enthält den EC-Treiber, über
   den fw-ams ohne Adminrechte liest) – [Framework Support](https://knowledgebase.frame.work/).
2. `fw-ams_<version>_x64-setup.exe` aus den
   [Releases](https://github.com/M-0-K/fw-ams/releases/latest) herunterladen und ausführen.
3. fw-ams startet im Tray. Unter Einstellungen kannst du „Mit dem System starten“ einschalten.

> [!IMPORTANT]
> Der Installer ist (noch) nicht code-signiert. Windows SmartScreen zeigt deshalb eine Warnung:
> **Weitere Informationen → Trotzdem ausführen**. Die SHA-256-Prüfsummen stehen in
> `SHA256SUMS.txt` beim Release.

**Optional für CPU-Tuning:** Unter Erweitert führt die App durch die Einrichtung:
[PawnIO](https://pawnio.eu) installieren (signierter Treiber, Nachfolger von WinRing0), dann
„Hilfsdienst installieren“ (eine Admin-Abfrage).

### Linux

Pakete für Debian/Ubuntu (`.deb`), Fedora/openSUSE (`.rpm`) und alle anderen (`.AppImage`) gibt es
bei den [Releases](https://github.com/M-0-K/fw-ams/releases/latest).

```sh
sudo apt install ./fw-ams_<version>_amd64.deb      # Debian, Ubuntu
sudo dnf install ./fw-ams-<version>-1.x86_64.rpm   # Fedora
```

Für Zugriff auf den Embedded Controller braucht es das Kernel-Modul `cros_ec_lpcs` (ab Linux 5.19)
und eine udev-Regel. `.deb` und `.rpm` installieren sie mit; beim AppImage einmalig:

```sh
echo 'KERNEL=="cros_ec", SUBSYSTEM=="chromeos", TAG+="uaccess"' | sudo tee /etc/udev/rules.d/99-fw-ams.rules
sudo udevadm control --reload && sudo udevadm trigger
```

Ohne EC-Zugriff zeigt fw-ams die Werte aus sysfs/hwmon. Die erweiterten Funktionen gibt es unter
Linux noch nicht ([Stand der Portierung](docs/linux-advanced.md)).

## Sicherheit und Verantwortung

- fw-ams verändert **nichts**, solange du es nicht einschaltest. Die Überwachung liest nur.
- Ladelimit und Ladestrom setzen dieselben Werte wie die BIOS-Einstellung bzw. `framework_tool`.
- CPU-Tuning wirkt direkt auf die Hardware. Zu hohe Powerlimits erhöhen Temperatur und Lautstärke,
  zu viel Curve Optimizer führt zu Abstürzen. fw-ams begrenzt alle Werte auf sichere Bereiche,
  testet einen neuen Curve-Optimizer-Wert zwei Minuten lang und wendet ihn nach einem Absturz
  **nie** automatisch wieder an.
- Der Hilfsdienst nimmt nur typisierte, geprüfte Werte an – nie rohe Befehle – und ist nur für
  Administratoren und den installierenden Benutzer erreichbar.
- Manche Anti-Cheat-Systeme in Spielen bemängeln geladene Low-Level-Treiber wie PawnIO.

Die Nutzung erfolgt auf eigene Verantwortung; es gibt keine Gewährleistung (siehe [Lizenz](LICENSE)).
Sicherheitslücken bitte vertraulich melden: [SECURITY.md](SECURITY.md).

## Zurücksetzen und Deinstallieren

fw-ams merkt sich jeden Originalwert, bevor es ihn ändert, und kann alles zurückdrehen:

- **Einstellungen → Erweiterte Funktionen ausschalten:** CPU-Profile leeren, Powerlimits, Curve
  Optimizer und Windows-Energieplan auf die Werte von vorher, ursprüngliches Ladelimit zurück in
  den EC, Ladestrom ohne Limit, Sparmodus beenden; optional den Hilfsdienst entfernen. Danach
  überwacht fw-ams nur noch.
- **Einstellungen → Werkseinstellungen:** zusätzlich alle Einstellungen auf Standard, Autostart
  entfernen, gemerkte Karten löschen.
- **App deinstallieren** (Windows → Apps): erledigt dasselbe automatisch, auch den Hilfsdienst
  (eine Admin-Abfrage). Bei Updates bleibt alles erhalten.

Jeder Schritt wird mit Ergebnis angezeigt. Den separat installierten PawnIO-Treiber entfernt
fw-ams nicht – bei Bedarf unter Windows → Apps deinstallieren.

## Datenschutz

fw-ams arbeitet vollständig lokal: keine Telemetrie, keine Konten, keine Netzwerkverbindungen.
Gespeichert wird nur:

| Datei | Inhalt |
|---|---|
| `%APPDATA%\io.github.fw-ams\` bzw. `~/.config/io.github.fw-ams/` | `settings.json` (Einstellungen), `cards.json` (gemerkte Karten), `charge-restore.json` (Ladelimit vor fw-ams) |
| `%ProgramData%\fw-ams\` (nur mit Hilfsdienst) | Tuning-Profile, Originalwerte des Energieplans, Dienst-Protokoll |

## Häufige Fragen

**„Kein Zugriff auf den Framework-EC“** – Unter Windows das Framework Driver Bundle und ein
aktuelles BIOS installieren, unter Linux `cros_ec_lpcs` laden und die udev-Regel einrichten
(siehe oben), dann neu anmelden.

**Eine Karte wird falsch oder gar nicht angezeigt** – Passive Karten (USB-A, USB-C) sind ohne
Gerät für den Laptop unsichtbar; fw-ams zeigt sie nach der ersten sicheren Erkennung weiter an.
Nach dem Umstecken unter Ports & Karten „Slot vergessen“. Falsch zugeordnete aktive Karten bitte
als [Issue](https://github.com/M-0-K/fw-ams/issues/new/choose) melden – mit der Ausgabe von
`fw-ams usb` ([CLI](docs/development.md#cli)), falls möglich.

**Ein Tuning-Wert steht auf „begrenzt“** – BIOS bzw. Firmware lassen diesen Wert nicht zu, die
Hardware hält ihren eigenen. Das ist kein Fehler von fw-ams.

**Gibt es eine englische Oberfläche?** – Noch nicht. Übersetzungen sind willkommen,
siehe [CONTRIBUTING.md](CONTRIBUTING.md).

## Mitmachen

Fehlerberichte, Messdaten von anderen Modellen und Pull Requests sind willkommen –
siehe [CONTRIBUTING.md](CONTRIBUTING.md) und [docs/development.md](docs/development.md)
(Bauen, Aufbau, CLI, Slots kalibrieren).

## Lizenz

Copyright © 2026 Moritz Kockert und Mitwirkende.

fw-ams ist freie Software: Du kannst sie unter den Bedingungen der
[GNU General Public License](LICENSE), Version 3 oder (nach deiner Wahl) jeder späteren Version,
weitergeben und verändern. Sie wird ohne jede Gewährleistung bereitgestellt.

Verwendete Komponenten Dritter und ihre Lizenzen: [THIRD-PARTY.md](THIRD-PARTY.md).
