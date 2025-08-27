mod common;
mod config;
mod frozen;
mod led;
mod mqtt;
mod reset;
mod sensor;

use config::Config;
use tokio::sync::{mpsc, watch};

use crate::{led::IS31FL3194Controller, reset::ResetController};

#[tokio::main]
pub async fn main() {
    env_logger::init();
    log::info!("Starting OpenSleep...");

    let config = Config::load("config.ron").unwrap();
    log::info!("Configuration loaded successfully");
    let (config_tx, config_rx) = watch::channel(config.clone());
    tokio::spawn(config::auto_save(config_rx.clone()));

    log::info!(
        "Using timezone: {}",
        config.timezone.iana_name().unwrap_or("Unknown")
    );

    // reset
    let mut resetter = ResetController::new().unwrap();
    resetter.reset_subsystems().await.unwrap();
    let led = IS31FL3194Controller::new(resetter.take());

    // make channels
    let (presence_tx, presence_rx) = mpsc::channel(32);
    let (calibrate_tx, calibrate_rx) = mpsc::channel(32);
    let (frozen_update_tx, frozen_update_rx) = mpsc::channel(32);
    let (sensor_update_tx, sensor_update_rx) = mpsc::channel(32);

    mqtt::spawn(
        config_tx.clone(),
        config_rx.clone(),
        sensor_update_rx,
        frozen_update_rx,
        presence_rx,
        calibrate_tx,
    );

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("Received ctrl+c signal");
        }

        res = frozen::run(
            frozen::PORT,
            frozen_update_tx,
            config_rx.clone(),
            led
        ) => {
            match res {
                Ok(_) => log::warn!("Frozen task unexpectedly exited"),
                Err(e) => log::error!("Frozen task failed: {e}"),
            }
        }

        res = sensor::run(
            sensor::PORT,
            sensor_update_tx,
            config_tx,
            config_rx,
            calibrate_rx,
            presence_tx
        ) => {
            match res {
                Ok(_) => log::warn!("Sensor task unexpectedly exited"),
                Err(e) => log::error!("Sensor task failed: {e}"),
            }
        }
    }

    log::info!("Shutting down OpenSleep...");
}
