//! fw-ams core: reads Framework laptop hardware state (battery, USB-C PD, thermal, expansion
//! cards) on Windows and Linux and turns it into a platform independent [`model::Snapshot`].

pub mod alerts;
pub mod cards;
pub mod collector;
pub mod mock;
pub mod model;
pub mod saver;
pub mod source;

pub use collector::Collector;
pub use model::Snapshot;
