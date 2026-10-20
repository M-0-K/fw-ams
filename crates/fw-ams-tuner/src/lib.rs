//! fw-ams tuner: CPU power/temperature/voltage tuning for AMD Ryzen mobile APUs through the
//! SMU, plus Windows power plan values. Shared by the privileged service (`fw-ams-service`),
//! the app and the CLI.

#[cfg(windows)]
pub mod client;
pub mod engine;
pub mod family;
pub mod oscpu;
pub mod pm_table;
pub mod protocol;
pub mod smu;
pub mod types;
pub mod winpower;

pub use engine::Engine;
pub use types::*;

/// Open the real SMU backend for this platform
pub fn open_smu() -> Result<Box<dyn smu::SmuBackend>, smu::SmuError> {
    #[cfg(windows)]
    {
        smu::pawnio::PawnIo::open().map(|p| Box::new(p) as Box<dyn smu::SmuBackend>)
    }
    #[cfg(not(windows))]
    {
        Err(smu::SmuError::DriverMissing(
            "SMU-Zugriff ist bisher nur unter Windows umgesetzt (siehe docs/linux-advanced.md)".into(),
        ))
    }
}

/// Engine with simulated hardware for `--mock`
pub fn mock_engine() -> Engine {
    let mut e = Engine::new(
        Ok(Box::new(smu::mock::MockSmu::default())),
        Box::new(winpower::MockPowerPlan::default()),
        None,
        true,
    );
    e.set_mock();
    e.start();
    e
}
