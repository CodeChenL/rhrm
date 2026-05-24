use eframe::egui;
use std::time::Duration;

use crate::app_state::AppState;
use super::float_window_controller::FloatWindowLayout;

const FLOAT_WINDOW_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY_MAX_SECONDS: usize = 60;
const HISTORY_MAX_POINTS: f32 = (HISTORY_MAX_SECONDS.saturating_sub(1)) as f32;
const HISTORY_GRID_LINES: usize = 4;
const ALERT_HEART_RATE_THRESHOLD: u16 = 120;
const ALERT_FLASH_INTERVAL_SECONDS: f32 = 0.5;

pub struct FloatWindowApp {
    state: AppState,
    heart_rate: Option<u16>,
    connecting: bool,
    click_through: bool,
    opacity: f32,
    last_applied_layout: Option<FloatWindowLayout>,
    last_applied_click_through: Option<bool>,
    last_plotted_heart_rate: Option<u16>,
}

impl FloatWindowApp {
    pub(crate) fn new(state: AppState, click_through: bool, opacity: f32) -> Self {
        Self {
            state,
            heart_rate: None,
            connecting: false,
            click_through,
            opacity,
            last_applied_layout: None,
            last_applied_click_through: None,
            last_plotted_heart_rate: None,
        }
    }

    fn sync_heart_rate(&mut self) {
        let snapshot = self.state.shared_snapshot();
        self.connecting = snapshot.connecting;
        self.heart_rate = snapshot.heart_rate;

        if let Some(heart_rate) = snapshot.heart_rate {
            self.last_plotted_heart_rate = Some(heart_rate);
        } else if !snapshot.connecting {
            self.last_plotted_heart_rate = None;
        }
    }

    pub(crate) fn set_click_through(&mut self, click_through: bool) {
        self.click_through = click_through;
    }

    pub(crate) fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    pub(crate) fn show_as_root(&mut self, ctx: &egui::Context) {
        self.apply_click_through_if_needed(ctx);
        self.render_contents(ctx);
    }

    pub(crate) fn show_as_root_with_layout(
        &mut self,
        ctx: &egui::Context,
        layout: FloatWindowLayout,
    ) {
        self.apply_root_layout_if_needed(ctx, layout);
        self.apply_click_through_if_needed(ctx);
        self.render_contents(ctx);
    }

    fn apply_root_layout_if_needed(&mut self, ctx: &egui::Context, layout: FloatWindowLayout) {
        if self.last_applied_layout == Some(layout) {
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            layout.x as f32,
            layout.y as f32,
        )));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            layout.width,
            layout.height,
        )));
        self.last_applied_layout = Some(layout);
    }

    fn apply_click_through_if_needed(&mut self, ctx: &egui::Context) {
        if self.last_applied_click_through == Some(self.click_through) {
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(self.click_through));
        self.last_applied_click_through = Some(self.click_through);
    }

    fn render_contents(&mut self, ctx: &egui::Context) {
        let background_alpha = (self.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        ctx.style_mut(|style| {
            style.visuals.panel_fill = egui::Color32::TRANSPARENT;
            style.visuals.window_fill = egui::Color32::TRANSPARENT;
        });

        self.sync_heart_rate();

        let heart_rate = self.heart_rate;
        let connecting = self.connecting;
        let history = self.state.history_snapshot();
        ctx.request_repaint_after(FLOAT_WINDOW_REFRESH_INTERVAL);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();

                if background_alpha > 0 {
                    ui.painter().rect_filled(
                        panel_rect,
                       0.0,
                        egui::Color32::from_rgba_unmultiplied(15, 15, 20, background_alpha),
                    );
                }

                if let Some(heart_rate) = heart_rate.filter(|rate| *rate > ALERT_HEART_RATE_THRESHOLD)
                {
                    let phase = (ctx.input(|input| input.time) as f32 / ALERT_FLASH_INTERVAL_SECONDS)
                        .fract();
                    let flash_alpha = if phase < 0.5 { 96 } else { 24 };
                    ui.painter().rect_filled(
                        panel_rect,
                       0.0,
                        egui::Color32::from_rgba_unmultiplied(255, 0, 0, flash_alpha),
                    );
                    let _ = heart_rate;
                }

                let text = if let Some(heart_rate) = heart_rate {
                    format!("❤ {} bpm", heart_rate)
                } else if connecting {
                    "❤ ...".to_string()
                } else {
                    "❤ --".to_string()
                };

                ui.painter().text(
                    panel_rect.min,
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                );

                if history.values.len() > 1 {
                    let desired_size = egui::vec2(
                        panel_rect.width().max(0.0),
                        panel_rect.height().max(0.0),
                    );
                    let (wave_rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
                    let history_min = history.values.iter().copied().min().unwrap_or(60) as f32;
                    let history_max = history.values.iter().copied().max().unwrap_or(100) as f32;
                    let hr_padding = 4.0;
                    let min_hr = (history_min - hr_padding).max(30.0);
                    let max_hr = (history_max + hr_padding).min(220.0).max(min_hr + 1.0);
                    let mut points = Vec::new();
                    for (index, heart_rate) in history.values.iter().enumerate() {
                        let x_ratio = index as f32 / HISTORY_MAX_POINTS;
                        let x = wave_rect.min.x + x_ratio * wave_rect.width();
                        let y = wave_rect.max.y
                            - ((*heart_rate as f32 - min_hr) / (max_hr - min_hr))
                                * wave_rect.height();
                        points.push(egui::pos2(x, y));
                    }

                    if points.len() > 1 {
                        for step in 1..=HISTORY_GRID_LINES {
                            let t = step as f32 / (HISTORY_GRID_LINES + 1) as f32;
                            let y = egui::lerp(wave_rect.bottom()..=wave_rect.top(), t);
                            ui.painter().line_segment(
                                [egui::pos2(wave_rect.left(), y), egui::pos2(wave_rect.right(), y)],
                                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 32)),
                            );
                        }
                        ui.painter().line(
                            points,
                            egui::Stroke::new(3.0, egui::Color32::from_rgb(80, 255, 120)),
                        );
                    }
                }
            });
    }

}

impl eframe::App for FloatWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.show_as_root(ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }
}
