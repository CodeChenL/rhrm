use std::sync::mpsc;
use std::thread;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};

use super::{connect_and_monitor_hr, run_bluetooth_scan};

enum BluetoothCommand {
    StartScan,
    Connect(String),
}

#[derive(Clone)]
pub struct BluetoothService {
    sender: mpsc::Sender<BluetoothCommand>,
}

impl BluetoothService {
    pub fn new(state: AppState) -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let runtime = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => {
                    log::error!("Failed to create bluetooth runtime: {}", error);
                    state
                        .set_error_message(format!("Failed to start bluetooth runtime: {}", error));
                    return;
                }
            };

            while let Ok(command) = receiver.recv() {
                match command {
                    BluetoothCommand::StartScan => {
                        let state = state.clone();
                        runtime.spawn(async move {
                            if let Err(error) = run_bluetooth_scan(state.clone()).await {
                                log::error!("Scan error: {}", error);
                                state.set_scanning(false);
                                state.set_error_message(error.to_string());
                            }
                        });
                    }
                    BluetoothCommand::Connect(addr) => {
                        let state = state.clone();
                        runtime.spawn(async move {
                            if let Err(error) =
                                connect_and_monitor_hr(addr.clone(), state.clone()).await
                            {
                                log::error!("Connection error: {}", error);
                                state.set_connecting(false);
                                state.update_device_connection(&addr, false);
                                state.mark_shared_heart_rate(None, false);
                                state.set_error_message(error.to_string());
                            }
                        });
                    }
                }
            }
        });

        Self { sender }
    }

    pub fn start_scan(&self) -> AppResult<()> {
        self.sender
            .send(BluetoothCommand::StartScan)
            .map_err(|error| {
                AppError::Bluetooth(format!("failed to queue scan command: {}", error))
            })
    }

    pub fn connect(&self, addr: String) -> AppResult<()> {
        self.sender
            .send(BluetoothCommand::Connect(addr))
            .map_err(|error| {
                AppError::Bluetooth(format!("failed to queue connect command: {}", error))
            })
    }
}
