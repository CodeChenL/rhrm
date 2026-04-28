mod float_window;
mod float_window_controller;
mod main_window;
mod wayland_overlay;

pub(crate) use float_window_controller::{
	FloatWindowLayout, FloatWindowPreset, FloatWindowSharedSnapshot, FloatWindowSharedState,
	FLOAT_WINDOW_MARGIN,
};
pub(crate) use wayland_overlay::spawn_wayland_overlay;
pub use main_window::run_main_window;
