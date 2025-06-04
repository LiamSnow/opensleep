use frank::error::FrankError;
use log::{info, LevelFilter, SetLoggerError};
use scheduler::SchedulerError;
use settings::{Settings, SettingsError};
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use thiserror::Error;
use std::{fs::File, io};
use tokio::sync::watch;

mod frank;
mod scheduler;
mod settings;
mod test;
mod api;

pub const SETTINGS_FILE: &str = "settings.json";
const LOG_FILE: &str = "opensleep.log";

#[derive(Error, Debug)]
pub enum MainError {
    #[error("api error: `{0}`")]
    API(#[from] io::Error),
    #[error("log file creation error: `{0}`")]
    LogFileCreation(io::Error),
    #[error("set logger error: `{0}`")]
    SetLogger(#[from] SetLoggerError),
    #[error("frank error: `{0}`")]
    Frank(#[from] FrankError),
    #[error("scheduler error: `{0}`")]
    Scheduler(#[from] SchedulerError),
    #[error("settings error: `{0}`")]
    Settings(#[from] SettingsError),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), MainError> {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            File::create(LOG_FILE)
                .map_err(|e| MainError::LogFileCreation(e))?,
        ),
    ])?;

    info!("[Main] Open Sleep started");

    info!("[Main] Reading settings file: {SETTINGS_FILE}");
    let (settings_tx, settings_rx) = watch::channel(Settings::from_file(SETTINGS_FILE)?);

    info!("[Main] Finding a Frank");
    let (frank_tx, frank_state) = frank::run().await?;

    info!("[Main] Starting API server");
    api::run(frank_state, settings_tx, settings_rx.clone()).await?;

    info!("[Main] Starting Scheduler...");
    scheduler::run(frank_tx, settings_rx).await?;

    Ok(())
}
