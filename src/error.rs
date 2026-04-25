use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("bluetooth adapter unavailable")]
    NoBluetoothAdapter,
    #[error("heart rate service not found")]
    NoHeartRateService,
    #[error("heart rate characteristic not found after retries")]
    NoHeartRateCharacteristic,
    #[error("scan timed out before the selected device was found")]
    ScanTimeout,
    #[error("bluetooth error: {0}")]
    Bluetooth(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
