use bluest::Adapter;
use chrono::Local;
use futures_lite::stream::StreamExt;
use std::time::Duration;

use crate::app_state::{AppState, DeviceInfo};
use crate::error::{AppError, AppResult};

use super::{bluetooth_error, HRS_UUID};

const DEVICE_PROP_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) async fn run_bluetooth_scan(state: AppState) -> AppResult<()> {
    let adapter = Adapter::default()
        .await
        .ok_or(AppError::NoBluetoothAdapter)?;

    log::info!("Using adapter, waiting for availability...");
    adapter.wait_available().await.map_err(bluetooth_error)?;
    log::info!("Adapter available");

    let connected_devices = adapter
        .connected_devices_with_services(&[HRS_UUID])
        .await
        .map_err(bluetooth_error)?;
    for device in connected_devices {
        let addr = format!("{:?}", device.id());
        let now = Local::now().format("%H:%M:%S").to_string();
        state.upsert_device(DeviceInfo {
            name: device.name().unwrap_or_else(|_| "Unknown".to_string()),
            addr,
            rssi: 0,
            heart_rate: None,
            last_seen: now,
            connected: true,
        });
    }

    while state.is_scanning() {
        log::debug!("Starting scan...");
        state.set_error_message("Scanning...");

        let mut scan = match adapter.discover_devices(&[HRS_UUID]).await {
            Ok(scan) => scan,
            Err(error) => {
                log::warn!("Scan error: {}", error);
                state.set_error_message(format!("Scan error: {}", error));
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };

        loop {
            tokio::select! {
                result = scan.next() => {
                    match result {
                        Some(Ok(device)) => {
                            let name = match tokio::time::timeout(
                                DEVICE_PROP_TIMEOUT,
                                device.name_async(),
                            ).await {
                                Ok(Ok(n)) if !n.is_empty() => n,
                                _ => continue,
                            };

                            let addr = format!("{:?}", device.id());
                            let now = Local::now().format("%H:%M:%S").to_string();
                            let rssi = tokio::time::timeout(DEVICE_PROP_TIMEOUT, device.rssi())
                                .await
                                .ok()
                                .and_then(|r| r.ok())
                                .unwrap_or(0);
                            state.upsert_device(DeviceInfo {
                                name,
                                addr,
                                rssi,
                                heart_rate: None,
                                last_seen: now,
                                connected: false,
                            });
                        }
                        Some(Err(error)) => {
                            log::debug!("Device discovery error: {}", error);
                        }
                        None => {
                            log::debug!("Scan finished, restarting...");
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => {
                    log::debug!("Scan timeout, restarting...");
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}
