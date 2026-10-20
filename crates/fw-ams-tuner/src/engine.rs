//! Tuning engine: applies the active profile, reads values back and keeps them in place.
//!
//! Runs inside the privileged service (real hardware) or inside the app (`--mock`). Call
//! [`Engine::tick`] about once per second.
//!
//! Status rules per parameter (see [`ParamStatus`]):
//! - never written → `Default`, the observed value is remembered as firmware default
//! - written and read back equal → `Applied`
//! - read back differs right after writing (twice) → `Limited` (BIOS/firmware clamps it)
//! - was `Applied`, later differs → firmware overwrote it (e.g. charger plugged in): the new
//!   value becomes the default for this power source, then it is restored (`restored += 1`)
//!
//! SMU values are volatile (a reboot resets them). Windows power plan values are not: before
//! the first write to a plan value (per AC/DC index) the original is stored in
//! `plan_originals.json`, independent of the boot, and written back on reset, when the
//! profile no longer sets it, after a boot without "apply on boot", and on uninstall.

use crate::family::{self, encode_arg, SmuMessages};
use crate::pm_table;
use crate::smu::{format_smu_version, SmuBackend, SmuError};
use crate::types::*;
use crate::winpower::PowerPlan;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_EVENTS: usize = 200;
const EVENTS_IN_STATE: usize = 100;
/// A new Curve Optimizer value counts as stable after this time
pub const CO_TRIAL: Duration = Duration::from_secs(120);
/// Give the SMU this long before judging a readback after a write
const SETTLE: Duration = Duration::from_millis(1500);
/// Retry interval for values the firmware refuses
const LIMITED_RETRY: Duration = Duration::from_secs(60);

const CONFIG_FILE: &str = "tuner.json";
const DEFAULTS_FILE: &str = "defaults.json";
const PLAN_ORIGINALS_FILE: &str = "plan_originals.json";
const CO_MARKER: &str = "co_trial";
/// Everything the engine stores, for a full cleanup
pub const STORE_FILES: [&str; 4] = [CONFIG_FILE, DEFAULTS_FILE, PLAN_ORIGINALS_FILE, CO_MARKER];
/// Power plan values are integers
const PLAN_TOLERANCE: f32 = 0.5;

#[derive(Default)]
struct Runtime {
    state: Option<ParamState>,
    last_write: Option<Instant>,
    attempts: u32,
    confirmed: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct StoredDefaults {
    boot_id: u64,
    values: Vec<(ParamId, PowerSource, f32)>,
}

pub struct Engine {
    smu: Option<Box<dyn SmuBackend>>,
    plan: Box<dyn PowerPlan>,
    msgs: Option<SmuMessages>,
    availability: Availability,
    message: Option<String>,
    cpu: Option<CpuInfo>,
    specs: Vec<ParamSpec>,
    rt: BTreeMap<ParamId, Runtime>,
    defaults: HashMap<(ParamId, PowerSource), f32>,
    /// Windows power plan values before fw-ams changed them, per AC/DC index
    plan_originals: HashMap<(ParamId, PowerSource), f32>,
    config: TunerConfig,
    saver: bool,
    power_source: PowerSource,
    telemetry: Option<SmuTelemetry>,
    events: VecDeque<TunerEvent>,
    co_trial: CoTrial,
    co_trial_started: Option<Instant>,
    /// CO from the config is not applied until the user confirms (after boot or a crash)
    co_blocked: bool,
    store: Option<PathBuf>,
    cores: usize,
}

fn cores_from_threads() -> (usize, usize) {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    ((threads / 2).max(1), threads)
}

/// Unix ms of the last boot, rounded so it is stable between calls
pub fn boot_id() -> u64 {
    #[cfg(windows)]
    {
        // SAFETY: no arguments
        let up = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
        (now_ms().saturating_sub(up)) / 10_000
    }
    #[cfg(not(windows))]
    {
        0
    }
}

fn source_label(src: PowerSource) -> &'static str {
    match src {
        PowerSource::Ac => "Netzteil",
        PowerSource::Battery => "Akku",
    }
}

fn other(src: PowerSource) -> PowerSource {
    match src {
        PowerSource::Ac => PowerSource::Battery,
        PowerSource::Battery => PowerSource::Ac,
    }
}

fn load_plan_originals(dir: &Path) -> HashMap<(ParamId, PowerSource), f32> {
    std::fs::read_to_string(dir.join(PLAN_ORIGINALS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<(ParamId, PowerSource, f32)>>(&s).ok())
        .map(|v| v.into_iter().map(|(p, s, v)| ((p, s), v)).collect())
        .unwrap_or_default()
}

/// Write the stored power plan originals back and drop the file. Used by the service's
/// uninstall, when no engine is running. Returns how many values were restored.
pub fn restore_plan_originals(dir: &Path, plan: &mut dyn PowerPlan) -> Result<usize, String> {
    let originals = load_plan_originals(dir);
    let errors: Vec<String> = originals
        .iter()
        .filter_map(|((id, src), v)| {
            plan.write(*id, *src == PowerSource::Ac, *v)
                .err()
                .map(|e| format!("{} ({}): {e}", label(*id), source_label(*src)))
        })
        .collect();
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    let _ = std::fs::remove_file(dir.join(PLAN_ORIGINALS_FILE));
    Ok(originals.len())
}

fn label(id: ParamId) -> &'static str {
    match id {
        ParamId::StapmW => "STAPM-Limit",
        ParamId::FastW => "PPT Fast",
        ParamId::SlowW => "PPT Slow",
        ParamId::TctlC => "Tctl-Grenze",
        ParamId::CoAll => "Curve Optimizer",
        ParamId::MaxFreqMhz => "Max. Takt",
        ParamId::Boost => "Boost-Modus",
        ParamId::Epp => "EPP",
    }
}

fn fmt_value(id: ParamId, v: f32) -> String {
    match id {
        ParamId::StapmW | ParamId::FastW | ParamId::SlowW => format!("{v:.1} W"),
        ParamId::TctlC => format!("{v:.0} °C"),
        ParamId::CoAll => format!("{v:+.0}"),
        ParamId::MaxFreqMhz if v == 0.0 => "unbegrenzt".into(),
        ParamId::MaxFreqMhz => format!("{v:.0} MHz"),
        ParamId::Epp => format!("{v:.0} %"),
        ParamId::Boost => format!("{v:.0}"),
    }
}

impl Engine {
    /// `smu` is `Err` when the driver/module is missing; the engine then only handles the
    /// power plan values and reports why.
    pub fn new(
        smu: Result<Box<dyn SmuBackend>, SmuError>,
        plan: Box<dyn PowerPlan>,
        store: Option<PathBuf>,
        on_ac: bool,
    ) -> Self {
        let id = family::cpuid();
        let (cores, threads) = cores_from_threads();
        let mut family = family::family_from_cpuid(id.family, id.model);
        let (mut smu, availability, message) = match smu {
            Ok(s) => (Some(s), Availability::Ready, None),
            Err(e) => {
                let a = match e {
                    SmuError::DriverMissing(_) => Availability::PawnIoMissing,
                    SmuError::ModuleMissing(_) => Availability::ModuleMissing,
                    _ => Availability::Error,
                };
                (None, a, Some(e.to_string()))
            }
        };
        let mut cpu = CpuInfo {
            name: id.name,
            family,
            cores: cores as u32,
            threads: threads as u32,
            smu_version: None,
            pm_table_version: None,
            pm_table_full: false,
        };
        let (mut availability, mut message) = (availability, message);
        if let Some(s) = smu.as_mut() {
            if let Ok(code) = s.code_name() {
                let f = family::family_from_code_name(code);
                if f != CpuFamily::Other {
                    family = f;
                    cpu.family = f;
                }
            }
            cpu.smu_version = s.smu_version().ok().map(format_smu_version);
            match s.pm_table() {
                Ok((v, _)) => {
                    cpu.pm_table_version = Some(v);
                    cpu.pm_table_full = pm_table::is_fully_known(v);
                }
                Err(e) => log::warn!("pm table: {e}"),
            }
        }
        let msgs = SmuMessages::for_family(family);
        if smu.is_some() && msgs.is_none() {
            availability = Availability::Unsupported;
            message = Some(format!(
                "SMU-Tuning ist für diese CPU ({:?}) noch nicht freigegeben. Unterstützt: Ryzen AI 300 \
                 (Strix Point, Krackan Point), Ryzen AI Max (Strix Halo).",
                family
            ));
        }
        let specs = family::specs(family, smu.is_some() && msgs.is_some());
        let mut e = Self {
            smu,
            plan,
            msgs,
            availability,
            message,
            cpu: Some(cpu),
            specs,
            rt: ParamId::ALL.iter().map(|id| (*id, Runtime::default())).collect(),
            defaults: HashMap::new(),
            plan_originals: HashMap::new(),
            config: TunerConfig::default(),
            saver: false,
            power_source: if on_ac { PowerSource::Ac } else { PowerSource::Battery },
            telemetry: None,
            events: VecDeque::new(),
            co_trial: CoTrial::Idle,
            co_trial_started: None,
            co_blocked: false,
            store,
            cores,
        };
        e.event(
            EventKind::Info,
            None,
            match e.availability {
                Availability::Ready => format!(
                    "Tuner gestartet: {} ({:?}), SMU {}",
                    e.cpu.as_ref().map(|c| c.name.as_str()).unwrap_or("?"),
                    family,
                    e.cpu.as_ref().and_then(|c| c.smu_version.clone()).unwrap_or_default()
                ),
                _ => format!(
                    "Tuner gestartet ohne SMU-Zugriff: {}",
                    e.message.clone().unwrap_or_default()
                ),
            },
        );
        e
    }

    /// Mark the engine as simulated
    pub fn set_mock(&mut self) {
        self.availability = Availability::Mock;
        self.message = Some("Simulierte Hardware (--mock)".into());
    }

    /// Load the stored config and apply it. Call once after `new`.
    pub fn start(&mut self) {
        let boot = boot_id();
        let mut same_boot = false;
        if let Some(dir) = &self.store {
            if let Some(cfg) = std::fs::read_to_string(dir.join(CONFIG_FILE))
                .ok()
                .and_then(|s| serde_json::from_str::<TunerConfig>(&s).ok())
            {
                self.config = self.sanitize(cfg);
            }
            if let Some(d) = std::fs::read_to_string(dir.join(DEFAULTS_FILE))
                .ok()
                .and_then(|s| serde_json::from_str::<StoredDefaults>(&s).ok())
            {
                if d.boot_id == boot {
                    same_boot = true;
                    self.defaults = d.values.into_iter().map(|(p, s, v)| ((p, s), v)).collect();
                }
            }
            self.plan_originals = load_plan_originals(dir);
            if let Ok(marker) = std::fs::read_to_string(dir.join(CO_MARKER)) {
                if marker.trim().parse::<u64>().ok() == Some(boot) {
                    // Service restarted during a trial: keep testing
                    self.co_trial = CoTrial::Running { seconds: CO_TRIAL.as_secs() as u32 };
                    self.co_trial_started = Some(Instant::now());
                } else {
                    self.co_trial = CoTrial::CrashedLastTime;
                    self.co_blocked = true;
                    let _ = std::fs::remove_file(dir.join(CO_MARKER));
                    self.event(
                        EventKind::Warning,
                        Some(ParamId::CoAll),
                        "System während eines Curve-Optimizer-Tests neu gestartet – CO wurde nicht \
                         automatisch angewendet."
                            .into(),
                    );
                }
            }
        }
        if !same_boot && !self.config.co_on_boot {
            self.co_blocked = true;
        }
        // Observe firmware defaults before writing anything
        self.read_telemetry();
        self.observe_defaults();
        if same_boot || self.config.apply_on_boot {
            self.apply_active();
        } else {
            // Power plan values survive the reboot; without "apply on boot" they go back too
            self.restore_plan();
        }
    }

    pub fn config(&self) -> &TunerConfig {
        &self.config
    }

    pub fn active_slot(&self) -> ProfileSlot {
        if self.saver {
            ProfileSlot::Saver
        } else if self.power_source == PowerSource::Ac {
            ProfileSlot::Ac
        } else {
            ProfileSlot::Dc
        }
    }

    fn spec(&self, id: ParamId) -> Option<&ParamSpec> {
        self.specs.iter().find(|s| s.id == id)
    }

    /// Clamp all values to the allowed ranges and drop unsupported ones
    fn sanitize(&self, mut cfg: TunerConfig) -> TunerConfig {
        for p in [&mut cfg.ac, &mut cfg.dc, &mut cfg.saver] {
            for id in ParamId::ALL {
                let v = p.get(id).and_then(|v| {
                    let s = self.specs.iter().find(|s| s.id == id)?;
                    (s.supported && v.is_finite()).then(|| s.clamp(v))
                });
                p.set(id, v);
            }
        }
        cfg
    }

    /// New config from the UI: store it and apply the active profile right away
    pub fn set_config(&mut self, cfg: TunerConfig) {
        let cfg = self.sanitize(cfg);
        let co_changed = self.config.profile(self.active_slot()).co_all != cfg.profile(self.active_slot()).co_all;
        self.config = cfg;
        // An explicit apply confirms the Curve Optimizer value
        self.co_blocked = false;
        if co_changed && self.co_trial == CoTrial::CrashedLastTime {
            self.co_trial = CoTrial::Idle;
        }
        self.persist_config();
        for rt in self.rt.values_mut() {
            rt.attempts = 0;
        }
        self.event(EventKind::Info, None, format!("Profil „{}“ übernommen", slot_label(self.active_slot())));
        self.apply_active();
    }

    pub fn set_saver(&mut self, active: bool) {
        if self.saver != active {
            self.saver = active;
            self.event(
                EventKind::Info,
                None,
                if active {
                    "Energiesparmodus aktiv – Sparprofil wird angewendet".into()
                } else {
                    "Energiesparmodus beendet".into()
                },
            );
            self.apply_active();
        }
    }

    /// Drop all profiles and restore firmware defaults
    pub fn reset(&mut self) {
        self.config.ac = TuneProfile::default();
        self.config.dc = TuneProfile::default();
        self.config.saver = TuneProfile::default();
        self.persist_config();
        self.event(EventKind::Info, None, "Alle Werte auf Standard zurückgesetzt".into());
        self.apply_active();
        // Anything left over, e.g. written by an older version for a profile now inactive
        self.restore_plan();
    }

    /// Raw PM table for diagnostics
    pub fn pm_dump(&mut self) -> Result<(u32, Vec<u8>), SmuError> {
        match self.smu.as_mut() {
            Some(s) => s.pm_table(),
            None => Err(SmuError::DriverMissing("kein SMU-Zugriff".into())),
        }
    }

    pub fn tick(&mut self, on_ac: Option<bool>) {
        if let Some(ac) = on_ac {
            let src = if ac { PowerSource::Ac } else { PowerSource::Battery };
            if src != self.power_source {
                self.power_source = src;
                self.event(
                    EventKind::Info,
                    None,
                    format!(
                        "{} – Profil „{}“ wird angewendet",
                        if ac { "Netzteil angeschlossen" } else { "Akkubetrieb" },
                        slot_label(self.active_slot())
                    ),
                );
                for rt in self.rt.values_mut() {
                    rt.attempts = 0;
                    rt.confirmed = false;
                }
                self.read_telemetry();
                self.observe_defaults();
                self.apply_active();
            }
        }
        self.read_telemetry();
        self.verify();
        self.update_co_trial();
    }

    fn read_telemetry(&mut self) {
        let Some(smu) = self.smu.as_mut() else { return };
        match smu.pm_table() {
            Ok((version, table)) => {
                self.telemetry = Some(pm_table::parse(version, &table, self.cores, now_ms()));
            }
            Err(e) => {
                log::warn!("pm table: {e}");
                self.telemetry = None;
            }
        }
    }

    /// Current value of a parameter as reported by the hardware
    fn read_actual(&mut self, id: ParamId) -> Option<f32> {
        let t = self.telemetry.as_ref();
        match id {
            ParamId::StapmW => t.and_then(|t| t.stapm.limit),
            ParamId::FastW => t.and_then(|t| t.fast.limit),
            ParamId::SlowW => t.and_then(|t| t.slow.limit),
            ParamId::TctlC => t.and_then(|t| t.tctl.limit),
            ParamId::CoAll => None,
            ParamId::MaxFreqMhz | ParamId::Boost | ParamId::Epp => {
                let ac = self.power_source == PowerSource::Ac;
                self.plan.read(id, ac).ok()
            }
        }
    }

    /// Remember what the firmware uses for every value we have not touched
    fn observe_defaults(&mut self) {
        for id in ParamId::ALL {
            // After a reboot the plan still holds fw-ams values; the stored original is the default
            if let Some(o) = self.plan_originals.get(&(id, self.power_source)).copied() {
                self.defaults.insert((id, self.power_source), o);
                continue;
            }
            let untouched = self.rt[&id].state.as_ref().is_none_or(|s| s.sent.is_none());
            if untouched {
                if let Some(v) = self.read_actual(id) {
                    self.defaults.insert((id, self.power_source), v);
                }
            }
        }
        // CO has no readback; firmware default is always 0
        self.defaults.insert((ParamId::CoAll, self.power_source), 0.0);
        self.persist_defaults();
    }

    fn apply_active(&mut self) {
        let profile = self.config.profile(self.active_slot()).clone();
        for id in ParamId::ALL {
            let Some(spec) = self.spec(id).cloned() else { continue };
            let st = self.rt[&id].state.clone().unwrap_or_else(|| ParamState::new(id));
            let mut desired = profile.get(id).filter(|_| spec.supported).map(|v| spec.clamp(v));
            if id == ParamId::CoAll && self.co_blocked && desired.is_some_and(|v| v != 0.0) {
                self.set_state(id, |s| {
                    s.desired = desired;
                    s.status = ParamStatus::Pending;
                    s.error = Some("Nach Neustart nicht automatisch angewendet – zum Aktivieren übernehmen".into());
                });
                continue;
            }
            if id == ParamId::CoAll && desired == Some(0.0) {
                desired = None;
            }
            match (desired, st.sent) {
                (Some(v), sent) if sent != Some(v) || st.status == ParamStatus::Rejected => self.write(id, v),
                (Some(_), _) => self.set_state(id, |s| s.desired = desired),
                (None, sent) => {
                    // Back to default: SMU values get the firmware value seen for this power
                    // source, plan values their stored original (also after a reboot)
                    let restore = if id.is_smu() {
                        sent.and(self.defaults.get(&(id, self.power_source)).copied())
                    } else {
                        self.plan_originals.get(&(id, self.power_source)).copied()
                    };
                    if let Some(d) = restore {
                        self.write(id, d);
                    }
                    if sent.is_some() || restore.is_some() {
                        self.set_state(id, |s| {
                            s.desired = None;
                            s.sent = None;
                            s.status = ParamStatus::Default;
                            s.restored = 0;
                            s.error = None;
                        });
                        let rt = self.rt.get_mut(&id).unwrap();
                        rt.confirmed = false;
                        rt.attempts = 0;
                    } else {
                        self.set_state(id, |s| {
                            s.desired = None;
                            s.status = if spec.supported { ParamStatus::Default } else { ParamStatus::Unsupported };
                        });
                    }
                }
            }
        }
        self.sync_inactive_plan();
    }

    /// The power plan keeps separate AC and DC values. The engine writes the active one while
    /// running, so bring the other index in line with its profile (or its original) as well –
    /// otherwise a value removed from the inactive profile would stay in Windows for good.
    fn sync_inactive_plan(&mut self) {
        let src = other(self.power_source);
        // The saver profile only applies while it is active, which is never on AC
        let slot = if src == PowerSource::Ac { ProfileSlot::Ac } else { ProfileSlot::Dc };
        let profile = self.config.profile(slot).clone();
        for id in ParamId::ALL.into_iter().filter(|id| !id.is_smu()) {
            let Some(spec) = self.spec(id).filter(|s| s.supported).cloned() else { continue };
            let want = profile
                .get(id)
                .map(|v| spec.clamp(v))
                .or_else(|| self.plan_originals.get(&(id, src)).copied());
            let Some(want) = want else { continue };
            let ac = src == PowerSource::Ac;
            if self.plan.read(id, ac).is_ok_and(|v| (v - want).abs() < PLAN_TOLERANCE) {
                continue;
            }
            let what = format!("{} ({})", label(id), source_label(src));
            match self.plan_write(id, src, want) {
                Ok(()) => self.event(EventKind::Applied, Some(id), format!("{what} → {}", fmt_value(id, want))),
                Err(e) => self.event(EventKind::Error, Some(id), format!("{what}: {e}")),
            }
        }
    }

    /// Write a power plan value, storing the original first. Writing the original back
    /// forgets it, so a later change captures the then current Windows value again.
    fn plan_write(&mut self, id: ParamId, src: PowerSource, v: f32) -> Result<(), String> {
        let ac = src == PowerSource::Ac;
        if !self.plan_originals.contains_key(&(id, src)) {
            let current = self.plan.read(id, ac)?;
            if (current - v).abs() >= PLAN_TOLERANCE {
                self.plan_originals.insert((id, src), current);
                self.persist_plan_originals();
            }
        }
        self.plan.write(id, ac, v)?;
        if self.plan_originals.get(&(id, src)).is_some_and(|o| (o - v).abs() < PLAN_TOLERANCE) {
            self.plan_originals.remove(&(id, src));
            self.persist_plan_originals();
        }
        Ok(())
    }

    /// Put every changed power plan value (AC and DC) back to its original
    fn restore_plan(&mut self) {
        let mut originals: Vec<_> = self.plan_originals.iter().map(|(k, v)| (*k, *v)).collect();
        originals.sort_by_key(|((id, src), _)| (*id, *src == PowerSource::Ac));
        for ((id, src), v) in originals {
            let what = format!("{} ({})", label(id), source_label(src));
            match self.plan_write(id, src, v) {
                Ok(()) => self.event(
                    EventKind::Info,
                    Some(id),
                    format!("{what} auf Windows-Wert {} zurückgesetzt", fmt_value(id, v)),
                ),
                Err(e) => self.event(EventKind::Error, Some(id), format!("{what}: {e}")),
            }
        }
    }

    fn set_state(&mut self, id: ParamId, f: impl FnOnce(&mut ParamState)) {
        let rt = self.rt.get_mut(&id).unwrap();
        let st = rt.state.get_or_insert_with(|| ParamState::new(id));
        f(st);
    }

    fn write(&mut self, id: ParamId, v: f32) {
        let result: Result<(), String> = if id.is_smu() {
            match (self.smu.as_mut(), self.msgs.and_then(|m| m.message(id).map(|msg| (m.mailbox, msg)))) {
                (Some(smu), Some((mb, msg))) => smu
                    .send(mb, msg, [encode_arg(id, v), 0, 0, 0, 0, 0])
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
                _ => Err("Kein SMU-Zugriff".into()),
            }
        } else {
            self.plan_write(id, self.power_source, v)
        };
        let rt = self.rt.get_mut(&id).unwrap();
        rt.last_write = Some(Instant::now());
        rt.attempts += 1;
        rt.confirmed = false;
        let desired = self.config.profile(self.active_slot()).get(id);
        match result {
            Ok(()) => {
                self.set_state(id, |s| {
                    s.desired = desired;
                    s.sent = Some(v);
                    s.status = if id == ParamId::CoAll { ParamStatus::Unverified } else { ParamStatus::Pending };
                    s.error = None;
                    s.changed_ms = Some(now_ms());
                });
                self.event(EventKind::Applied, Some(id), format!("{} → {}", label(id), fmt_value(id, v)));
                if id == ParamId::CoAll && v != 0.0 {
                    self.start_co_trial();
                }
            }
            Err(e) => {
                self.set_state(id, |s| {
                    s.desired = desired;
                    s.status = ParamStatus::Rejected;
                    s.error = Some(e.clone());
                    s.changed_ms = Some(now_ms());
                });
                self.event(EventKind::Error, Some(id), format!("{}: {}", label(id), e));
            }
        }
    }

    /// Compare sent and actual values, restore what the firmware overwrote
    fn verify(&mut self) {
        for id in ParamId::ALL {
            let actual = self.read_actual(id);
            let Some(spec) = self.spec(id).cloned() else { continue };
            self.set_state(id, |s| s.actual = actual);
            if !spec.supported {
                self.set_state(id, |s| s.status = ParamStatus::Unsupported);
                continue;
            }
            let st = self.rt[&id].state.clone().unwrap();
            let Some(sent) = st.sent else {
                let original = self.plan_originals.get(&(id, self.power_source)).copied();
                if let Some(a) = original.or(actual) {
                    self.defaults.insert((id, self.power_source), a);
                    self.set_state(id, |s| s.default = Some(a));
                }
                continue;
            };
            if !spec.readable {
                continue;
            }
            let Some(a) = actual else { continue };
            let rt = &self.rt[&id];
            let settling = rt.last_write.is_some_and(|t| t.elapsed() < SETTLE);
            if (a - sent).abs() <= spec.tolerance() {
                let rt = self.rt.get_mut(&id).unwrap();
                rt.confirmed = true;
                rt.attempts = 0;
                self.set_state(id, |s| {
                    s.status = ParamStatus::Applied;
                    s.error = None;
                });
                continue;
            }
            if settling {
                continue;
            }
            if rt.confirmed {
                // It was in place and changed: the firmware overwrote it
                let rt = self.rt.get_mut(&id).unwrap();
                rt.confirmed = false;
                rt.attempts = 0;
                self.defaults.insert((id, self.power_source), a);
                self.persist_defaults();
                if self.config.auto_reapply {
                    self.set_state(id, |s| {
                        s.restored += 1;
                        s.default = Some(a);
                    });
                    self.event(
                        EventKind::Overridden,
                        Some(id),
                        format!(
                            "{} von Firmware auf {} gesetzt → {} wiederhergestellt",
                            label(id),
                            fmt_value(id, a),
                            fmt_value(id, sent)
                        ),
                    );
                    self.write(id, sent);
                } else {
                    self.set_state(id, |s| {
                        s.status = ParamStatus::Overridden;
                        s.default = Some(a);
                    });
                    self.event(
                        EventKind::Overridden,
                        Some(id),
                        format!("{} von Firmware auf {} gesetzt", label(id), fmt_value(id, a)),
                    );
                }
            } else if rt.attempts < 2 {
                self.write(id, sent);
            } else if st.status != ParamStatus::Limited {
                self.set_state(id, |s| {
                    s.status = ParamStatus::Limited;
                    s.error = Some(format!("Hardware hält {} (BIOS-/Firmware-Grenze)", fmt_value(id, a)));
                });
                self.event(
                    EventKind::Warning,
                    Some(id),
                    format!("{}: {} angefordert, Hardware bleibt bei {}", label(id), fmt_value(id, sent), fmt_value(id, a)),
                );
            } else if rt.last_write.is_some_and(|t| t.elapsed() > LIMITED_RETRY) {
                self.rt.get_mut(&id).unwrap().attempts = 1;
                self.write(id, sent);
            }
        }
        // Defaults for display
        for id in ParamId::ALL {
            let d = self.defaults.get(&(id, self.power_source)).copied();
            self.set_state(id, |s| s.default = d);
        }
    }

    fn start_co_trial(&mut self) {
        self.co_trial = CoTrial::Running { seconds: CO_TRIAL.as_secs() as u32 };
        self.co_trial_started = Some(Instant::now());
        if let Some(dir) = &self.store {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::write(dir.join(CO_MARKER), boot_id().to_string());
        }
    }

    fn update_co_trial(&mut self) {
        if let (CoTrial::Running { .. }, Some(start)) = (self.co_trial, self.co_trial_started) {
            let left = CO_TRIAL.saturating_sub(start.elapsed());
            if left.is_zero() {
                self.co_trial = CoTrial::Idle;
                self.co_trial_started = None;
                if let Some(dir) = &self.store {
                    let _ = std::fs::remove_file(dir.join(CO_MARKER));
                }
                self.event(EventKind::Info, Some(ParamId::CoAll), "Curve Optimizer läuft stabil".into());
            } else {
                self.co_trial = CoTrial::Running { seconds: left.as_secs() as u32 };
            }
        }
    }

    fn event(&mut self, kind: EventKind, param: Option<ParamId>, message: String) {
        log::info!("{kind:?}: {message}");
        self.events.push_back(TunerEvent {
            timestamp_ms: now_ms(),
            kind,
            param,
            message,
        });
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    fn persist_config(&self) {
        if let Some(dir) = &self.store {
            let _ = std::fs::create_dir_all(dir);
            if let Ok(s) = serde_json::to_string_pretty(&self.config) {
                if let Err(e) = std::fs::write(dir.join(CONFIG_FILE), s) {
                    log::warn!("tuner config not saved: {e}");
                }
            }
        }
    }

    fn persist_plan_originals(&self) {
        let Some(dir) = &self.store else { return };
        let path = dir.join(PLAN_ORIGINALS_FILE);
        let values: Vec<(ParamId, PowerSource, f32)> =
            self.plan_originals.iter().map(|((p, s), v)| (*p, *s, *v)).collect();
        let result = if values.is_empty() {
            match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        } else {
            let _ = std::fs::create_dir_all(dir);
            serde_json::to_string(&values)
                .map_err(std::io::Error::other)
                .and_then(|s| std::fs::write(&path, s))
        };
        if let Err(e) = result {
            log::warn!("plan originals not saved: {e}");
        }
    }

    fn persist_defaults(&self) {
        if let Some(dir) = &self.store {
            let d = StoredDefaults {
                boot_id: boot_id(),
                values: self.defaults.iter().map(|((p, s), v)| (*p, *s, *v)).collect(),
            };
            let _ = std::fs::create_dir_all(dir);
            if let Ok(s) = serde_json::to_string(&d) {
                let _ = std::fs::write(dir.join(DEFAULTS_FILE), s);
            }
        }
    }

    pub fn state(&self) -> TunerState {
        let skip = self.events.len().saturating_sub(EVENTS_IN_STATE);
        TunerState {
            availability: self.availability,
            message: self.message.clone(),
            service_version: Some(env!("CARGO_PKG_VERSION").into()),
            cpu: self.cpu.clone(),
            power_source: self.power_source,
            active_slot: self.active_slot(),
            config: self.config.clone(),
            specs: self.specs.clone(),
            params: ParamId::ALL
                .iter()
                .map(|id| self.rt[id].state.clone().unwrap_or_else(|| ParamState::new(*id)))
                .collect(),
            telemetry: self.telemetry.clone(),
            events: self.events.iter().skip(skip).cloned().collect(),
            co_trial: self.co_trial,
        }
    }
}

fn slot_label(slot: ProfileSlot) -> &'static str {
    match slot {
        ProfileSlot::Ac => "Netzteil",
        ProfileSlot::Dc => "Akku",
        ProfileSlot::Saver => "Energiesparen",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smu::mock::MockSmu;
    use crate::winpower::MockPowerPlan;

    fn engine() -> Engine {
        let mut e = Engine::new(
            Ok(Box::new(MockSmu::default())),
            Box::new(MockPowerPlan::default()),
            None,
            true,
        );
        e.start();
        e
    }

    fn param(e: &Engine, id: ParamId) -> ParamState {
        e.state().params.into_iter().find(|p| p.id == id).unwrap()
    }

    fn settle(e: &mut Engine) {
        for rt in e.rt.values_mut() {
            rt.last_write = rt.last_write.map(|t| t - SETTLE);
        }
        e.tick(Some(true));
    }

    #[test]
    fn captures_defaults_and_applies() {
        let mut e = engine();
        e.tick(Some(true));
        assert_eq!(param(&e, ParamId::StapmW).default, Some(28.0));
        assert_eq!(param(&e, ParamId::StapmW).status, ParamStatus::Default);

        let mut cfg = e.config().clone();
        cfg.ac.stapm_w = Some(15.0);
        e.set_config(cfg);
        settle(&mut e);
        let p = param(&e, ParamId::StapmW);
        assert_eq!(p.sent, Some(15.0));
        assert_eq!(p.actual, Some(15.0));
        assert_eq!(p.status, ParamStatus::Applied);
    }

    #[test]
    fn bios_cap_becomes_limited() {
        let mut e = engine();
        let mut cfg = e.config().clone();
        cfg.ac.fast_w = Some(45.0);
        e.set_config(cfg);
        for _ in 0..3 {
            settle(&mut e);
        }
        let p = param(&e, ParamId::FastW);
        assert_eq!(p.status, ParamStatus::Limited);
        assert_eq!(p.actual, Some(35.0));
    }

    #[test]
    fn values_are_clamped() {
        let mut e = engine();
        let mut cfg = e.config().clone();
        cfg.ac.stapm_w = Some(500.0);
        cfg.ac.co_all = Some(-80.0);
        e.set_config(cfg);
        assert_eq!(e.config().ac.stapm_w, Some(54.0));
        assert_eq!(e.config().ac.co_all, Some(-30.0));
    }

    #[test]
    fn reset_restores_default() {
        let mut e = engine();
        let mut cfg = e.config().clone();
        cfg.ac.stapm_w = Some(15.0);
        e.set_config(cfg);
        settle(&mut e);
        e.reset();
        settle(&mut e);
        let p = param(&e, ParamId::StapmW);
        assert_eq!(p.status, ParamStatus::Default);
        assert_eq!(p.actual, Some(28.0));
    }

    /// Power plan shared between engines, like the real one that survives a service restart
    #[derive(Clone, Default)]
    struct SharedPlan(std::sync::Arc<std::sync::Mutex<MockPowerPlan>>);

    impl PowerPlan for SharedPlan {
        fn read(&mut self, id: ParamId, ac: bool) -> Result<f32, String> {
            self.0.lock().unwrap().read(id, ac)
        }
        fn write(&mut self, id: ParamId, ac: bool, value: f32) -> Result<(), String> {
            self.0.lock().unwrap().write(id, ac, value)
        }
    }

    fn engine_with(plan: &SharedPlan, store: Option<PathBuf>) -> Engine {
        let mut e = Engine::new(Ok(Box::new(MockSmu::default())), Box::new(plan.clone()), store, true);
        e.start();
        e
    }

    fn temp_store(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fw-ams-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn inactive_plan_index_follows_profile_and_reset() {
        let mut plan = SharedPlan::default();
        let mut e = engine_with(&plan, None);
        let mut cfg = e.config().clone();
        cfg.dc.epp = Some(80.0);
        e.set_config(cfg);
        // On AC, but the DC index of the power plan is written right away
        assert_eq!(plan.read(ParamId::Epp, false), Ok(80.0));
        assert_eq!(plan.read(ParamId::Epp, true), Ok(33.0));

        // Removing the value from the inactive profile restores the Windows value
        let mut cfg = e.config().clone();
        cfg.dc.epp = None;
        e.set_config(cfg);
        assert_eq!(plan.read(ParamId::Epp, false), Ok(50.0));

        let mut cfg = e.config().clone();
        cfg.ac.epp = Some(10.0);
        cfg.dc.boost = Some(0.0);
        e.set_config(cfg);
        e.reset();
        assert_eq!(plan.read(ParamId::Epp, true), Ok(33.0));
        assert_eq!(plan.read(ParamId::Boost, false), Ok(2.0));
    }

    #[test]
    fn plan_originals_survive_restart_and_uninstall() {
        let dir = temp_store("originals");
        let mut plan = SharedPlan::default();
        let mut e = engine_with(&plan, Some(dir.clone()));
        let mut cfg = e.config().clone();
        cfg.ac.epp = Some(10.0);
        cfg.apply_on_boot = false;
        e.set_config(cfg);
        assert_eq!(plan.read(ParamId::Epp, true), Ok(10.0));
        drop(e);

        // Service restart: the default shown is the Windows value, not the one fw-ams wrote
        let mut e = engine_with(&plan, Some(dir.clone()));
        e.tick(Some(true));
        assert_eq!(param(&e, ParamId::Epp).default, Some(33.0));
        drop(e);

        // Uninstall without a running engine
        plan.write(ParamId::Epp, true, 10.0).unwrap();
        assert_eq!(restore_plan_originals(&dir, &mut plan), Ok(1));
        assert_eq!(plan.read(ParamId::Epp, true), Ok(33.0));
        assert_eq!(restore_plan_originals(&dir, &mut plan), Ok(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn power_source_switch_applies_dc_profile() {
        let mut e = engine();
        let mut cfg = e.config().clone();
        cfg.dc.epp = Some(80.0);
        e.set_config(cfg);
        assert_eq!(param(&e, ParamId::Epp).sent, None);
        e.tick(Some(false));
        assert_eq!(e.active_slot(), ProfileSlot::Dc);
        assert_eq!(param(&e, ParamId::Epp).sent, Some(80.0));
    }
}
