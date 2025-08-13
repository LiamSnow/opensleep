use std::time::Duration;

use crate::common::codec::PacketCodec;
use crate::common::serial::{DeviceMode, SerialError, create_framed_port};
use crate::sensor::state::SensorUpdate;
use crate::sensor::{SensorCommand, SensorPacket, SensorStateManager};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, sleep, timeout};
use tokio_serial::SerialStream;
use tokio_util::codec::Framed;

pub const PORT: &str = "/dev/ttymxc0";
const BOOTLOADER_BAUD: u32 = 38400;
const FIRMWARE_BAUD: u32 = 115200;

type Reader = SplitStream<Framed<SerialStream, PacketCodec<SensorPacket>>>;
type Writer = SplitSink<Framed<SerialStream, PacketCodec<SensorPacket>>, SensorCommand>;

pub async fn run(
    port: &'static str,
    update_tx: broadcast::Sender<SensorUpdate>,
) -> Result<(), SerialError> {
    log::info!("Initializing Sensor Subsystem...");

    let state_manager = SensorStateManager::new(update_tx);

    let (mut writer, mut reader) = discovery_task(port, &state_manager).await.unwrap();
    log::info!("Connected");

    let read_handle = tokio::spawn(read_task(reader, state_manager.clone()));

    let (mut write_tx, write_rx) = mpsc::channel(10);
    let write_handle = tokio::spawn(write_task(writer, write_rx));

    if !state_manager.has_hardware_info().await {
        tokio::spawn(try_get_hardware_info(
            write_tx.clone(),
            state_manager.clone(),
        ));
    }

    tokio::spawn(try_enable_vibration(
        write_tx.clone(),
        state_manager.clone(),
    ));

    try_set_piezo_gain(&mut write_tx, &state_manager).await;

    try_enable_piezo(&mut write_tx, &state_manager).await;

    tokio::select! {
        _ = read_handle => {
            // TODO log actual error
            log::error!("Read task quit");
        }
        _ = write_handle => {
            // TODO log actual error
            log::error!("Read task quit");
        }
    }
}

async fn try_get_hardware_info(
    write_tx: mpsc::Sender<SensorCommand>,
    state_manager: SensorStateManager,
) {
    for _ in 0..10 {
        if let Err(e) = write_tx.send(SensorCommand::GetHardwareInfo).await {
            log::error!("Failed to send GetHardwareInfo: {e}");
        }

        sleep(Duration::from_millis(500)).await;

        if state_manager.has_hardware_info().await {
            return;
        }

        sleep(Duration::from_millis(200)).await;
    }

    log::error!("Failed to get hardware info");
}

async fn try_enable_vibration(
    write_tx: mpsc::Sender<SensorCommand>,
    state_manager: SensorStateManager,
) {
    for _ in 0..10 {
        if let Err(e) = write_tx.send(SensorCommand::EnableVibration).await {
            log::error!("Failed to send EnableVibration: {e}");
        }

        sleep(Duration::from_millis(500)).await;

        if state_manager.vibration_enabled().await {
            return;
        }

        sleep(Duration::from_millis(200)).await;
    }

    log::error!("Failed to enable vibration");
}

async fn try_set_piezo_gain(
    write_tx: &mut mpsc::Sender<SensorCommand>,
    state_manager: &SensorStateManager,
) {
    for _ in 0..10 {
        if let Err(e) = write_tx.send(SensorCommand::SetPiezoGain400400).await {
            log::error!("Failed to send SetPiezoGain400400: {e}");
        }

        sleep(Duration::from_millis(200)).await;

        if state_manager.piezo_gain_ok().await {
            return;
        }

        sleep(Duration::from_millis(200)).await;
    }

    log::error!("Failed to set piezo gain");
}

async fn try_enable_piezo(
    write_tx: &mut mpsc::Sender<SensorCommand>,
    state_manager: &SensorStateManager,
) {
    for _ in 0..10 {
        if let Err(e) = write_tx.send(SensorCommand::SetPiezoFreq1KHz).await {
            log::error!("Failed to send SetPiezoFreq1KHz: {e}");
        }

        sleep(Duration::from_millis(200)).await;

        if let Err(e) = write_tx.send(SensorCommand::EnablePiezo).await {
            log::error!("Failed to send SetPiezoFreq1KHz: {e}");
        }

        sleep(Duration::from_millis(200)).await;

        if state_manager.piezo_ok().await {
            log::info!(
                "Piezo sampling at {}Hz",
                state_manager.piezo_freq().await.unwrap_or(0)
            );
            return;
        }

        sleep(Duration::from_millis(200)).await;
    }

    log::error!("Failed to enable piezo");
}

async fn write_task(mut writer: Writer, mut write_tx: mpsc::Receiver<SensorCommand>) {
    let mut interval = interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            Some(cmd) = write_tx.recv() => {
                if let Err(e) = writer.send(cmd).await {
                    log::error!("Failed to send command: {e}");
                }
            }
            _ = interval.tick() => {
                log::debug!("Ping");
                if let Err(e) = writer.send(SensorCommand::Ping).await {
                    log::error!("Failed to ping: {e}");
                }
                sleep(Duration::from_millis(200)).await;
                if let Err(e) = writer.send(SensorCommand::ProbeTemperature).await {
                    log::error!("Failed to probe temp: {e}");
                }
            }
        }
    }
}

async fn read_task(mut reader: Reader, state_manager: SensorStateManager) {
    while let Some(result) = reader.next().await {
        match result {
            Ok(packet) => {
                state_manager.handle_packet(packet).await;
            }
            Err(e) => {
                log::error!("Packet decode error: {e}");
            }
        }
    }
}

async fn discovery_task(
    port: &'static str,
    state_manager: &SensorStateManager,
) -> Result<(Writer, Reader), SerialError> {
    // try bootloader first
    if let Ok((mut writer, mut reader)) =
        ping_device(port, DeviceMode::Bootloader, state_manager).await
    {
        writer
            .send(SensorCommand::JumpToFirmware)
            .await
            .map_err(|e| SerialError::Io(std::io::Error::other(e)))?;

        // wait for mode switch
        wait_for_mode(&mut reader, state_manager, DeviceMode::Firmware).await?;

        return Ok(create_framed_port::<SensorPacket>(port, FIRMWARE_BAUD)?.split());
    }

    // try firmware (happens if program was recently running)
    log::info!("Trying Firmware mode");
    ping_device(port, DeviceMode::Firmware, state_manager).await
}

async fn ping_device(
    port: &'static str,
    mode: DeviceMode,
    state_manager: &SensorStateManager,
) -> Result<(Writer, Reader), SerialError> {
    let baud = if mode == DeviceMode::Bootloader {
        BOOTLOADER_BAUD
    } else {
        FIRMWARE_BAUD
    };
    let (mut writer, mut reader) = create_framed_port::<SensorPacket>(port, baud)?.split();

    for _ in 0..3 {
        writer
            .send(SensorCommand::Ping)
            .await
            .map_err(|e| SerialError::Io(std::io::Error::other(e)))?;

        if let Ok(Some(Ok(packet))) = timeout(Duration::from_millis(500), reader.next()).await {
            state_manager.set_device_mode(mode).await;
            state_manager.handle_packet(packet).await;
            return Ok((writer, reader));
        }
    }

    Err(SerialError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Device not responding",
    )))
}

async fn wait_for_mode(
    reader: &mut Reader,
    state_manager: &SensorStateManager,
    target_mode: DeviceMode,
) -> Result<(), SerialError> {
    let timeout_duration = Duration::from_secs(5);
    let start = std::time::Instant::now();

    while state_manager.device_mode().await != target_mode {
        if start.elapsed() > timeout_duration {
            return Err(SerialError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timed out waiting for mode change",
            )));
        }

        if let Some(Ok(packet)) = reader.next().await {
            state_manager.handle_packet(packet).await;
        }
    }

    Ok(())
}
