//! App side of the CPU tuner: talks to fw-ams-service (or a simulated engine with `--mock`),
//! samples OS CPU counters and keeps a short history for the live charts.

use fw_ams_core::model::{ChargeState, Snapshot};
use fw_ams_tuner::oscpu::{self, OsCpuMonitor};
use fw_ams_tuner::protocol::{Request, Response};
use fw_ams_tuner::winpower::{self, PowerMode};
use fw_ams_tuner::{now_ms, Availability, Engine, OsCpuTelemetry, TunerConfig, TunerState};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;
use ts_rs::TS;

/// 10 minutes at one sample per second
const HISTORY: usize = 600;

/// What is needed to enable SMU tuning on this machine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SetupInfo {
    pub platform_supported: bool,
    pub pawnio_installed: bool,
    pub service_exe_found: bool,
    pub module_found: bool,
    pub service_installed: bool,
    pub service_running: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TunerView {
    pub state: TunerState,
    pub os: OsCpuTelemetry,
    /// Windows power mode (Efficiency/Balanced/Performance)
    pub power_mode: Option<PowerMode>,
    pub setup: SetupInfo,
}

/// One point of the live charts. Powers in W, positive = consumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistorySample {
    #[ts(type = "number")]
    pub t: u64,
    /// Whole laptop – only measurable on battery (discharge power)
    pub system_w: Option<f32>,
    /// Battery charge power on AC
    pub charge_w: Option<f32>,
    /// Current APU power (PPT fast value)
    pub apu_w: Option<f32>,
    /// Time-averaged APU power (STAPM value)
    pub stapm_w: Option<f32>,
    /// Sum of core powers (only where the PM table layout is known)
    pub cores_w: Option<f32>,
    pub stapm_limit: Option<f32>,
    pub fast_limit: Option<f32>,
    pub slow_limit: Option<f32>,
    pub tctl_c: Option<f32>,
    pub tctl_limit: Option<f32>,
    pub usage: Option<f32>,
    pub avg_mhz: Option<f32>,
    pub max_mhz: Option<f32>,
}

enum Backend {
    Mock { engine: Mutex<Engine>, start: Instant },
    Service,
}

pub struct Tuner {
    backend: Backend,
    os: Mutex<Option<OsCpuMonitor>>,
    history: Mutex<VecDeque<HistorySample>>,
    latest: Mutex<Option<TunerView>>,
    service_exe: Option<PathBuf>,
    module: Option<PathBuf>,
}

impl Tuner {
    pub fn new(mock: bool, service_exe: Option<PathBuf>, module: Option<PathBuf>) -> Self {
        let backend = if mock {
            Backend::Mock {
                engine: Mutex::new(fw_ams_tuner::mock_engine()),
                start: Instant::now(),
            }
        } else {
            Backend::Service
        };
        Self {
            backend,
            os: Mutex::new(if mock { None } else { OsCpuMonitor::new() }),
            history: Mutex::new(VecDeque::with_capacity(HISTORY)),
            latest: Mutex::new(None),
            service_exe,
            module,
        }
    }

    fn request(&self, req: Request) -> Result<TunerState, String> {
        match &self.backend {
            Backend::Mock { engine, .. } => {
                let mut e = engine.lock().unwrap();
                match req {
                    Request::GetState | Request::PmDump => {}
                    Request::SetConfig { config } => e.set_config(config),
                    Request::SetSaver { active } => e.set_saver(active),
                    Request::Reset => e.reset(),
                }
                Ok(e.state())
            }
            Backend::Service => service_request(req),
        }
    }

    pub fn set_config(&self, config: TunerConfig) -> Result<TunerState, String> {
        self.request(Request::SetConfig { config })
    }

    pub fn reset(&self) -> Result<TunerState, String> {
        self.request(Request::Reset)
    }

    pub fn set_saver(&self, active: bool) {
        if let Err(e) = self.request(Request::SetSaver { active }) {
            log::info!("tuner saver not forwarded: {e}");
        }
    }

    pub fn latest(&self) -> Option<TunerView> {
        self.latest.lock().unwrap().clone()
    }

    pub fn history(&self) -> Vec<HistorySample> {
        self.history.lock().unwrap().iter().cloned().collect()
    }

    pub fn setup(&self) -> SetupInfo {
        #[cfg(windows)]
        {
            use fw_ams_tuner::client::{service_status, ServiceStatus};
            let status = service_status();
            SetupInfo {
                platform_supported: true,
                pawnio_installed: fw_ams_tuner::smu::pawnio::driver_installed(),
                service_exe_found: self.service_exe.as_ref().is_some_and(|p| p.is_file()),
                module_found: self.module.as_ref().is_some_and(|p| p.is_file()),
                service_installed: !matches!(status, ServiceStatus::NotInstalled),
                service_running: status == ServiceStatus::Running,
            }
        }
        #[cfg(not(windows))]
        {
            SetupInfo {
                platform_supported: false,
                pawnio_installed: false,
                service_exe_found: false,
                module_found: false,
                service_installed: false,
                service_running: false,
            }
        }
    }

    /// One sampling step (about once per second)
    pub fn tick(&self, snapshot: Option<&Snapshot>) -> (TunerView, HistorySample) {
        let (state, os) = match &self.backend {
            Backend::Mock { engine, start } => {
                let mut e = engine.lock().unwrap();
                e.tick(snapshot.map(|s| s.charger.ac_present));
                let t = start.elapsed().as_secs_f32();
                (e.state(), oscpu::mock_sample(t, 16))
            }
            Backend::Service => {
                let state = service_request(Request::GetState).unwrap_or_else(|e| self.unavailable(e));
                let os = self.os.lock().unwrap().as_mut().map(|m| m.sample()).unwrap_or_default();
                (state, os)
            }
        };
        let view = TunerView {
            state,
            os,
            power_mode: winpower::power_mode(),
            setup: self.setup(),
        };
        let sample = sample(&view, snapshot);
        {
            let mut h = self.history.lock().unwrap();
            if h.len() >= HISTORY {
                h.pop_front();
            }
            h.push_back(sample.clone());
        }
        *self.latest.lock().unwrap() = Some(view.clone());
        (view, sample)
    }

    fn unavailable(&self, error: String) -> TunerState {
        if !cfg!(windows) {
            return TunerState::unavailable(
                Availability::PlatformUnsupported,
                "CPU-Tuning ist bisher nur unter Windows verfügbar.",
            );
        }
        let setup = self.setup();
        if !setup.service_installed {
            TunerState::unavailable(
                Availability::NotInstalled,
                "Der fw-ams-Hilfsdienst ist nicht installiert. Er wird für Powerlimits, \
                 Temperaturgrenze und Curve Optimizer benötigt.",
            )
        } else {
            TunerState::unavailable(Availability::ServiceStopped, format!("Hilfsdienst nicht erreichbar: {error}"))
        }
    }

    /// Install (or reinstall) the service with one UAC prompt
    pub fn install(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            let exe = self
                .service_exe
                .as_ref()
                .filter(|p| p.is_file())
                .ok_or("fw-ams-service.exe nicht gefunden (Entwicklung: cargo build -p fw-ams-service)")?;
            let mut args = String::from("install");
            if let Some(sid) = fw_ams_tuner::client::current_user_sid() {
                args.push_str(&format!(" --user-sid {sid}"));
            }
            if let Some(m) = self.module.as_ref().filter(|p| p.is_file()) {
                args.push_str(&format!(" --module \"{}\"", m.display()));
            }
            match fw_ams_tuner::client::run_elevated(exe, &args)? {
                0 => Ok(()),
                code => Err(format!(
                    "Installation fehlgeschlagen (Code {code}), Details in %ProgramData%\\fw-ams\\service.log"
                )),
            }
        }
        #[cfg(not(windows))]
        {
            Err("Nur unter Windows verfügbar".into())
        }
    }

    /// Remove the service (one UAC prompt). It restores the Windows power plan values it
    /// changed; `purge` also deletes the stored profiles and the log.
    pub fn uninstall(&self, purge: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            let exe = self
                .service_exe
                .as_ref()
                .filter(|p| p.is_file())
                .ok_or("fw-ams-service.exe nicht gefunden")?;
            let args = if purge { "uninstall --purge" } else { "uninstall" };
            match fw_ams_tuner::client::run_elevated(exe, args)? {
                0 => Ok(()),
                code => Err(format!("Deinstallation fehlgeschlagen (Code {code})")),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = purge;
            Err("Nur unter Windows verfügbar".into())
        }
    }

    pub fn is_mock(&self) -> bool {
        matches!(self.backend, Backend::Mock { .. })
    }
}

#[cfg(windows)]
fn service_request(req: Request) -> Result<TunerState, String> {
    match fw_ams_tuner::client::request(&req).map_err(|e| e.to_string())? {
        Response::State { state } => Ok(*state),
        Response::Error { message } => Err(message),
        Response::PmDump { .. } => Err("unerwartete Antwort".into()),
    }
}

#[cfg(not(windows))]
fn service_request(_req: Request) -> Result<TunerState, String> {
    Err("Nur unter Windows verfügbar".into())
}

fn sample(view: &TunerView, s: Option<&Snapshot>) -> HistorySample {
    let t = view.state.telemetry.as_ref();
    let battery = s.and_then(|s| s.battery.as_ref());
    let on_battery = s.is_some_and(|s| !s.charger.ac_present)
        && battery.is_some_and(|b| b.state == ChargeState::Discharging);
    let battery_w = battery.and_then(|b| b.power_w);
    let mhz = &view.os.thread_mhz;
    let cores_w = t
        .filter(|t| !t.cores.is_empty())
        .map(|t| t.cores.iter().filter_map(|c| c.power_w).sum());
    HistorySample {
        t: now_ms(),
        system_w: battery_w.filter(|_| on_battery).map(|w| -w),
        charge_w: battery_w.filter(|w| !on_battery && *w > 0.0),
        apu_w: t.and_then(|t| t.socket_w.or(t.fast.value)),
        stapm_w: t.and_then(|t| t.stapm.value),
        cores_w,
        stapm_limit: t.and_then(|t| t.stapm.limit),
        fast_limit: t.and_then(|t| t.fast.limit),
        slow_limit: t.and_then(|t| t.slow.limit),
        tctl_c: t.and_then(|t| t.tctl.value),
        tctl_limit: t.and_then(|t| t.tctl.limit),
        usage: view.os.usage_percent,
        avg_mhz: (!mhz.is_empty()).then(|| mhz.iter().sum::<f32>() / mhz.len() as f32),
        max_mhz: mhz.iter().copied().reduce(f32::max),
    }
}
