//! fw-ams CLI – debugging and hardware calibration tool.

mod smu;
mod spike;

use fw_ams_core::model::{Confidence, Severity, Snapshot};
use fw_ams_core::{mock, Collector};

const USAGE: &str = "\
fw-ams – Framework Advanced Monitoring System

Verwendung:
  fw-ams status [--mock]   Übersicht: Akku, Ladegerät, Thermik
  fw-ams cards  [--mock]   Expansion Cards je Slot
  fw-ams usb               USB-Geräte mit Root-Port (zur Kalibrierung)
  fw-ams dump   [--mock]   kompletter Snapshot als JSON
  fw-ams spike             EC-Rohdaten als JSON
  fw-ams smu info [--mock] CPU, SMU-Version, PM-Tabelle (Tuning-Diagnose)
  fw-ams smu dump [--mock] PM-Tabelle roh als JSON (Offsets zuordnen)";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let use_mock = args.iter().any(|a| a == "--mock");
    let snapshot = || {
        if use_mock {
            mock::snapshot()
        } else {
            Collector::new().snapshot()
        }
    };
    match args.first().map(String::as_str) {
        Some("status") => print_status(&snapshot()),
        Some("cards") => print_cards(&snapshot()),
        Some("usb") => print_usb(),
        Some("dump") => println!("{}", serde_json::to_string_pretty(&snapshot()).unwrap()),
        Some("spike") => spike::run(),
        Some("smu") => smu::run(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn opt<T: std::fmt::Display>(v: Option<T>, unit: &str) -> String {
    v.map(|v| format!("{v}{unit}")).unwrap_or_else(|| "n/v".into())
}

fn print_status(s: &Snapshot) {
    println!(
        "{} – {:?}",
        s.system.product.as_deref().unwrap_or("Unbekanntes Gerät"),
        s.system.model
    );
    println!(
        "  BIOS {}  EC {}  PD {}",
        opt(s.system.bios_version.as_deref(), ""),
        opt(s.system.ec_version.as_deref(), ""),
        s.system.pd_versions.join(" / ")
    );
    println!("  EC-Zugriff: {:?}", s.access.ec);
    if let Some(h) = &s.access.hint {
        println!("  Hinweis: {h}");
    }

    println!("\nAkku");
    match &s.battery {
        Some(b) => {
            println!("  Ladestand     {:.1} %  ({:?})", b.soc_percent, b.state);
            println!(
                "  Leistung      {}  ({} × {})",
                opt(b.power_w.map(|v| format!("{v:+.1}")), " W"),
                opt(b.voltage_v.map(|v| format!("{v:.2}")), " V"),
                opt(b.current_a.map(|v| format!("{v:+.2}")), " A")
            );
            println!("  Restzeit      {}", opt(b.minutes_remaining.map(fmt_minutes), ""));
            println!(
                "  Kapazität     {} / {} mAh (Design {} mAh)",
                opt(b.remaining_mah, ""),
                opt(b.full_mah, ""),
                opt(b.design_mah, "")
            );
            println!(
                "  Gesundheit    {}   Zyklen {}",
                opt(b.health_percent.map(|v| format!("{v:.1}")), " %"),
                opt(b.cycle_count, "")
            );
            println!("  Ladelimit     {}", opt(b.charge_limit_percent, " %"));
            println!("  Temperatur    {}", opt(b.temperature_c.map(|v| format!("{v:.1}")), " °C"));
            println!("  Quelle        {:?}", b.source);
        }
        None => println!("  n/v"),
    }

    println!("\nLadegerät");
    let c = &s.charger;
    println!("  Netzteil      {}", if c.ac_present { "angeschlossen" } else { "nicht angeschlossen" });
    if let Some(port) = c.port {
        let loc = s.ports.iter().find(|p| p.index == port).map(|p| p.location.label.as_str());
        println!(
            "  Port          {} ({})  {} V × {} A = {} W{}",
            port,
            loc.unwrap_or("?"),
            opt(c.voltage_v, ""),
            opt(c.current_a, ""),
            opt(c.contract_w, ""),
            if c.epr { "  EPR" } else { "" }
        );
    }

    if let Some(t) = &s.thermal {
        println!("\nThermik");
        for sensor in &t.sensors {
            println!(
                "  {:<16} {:>8}   ({})",
                sensor.label,
                sensor
                    .celsius
                    .map(|c| format!("{c:.0} °C"))
                    .or(sensor.error.clone())
                    .unwrap_or_default(),
                sensor.name
            );
        }
        for fan in &t.fans {
            println!(
                "  {:<16} {:>8}{}",
                fan.name,
                opt(fan.rpm, " RPM"),
                if fan.stalled { "  BLOCKIERT" } else { "" }
            );
        }
        if let Some(th) = t.throttle {
            println!("  Drosselung       soft: {}  hard: {}", th.soft, th.hard);
        }
    }
}

fn fmt_minutes(m: u32) -> String {
    format!("{}:{:02} h", m / 60, m % 60)
}

fn print_cards(s: &Snapshot) {
    for slot in &s.slots {
        let conf = match slot.card.confidence {
            Confidence::Certain => "",
            Confidence::Likely => " (vermutlich)",
            Confidence::Unknown => " (unklar)",
        };
        println!(
            "Slot {} – {}{}: {}{}",
            slot.number,
            slot.location.label,
            slot.pd_port.map(|p| format!(" [PD {p}]")).unwrap_or_default(),
            slot.card.name,
            conf
        );
        let c = &slot.capabilities;
        println!(
            "  Slot kann:  USB4 {}  max {}  Display {}  Laden {}{}",
            yes_no(c.usb4),
            opt(c.max_gbps, " Gbit/s"),
            yes_no(c.display),
            yes_no(c.charging),
            if c.verified { "" } else { "  (nicht verifiziert)" }
        );
        for f in &slot.card.features {
            println!("  Karte:      {f}");
        }
        for p in &slot.protocols {
            println!("  Aktiv:      {p}");
        }
        for h in &slot.hints {
            let tag = match h.severity {
                Severity::Info => "Hinweis",
                Severity::Warning => "WARNUNG",
            };
            println!("  {tag}:    {}", h.message);
        }
        println!();
    }
}

fn yes_no(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "ja",
        Some(false) => "nein",
        None => "?",
    }
}

fn print_usb() {
    for d in fw_ams_core::source::usb::list() {
        println!(
            "{}  {:<12} {:<10} {}",
            d.id,
            d.root_port.as_deref().unwrap_or("-"),
            format!("{:?}", d.speed),
            d.product.as_deref().unwrap_or("")
        );
    }
}
