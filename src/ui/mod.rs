mod float_window;
mod float_window_controller;
mod main_window;
#[cfg(target_os = "linux")]
mod wayland_overlay;

pub(crate) use float_window_controller::{
	FloatWindowLayout, FloatWindowPreset,
};
#[cfg(target_os = "linux")]
pub(crate) use float_window_controller::{
	FloatWindowSharedSnapshot, FloatWindowSharedState,
	FLOAT_WINDOW_MARGIN,
};
#[cfg(target_os = "linux")]
pub(crate) use wayland_overlay::spawn_wayland_overlay;
pub use main_window::run_main_window;
