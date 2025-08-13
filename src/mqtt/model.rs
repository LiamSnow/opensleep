use crate::config::Config;
use thiserror::Error;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Error)]
pub enum MqttError {
    #[error("MQTT client error: {0}")]
    Client(#[from] rumqttc::ClientError),
    #[error("MQTT connection error: {0}")]
    Connection(#[from] rumqttc::ConnectionError),
    #[error("Invalid command: {0}")]
    InvalidCommand(String),
    #[error("Config update failed")]
    ConfigUpdate(#[from] watch::error::SendError<Config>),
    #[error("Calibrate channel error")]
    CalibrateChannel(#[from] mpsc::error::SendError<()>),
    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),
    #[error("Invalid side parameter or couples mode requires 'side' parameter (left/right)")]
    ProfileSide,
    #[error("Invalid time format: {0}")]
    InvalidTime(String),
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
}
