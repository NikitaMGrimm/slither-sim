//! Headless Slither.io simulator for bot development, training and custom protocol-19 servers.
//!
//! The default physics tick is 7 ms. Mechanics include measured rules and explicit
//! approximations; see `docs/server-rules.md` for mechanics and evidence limits.
pub mod boundary;
mod bounds_index;
#[cfg(feature = "networking")]
pub mod client;
pub mod kiosk;
pub mod model;
pub mod observation;
pub mod physics;
pub mod prey;
pub mod profiling;
pub mod simulator;
pub mod spawn;
pub mod transport;

pub use boundary::{BoundaryMode, RadiusUpdate};
pub use kiosk::KioskConfig;
pub use model::*;
pub use physics::{SpeedModel, SpeedState};
pub use prey::{Prey, PreyConfig, PreyMode};
pub use simulator::{
    AmbientFoodConfig, BoostAccounting, BoostFoodConfig, Config, DeathFoodConfig, FoodSpawnEvent,
    PickupEvent, PickupItem, Simulator,
};
