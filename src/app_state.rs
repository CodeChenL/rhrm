use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub struct SharedHeartRateSnapshot {
    pub heart_rate: Option<u16>,
    pub connecting: bool,
    pub version: u64,
}

#[derive(Clone, Debug)]
pub struct HistorySnapshot {
    pub values: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub addr: String,
    pub rssi: i16,
    pub heart_rate: Option<u16>,
    pub last_seen: String,
    pub connected: bool,
}

const HISTORY_MAX_SECONDS: usize = 60;
const HISTORY_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct SharedHrData {
    pub heart_rate: Arc<Mutex<Option<u16>>>,
    pub connecting: Arc<Mutex<bool>>,
    pub version: Arc<Mutex<u64>>,
    pub hr_history: Arc<Mutex<VecDeque<u16>>>,
    pub last_history_sample: Arc<Mutex<Option<Instant>>>,
}

impl Default for SharedHrData {
    fn default() -> Self {
        Self {
            heart_rate: Arc::new(Mutex::new(None)),
            connecting: Arc::new(Mutex::new(false)),
            version: Arc::new(Mutex::new(0)),
            hr_history: Arc::new(Mutex::new(VecDeque::new())),
            last_history_sample: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Clone, Default)]
pub struct AppState {
    devices: Arc<Mutex<HashMap<String, DeviceInfo>>>,
    scanning: Arc<Mutex<bool>>,
    selected_device: Arc<Mutex<Option<String>>>,
    connecting: Arc<Mutex<bool>>,
    error_message: Arc<Mutex<Option<String>>>,
    shared_data: SharedHrData,
}

impl AppState {
    pub fn toggle_scanning(&self) -> bool {
        let mut scanning = self.scanning.lock().unwrap();
        *scanning = !*scanning;
        *scanning
    }

    pub fn set_scanning(&self, value: bool) {
        *self.scanning.lock().unwrap() = value;
    }

    pub fn is_scanning(&self) -> bool {
        *self.scanning.lock().unwrap()
    }

    pub fn set_selected_device(&self, value: Option<String>) {
        *self.selected_device.lock().unwrap() = value;
    }

    pub fn selected_device(&self) -> Option<String> {
        self.selected_device.lock().unwrap().clone()
    }

    pub fn is_selected(&self, addr: &str) -> bool {
        self.selected_device.lock().unwrap().as_deref() == Some(addr)
    }

    pub fn set_connecting(&self, value: bool) {
        *self.connecting.lock().unwrap() = value;
    }

    pub fn is_connecting(&self) -> bool {
        *self.connecting.lock().unwrap()
    }

    pub fn clear_error(&self) {
        *self.error_message.lock().unwrap() = None;
    }

    pub fn set_error_message(&self, message: impl Into<String>) {
        *self.error_message.lock().unwrap() = Some(message.into());
    }

    pub fn error_message(&self) -> Option<String> {
        self.error_message.lock().unwrap().clone()
    }

    pub fn upsert_device(&self, device: DeviceInfo) {
        self.devices
            .lock()
            .unwrap()
            .insert(device.addr.clone(), device);
    }

    pub fn update_device_connection(&self, addr: &str, connected: bool) {
        if let Some(device) = self.devices.lock().unwrap().get_mut(addr) {
            device.connected = connected;
        }
    }

    pub fn update_device_heart_rate(&self, addr: &str, heart_rate: u16, last_seen: String) {
        if let Some(device) = self.devices.lock().unwrap().get_mut(addr) {
            device.heart_rate = Some(heart_rate);
            device.last_seen = last_seen;
        }
    }

    pub fn sorted_devices(&self) -> Vec<DeviceInfo> {
        let mut devices: Vec<_> = self.devices.lock().unwrap().values().cloned().collect();
        devices.sort_by(|left, right| right.rssi.cmp(&left.rssi));
        devices
    }

    pub fn device_count(&self) -> usize {
        self.devices.lock().unwrap().len()
    }

    pub fn mark_shared_heart_rate(&self, heart_rate: Option<u16>, connecting: bool) {
        *self.shared_data.heart_rate.lock().unwrap() = heart_rate;
        *self.shared_data.connecting.lock().unwrap() = connecting;
        *self.shared_data.version.lock().unwrap() += 1;

        if let Some(hr) = heart_rate {
            let now = Instant::now();
            let mut last_sample = self.shared_data.last_history_sample.lock().unwrap();
            let should_sample = last_sample
                .map(|t| now.duration_since(t) >= HISTORY_SAMPLE_INTERVAL)
                .unwrap_or(true);
            if should_sample {
                let mut history = self.shared_data.hr_history.lock().unwrap();
                history.push_back(hr);
                if history.len() > HISTORY_MAX_SECONDS {
                    history.pop_front();
                }
                *last_sample = Some(now);
            }
        } else if !connecting {
            self.shared_data.hr_history.lock().unwrap().clear();
            *self.shared_data.last_history_sample.lock().unwrap() = None;
        }
    }

    pub fn shared_snapshot(&self) -> SharedHeartRateSnapshot {
        SharedHeartRateSnapshot {
            heart_rate: *self.shared_data.heart_rate.lock().unwrap(),
            connecting: *self.shared_data.connecting.lock().unwrap(),
            version: *self.shared_data.version.lock().unwrap(),
        }
    }

    pub fn history_snapshot(&self) -> HistorySnapshot {
        HistorySnapshot {
            values: self.shared_data.hr_history.lock().unwrap().iter().copied().collect(),
        }
    }
}
