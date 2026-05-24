use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::ui::{FloatWindowLayout, FloatWindowPreset};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub float_layout: FloatWindowLayout,
    pub float_preset: FloatWindowPreset,
    pub last_device_addr: Option<String>,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            float_layout: FloatWindowLayout::default(),
            float_preset: FloatWindowPreset::TopLeft,
            last_device_addr: None,
        }
    }
}
impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(contents) = fs::read_to_string(path) else {
            return Self::default();
        };
        toml::from_str(&contents).unwrap_or_default()
    }
    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            if fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        let Ok(contents) = toml::to_string_pretty(self) else {
            return;
        };
        let _ = fs::write(path, contents);
    }
}
fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                let mut path = PathBuf::from(home);
                path.push(".config");
                path
            })
        })
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("rhrm").join("rhrm.toml")
}
