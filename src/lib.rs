//! Headless Slither.io simulator for bot development, training and custom protocol-19 servers.
//!
//! The default physics tick is 7 ms. Mechanics include measured rules and explicit
//! approximations; see `docs/server-rules.md` for mechanics and evidence limits.
#[cfg(feature = "bot")]
pub mod bot;
pub mod boundary;
mod bounds_index;
#[cfg(feature = "networking")]
pub mod client;
pub mod error;
mod food_grid;
mod json;
pub mod kiosk;
pub mod model;
pub mod observation;
#[cfg(feature = "parallel")]
mod parallel;
pub mod physics;
pub mod prey;
pub mod profiling;
pub mod simulator;
pub mod spawn;
pub mod transport;

pub use boundary::{BoundaryMode, RadiusUpdate};
pub use error::{Constraint, CoreStepError, ErrorCode, ErrorKind, SimError, WorldStatus};
pub use kiosk::KioskConfig;
pub use model::*;
pub use physics::{SpeedModel, SpeedState};
pub use prey::{Prey, PreyConfig, PreyMode};
pub use simulator::checkpoint::{Checkpoint, EventRecording, RestoreOptions};
pub use simulator::{
    AmbientFoodConfig, BoostAccounting, BoostFoodConfig, Config, DeathFoodConfig, FoodSpawnEvent,
    PickupEvent, PickupItem, PitConfig, Preset, Simulator,
};
