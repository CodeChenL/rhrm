use eframe::egui;

use super::float_window_controller::FloatWindowLayout;
use crate::app_state::AppState;

pub struct FloatWindowApp {
    state: AppState,
    heart_rate: Option<u16>,
    connecting: bool,
    click_through: bool,
    opacity: f32,
    hr_history: Vec<u16>,
}

impl FloatWindowApp {
    pub(crate) fn new(state: AppState, click_through: bool, opacity: f32) -> Self {
        Self {
            state,
            heart_rate: None,
            connecting: false,
            click_through,
            opacity,
            hr_history: Vec::with_capacity(200),
        }
    }

    fn sync_heart_rate(&mut self) -> bool {
        let snapshot = self.state.shared_snapshot();
        self.connecting = snapshot.connecting;
        self.heart_rate = snapshot.heart_rate;
        if let Some(heart_rate) = snapshot.heart_rate {
            if self.hr_history.last().copied() != Some(heart_rate) {
                self.hr_history.push(heart_rate);
                if self.hr_history.len() > 200 {
                    self.hr_history.remove(0);
                }
            }
        }
        true
    }

    pub(crate) fn set_click_through(&mut self, click_through: bool) {
        self.click_through = click_through;
    }

    pub(crate) fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.1, 1.0);
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context, layout: FloatWindowLayout) {
        ctx.send_viewport_cmd_to(
            egui::ViewportId::from_hash_of("float_window"),
            egui::ViewportCommand::MousePassthrough(self.click_through),
        );
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("float_window"),
            egui::ViewportBuilder::default()
                .with_title("HR Float")
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top()
                .with_inner_size(egui::vec2(layout.width, layout.height))
                .with_position(egui::pos2(layout.x as f32, layout.y as f32))
                .with_resizable(true),
            |ctx, _class| {
                self.render_contents(ctx);
            },
        );
    }

    fn render_contents(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        let background_alpha = (self.opacity * 160.0).round() as u8;
        ctx.style_mut(|style| {
            style.visuals.panel_fill = egui::Color32::TRANSPARENT;
            style.visuals.window_fill = egui::Color32::TRANSPARENT;
        });

        if self.sync_heart_rate() {
            ctx.request_repaint();
        }

        let heart_rate = self.heart_rate;
        let connecting = self.connecting;
        let hr_history = &self.hr_history;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();
                ui.painter().rect_filled(
                    panel_rect,
                    8.0,
                    egui::Color32::from_rgba_unmultiplied(15, 15, 20, background_alpha),
                );

                let text = if let Some(heart_rate) = heart_rate {
                    format!("❤️ {} bpm", heart_rate)
                } else if connecting {
                    "❤️ ...".to_string()
                } else {
                    "❤️ --".to_string()
                };

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(text)
                            .size(16.0)
                            .color(egui::Color32::WHITE),
                    );
                });

                if hr_history.len() > 1 {
                    let min_hr = 50.0;
                    let max_hr = 180.0;
                    let max_points = 100.0;
                    let desired_size = egui::vec2(
                        (panel_rect.width() - 16.0).max(0.0),
                        (panel_rect.height() - 36.0).max(0.0),
                    );
                    let (wave_rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
                    let mut points = Vec::new();
                    for (index, heart_rate) in hr_history.iter().enumerate() {
                        let x = wave_rect.min.x + (index as f32 / max_points) * wave_rect.width();
                        let y = wave_rect.max.y
                            - ((*heart_rate as f32 - min_hr) / (max_hr - min_hr))
                                * wave_rect.height();
                        points.push(egui::pos2(x, y));
                    }

                    if points.len() > 1 {
                        ui.painter().line(
                            points,
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 100)),
                        );
                    }
                }
            });
    }
}

impl eframe::App for FloatWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render_contents(ctx);
    }
}
