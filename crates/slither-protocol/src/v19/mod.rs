//! Protocol-19 wire layouts. Unknown or disabled families can remain opaque at
//! ingress: only call `decode_command` for lanes the consumer currently enables.
pub mod client;
pub mod hud;
mod message;
pub mod minimap;
pub use message::*;
