mod common;
mod config;
mod frozen;
mod led;
mod mqtt;
mod presence;
mod profile;
mod reset;
mod sensor;

use config::Config;
use tokio::sync::{broadcast, mpsc, watch};

use crate::{led::LEDController, presence::PresenseManager, reset::ResetController};

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
    let mut rc = ResetController::new().unwrap();
    rc.reset_subsystems().await.unwrap();
    let i2cdev = rc.take();
    let mut led = LEDController::new(i2cdev);
    // TODO proper led control in profile.rs
    led.start_breathing((255, 0, 0)).unwrap();

    // make channels
    let (presence_tx, presence_rx) = mpsc::channel(32);
    let (calibrate_tx, calibrate_rx) = mpsc::channel(32);
    let (frozen_command_tx, frozen_command_rx) = mpsc::channel(32);
    let (frozen_update_tx, frozen_update_rx) = mpsc::channel(32);
    let (sensor_update_tx, sensor_update_rx) = broadcast::channel(32);

    mqtt::spawn(
        config_tx.clone(),
        config_rx.clone(),
        sensor_update_rx.resubscribe(),
        frozen_update_rx,
        presence_rx,
        calibrate_tx,
    );

    tokio::select! {
        biased;

        _ = tokio::signal::ctrl_c() => {
            log::info!("Received ctrl+c signal");
        }

        res = frozen::run(frozen::PORT, frozen_command_rx, frozen_update_tx) => {
            match res {
                Ok(_) => log::warn!("Frozen task unexpectedly exited"),
                Err(e) => log::error!("Frozen task failed: {e}"),
            }
        }

        res = sensor::run(sensor::PORT, sensor_update_tx) => {
            match res {
                Ok(_) => log::warn!("Sensor task unexpectedly exited"),
                Err(e) => log::error!("Sensor task failed: {e}"),
            }
        }

        _ = PresenseManager::run(
            config_tx.clone(),
            config_rx.clone(),
            sensor_update_rx.resubscribe(),
            calibrate_rx,
            presence_tx,
        ) => {
            log::warn!("Presence manager task unexpectedly completed");
        }

        result = profile::run(frozen_command_tx, config_rx.clone()) => {
            match result {
                Ok(_) => log::warn!("Profile task unexpectedly exited"),
                Err(e) => log::error!("Profile task failed: {e}"),
            }
        }
    }

    log::info!("Shutting down OpenSleep...");
}
