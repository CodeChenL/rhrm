use bluest::{btuuid::bluetooth_uuid_from_u16, Adapter, Device, Uuid};
use chrono::Local;
use eframe::egui;
use futures_lite::stream::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// 心率服务 UUID (16位)
const HRS_UUID: Uuid = bluetooth_uuid_from_u16(0x180D);
const HRM_UUID: Uuid = bluetooth_uuid_from_u16(0x2A37);

// 设备信息
#[derive(Clone, Debug)]
struct DeviceInfo {
    name: String,
    addr: String,
    rssi: i16,
    heart_rate: Option<u16>,
    last_seen: String,
    connected: bool,
}

// 应用状态
struct BTMonitorApp {
    devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    scanning: Arc<Mutex<bool>>,
    selected_device: Arc<Mutex<Option<String>>>,
    connecting: Arc<Mutex<bool>>,
    error_message: Arc<Mutex<Option<String>>>,
    float_mode: bool,
}

impl Default for BTMonitorApp {
    fn default() -> Self {
        Self {
            devices: Arc::new(Mutex::new(HashMap::new())),
            scanning: Arc::new(Mutex::new(false)),
            selected_device: Arc::new(Mutex::new(None)),
            connecting: Arc::new(Mutex::new(false)),
            error_message: Arc::new(Mutex::new(None)),
            float_mode: false,
        }
    }
}

// 浮窗应用状态
struct FloatWindowApp {
    heart_rate: Arc<Mutex<Option<u16>>>,
    connecting: Arc<Mutex<bool>>,
    device_addr: Arc<Mutex<Option<String>>>,
}

impl Default for FloatWindowApp {
    fn default() -> Self {
        Self {
            heart_rate: Arc::new(Mutex::new(None)),
            connecting: Arc::new(Mutex::new(false)),
            device_addr: Arc::new(Mutex::new(None)),
        }
    }
}

impl FloatWindowApp {
    fn start_connection(&self) {
        let heart_rate = Arc::clone(&self.heart_rate);
        let connecting = Arc::clone(&self.connecting);
        let device_addr = Arc::clone(&self.device_addr);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                *connecting.lock().unwrap() = true;

                let adapter = match Adapter::default().await {
                    Some(a) => a,
                    None => {
                        *connecting.lock().unwrap() = false;
                        return;
                    }
                };

                if let Err(e) = adapter.wait_available().await {
                    log::error!("Adapter not available: {}", e);
                    *connecting.lock().unwrap() = false;
                    return;
                }

                // 查找已连接的心率设备
                let connected_devices = match adapter.connected_devices_with_services(&[HRS_UUID]).await {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("Failed to get connected devices: {}", e);
                        *connecting.lock().unwrap() = false;
                        return;
                    }
                };

                let device = match connected_devices.into_iter().find(|_d| {
                    // 检查设备是否有心率服务
                    true // 简化：尝试连接任何已连接设备
                }) {
                    Some(d) => d,
                    None => {
                        log::info!("No connected HR device found, scanning...");
                        *connecting.lock().unwrap() = false;
                        return;
                    }
                };

                let addr = format!("{:?}", device.id());
                *device_addr.lock().unwrap() = Some(addr.clone());
                log::info!("Connecting to {}...", addr);

                if !device.is_connected() {
                    if let Err(e) = adapter.connect_device(&device).await {
                        log::error!("Failed to connect: {}", e);
                        *connecting.lock().unwrap() = false;
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                // 查找心率特征
                match find_heart_rate_characteristic(&device).await {
                    Ok(char) => {
                        log::info!("Found HR characteristic, subscribing...");
                        match char.notify().await {
                            Ok(mut updates) => {
                                while let Some(result) = updates.next().await {
                                    if let Ok(data) = result {
                                        if let Some(hr) = parse_heart_rate(&data) {
                                            *heart_rate.lock().unwrap() = Some(hr);
                                            log::info!("HR: {} bpm", hr);
                                        }
                                    }
                                }
                            }
                            Err(e) => log::error!("Notify error: {}", e),
                        }
                    }
                    Err(e) => log::error!("Find char error: {}", e),
                }

                *connecting.lock().unwrap() = false;
            });
        });
    }
}

impl eframe::App for FloatWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(200));

        let heart_rate = *self.heart_rate.lock().unwrap();
        let connecting = *self.connecting.lock().unwrap();

        // 启动连接（如果没有正在连接且没有心率数据）
        if !connecting && heart_rate.is_none() {
            self.start_connection();
        }

        let bg_color = if heart_rate.is_some() {
            egui::Color32::from_rgba_unmultiplied(20, 40, 30, 230)
        } else if connecting {
            egui::Color32::from_rgba_unmultiplied(40, 40, 30, 230)
        } else {
            egui::Color32::from_rgba_unmultiplied(30, 30, 40, 230)
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg_color).corner_radius(8.0))
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    // 标题
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("❤️").size(14.0));
                        ui.label(egui::RichText::new("HR").size(12.0).color(egui::Color32::LIGHT_GRAY));
                    });

                    ui.separator();

                    // 心率显示
                    if let Some(hr) = heart_rate {
                        let color = match hr {
                            60..=100 => egui::Color32::GREEN,
                            101..=120 => egui::Color32::YELLOW,
                            _ => egui::Color32::RED,
                        };
                        ui.label(egui::RichText::new(format!("{}", hr)).size(36.0).color(color));
                        ui.label(egui::RichText::new("bpm").size(11.0).color(egui::Color32::GRAY));
                    } else if connecting {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("...").size(11.0).color(egui::Color32::LIGHT_GRAY));
                        });
                    } else {
                        ui.label(egui::RichText::new("--").size(36.0).color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new("bpm").size(11.0).color(egui::Color32::GRAY));
                    }
                });
            });
    }
}

impl BTMonitorApp {
    fn toggle_scan(&self) {
        let mut scanning = self.scanning.lock().unwrap();
        *scanning = !*scanning;

        if *scanning {
            *self.error_message.lock().unwrap() = None;
            let devices = Arc::clone(&self.devices);
            let error_msg = Arc::clone(&self.error_message);
            let scanning_flag = Arc::clone(&self.scanning);

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = run_bluetooth_scan(devices, error_msg, scanning_flag).await {
                        log::error!("Scan error: {}", e);
                    }
                });
            });
        }
    }

    fn connect_device(&self, addr: String) {
        *self.selected_device.lock().unwrap() = Some(addr.clone());
        *self.connecting.lock().unwrap() = true;

        let devices = Arc::clone(&self.devices);
        let selected = Arc::clone(&self.selected_device);
        let connecting = Arc::clone(&self.connecting);
        let error_msg = Arc::clone(&self.error_message);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = connect_and_monitor_hr(addr, devices, selected, connecting, error_msg).await {
                    log::error!("Connection error: {}", e);
                }
            });
        });
    }

    fn disconnect_device(&self) {
        *self.selected_device.lock().unwrap() = None;
        *self.connecting.lock().unwrap() = false;
    }

    fn show_float_window(&mut self, ctx: &egui::Context) {
        // 获取当前连接设备的心率
        let devices = self.devices.lock().unwrap();
        let selected = self.selected_device.lock().unwrap();
        let connecting = *self.connecting.lock().unwrap();

        let (heart_rate, _device_name, connected) = if let Some(addr) = selected.as_ref() {
            if let Some(d) = devices.get(addr) {
                (d.heart_rate, d.name.clone(), d.connected)
            } else {
                (None, String::new(), false)
            }
        } else {
            (None, String::new(), false)
        };
        drop(devices);

        // 浮窗背景
        let bg_color = if connected {
            egui::Color32::from_rgba_unmultiplied(20, 40, 30, 230)
        } else if connecting {
            egui::Color32::from_rgba_unmultiplied(40, 40, 30, 230)
        } else {
            egui::Color32::from_rgba_unmultiplied(30, 30, 40, 230)
        };

        egui::Window::new("HR")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
            .fixed_size(egui::vec2(90.0, 85.0))
            .frame(egui::Frame::new().fill(bg_color).corner_radius(8.0))
            .show(ctx, |ui| {
                // 标题栏
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("❤️").size(14.0));
                    ui.label(egui::RichText::new("HR").size(12.0).color(egui::Color32::LIGHT_GRAY));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("✕").size(12.0)).clicked() {
                            self.float_mode = false;
                        }
                    });
                });

                ui.separator();

                // 心率显示
                if let Some(hr) = heart_rate {
                    let color = match hr {
                        60..=100 => egui::Color32::GREEN,
                        101..=120 => egui::Color32::YELLOW,
                        _ => egui::Color32::RED,
                    };
                    ui.label(egui::RichText::new(format!("{}", hr)).size(36.0).color(color));
                    ui.label(egui::RichText::new("bpm").size(11.0).color(egui::Color32::GRAY));
                } else if connecting {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new("...").size(11.0).color(egui::Color32::LIGHT_GRAY));
                    });
                } else {
                    ui.label(egui::RichText::new("--").size(36.0).color(egui::Color32::GRAY));
                    ui.label(egui::RichText::new("bpm").size(11.0).color(egui::Color32::GRAY));
                }
            });
    }
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Starting BT Heart Rate Monitor...");

    let args: Vec<String> = std::env::args().collect();
    let float_mode = args.iter().any(|a| a == "--float");

    if float_mode {
        run_float_window()
    } else {
        run_main_window()
    }
}

fn run_main_window() -> eframe::Result<()> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options.viewport.clone()
        .with_transparent(true)
        .with_decorations(true)
        .with_always_on_top();

    eframe::run_native(
        "BT Heart Rate",
        native_options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();
            if let Ok(data) = std::fs::read("MonaspaceArgon-ExtraBold.otf") {
                fonts.font_data.insert(
                    "noto_cjk".to_owned(),
                    std::sync::Arc::new(egui::FontData::from_owned(data)),
                );
                fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "noto_cjk".to_owned());
                fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "noto_cjk".to_owned());
            }
            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(BTMonitorApp::default()))
        }),
    )
}

fn run_float_window() -> eframe::Result<()> {
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options.viewport.clone()
        .with_transparent(true)
        .with_decorations(false)
        .with_always_on_top()
        .with_titlebar_shown(false)
        .with_inner_size(egui::vec2(90.0, 85.0))
        .with_resizable(false);

    eframe::run_native(
        "HR Float",
        native_options,
        Box::new(|_cc| {
            Ok(Box::new(FloatWindowApp::default()))
        }),
    )
}

fn parse_heart_rate(data: &[u8]) -> Option<u16> {
    if data.is_empty() {
        return None;
    }
    let flag = data[0];
    if data.len() >= 2 {
        let mut hr = data[1] as u16;
        if flag & 0b00001 != 0 {
            if data.len() >= 3 {
                hr |= (data[2] as u16) << 8;
            }
        }
        Some(hr)
    } else {
        None
    }
}

async fn find_heart_rate_characteristic(device: &Device) -> Result<bluest::Characteristic, Box<dyn std::error::Error + Send + Sync>> {
    const MAX_RETRIES: usize = 10;
    const RETRY_DELAY_MS: u64 = 500;

    for attempt in 1..=MAX_RETRIES {
        // 发现心率服务
        let heart_rate_services = device.discover_services_with_uuid(HRS_UUID).await?;
        let heart_rate_service = heart_rate_services
            .first()
            .ok_or("No heart rate service")?;

        // 发现心率服务所有特征
        let all_characteristics = heart_rate_service.discover_characteristics().await?;

        // 查找心率测量特征 (0x2A37)
        if let Some(char) = all_characteristics
            .iter()
            .find(|c| c.uuid() == HRM_UUID)
        {
            log::info!("Found HR characteristic on attempt {}", attempt);
            return Ok(char.clone());
        }

        log::warn!("Attempt {}: No HR characteristic found, retrying...", attempt);
        tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
    }

    Err("Failed to find heart rate characteristic after retries".into())
}

async fn run_bluetooth_scan(
    devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    error_msg: Arc<Mutex<Option<String>>>,
    scanning_flag: Arc<Mutex<bool>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let adapter = match Adapter::default().await {
        Some(a) => a,
        None => {
            *error_msg.lock().unwrap() = Some("No Bluetooth adapter".to_string());
            return Ok(());
        }
    };

    log::info!("Using adapter, waiting for available...");
    adapter.wait_available().await?;
    log::info!("Adapter available");

    // 先检查已连接的设备
    let connected = adapter.connected_devices_with_services(&[HRS_UUID]).await?;
    for device in connected {
        let addr = format!("{:?}", device.id());
        let name = device.name_async().await.unwrap_or_else(|| "Unknown".to_string());
        let now = Local::now().format("%H:%M:%S").to_string();

        log::info!("Found connected device: {} [{}]", name, addr);

        let mut devs = devices.lock().unwrap();
        devs.insert(addr.clone(), DeviceInfo {
            name,
            addr,
            rssi: 0,
            heart_rate: None,
            last_seen: now,
            connected: true,
        });
    }

    while *scanning_flag.lock().unwrap() {
        log::info!("Starting scan...");
        *error_msg.lock().unwrap() = Some("Scanning...".to_string());

        let mut scan = match adapter.discover_devices(&[HRS_UUID]).await {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Scan error: {}", e);
                *error_msg.lock().unwrap() = Some(format!("Scan error: {}", e));
                tokio::time::sleep(Duration::from_millis(1000)).await;
                continue;
            }
        };

        loop {
            tokio::select! {
                result = scan.next() => {
                    match result {
                        Some(Ok(device)) => {
                            let addr = format!("{:?}", device.id());
                            let name = device.name_async().await.unwrap_or_else(|| "Unknown".to_string());
                            let now = Local::now().format("%H:%M:%S").to_string();

                            log::info!("Found device: {} [{}]", name, addr);

                            let mut devs = devices.lock().unwrap();
                            devs.insert(addr.clone(), DeviceInfo {
                                name,
                                addr,
                                rssi: 0,
                                heart_rate: None,
                                last_seen: now,
                                connected: false,
                            });
                        }
                        Some(Err(e)) => {
                            log::warn!("Device error: {}", e);
                        }
                        None => {
                            log::info!("Scan finished, restarting...");
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    log::info!("Scan timeout, restarting...");
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

async fn connect_and_monitor_hr(
    addr: String,
    devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    selected: Arc<Mutex<Option<String>>>,
    connecting: Arc<Mutex<bool>>,
    error_msg: Arc<Mutex<Option<String>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let adapter = match Adapter::default().await {
        Some(a) => a,
        None => {
            *error_msg.lock().unwrap() = Some("No adapter".to_string());
            *connecting.lock().unwrap() = false;
            return Ok(());
        }
    };

    adapter.wait_available().await?;

    // 获取目标设备：先检查已连接的，再扫描
    let device: Device = {
        let connected_devices = adapter.connected_devices_with_services(&[HRS_UUID]).await?;
        if let Some(d) = connected_devices.into_iter().find(|d| format!("{:?}", d.id()) == addr) {
            log::info!("Using already connected device");
            d
        } else {
            log::info!("Scanning for device {}...", addr);
            *error_msg.lock().unwrap() = Some("Scanning...".to_string());

            let mut scan = adapter.discover_devices(&[HRS_UUID]).await?;
            let found = scan.next().await.ok_or("Scan timeout")??;
            found
        }
    };

    // 连接设备（如果未连接）
    log::info!("Connecting to {}...", addr);
    *error_msg.lock().unwrap() = Some("Connecting...".to_string());

    if !device.is_connected() {
        adapter.connect_device(&device).await?;
        // 等待设备准备好
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    log::info!("Connected to {}", addr);
    *error_msg.lock().unwrap() = Some("Reading HR...".to_string());

    // 更新连接状态
    { devices.lock().unwrap().entry(addr.clone()).and_modify(|d| d.connected = true); }

    // 查找心率特征，支持重试
    let heart_rate_measurement = find_heart_rate_characteristic(&device).await?;

    log::info!("Subscribing to notifications...");

    // 打印特征属性
    log::info!("Subscribing to notifications...");

    // 订阅通知
    let mut updates = heart_rate_measurement.notify().await?;

    log::info!("Listening for heart rate...");

    // 监听心率数据
    loop {
        tokio::select! {
            result = updates.next() => {
                match result {
                    Some(Ok(heart_rate)) => {
                        if let Some(hr) = parse_heart_rate(&heart_rate) {
                            let now = Local::now().format("%H:%M:%S").to_string();
                            let mut devs = devices.lock().unwrap();
                            if let Some(d) = devs.get_mut(&addr) {
                                d.heart_rate = Some(hr);
                                d.last_seen = now;
                            }
                            log::info!("HR: {} bpm", hr);
                        }
                    }
                    Some(Err(e)) => {
                        log::error!("Error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if selected.lock().unwrap().as_ref() != Some(&addr) {
                    break;
                }
            }
        }
    }

    *connecting.lock().unwrap() = false;
    { devices.lock().unwrap().entry(addr.clone()).and_modify(|d| d.connected = false); }

    Ok(())
}

impl eframe::App for BTMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(200));

        // 如果是浮窗模式，显示浮窗
        if self.float_mode {
            self.show_float_window(ctx);
            return;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("BT Heart Rate");
                    ui.separator();
                    let is_scanning = *self.scanning.lock().unwrap();
                    if ui.button(if is_scanning { "Stop" } else { "Scan" }).clicked() {
                        self.toggle_scan();
                    }
                    ui.separator();
                    if ui.button(if self.float_mode { "📐 Normal" } else { "🪟 Float" }).clicked() {
                        self.float_mode = !self.float_mode;
                    }
                });

                ui.separator();

                if *self.connecting.lock().unwrap() {
                    ui.label(egui::RichText::new("Connected - Receiving HR...").color(egui::Color32::GREEN));
                }

                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    let devices = self.devices.lock().unwrap();
                    let mut list: Vec<_> = devices.values().collect();
                    list.sort_by(|a, b| b.rssi.cmp(&a.rssi));

                    if list.is_empty() {
                        ui.label(egui::RichText::new("Scanning... Click device to connect").color(egui::Color32::GRAY));
                    }

                    for device in list {
                        let selected = self.selected_device.lock().unwrap().clone();
                        let is_selected = selected.as_ref().map(|s| s == &device.addr).unwrap_or(false);

                        let bg = if is_selected {
                            if device.connected { egui::Color32::from_rgba_unmultiplied(40, 100, 60, 200) }
                            else { egui::Color32::from_rgba_unmultiplied(60, 80, 120, 200) }
                        } else {
                            egui::Color32::from_rgba_unmultiplied(40, 40, 60, 150)
                        };

                        let rect = ui.available_rect_before_wrap();
                        let card_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), if is_selected { 110.0 } else { 80.0 }));

                        ui.painter().rect_filled(card_rect.expand(2.0), 4.0, bg);

                        let resp = ui.interact(card_rect, egui::Id::new(&device.addr), egui::Sense::click());
                        if resp.clicked() {
                            if is_selected && device.connected { self.disconnect_device(); }
                            else { self.connect_device(device.addr.clone()); }
                        }

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&device.name).strong().color(egui::Color32::WHITE));
                            if device.connected { ui.label(egui::RichText::new("●").color(egui::Color32::GREEN)); }
                        });

                        ui.horizontal(|ui| ui.label(egui::RichText::new(&device.addr).small().color(egui::Color32::GRAY)));

                        if let Some(hr) = device.heart_rate {
                            ui.horizontal(|ui| {
                                let c = match hr { 60..=100 => egui::Color32::GREEN, 101..=120 => egui::Color32::YELLOW, _ => egui::Color32::RED };
                                ui.label(egui::RichText::new(format!("❤️ {} bpm", hr)).size(20.0).color(c));
                            });
                        } else if is_selected && !device.connected {
                            ui.label(egui::RichText::new("Click to connect...").color(egui::Color32::LIGHT_GRAY));
                        }

                        ui.horizontal(|ui| ui.label(egui::RichText::new(format!("Time: {}", device.last_seen)).small().color(egui::Color32::GRAY)));
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("Devices: {}", self.devices.lock().unwrap().len()));
                    if let Some(e) = &*self.error_message.lock().unwrap() {
                        ui.colored_label(egui::Color32::YELLOW, e);
                    }
                });
            });
    }
}
