//! Bounded, offline protocol primitives for an independent browser prototype.
//!
//! Runtime credentials and existing browser image objects are supplied by callers.
//! This crate neither opens connections nor fetches assets.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod math;

pub mod appearance_preview;

#[cfg(target_arch = "wasm32")]
mod appearance_preview_browser;

#[cfg(target_arch = "wasm32")]
mod accessory_browser;

#[cfg(target_arch = "wasm32")]
mod authored_sources_browser;

pub mod authored_sources;
pub mod challenge;
pub mod champion;
pub mod desktop_input;
pub mod desktop_layout;
#[cfg(feature = "feel-trace")]
pub mod feel_trace;
pub mod gameplay_policy;
pub mod login_controls;
pub mod menu_controls;
pub mod menu_input;
pub mod menu_layout;
pub mod menu_loading;
pub mod menu_preferences;
pub mod menu_theme;
mod menu_transition;
pub mod play_admission;
pub mod presentation;
pub mod profile_literals;
pub mod protocol;
pub mod session_policy;
pub mod settings;
pub mod start_page;
pub mod state;
pub mod stock_controller_policy;
pub mod world_resolution;

#[cfg(target_arch = "wasm32")]
mod start_page_browser;

#[cfg(target_arch = "wasm32")]
pub mod login_browser;

#[cfg(target_arch = "wasm32")]
mod menu_preferences_browser;

#[cfg(target_arch = "wasm32")]
mod menu_loading_browser;

#[cfg(target_arch = "wasm32")]
mod play_admission_browser;

#[cfg(target_arch = "wasm32")]
mod desktop_layout_browser;

#[cfg(target_arch = "wasm32")]
mod profile_literals_browser;

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
mod desktop_input_browser;

#[cfg(target_arch = "wasm32")]
mod session_policy_browser;

#[cfg(target_arch = "wasm32")]
mod stock_controller_policy_browser;

#[cfg(target_arch = "wasm32")]
mod gameplay_policy_browser;

#[cfg(target_arch = "wasm32")]
mod canvas_browser;

#[cfg(target_arch = "wasm32")]
mod pattern_browser;

#[cfg(target_arch = "wasm32")]
mod animated_browser;

#[cfg(target_arch = "wasm32")]
mod announcement_browser;

#[cfg(target_arch = "wasm32")]
mod body_browser;

#[cfg(target_arch = "wasm32")]
mod face_browser;

#[cfg(target_arch = "wasm32")]
mod antenna_browser;

#[cfg(target_arch = "wasm32")]
mod food_browser;

#[cfg(target_arch = "wasm32")]
mod prey_browser;

#[cfg(target_arch = "wasm32")]
mod scene_browser;

#[cfg(target_arch = "wasm32")]
mod presentation_browser;

#[cfg(target_arch = "wasm32")]
mod quality_browser;

#[cfg(target_arch = "wasm32")]
mod hud_history_browser;

#[cfg(target_arch = "wasm32")]
mod leaderboard_history_browser;

/// Shared server directory policy.
pub mod server_directory;
/// Shared server picker.
pub mod server_picker;
pub mod skin_builder;
pub mod skin_studio;
pub mod studio_preview;
