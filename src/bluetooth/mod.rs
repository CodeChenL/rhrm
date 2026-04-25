use bluest::{btuuid::bluetooth_uuid_from_u16, Uuid};

use crate::error::AppError;

mod connection;
mod parser;
mod scan;
mod service;

pub(crate) use connection::connect_and_monitor_hr;
pub use parser::parse_heart_rate;
pub(crate) use scan::run_bluetooth_scan;
pub use service::BluetoothService;

pub(crate) const HRS_UUID: Uuid = bluetooth_uuid_from_u16(0x180D);
pub(crate) const HRM_UUID: Uuid = bluetooth_uuid_from_u16(0x2A37);

pub(crate) fn bluetooth_error(error: impl std::fmt::Display) -> AppError {
    AppError::Bluetooth(error.to_string())
}
