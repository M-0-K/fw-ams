# Changelog

Alle nennenswerten Änderungen. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/),
Versionen nach [Semantic Versioning](https://semver.org/lang/de/).

## [0.2.0] – 2026-11-12

### Neu
- Seite „System“ zeigt Betriebssystem (Windows-Edition, Version, Build bzw. Linux-Distribution
  und Kernel) und die fw-ams-Version (#2)

### Behoben
- Live-Diagramme zeigen einen festen Zeitraum statt einer festen Anzahl Messpunkte; Lücken
  (z. B. Ruhezustand) werden nicht mehr überbrückt. Zeitraum wählbar: 1, 5, 10 oder 30 min (#1)
- Linux ohne root: Framework-Laptops wurden nicht erkannt (Modell, BIOS, EC-Zugriff fehlten), weil
  die SMBIOS-Tabelle nur für root lesbar ist. Die Angaben kommen jetzt aus `/sys/class/dmi/id` (#3)
- Nach längerem Standby blieb die App ohne EC-Daten hängen, bis sie neu gestartet wurde, und
  das Ladelimit wurde nicht mehr gesetzt. Ursache war ein Fehler in `framework_lib`: ein einziger
  fehlgeschlagener Treiberaufruf legte den EC-Zugriff bis zum Neustart lahm. Jetzt verbindet sich
  fw-ams neu, erkennt das Aufwachen und setzt Ladelimit und Ladestrom erneut; meldet der EC ein
  anderes Ladelimit als eingestellt, wird es neu geschrieben (#4)

## [0.1.0] – 2026-10-20

Erste öffentliche Version.

### Überwachung
- Akku: Ladestand, Leistung, Restlaufzeit, Gesundheit, Ladezyklen, Temperatur
- USB-C Power Delivery je Port: Rolle, Spannung, Strom, Leistung, EPR, DisplayPort Alt Mode
- Expansion Cards je Slot mit Draufsicht, Slot-Fähigkeiten und Hinweisen; Kartengedächtnis für
  passive USB-A/USB-C-Karten, die sich nicht melden
- Thermik: Sensoren, Lüfter, Drosselung
- Tray-Schnellansicht beim Überfahren des Symbols (Linux: über das Tray-Menü)
- Benachrichtigungen: Akku fast leer/kritisch, Ladelimit, schwaches Ladegerät, Drosselung,
  hohe Temperatur, Kartenwechsel
- Hell/Dunkel/System, Autostart

### Erweiterte Funktionen (Windows, abschaltbar)
- Ladelimit und Ladestrom über den Embedded Controller, „einmalig voll laden“
- Automatischer Energiesparmodus bei niedrigem Akku
- CPU-Tuning für Ryzen AI 300 und Ryzen AI Max über den Hilfsdienst und PawnIO: Powerlimits,
  Tctl, Curve Optimizer (mit Absturzschutz), max. Takt, Boost, EPP – je Netzteil, Akku, Sparmodus
- Live-Analyse der letzten 10 Minuten
- Vollständiges Zurücksetzen: Erweiterte Funktionen aus, Werkseinstellungen und Deinstallation
  stellen alle Originalwerte wieder her

[0.2.0]: https://github.com/M-0-K/fw-ams/releases/tag/v0.2.0
[0.1.0]: https://github.com/M-0-K/fw-ams/releases/tag/v0.1.0
