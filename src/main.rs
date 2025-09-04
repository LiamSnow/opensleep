mod common;
mod config;
mod frozen;
mod led;
mod mqtt;
mod reset;
mod sensor;

use std::fs;

use config::Config;
use tokio::sync::{mpsc, watch};

use crate::{led::IS31FL3194Controller, mqtt::MqttManager, reset::ResetController};

pub const VERSION: &str = "2.0.0";
pub const NAME: &str = "opensleep";

#[tokio::main]
pub async fn main() {
    env_logger::init();
    log::info!("Starting OpenSleep...");

    // read device label
    let device_label =
        fs::read_to_string("/home/dac/app/sewer/device-label").unwrap_or("unknown".to_string());

    // read config
    let config = Config::load("config.ron").await.unwrap();
    log::info!("`config.ron` loaded");
    let (config_tx, config_rx) = watch::channel(config.clone());
    log::info!(
        "Using timezone: {}",
        config.timezone.iana_name().unwrap_or("ERROR")
    );

    // reset
    let mut resetter = ResetController::new().unwrap();
    resetter.reset_subsystems().await.unwrap();
    let led = IS31FL3194Controller::new(resetter.take());

    let (calibrate_tx, calibrate_rx) = mpsc::channel(32);

    let mut mqtt_man = MqttManager::new(
        config_tx.clone(),
        config_rx.clone(),
        calibrate_tx,
        device_label,
    );
    mqtt_man.wait_for_conn().await;

    tokio::select! {
        res = frozen::run(
            frozen::PORT,
            config_rx.clone(),
            led,
            mqtt_man.client.clone()
        ) => {
            match res {
                Ok(_) => log::error!("Frozen task unexpectedly exited"),
                Err(e) => log::error!("Frozen task failed: {e}"),
            }
        }

        res = sensor::run(
            sensor::PORT,
            config_tx,
            config_rx,
            calibrate_rx,
            mqtt_man.client.clone()
        ) => {
            match res {
                Ok(_) => log::error!("Sensor task unexpectedly exited"),
                Err(e) => log::error!("Sensor task failed: {e}"),
            }
        }

        _ = mqtt_man.run() => {
            log::error!("MQTT manager unexpectedly exited");
        }
    }

    log::info!("Shutting down OpenSleep...");
}
