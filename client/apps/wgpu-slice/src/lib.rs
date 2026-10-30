//! Shared explicit local gameplay and offline diagnostics. The existing core owns state/plans.
#![forbid(unsafe_code)]

pub mod announcement;
pub mod bank;
pub mod cosmetics;
pub mod frame;
pub mod game;
pub mod gpu;
pub mod hud;
pub mod materials;
// Same targets as the winit dependency (Cargo.toml); Linux builds only the GPU-free e2e probe.
#[cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
pub mod menu_input_native;
pub mod minimap;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
pub mod official;
pub mod render;
pub mod scene;
pub mod text;

#[cfg(target_arch = "wasm32")]
mod web;

pub type Result<T> = std::result::Result<T, String>;
