use std::time::Duration;

use crate::common::codec::PacketCodec;
use crate::common::serial::{DeviceMode, SerialError, create_framed_port};
use crate::config::Config;
use crate::sensor::command::AlarmCommand;
use crate::sensor::presence::{PresenceState, PresenseManager};
use crate::sensor::state::{PIEZO_FREQ, PIEZO_GAIN, SensorState, SensorUpdate};
use crate::sensor::{SensorCommand, SensorPacket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, interval, timeout};
use tokio_serial::SerialStream;
use tokio_util::codec::Framed;

pub const PORT: &str = "/dev/ttymxc0";
const BOOTLOADER_BAUD: u32 = 38400;
const FIRMWARE_BAUD: u32 = 115200;

const COMMAND_INT: Duration = Duration::from_millis(50);
const PING_INT: Duration = Duration::from_secs(4); // matches frankenfirmware
const PROBE_INT: Duration = Duration::from_secs(10); // matches frankenfirmware
const CONFIG_RES_TIME: Duration = Duration::from_millis(800);

type Reader = SplitStream<Framed<SerialStream, PacketCodec<SensorPacket>>>;
type Writer = SplitSink<Framed<SerialStream, PacketCodec<SensorPacket>>, SensorCommand>;

struct CommandTimers {
    last_ping: Instant,
    last_probe: Instant,
    last_hwinfo: Instant,
    last_viben: Instant,
    last_gain: Instant,
    last_freq: Instant,
    last_piezo: Instant,
}

pub async fn run(
    port: &'static str,
    mut update_tx: mpsc::Sender<SensorUpdate>,
    config_tx: watch::Sender<Config>,
    config_rx: watch::Receiver<Config>,
    mut calibrate_rx: mpsc::Receiver<()>,
    presence_tx: mpsc::Sender<PresenceState>,
) -> Result<(), SerialError> {
    log::info!("Initializing Sensor Subsystem...");

    let mut state = SensorState::default();
    let mut presense_man = PresenseManager::new(config_tx, config_rx, presence_tx);

    let (mut writer, mut reader) = run_discovery(port, &mut update_tx, &mut state)
        .await
        .unwrap();
    log::info!("Connected");

    let mut interval = interval(COMMAND_INT);
    let mut timers = CommandTimers::default();

    loop {
        tokio::select! {
            Some(result) = reader.next() => match result {
                Ok(packet) => {
                    if let SensorPacket::Capacitance(data) = &packet {
                        presense_man.update(data);
                    }

                    state.handle_packet(&mut update_tx, packet);
                }
                Err(e) => {
                    log::error!("Packet decode error: {e}");
                }
            },

            _ = interval.tick() => {
                if let Some(cmd) = get_next_command(&mut timers, &state) {
                    log::debug!(" -> {cmd:?}");
                    if let Err(e) = writer.send(cmd).await {
                        log::error!("Failed to send command: {e}");
                    }
                }
            }

            Some(_) = calibrate_rx.recv() => presense_man.start_calibration()
        }
    }
}

fn get_next_command(timers: &mut CommandTimers, state: &SensorState) -> Option<SensorCommand> {
    let now = Instant::now();
    // TODO
    // if state.vibration_enabled
    //     && let Ok(acmd) = alarm_rx.try_recv()
    // {
    //     Some(SensorCommand::SetAlarm(acmd))
    // } else
    if now.duration_since(timers.last_ping) > PING_INT {
        timers.last_ping = now;
        Some(SensorCommand::Ping)
    } else if now.duration_since(timers.last_probe) > PROBE_INT {
        timers.last_probe = now;
        Some(SensorCommand::ProbeTemperature)
    } else if now.duration_since(timers.last_hwinfo) > CONFIG_RES_TIME
        && state.hardware_info.is_none()
    {
        timers.last_hwinfo = now;
        Some(SensorCommand::GetHardwareInfo)
    } else if now.duration_since(timers.last_viben) > CONFIG_RES_TIME && !state.vibration_enabled {
        timers.last_viben = now;
        Some(SensorCommand::EnableVibration)
    } else if now.duration_since(timers.last_gain) > CONFIG_RES_TIME && !state.piezo_gain_ok() {
        timers.last_gain = now;
        Some(SensorCommand::SetPiezoGain(PIEZO_GAIN, PIEZO_GAIN))
    } else if now.duration_since(timers.last_freq) > CONFIG_RES_TIME
        && state.piezo_enabled
        && !state.piezo_freq_ok()
    {
        timers.last_freq = now;
        Some(SensorCommand::SetPiezoFreq(PIEZO_FREQ))
    } else if now.duration_since(timers.last_piezo) > CONFIG_RES_TIME && !state.piezo_enabled {
        timers.last_piezo = now;
        Some(SensorCommand::EnablePiezo)
    } else {
        None
    }
}

async fn run_discovery(
    port: &'static str,
    update_tx: &mut mpsc::Sender<SensorUpdate>,
    state: &mut SensorState,
) -> Result<(Writer, Reader), SerialError> {
    // try bootloader first
    if let Ok((mut writer, mut reader)) =
        ping_device(port, update_tx, state, DeviceMode::Bootloader).await
    {
        writer
            .send(SensorCommand::JumpToFirmware)
            .await
            .map_err(|e| SerialError::Io(std::io::Error::other(e)))?;

        // wait for mode switch
        wait_for_mode(&mut reader, update_tx, state, DeviceMode::Firmware).await?;

        return Ok(create_framed_port::<SensorPacket>(port, FIRMWARE_BAUD)?.split());
    }

    // try firmware (happens if program was recently running)
    log::info!("Trying Firmware mode");
    ping_device(port, update_tx, state, DeviceMode::Firmware).await
}

async fn ping_device(
    port: &'static str,
    update_tx: &mut mpsc::Sender<SensorUpdate>,
    state: &mut SensorState,
    mode: DeviceMode,
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
            state.set_device_mode(update_tx, mode);
            state.handle_packet(update_tx, packet);
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
    update_tx: &mut mpsc::Sender<SensorUpdate>,
    state: &mut SensorState,
    target_mode: DeviceMode,
) -> Result<(), SerialError> {
    let timeout_duration = Duration::from_secs(5);
    let start = std::time::Instant::now();

    while state.device_mode != target_mode {
        if start.elapsed() > timeout_duration {
            return Err(SerialError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timed out waiting for mode change",
            )));
        }

        if let Some(Ok(packet)) = reader.next().await {
            state.handle_packet(update_tx, packet);
        }
    }

    Ok(())
}

impl Default for CommandTimers {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            last_ping: now,
            // stagger commands
            last_probe: now + Duration::from_millis(2500),
            last_hwinfo: now,
            last_viben: now,
            last_gain: now,
            last_freq: now,
            last_piezo: now,
        }
    }
}
