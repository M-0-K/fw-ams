# Erweiterte Funktionen unter Linux – Entwicklernotizen

Stand: Die Seite „Erweitert“ (CPU-Tuner, Akkupflege) ist bisher nur für Windows umgesetzt.
Diese Notiz beschreibt, wie jede Funktion unter Linux abgebildet wird, damit die Portierung
ohne erneute Recherche möglich ist. Die Architektur ist dafür vorbereitet: Engine, Statusmodell,
Protokoll und UI sind plattformneutral, nur die Backends fehlen.

## Was schon plattformneutral ist

| Teil | Datei | Linux-Aufwand |
|---|---|---|
| Statusmodell (Standard/Gesendet/Ist/Status), Override-Erkennung, Ereignislog, CO-Testlauf | `crates/fw-ams-tuner/src/engine.rs` | keiner |
| PM-Tabellen-Parser, Message-IDs je CPU-Familie | `pm_table.rs`, `family.rs` | keiner |
| Pipe-Protokoll (JSON-Zeilen) | `protocol.rs` | Transport tauschen (Unix-Socket) |
| UI inkl. Diagramme | `app/src/pages/Advanced.tsx`, `app/src/pages/advanced/*` | keiner |
| Akku-Warnstufen, Sparmodus-Zustandsmaschine | `fw-ams-core/src/{alerts,saver}.rs` | keiner |
| Ladelimit über den EC | `fw-ams-core/src/source/ec.rs` | läuft schon, wenn `/dev/cros_ec` zugänglich ist (udev-Regel) |

## Abbildung je Funktion

### SMU (Powerlimits, Tctl, Curve Optimizer, PM-Tabelle)
- Kernelmodul **ryzen_smu** (https://github.com/amkillam/ryzen_smu, DKMS). RyzenAdj unterstützt es ab v0.17 vollständig.
- sysfs unter `/sys/kernel/ryzen_smu_drv/`:
  - `version`, `codename`, `pm_table_version`, `pm_table_size`, `pm_table` (binär, f32 LE)
  - `smu_args` (6 × u32 LE schreiben/lesen), `mp1_smu_cmd`, `rsmu_cmd` (Befehl schreiben, Antwort lesen)
- Umsetzung: neues `smu/ryzen_smu.rs` mit `impl SmuBackend`. Der Trait verlangt heute Register-
  Zugriff (`read_register`/`write_register`) und baut `send` darauf auf. Unter Linux stattdessen
  `send` überschreiben: Argumente nach `smu_args`, Befehl nach `mp1_smu_cmd`, Antwort lesen
  (`0x01` = OK), Argumente zurücklesen. `pm_table()` liest einfach die Datei `pm_table`.
- `open_smu()` in `lib.rs` hat bereits den `#[cfg(not(windows))]`-Zweig – dort das neue Backend öffnen.
- Achtung: Secure Boot verhindert unsignierte Module; Nutzer müssen ryzen_smu per MOK signieren.

### Takt, Boost, EPP (statt Windows-Energieplan)
- `amd_pstate` im Modus `active` (Standard ab Kernel 6.5 auf Zen 4/5):
  - `/sys/devices/system/cpu/cpu*/cpufreq/scaling_max_freq` (kHz) → `MaxFreqMhz`
  - `/sys/devices/system/cpu/cpu*/cpufreq/energy_performance_preference`
    (`performance`, `balance_performance`, `balance_power`, `power`) → `Epp` auf 4 Stufen abbilden
  - `/sys/devices/system/cpu/cpufreq/boost` (0/1) → `Boost` (nur aus/an)
- Umsetzung: `LinuxPowerPlan` als `impl PowerPlan` in `winpower.rs` (oder eigene Datei). In
  `family::specs()` die Bereiche für Linux anpassen (Boost 0–1, EPP in 4 Stufen).

### Energiemodus (statt Windows-Overlay)
- `/sys/firmware/acpi/platform_profile` mit `/sys/firmware/acpi/platform_profile_choices`
  (`low-power`, `balanced`, `performance`). Framework ordnet die Profile wie unter Windows den
  Firmware-Limits zu (Phoronix-Test FW13 AI 300). Alternativ über power-profiles-daemon (D-Bus
  `net.hadess.PowerProfiles`), dann ohne Root.
- `winpower::power_mode()`/`set_power_mode()` haben schon `#[cfg(not(windows))]`-Stubs.

### Live-Telemetrie ohne Treiber
- CPU-Last: `/proc/stat` (Differenz zweier Messungen), pro Thread die `cpuN`-Zeilen.
- Effektiver Takt: `scaling_cur_freq` je CPU (bei amd_pstate brauchbar), sonst `/proc/cpuinfo`.
- Paketleistung ohne ryzen_smu: RAPL über powercap, `/sys/class/powercap/intel-rapl:0/energy_uj`
  (funktioniert auch auf AMD Zen). Seit CVE-2020-8694 nur für root lesbar, also im Dienst lesen.
- Umsetzung: `oscpu.rs` hat einen `#[cfg(not(windows))]`-Stub `OsCpuMonitor` – dort `/proc/stat`
  und `scaling_cur_freq` einsetzen.

### Ladelimit, Ladestrom
- Mit EC-Zugriff (udev-Regel `packaging/linux/99-fw-ams.rules`) funktioniert `ChargeController`
  unverändert über framework_lib.
- Ohne EC: Kernel ≥ 6.12 mit `cros_charge-control` stellt
  `/sys/class/power_supply/BAT1/charge_control_{start,end}_threshold` bereit (start = Minimum,
  wird hier tatsächlich beachtet). Lesen passiert bereits in `source/linux.rs`; Schreiben braucht
  root oder eine udev-Regel mit `MODE="0664"`/Gruppe.

### Rechte (statt Windows-Dienst)
- Gleiches Muster wie unter Windows: kleiner Root-Dienst, die App bleibt unprivilegiert.
- `fw-ams-service` bekommt einen Linux-Zweig: systemd-Unit (`packaging/linux/fw-ams-tuner.service`,
  `Type=simple`, `ExecStart=/usr/lib/fw-ams/fw-ams-service run`) und einen Unix-Socket
  `/run/fw-ams-tuner.sock` mit Gruppe `fw-ams` (oder Socket-Activation). Protokoll unverändert
  (`protocol.rs`, eine JSON-Zeile pro Anfrage).
- Client: `client.rs` ist heute `#[cfg(windows)]`; für Linux `std::os::unix::net::UnixStream`
  mit identischem `request()`.
- Installation über das deb/rpm-Paket statt UAC (`tauri.conf.json` → `bundle.linux.deb.files`).
- Alternative ohne Dauerdienst: polkit + `pkexec` für einzelne Schreibvorgänge. Nachteil: keine
  Reapply-Schleife, wenn die Firmware Limits zurücksetzt – daher Dienst bevorzugen.

## Reihenfolge für die Portierung
1. `OsCpuMonitor` für Linux (Live-Takt/-Last, kein Root nötig).
2. `power_mode()` über platform_profile / power-profiles-daemon.
3. Dienst + Unix-Socket + systemd-Unit, `LinuxPowerPlan`.
4. ryzen_smu-Backend, PM-Tabelle mit `fw-ams smu dump` gegen RyzenAdj `--info` abgleichen.
5. In `tuner.rs` (App) die `cfg(windows)`-Zweige für `setup()`, `install()`, `service_request()` ergänzen.

## Quellen
- RyzenAdj: https://github.com/FlyGoat/RyzenAdj (`lib/api.c`, `lib/nb_smu_ops.c`)
- ryzen_smu: https://github.com/amkillam/ryzen_smu
- Phoronix, Framework 13 AI 300 Power-Tuning: https://www.phoronix.com/review/framework-13-ryzen-ai-power
- amd_pstate: https://docs.kernel.org/admin-guide/pm/amd-pstate.html
