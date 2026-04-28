use bluest::{Adapter, Device};
use chrono::Local;
use futures_lite::stream::StreamExt;
use std::time::Duration;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};

use super::{bluetooth_error, parse_heart_rate, HRM_UUID, HRS_UUID};

async fn find_heart_rate_characteristic(device: &Device) -> AppResult<bluest::Characteristic> {
    const MAX_RETRIES: usize = 10;
    const RETRY_DELAY_MS: u64 = 500;

    for attempt in 1..=MAX_RETRIES {
        let services = device
            .discover_services_with_uuid(HRS_UUID)
            .await
            .map_err(bluetooth_error)?;
        let service = services.first().ok_or(AppError::NoHeartRateService)?;
        let characteristics = service
            .discover_characteristics()
            .await
            .map_err(bluetooth_error)?;

        if let Some(characteristic) = characteristics.iter().find(|item| item.uuid() == HRM_UUID) {
            log::info!("Found HR characteristic on attempt {}", attempt);
            return Ok(characteristic.clone());
        }

        log::warn!(
            "Attempt {}: no HR characteristic found, retrying...",
            attempt
        );
        tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
    }

    Err(AppError::NoHeartRateCharacteristic)
}

pub(crate) async fn connect_and_monitor_hr(addr: String, state: AppState) -> AppResult<()> {
    let adapter = Adapter::default()
        .await
        .ok_or(AppError::NoBluetoothAdapter)?;
    adapter.wait_available().await.map_err(bluetooth_error)?;

    let device = {
        let connected = adapter
            .connected_devices_with_services(&[HRS_UUID])
            .await
            .map_err(bluetooth_error)?;
        if let Some(device) = connected
            .into_iter()
            .find(|device| format!("{:?}", device.id()) == addr)
        {
            log::info!("Using already connected device");
            device
        } else {
            log::info!("Scanning for device {}...", addr);
            state.set_error_message("Scanning...");
            let mut scan = adapter
                .discover_devices(&[])
                .await
                .map_err(bluetooth_error)?;
            let mut found_device = None;
            while let Some(result) = scan.next().await {
                let candidate = result.map_err(bluetooth_error)?;
                if format!("{:?}", candidate.id()) == addr {
                    found_device = Some(candidate);
                    break;
                }
            }
            found_device.ok_or(AppError::ScanTimeout)?
        }
    };

    log::info!("Connecting to {}...", addr);
    state.set_error_message("Connecting...");

    if !device.is_connected() {
        adapter
            .connect_device(&device)
            .await
            .map_err(bluetooth_error)?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    state.update_device_connection(&addr, true);
    state.set_error_message("Reading HR...");

    let heart_rate_measurement = find_heart_rate_characteristic(&device).await?;
    let mut updates = heart_rate_measurement
        .notify()
        .await
        .map_err(bluetooth_error)?;
    log::info!("Listening for heart rate updates from {}", addr);

    loop {
        tokio::select! {
            result = async { updates.next().await } => {
                match result {
                    Some(Ok(data)) => {
                        if let Some(heart_rate) = parse_heart_rate(&data) {
                            let now = Local::now().format("%H:%M:%S").to_string();
                            state.update_device_heart_rate(&addr, heart_rate, now);
                            state.mark_shared_heart_rate(Some(heart_rate), true);
                            log::debug!("HR: {} bpm", heart_rate);
                        }
                    }
                    Some(Err(error)) => {
                        return Err(bluetooth_error(error));
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if !state.is_selected(&addr) {
                    break;
                }
            }
        }
    }

    state.set_connecting(false);
    state.update_device_connection(&addr, false);
    state.mark_shared_heart_rate(None, false);
    Ok(())
}
