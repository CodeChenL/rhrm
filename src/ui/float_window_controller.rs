#[derive(Debug, Clone, Copy)]
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

pub(crate) struct FloatWindowController {
    open: bool,
    layout: FloatWindowLayout,
}

impl Default for FloatWindowController {
    fn default() -> Self {
        Self {
            open: false,
            layout: FloatWindowLayout::default(),
        }
    }
}

impl FloatWindowController {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn layout(&self) -> FloatWindowLayout {
        self.layout
    }

    pub(crate) fn apply_layout(&mut self, layout: FloatWindowLayout) -> bool {
        self.layout = layout;
        true
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn open(&mut self) {
        self.open = true;
    }
}
