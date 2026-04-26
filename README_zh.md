# RHRM — Rust Heart Rate Monitor

基于 Rust 开发的蓝牙低功耗（BLE）心率监测桌面应用。

[English](README.md)

## 功能特性

- **蓝牙设备扫描** — 一键搜索附近的 BLE 心率设备。
- **实时心率显示** — 实时 BPM 读数，带颜色状态指示（绿 / 黄 / 红），心率超过 120 bpm 时触发视觉告警。
- **浮动窗口** — 置顶悬浮窗显示心率和 60 秒历史曲线，方便在使用其他应用时持续监控。
- **自动重连** — 启动时自动连接上次使用的设备。
- **可定制悬浮窗** — 支持 9 种预设位置、尺寸调整、透明度设置以及鼠标穿透模式。

## 系统要求

- **操作系统**：Windows 10+ / Linux / macOS
- **硬件**：支持 BLE 的蓝牙 4.0+ 适配器
- **心率设备**：标准 BLE 心率监测器（HRS 服务 `0x180D`）

## 安装

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/CodeChenL/rhrm.git
cd rhrm/bt-monitor

# 构建（Release 模式）
cargo build --release

# 运行
cargo run
```

编译产物位于 `target/release/rhrm`（Windows 下为 `rhrm.exe`）。

## 使用说明

1. **启动**应用，主控制面板自动打开。
2. **点击"扫描"**搜索附近的 BLE 心率设备。
3. **在设备列表中点击设备**进行连接，收到数据后心率开始显示。
4. **浮动窗口**自动打开，展示实时心率和历史曲线图。
5. **在控制面板中调整浮动窗口**：
   - 从 3×3 网格中选择预设位置。
   - 调整宽度、高度和透明度。
   - 开启鼠标穿透模式以让鼠标事件穿透窗口。

## 项目架构

```
bt-monitor/
├── src/
│   ├── main.rs              # 入口、日志初始化
│   ├── app_state.rs         # 线程安全的应用状态
│   ├── config.rs            # TOML 配置持久化
│   ├── error.rs             # 自定义错误类型
│   ├── bluetooth/
│   │   ├── mod.rs           # BLE UUID 常量
│   │   ├── scan.rs          # BLE 设备发现
│   │   ├── connection.rs    # 设备连接与心率数据流
│   │   ├── parser.rs        # BLE 数据包解析
│   │   └── service.rs       # 异步命令队列处理
│   └── ui/
│       ├── mod.rs
│       ├── main_window.rs   # 主控制面板
│       ├── float_window.rs  # 浮动悬浮窗（含曲线图）
│       └── float_window_controller.rs  # 浮动窗口生命周期管理
├── Cargo.toml
├── assets_font_license.txt
├── README.md
└── README_zh.md
```

**数据流**：UI 发送指令 → `BluetoothService` 生成异步任务 → BLE 模块发现/连接/传输数据 → `AppState` 分发更新 → UI 渲染。

## 技术栈

| 模块         | 库            | 用途                          |
| --------- | ------------- | ----------------- |
| GUI          | egui / eframe | 跨平台 UI 框架                 |
| 蓝牙         | bluest        | BLE 设备通信                  |
| 异步运行时    | tokio         | 多线程异步执行                 |
| 配置         | serde / toml  | 序列化与 TOML 解析            |
| 错误处理     | thiserror     | 错误类型派生宏                 |
| 日志         | log / env_loger | 运行时日志                 |
| 时间         | chrono        | 时间戳格式化                  |

## 配置

配置文件以 TOML 格式存储在：

| 系统      | 路径                                          |
| --------- | --------------------------------- |
| Windows   | `%APDATA%\rhlm\rhlm.toml`                    |
| Linux     | `~/.config/rhlm/rhlm.toml`                    |

包含以下设置：
- 浮动窗口预设位置、尺寸和透明度。
- 上次连接的设备地址（用于自动重连）。
- 鼠标穿透模式开关。

## 许可证

本项目源代码以 MIT License（或在此指定你的许可证）发布。

应用中包含的字体（`UbuntuMono-R.ttf`、`NotoSansCJKsc-Regular.otf`）遵循 [SIL Open Font License 1.1](assets_font_license.txt)。
