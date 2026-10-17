//! `fw-ams smu …`: read-only SMU diagnostics (CPU, SMU version, PM table).
//!
//! Uses the running fw-ams-service when available, otherwise opens PawnIO directly
//! (needs an elevated prompt).

use fw_ams_tuner::pm_table::{self, read_f32};
use fw_ams_tuner::smu::{format_smu_version, mock::MockSmu, SmuBackend};
use fw_ams_tuner::{family, CpuFamily};

fn backend(use_mock: bool) -> Result<Box<dyn SmuBackend>, String> {
    if use_mock {
        return Ok(Box::new(MockSmu::default()));
    }
    fw_ams_tuner::open_smu().map_err(|e| e.to_string())
}

#[cfg(windows)]
fn table_from_service() -> Option<(u32, Vec<u8>)> {
    use fw_ams_tuner::protocol::{Request, Response};
    match fw_ams_tuner::client::request(&Request::PmDump) {
        Ok(Response::PmDump { version, data }) => Some((version, data)),
        _ => None,
    }
}

#[cfg(not(windows))]
fn table_from_service() -> Option<(u32, Vec<u8>)> {
    None
}

fn table(use_mock: bool) -> Result<(u32, Vec<u8>), String> {
    if !use_mock {
        if let Some(t) = table_from_service() {
            return Ok(t);
        }
    }
    backend(use_mock)?.pm_table().map_err(|e| e.to_string())
}

pub fn run(args: &[String]) {
    let use_mock = args.iter().any(|a| a == "--mock");
    let result = match args.first().map(String::as_str) {
        Some("info") => info(use_mock),
        Some("dump") => dump(use_mock),
        _ => Err("Verwendung: fw-ams smu info|dump [--mock]".into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn info(use_mock: bool) -> Result<(), String> {
    let id = family::cpuid();
    println!("CPU           {}", id.name);
    println!(
        "CPUID         Familie 0x{:X}, Modell 0x{:X} → {:?}",
        id.family,
        id.model,
        family::family_from_cpuid(id.family, id.model)
    );
    match backend(use_mock) {
        Ok(mut b) => {
            let code = b.code_name().ok();
            println!(
                "PawnIO-Modul  Codename {} → {:?}",
                code.map(|c| c.to_string()).unwrap_or("n/v".into()),
                code.map(family::family_from_code_name).unwrap_or(CpuFamily::Other)
            );
            println!(
                "SMU-Version   {}",
                b.smu_version().map(format_smu_version).unwrap_or("n/v".into())
            );
        }
        Err(e) => println!("Direkter SMU-Zugriff: {e}"),
    }
    let (version, t) = table(use_mock)?;
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    let s = pm_table::parse(version, &t, threads / 2, 0);
    println!(
        "PM-Tabelle    0x{version:06X} ({})",
        if pm_table::is_fully_known(version) {
            "vollständig bekannt"
        } else {
            "nur Limits/Tctl bekannt"
        }
    );
    let f = |v: Option<f32>| v.map(|v| format!("{v:.2}")).unwrap_or("n/v".into());
    println!("  STAPM       Limit {} W, Wert {} W", f(s.stapm.limit), f(s.stapm.value));
    println!("  PPT Fast    Limit {} W, Wert {} W", f(s.fast.limit), f(s.fast.value));
    println!("  PPT Slow    Limit {} W, Wert {} W", f(s.slow.limit), f(s.slow.value));
    println!("  Tctl        Limit {} °C, Wert {} °C", f(s.tctl.limit), f(s.tctl.value));
    println!("  Socket      {} W", f(s.socket_w));
    for (i, c) in s.cores.iter().enumerate() {
        println!(
            "  Kern {i:2}     {} MHz, {} V, {} W, {} °C",
            f(c.mhz),
            f(c.volt),
            f(c.power_w),
            f(c.temp_c)
        );
    }
    Ok(())
}

/// All non-zero floats with their offset, for mapping unknown table versions
fn dump(use_mock: bool) -> Result<(), String> {
    let (version, t) = table(use_mock)?;
    let values: Vec<serde_json::Value> = (0..t.len() / 4)
        .filter_map(|i| {
            let v = read_f32(&t, i * 4)?;
            (v != 0.0).then(|| serde_json::json!({ "offset": format!("0x{:03X}", i * 4), "value": v }))
        })
        .collect();
    let out = serde_json::json!({
        "version": format!("0x{version:06X}"),
        "cpu": family::cpuid().name,
        "values": values,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    Ok(())
}
