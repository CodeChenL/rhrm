use eframe::egui;
use std::sync::Arc;

use crate::app_state::{AppState, DeviceInfo};
use crate::bluetooth::BluetoothService;
use crate::config::AppConfig;

use super::float_window::FloatWindowApp;
use super::float_window_controller::{FloatWindowController, FloatWindowLayout};

const FLOAT_MIN_SIZE: f32 = 50.0;
const CONTROL_PANEL_REPAINT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
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
    fn label(self) -> &'static str {
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

#[derive(Clone)]
struct FloatWindowControls {
    preset: FloatWindowPreset,
    width: f32,
    height: f32,
    click_through: bool,
    opacity: f32,
}

impl Default for FloatWindowControls {
    fn default() -> Self {
        Self {
            preset: FloatWindowPreset::TopLeft,
            width: 100.0,
            height: 100.0,
            click_through: true,
            opacity: 0.85,
        }
    }
}

pub struct RhrmApp {
    state: AppState,
    float_window: FloatWindowController,
    float_window_app: FloatWindowApp,
    bluetooth_service: BluetoothService,
    float_controls: FloatWindowControls,
    config: AppConfig,
    auto_connect_pending: bool,
    control_panel_focused: bool,
    should_exit: bool,
}

impl Default for RhrmApp {
    fn default() -> Self {
        let config = AppConfig::load();
        let state = AppState::default();
        let mut float_window = FloatWindowController::default();
        float_window.open();
        float_window.apply_layout(config.float_layout);
        let layout = float_window.layout();
        Self {
            bluetooth_service: BluetoothService::new(state.clone()),
            float_window_app: FloatWindowApp::new(state.clone(), layout.click_through, layout.opacity),
            state,
            float_window,
            float_controls: FloatWindowControls {
                preset: config.float_preset,
                width: layout.width,
                height: layout.height,
                click_through: layout.click_through,
                opacity: layout.opacity,
            },
            config,
            auto_connect_pending: true,
            control_panel_focused: true,
            should_exit: false,
        }
    }
}

impl RhrmApp {
    fn toggle_scan(&self) {
        if !self.state.toggle_scanning() {
            return;
        }

        self.state.clear_error();
        if let Err(error) = self.bluetooth_service.start_scan() {
            self.state.set_scanning(false);
            self.state.set_error_message(error.to_string());
        }
    }

    fn connect_device(&mut self, addr: String) {
        self.state.set_selected_device(Some(addr.clone()));
        self.state.set_connecting(true);
        self.state.clear_error();
        self.config.last_device_addr = Some(addr.clone());
        self.config.save();

        if let Err(error) = self.bluetooth_service.connect(addr.clone()) {
            self.state.set_connecting(false);
            self.state.update_device_connection(&addr, false);
            self.state.mark_shared_heart_rate(None, false);
            self.state.set_error_message(error.to_string());
        }
    }

    fn disconnect_device(&mut self) {
        if let Some(addr) = self.state.selected_device() {
            self.state.update_device_connection(&addr, false);
        }
        self.state.set_selected_device(None);
        self.state.set_connecting(false);
        self.state.mark_shared_heart_rate(None, false);
    }

    fn try_auto_connect(&mut self) {
        if !self.auto_connect_pending {
            return;
        }
        self.auto_connect_pending = false;

        let Some(addr) = self.config.last_device_addr.clone() else {
            return;
        };

        self.connect_device(addr);
    }

    fn compute_preset_position(
        &self,
        preset: FloatWindowPreset,
        width: f32,
        height: f32,
    ) -> (i32, i32) {
        let margin = 24.0;
        let screen_width = 1920.0;
        let screen_height = 1080.0;
        let x_max = (screen_width - width - margin).max(margin);
        let y_max = (screen_height - height - margin).max(margin);
        let x_center = ((screen_width - width) / 2.0).round() as i32;
        let y_center = ((screen_height - height) / 2.0).round() as i32;

        match preset {
            FloatWindowPreset::TopLeft => (margin as i32, margin as i32),
            FloatWindowPreset::TopCenter => (x_center, margin as i32),
            FloatWindowPreset::TopRight => (x_max.round() as i32, margin as i32),
            FloatWindowPreset::MiddleLeft => (margin as i32, y_center),
            FloatWindowPreset::MiddleRight => (x_max.round() as i32, y_center),
            FloatWindowPreset::BottomLeft => (margin as i32, y_max.round() as i32),
            FloatWindowPreset::BottomCenter => (x_center, y_max.round() as i32),
            FloatWindowPreset::BottomRight => (x_max.round() as i32, y_max.round() as i32),
            FloatWindowPreset::Center => (x_center, y_center),
        }
    }

    fn current_float_layout(&self) -> FloatWindowLayout {
        self.float_window.layout()
    }

    fn apply_float_controls(&mut self) {
        let width = self.float_controls.width.max(FLOAT_MIN_SIZE);
        let height = self.float_controls.height.max(FLOAT_MIN_SIZE);
        self.float_controls.width = width;
        self.float_controls.height = height;
        let (x, y) = self.compute_preset_position(self.float_controls.preset, width, height);
        let layout = FloatWindowLayout {
            x,
            y,
            width,
            height,
            click_through: self.float_controls.click_through,
            opacity: self.float_controls.opacity.clamp(0.0, 1.0),
        };
        self.float_window_app
            .set_click_through(self.float_controls.click_through);
        self.float_window_app.set_opacity(layout.opacity);

        self.float_window.apply_layout(layout);
        self.config.float_layout = layout;
        self.config.float_preset = self.float_controls.preset;
        self.config.save();
    }

    fn reset_float_controls(&mut self) {
        self.float_controls = FloatWindowControls::default();
        self.apply_float_controls();
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("rhrm");
            ui.separator();
            let scanning = self.state.is_scanning();
            if ui.button(if scanning { "Stop" } else { "Scan" }).clicked() {
                self.toggle_scan();
            }
        });
    }

    fn update_control_panel_focus(&mut self, ctx: &egui::Context) {
        self.control_panel_focused = ctx
            .input(|input| input.viewport().focused)
            .unwrap_or(true);

        if self.control_panel_focused {
            ctx.request_repaint_after(CONTROL_PANEL_REPAINT_INTERVAL);
        }
    }

    fn show_main_panel_viewport(&mut self, ctx: &egui::Context) {
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("main_control_panel"),
            egui::ViewportBuilder::default()
                .with_title("rhrm control panel")
                .with_inner_size(egui::vec2(720.0, 820.0))
                .with_position(egui::pos2(80.0, 80.0))
                .with_resizable(true)
                .with_transparent(false)
                .with_decorations(true),
            |ctx, _class| {
                self.update_control_panel_focus(ctx);

                if ctx.input(|input| input.viewport().close_requested()) {
                    self.float_window.close();
                    self.should_exit = true;
                    return;
                }

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200)))
                    .show(ctx, |ui| {
                        self.render_toolbar(ui);
                        ui.separator();

                        if self.state.is_connecting() {
                            ui.label(
                                egui::RichText::new("Connected - Receiving HR...")
                                    .color(egui::Color32::GREEN),
                            );
                        }

                        self.render_device_list(ui);
                        ui.separator();
                        self.render_status_bar(ui);
                        ui.separator();
                        self.render_float_controls(ui);
                    });
            },
        );
    }

    fn render_device_list(&mut self, ui: &mut egui::Ui) {
        let devices = self.state.sorted_devices();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if devices.is_empty() {
                    ui.label(
                        egui::RichText::new("Scanning... Click device to connect")
                            .color(egui::Color32::GRAY),
                    );
                }

                for device in devices {
                    self.render_device_card(ui, device);
                }
            });
    }

    fn render_device_card(&mut self, ui: &mut egui::Ui, device: DeviceInfo) {
        let is_selected = self.state.is_selected(&device.addr);
        let background = if is_selected {
            if device.connected {
                egui::Color32::from_rgba_unmultiplied(40, 100, 60, 0)
            } else {
                egui::Color32::from_rgba_unmultiplied(60, 80, 120, 0)
            }
        } else {
            egui::Color32::from_rgba_unmultiplied(40, 40, 60, 0)
        };

        let rect = ui.available_rect_before_wrap();
        let height = if is_selected { 110.0 } else { 80.0 };
        let card_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), height));
        ui.painter()
            .rect_filled(card_rect.expand(2.0), 4.0, background);

        let response = ui.interact(card_rect, egui::Id::new(&device.addr), egui::Sense::click());
        if response.clicked() {
            if is_selected && device.connected {
                self.disconnect_device();
            } else {
                self.connect_device(device.addr.clone());
            }
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&device.name)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            if device.connected {
                ui.label(egui::RichText::new("●").color(egui::Color32::GREEN));
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&device.addr)
                    .small()
                    .color(egui::Color32::GRAY),
            )
        });

        if let Some(heart_rate) = device.heart_rate {
            let color = match heart_rate {
                60..=100 => egui::Color32::GREEN,
                101..=120 => egui::Color32::YELLOW,
                _ => egui::Color32::RED,
            };
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("❤ {} bpm", heart_rate))
                        .size(20.0)
                        .color(color),
                );
            });
        } else if is_selected && !device.connected {
            ui.label(egui::RichText::new("Click to connect...").color(egui::Color32::LIGHT_GRAY));
        }

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Time: {}", device.last_seen))
                    .small()
                    .color(egui::Color32::GRAY),
            )
        });
    }

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("Devices: {}", self.state.device_count()));
            if let Some(error) = self.state.error_message() {
                ui.colored_label(egui::Color32::YELLOW, error);
            }
        });
    }

    fn render_float_controls(&mut self, ui: &mut egui::Ui) {
        let layout = self.current_float_layout();
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("浮窗布局");
                    ui.label(
                        egui::RichText::new(format!(
                            "当前 {}×{} @ ({}, {})",
                            layout.width.round() as i32,
                            layout.height.round() as i32,
                            layout.x,
                            layout.y
                        ))
                        .small()
                        .color(egui::Color32::GRAY),
                    );
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label("位置预设:");
                });

                for row in [
                    [
                        FloatWindowPreset::TopLeft,
                        FloatWindowPreset::TopCenter,
                        FloatWindowPreset::TopRight,
                    ],
                    [
                        FloatWindowPreset::MiddleLeft,
                        FloatWindowPreset::Center,
                        FloatWindowPreset::MiddleRight,
                    ],
                    [
                        FloatWindowPreset::BottomLeft,
                        FloatWindowPreset::BottomCenter,
                        FloatWindowPreset::BottomRight,
                    ],
                ] {
                    ui.horizontal(|ui| {
                        for preset in row {
                            let changed = ui
                                .add_sized(
                                    [72.0, 24.0],
                                    egui::Button::new(preset.label()).selected(
                                        self.float_controls.preset == preset,
                                    ),
                                )
                                .clicked();
                            if changed {
                                self.float_controls.preset = preset;
                                self.apply_float_controls();
                            }
                        }
                    });
                }

                ui.horizontal(|ui| {
                    ui.label("宽度");
                    let width_changed = ui
                        .add(
                            egui::DragValue::new(&mut self.float_controls.width)
                                .speed(1.0),
                        )
                        .changed();
                    ui.label("高度");
                    let height_changed = ui
                        .add(
                            egui::DragValue::new(&mut self.float_controls.height)
                                .speed(1.0),
                        )
                        .changed();
                    let click_through_changed = ui
                        .checkbox(&mut self.float_controls.click_through, "鼠标穿透")
                        .changed();
                    let opacity_changed = ui
                        .add(
                            egui::Slider::new(&mut self.float_controls.opacity, 0.0..=1.0)
                                .text("透明度"),
                        )
                        .changed();
                    if width_changed || height_changed || click_through_changed || opacity_changed {
                        self.apply_float_controls();
                    }
                    if ui.button("重置").clicked() {
                        self.reset_float_controls();
                    }
                });
            });
        });
    }
}

impl eframe::App for RhrmApp {
    fn on_exit(&mut self, _ctx: Option<&eframe::glow::Context>) {
        self.float_window.close();
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            let _ = frame;
            return;
        }

        self.try_auto_connect();

        if self.float_window.is_open() {
            self.show_main_panel_viewport(ctx);
        }

        self.float_window_app.set_click_through(self.float_controls.click_through);
        self.float_window_app.set_opacity(self.float_controls.opacity);
        self.float_window_app
            .show_as_root_with_layout(ctx, self.float_window.layout());
    }
}

pub fn run_main_window() -> eframe::Result<()> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.renderer = eframe::Renderer::Glow;
    native_options.vsync = true;
    native_options.multisampling = 1;
    native_options.viewport = native_options
        .viewport
        .clone()
        .with_transparent(true)
        .with_decorations(false)
        .with_always_on_top();

    eframe::run_native(
        "rhrm",
        native_options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            let ubuntu_mono = "ubuntu_mono".to_owned();
            let noto_sans_sc = "noto_sans_sc".to_owned();

            fonts.font_data.insert(
                ubuntu_mono.clone(),
                Arc::new(egui::FontData::from_static(include_bytes!(
                    "../../UbuntuMono-R.ttf"
                ))),
            );
            fonts.font_data.insert(
                noto_sans_sc.clone(),
                Arc::new(egui::FontData::from_static(include_bytes!(
                    "../../NotoSansCJKsc-Regular.otf"
                ))),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, ubuntu_mono.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, ubuntu_mono);
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(1, noto_sans_sc.clone());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(1, noto_sans_sc);
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(RhrmApp::default()))
        }),
    )
}
