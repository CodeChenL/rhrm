use std::sync::{Arc, Mutex};

pub(crate) const FLOAT_WINDOW_MARGIN: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FloatWindowLayout {
    pub x: i32,
    pub y: i32,
    pub width: f32,
    pub height: f32,
    pub click_through: bool,
    pub opacity: f32,
}

impl Default for FloatWindowLayout {
    fn default() -> Self {
        Self {
            x: 24,
            y: 24,
            width: 100.0,
            height: 100.0,
            click_through: true,
            opacity: 0.85,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum FloatWindowPreset {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Center,
}

impl FloatWindowPreset {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "左上",
            Self::TopCenter => "上中",
            Self::TopRight => "右上",
            Self::MiddleLeft => "左中",
            Self::MiddleRight => "右中",
            Self::BottomLeft => "左下",
            Self::BottomCenter => "下中",
            Self::BottomRight => "右下",
            Self::Center => "居中",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FloatWindowSharedSnapshot {
    pub open: bool,
    pub layout: FloatWindowLayout,
    pub preset: FloatWindowPreset,
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct FloatWindowSharedState {
    inner: Arc<Mutex<FloatWindowSharedSnapshot>>,
}

impl FloatWindowSharedState {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FloatWindowSharedSnapshot {
                open: false,
                layout: FloatWindowLayout::default(),
                preset: FloatWindowPreset::TopLeft,
                revision: 0,
            })),
        }
    }

    pub(crate) fn snapshot(&self) -> FloatWindowSharedSnapshot {
        *self.inner.lock().unwrap()
    }

    fn update(&self, updater: impl FnOnce(&mut FloatWindowSharedSnapshot) -> bool) -> bool {
        let mut snapshot = self.inner.lock().unwrap();
        let changed = updater(&mut snapshot);
        if changed {
            snapshot.revision += 1;
        }
        changed
    }
}

pub(crate) struct FloatWindowController {
    shared: FloatWindowSharedState,
}

impl Default for FloatWindowController {
    fn default() -> Self {
        Self { shared: FloatWindowSharedState::new() }
    }
}

impl FloatWindowController {
    pub(crate) fn is_open(&self) -> bool {
        self.shared.snapshot().open
    }

    pub(crate) fn layout(&self) -> FloatWindowLayout {
        self.shared.snapshot().layout
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn shared_state(&self) -> FloatWindowSharedState {
        self.shared.clone()
    }

    pub(crate) fn apply_layout(&mut self, layout: FloatWindowLayout) -> bool {
        self.shared.update(|snapshot| {
            if snapshot.layout == layout {
                return false;
            }

            snapshot.layout = layout;
            true
        })
    }

    pub(crate) fn set_preset(&mut self, preset: FloatWindowPreset) -> bool {
        self.shared.update(|snapshot| {
            if snapshot.preset == preset {
                return false;
            }

            snapshot.preset = preset;
            true
        })
    }

    pub(crate) fn close(&mut self) {
        let _ = self.shared.update(|snapshot| {
            if !snapshot.open {
                return false;
            }

            snapshot.open = false;
            true
        });
    }

    pub(crate) fn open(&mut self) {
        let _ = self.shared.update(|snapshot| {
            if snapshot.open {
                return false;
            }

            snapshot.open = true;
            true
        });
    }
}
