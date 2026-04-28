use std::sync::{Arc, Mutex};
use std::time::Duration;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop, LoopHandle, RegistrationToken,
        },
        calloop_wayland_source::WaylandSource,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use crate::app_state::{AppState, SharedHeartRateSnapshot};
use crate::error::{AppError, AppResult};

use super::{FloatWindowPreset, FloatWindowSharedSnapshot, FloatWindowSharedState};

const OVERLAY_REDRAW_INTERVAL: Duration = Duration::from_millis(200);
const OVERLAY_BASELINE_WIDTH: u32 = 100;
const OVERLAY_BASELINE_HEIGHT: u32 = 100;
const TEXT_BLOCK_HEIGHT: u32 = 24;

#[derive(Clone)]
pub(crate) struct WaylandOverlayHandle {
    state: Arc<Mutex<WaylandOverlayState>>,
    ping: smithay_client_toolkit::reexports::calloop::ping::Ping,
}

struct WaylandOverlayState {
    stop_requested: bool,
    running: bool,
}

impl WaylandOverlayHandle {
    pub(crate) fn request_stop(&self) {
        let mut state = self.state.lock().unwrap();
        state.stop_requested = true;
        self.ping.ping();
    }

    pub(crate) fn is_running(&self) -> bool {
        self.state.lock().unwrap().running
    }
}

pub(crate) fn spawn_wayland_overlay(
    app_state: AppState,
    shared_window: FloatWindowSharedState,
) -> AppResult<WaylandOverlayHandle> {
    let (ping, ping_source) = smithay_client_toolkit::reexports::calloop::ping::make_ping().map_err(|error| {
        AppError::Bluetooth(format!("failed to create exit ping source: {error}"))
    })?;

    let handle = WaylandOverlayHandle {
        state: Arc::new(Mutex::new(WaylandOverlayState {
            stop_requested: false,
            running: true,
        })),
        ping,
    };

    let worker_handle = handle.clone();
    std::thread::Builder::new()
        .name("wayland-overlay".to_owned())
        .spawn(move || {
            if let Err(error) = run_wayland_overlay(app_state, shared_window, worker_handle.clone(), ping_source) {
                log::error!("Wayland overlay failed: {error}");
            }

            let mut state = worker_handle.state.lock().unwrap();
            state.running = false;
        })
        .map_err(AppError::Io)?;

    Ok(handle)
}

fn run_wayland_overlay(
    app_state: AppState,
    shared_window: FloatWindowSharedState,
    control: WaylandOverlayHandle,
    ping_source: smithay_client_toolkit::reexports::calloop::ping::PingSource,
) -> AppResult<()> {
    let conn = Connection::connect_to_env().map_err(|error| {
        AppError::Bluetooth(format!("failed to connect to Wayland compositor: {error}"))
    })?;

    let (globals, event_queue) = registry_queue_init(&conn)
        .map_err(|error| AppError::Bluetooth(format!("failed to initialize Wayland registry: {error}")))?;
    let qh: QueueHandle<WaylandOverlayApp> = event_queue.handle();
    let mut event_loop: EventLoop<WaylandOverlayApp> = EventLoop::try_new()
        .map_err(|error| AppError::Bluetooth(format!("failed to create Wayland event loop: {error}")))?;

    let compositor_state = CompositorState::bind(&globals, &qh)
        .map_err(|error| AppError::Bluetooth(format!("wl_compositor unavailable: {error}")))?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .map_err(|error| AppError::Bluetooth(format!("layer-shell unavailable: {error}")))?;
    let shm = Shm::bind(&globals, &qh)
        .map_err(|error| AppError::Bluetooth(format!("wl_shm unavailable: {error}")))?;
    let output_state = OutputState::new(&globals, &qh);
    let registry_state = RegistryState::new(&globals);

    let surface = compositor_state.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("rhrm-overlay"),
        None,
    );
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_exclusive_zone(-1);
    layer.set_size(OVERLAY_BASELINE_WIDTH, OVERLAY_BASELINE_HEIGHT);
    apply_anchor_and_margin(&layer, shared_window.snapshot().preset);
    layer.commit();

    let pool = SlotPool::new(
        OVERLAY_BASELINE_WIDTH as usize * OVERLAY_BASELINE_HEIGHT as usize * 4,
        &shm,
    )
    .map_err(|error| AppError::Bluetooth(format!("failed to create Wayland shm pool: {error}")))?;

    let mut app = WaylandOverlayApp {
        loop_handle: event_loop.handle(),
        registry_state,
        compositor_state,
        output_state,
        shm,
        app_state,
        shared_window,
        control,
        layer,
        pool,
        width: OVERLAY_BASELINE_WIDTH,
        height: OVERLAY_BASELINE_HEIGHT,
        first_configure: true,
        configured: false,
        last_rendered_revision: None,
        redraw_token: None,
        exit: false,
    };

    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|error| AppError::Bluetooth(format!("failed to install Wayland source: {error}")))?;
    app.install_redraw_timer()?;
    app.install_exit_ping(ping_source)?;

    while !app.exit {
        event_loop
            .dispatch(Duration::from_millis(50), &mut app)
            .map_err(|error| AppError::Bluetooth(format!("Wayland event loop error: {error}")))?;
    }

    Ok(())
}

struct WaylandOverlayApp {
    loop_handle: LoopHandle<'static, Self>,
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    shm: Shm,
    app_state: AppState,
    shared_window: FloatWindowSharedState,
    control: WaylandOverlayHandle,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    first_configure: bool,
    configured: bool,
    last_rendered_revision: Option<u64>,
    redraw_token: Option<RegistrationToken>,
    exit: bool,
}

impl WaylandOverlayApp {
    fn install_redraw_timer(&mut self) -> AppResult<()> {
        let token = self
            .loop_handle
            .insert_source(Timer::from_duration(OVERLAY_REDRAW_INTERVAL), |_, _, app| {
                app.tick();
                TimeoutAction::ToDuration(OVERLAY_REDRAW_INTERVAL)
            })
            .map_err(|error| AppError::Bluetooth(format!("failed to install redraw timer: {error}")))?;
        self.redraw_token = Some(token);
        Ok(())
    }

    fn install_exit_ping(&mut self, ping_source: smithay_client_toolkit::reexports::calloop::ping::PingSource) -> AppResult<()> {
        self.loop_handle
            .insert_source(ping_source, |_, _, app| {
                app.exit = true;
            })
            .map_err(|error| AppError::Bluetooth(format!("failed to install exit ping source: {error}")))?;
        Ok(())
    }

    fn tick(&mut self) {
        if self.exit {
            return;
        }

        if self.control.state.lock().unwrap().stop_requested {
            self.exit = true;
            return;
        }

        if !self.configured {
            return;
        }

        let shared = self.shared_window.snapshot();
        if !shared.open {
            return;
        }

        if self.last_rendered_revision == Some(shared.revision) {
            return;
        }

        if let Err(error) = self.render(shared) {
            log::error!("Wayland overlay render failed: {error}");
            self.exit = true;
        }
    }

    fn render(&mut self, shared: FloatWindowSharedSnapshot) -> AppResult<()> {
        self.width = shared.layout.width.max(1.0).round() as u32;
        self.height = shared.layout.height.max(1.0).round() as u32;

        apply_anchor_and_margin(&self.layer, shared.preset);
        self.layer.set_size(self.width, self.height);
        self.update_input_region(shared.layout.click_through)?;

        let stride = self.width as i32 * 4;
        let (buffer, canvas) = self
            .pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|error| AppError::Bluetooth(format!("failed to create Wayland buffer: {error}")))?;

        let hr = self.app_state.shared_snapshot();
        draw_overlay(canvas, self.width, self.height, shared.layout.opacity, hr);

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .map_err(|error| AppError::Bluetooth(format!("failed to attach Wayland buffer: {error}")))?;
        self.layer.commit();

        self.last_rendered_revision = Some(shared.revision);
        Ok(())
    }

    fn update_input_region(&self, click_through: bool) -> AppResult<()> {
        if click_through {
            let region = Region::new(&self.compositor_state)
                .map_err(|error| AppError::Bluetooth(format!("failed to create input region: {error}")))?;
            region.add(0, 0, 0, 0);
            self.layer.set_input_region(Some(region.wl_region()));
        } else {
            let region = Region::new(&self.compositor_state)
                .map_err(|error| AppError::Bluetooth(format!("failed to create input region: {error}")))?;
            region.add(0, 0, self.width as i32, self.height as i32);
            self.layer.set_input_region(Some(region.wl_region()));
        }
        Ok(())
    }
}

fn apply_anchor_and_margin(layer: &LayerSurface, preset: FloatWindowPreset) {
    use super::FLOAT_WINDOW_MARGIN;

    let margin = FLOAT_WINDOW_MARGIN;
    let (anchor, top, right, bottom, left) = match preset {
        FloatWindowPreset::TopLeft => (Anchor::TOP | Anchor::LEFT, margin, 0, 0, margin),
        FloatWindowPreset::TopCenter => (Anchor::TOP, margin, 0, 0, 0),
        FloatWindowPreset::TopRight => (Anchor::TOP | Anchor::RIGHT, margin, margin, 0, 0),
        FloatWindowPreset::MiddleLeft => (Anchor::LEFT, 0, 0, 0, margin),
        FloatWindowPreset::MiddleRight => (Anchor::RIGHT, 0, margin, 0, 0),
        FloatWindowPreset::BottomLeft => (Anchor::BOTTOM | Anchor::LEFT, 0, 0, margin, margin),
        FloatWindowPreset::BottomCenter => (Anchor::BOTTOM, 0, 0, margin, 0),
        FloatWindowPreset::BottomRight => (Anchor::BOTTOM | Anchor::RIGHT, 0, margin, margin, 0),
        FloatWindowPreset::Center => (Anchor::empty(), 0, 0, 0, 0),
    };

    layer.set_anchor(anchor);
    layer.set_margin(top, right, bottom, left);
}

fn draw_overlay(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    opacity: f32,
    snapshot: SharedHeartRateSnapshot,
) {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    let bg = [15, 15, 20, alpha];
    let fg = [255, 255, 255, 255];
    let alert = [255, 0, 0, 72];

    for pixel in canvas.chunks_exact_mut(4) {
        pixel.copy_from_slice(&bg);
    }

    if snapshot.heart_rate.unwrap_or_default() > 120 {
        for pixel in canvas.chunks_exact_mut(4) {
            let blended = blend_rgba([pixel[2], pixel[1], pixel[0], pixel[3]], alert);
            pixel[0] = blended[2];
            pixel[1] = blended[1];
            pixel[2] = blended[0];
            pixel[3] = blended[3];
        }
    }

    let text = if let Some(heart_rate) = snapshot.heart_rate {
        format!("❤ {heart_rate} bpm")
    } else if snapshot.connecting {
        "❤ ...".to_owned()
    } else {
        "❤ --".to_owned()
    };

    draw_text(canvas, width, height, 6, 4, &text, fg);
    draw_rule(canvas, width, height, TEXT_BLOCK_HEIGHT.min(height.saturating_sub(1)), [255, 255, 255, 40]);
}

fn blend_rgba(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let src_a = src[3] as f32 / 255.0;
    let dst_a = dst[3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        return [0, 0, 0, 0];
    }

    let blend = |s: u8, d: u8| -> u8 {
        (((s as f32 * src_a) + (d as f32 * dst_a * (1.0 - src_a))) / out_a)
            .round()
            .clamp(0.0, 255.0) as u8
    };

    [
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

fn draw_rule(canvas: &mut [u8], width: u32, height: u32, y: u32, color: [u8; 4]) {
    if y >= height {
        return;
    }

    for x in 0..width {
        put_pixel(canvas, width, x, y, color);
    }
}

fn draw_text(canvas: &mut [u8], width: u32, height: u32, start_x: u32, start_y: u32, text: &str, color: [u8; 4]) {
    let mut x = start_x;
    for ch in text.chars() {
        if let Some(bitmap) = glyph_rows(ch) {
            draw_glyph(canvas, width, height, x, start_y, bitmap, color);
        }
        x = x.saturating_add(6);
        if x + 5 >= width {
            break;
        }
    }
}

fn draw_glyph(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    start_x: u32,
    start_y: u32,
    rows: [u8; 7],
    color: [u8; 4],
) {
    for (row_index, row_bits) in rows.into_iter().enumerate() {
        let y = start_y + row_index as u32;
        if y >= height {
            break;
        }
        for bit in 0..5 {
            if row_bits & (1 << (4 - bit)) == 0 {
                continue;
            }
            let x = start_x + bit;
            if x >= width {
                continue;
            }
            put_pixel(canvas, width, x, y, color);
        }
    }
}

fn put_pixel(canvas: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= canvas.len() {
        return;
    }

    canvas[idx] = color[2];
    canvas[idx + 1] = color[1];
    canvas[idx + 2] = color[0];
    canvas[idx + 3] = color[3];
}

fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    if ch == '\u{20}' {
        return Some([0, 0, 0, 0, 0, 0, 0]);
    }

    match ch {
        '.' => Some([0, 0, 0, 0, 0, 0b00100, 0]),
        '-' => Some([0, 0, 0, 0b00111, 0, 0, 0]),
        '0' => Some([0b01110, 0b10001, 0b10011, 0b101, 0b11001, 0b10001, 0b01110]),
        '1' => Some([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => Some([0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b100, 0b111]),
        '3' => Some([0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110]),
        '4' => Some([0b00010, 0b00110, 0b010, 0b10010, 0b111, 0b00010, 0b00010]),
        '5' => Some([0b111, 0b100, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
        '6' => Some([0b00110, 0b01000, 0b100, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => Some([0b111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => Some([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => Some([0b01110, 0b10001, 0b10001, 0b011, 0b00001, 0b00010, 0b11100]),
        'a' | 'A' => Some([0b01110, 0b10001, 0b10001, 0b111, 0b10001, 0b10001, 0b10001]),
        'b' | 'B' => Some([0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
        'm' | 'M' => Some([0b10001, 0b11011, 0b101, 0b101, 0b10001, 0b10001, 0b10001]),
        'p' | 'P' => Some([0b11110, 0b10001, 0b10001, 0b11110, 0b100, 0b100, 0b100]),
        '❤' => Some([0b010, 0b111, 0b111, 0b111, 0b01110, 0b00100, 0]),
        _ => Some([0b111, 0b10001, 0b00110, 0b00100, 0b00110, 0b10001, 0b111]),
    }
}

impl CompositorHandler for WaylandOverlayApp {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandOverlayApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: wl_output::WlOutput) {}

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for WaylandOverlayApp {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 != 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 != 0 {
            self.height = configure.new_size.1;
        }

        self.configured = true;
        if self.first_configure {
            self.first_configure = false;
            self.last_rendered_revision = None;
        }
    }
}

impl ShmHandler for WaylandOverlayApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(WaylandOverlayApp);
delegate_output!(WaylandOverlayApp);
delegate_shm!(WaylandOverlayApp);
delegate_layer!(WaylandOverlayApp);
delegate_registry!(WaylandOverlayApp);

impl ProvidesRegistryState for WaylandOverlayApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}