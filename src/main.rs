mod app_state;
mod bluetooth;
mod config;
mod error;
mod ui;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Starting rhrm...");

    ui::run_main_window()
}
