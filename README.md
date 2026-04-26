# RHRM — Rust Heart Rate Monitor

A desktop application for monitoring heart rate data from Bluetooth Low Energy (BLE) heart rate devices, built with Rust.

[中文文档](README_zh.md)

## Features

- **Bluetooth Device Discovery** — Scan for nearby BLE heart rate monitors with one click.
- **Real-Time Heart Rate Display** — Live BPM readout with color-coded status (green / yellow / red) and visual alerts when HR exceeds 120 bpm.
- **Floating Window** — Always-on-top overlay showing heart rate and 60-second history graph, designed to stay visible while using other applications.
- **Auto-Reconnect** — Automatically reconnects to the last connected device on startup.
- **Customizable Overlay** — Adjust position (9 presets), size, opacity, and enable click-through mode.

## System Requirements

- **OS**: Windows 10+ / Linux / macOS
- **Hardware**: Bluetooth 4.0+ adapter with BLE support
- **Heart Rate Device**: Any standard BLE heart rate monitor (HRS service `0x180D`)

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/CodeChenL/rhrm.git
cd rhrm/bt-monitor

# Build (release)
cargo build --release

# Run
cargo run
```

The release binary will be at `target/release/rhrm` (or `rhrm.exe` on Windows).

## Usage

1. **Launch** the application. The main control panel opens.
2. **Click "Scan"** to discover nearby BLE heart rate devices.
3. **Click a device** in the list to connect. The HR will appear once data starts streaming.
4. **The floating window** opens automatically and displays real-time heart rate with a history graph.
5. **Customize the floating window** from the control panel:
   - Choose a position preset from the 3×3 grid.
   - Adjust width, height, and opacity.
   - Toggle click-through mode to let mouse events pass through.

## Architecture

```
bt-monitor/
├── src/
│   ├── main.rs              # Entry point, logging setup
│   ├── app_state.rs         # Thread-safe shared application state
│   ├── config.rs            # TOML configuration persistence
│   ├── error.rs             # Custom error types
│   ├── bluetooth/
│   │   ├── mod.rs           # BLE UUID constants
│   │   ├── scan.rs          # BLE device discovery
│   │   ├── connection.rs    # Device connection & HR streaming
│   │   ├── parser.rs        # BLE packet parsing
│   │   └── service.rs       # Async command queue handler
│   └── ui/
│       ├── mod.rs
│       ├── main_window.rs   # Control panel
│       ├── float_window.rs  # Floating overlay with graph
│       └── float_window_controller.rs  # Float window lifecycle
├── Cargo.toml
├── assets_font_license.txt
├── README.md
└── README_zh.md
```

**Data Flow**: UI dispatches commands → `BluetoothService` spawns async tasks → BLE module discovers / connects / streams → `AppState` distributes updates → UI renders.

## Tech Stack

| Component     | Library       | Purpose                        |
| ------------- | ------------- | ----------------- |
| GUI           | egui / eframe | Cross-platform UI framework    |
| Bluetooth     | bluest        | BLE device communication       |
| Async Runtime | tokio         | Multi-threaded async execution |
| Config        | serde / toml  | Serialization & TOML parsing   |
| Error Handling| thiserror     | Derive macros for error types  |
| Logging       | log / env_loger | Runtime logging            |
| Time          | chrono        | Timestamp formatting           |

## Configuration

Configuration is stored in TOML format at:

| OS      | Path                                          |
| ------- | --------------------------------- |
| Windows | `%APDATA%\rhlm\rhlm.toml`                    |
| Linux   | `~/.config/rhlm/rhlm.toml`                    |

Settings include:
- Float window position preset, size, and opacity.
- Last connected device address (for auto-reconnect).
- Click-through mode toggle.

## License

This project's source code is available under the MIT License (or specify your license here).

Fonts bundled with this application (`UbuntuMono-R.ttf`, `NotoSansCJKsc-Regular.otf`) are licensed under the [SIL Open Font License 1.1](assets_font_license.txt).
