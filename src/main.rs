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

    // spawn tasks
    frozen::spawn(frozen::PORT, frozen_command_rx, frozen_update_tx).unwrap();
    sensor::run(sensor::PORT, sensor_update_tx).await.unwrap();

    PresenseManager::run(
        config_tx.clone(),
        config_rx.clone(),
        sensor_update_rx.resubscribe(),
        calibrate_rx,
        presence_tx,
    );

    log::info!("Initializing Profile Manager...");
    profile::spawn(frozen_command_tx, config_rx.clone());

    log::info!("Initializing MQTT...");
    mqtt::spawn(
        config_tx.clone(),
        config_rx.clone(),
        sensor_update_rx,
        frozen_update_rx,
        presence_rx,
        calibrate_tx,
    );

    tokio::select! {}

    let _ = tokio::signal::ctrl_c().await;
    log::info!("Shutting down OpenSleep...");
}
