use std::time::Duration;

use crate::common::codec::PacketCodec;
use crate::common::packet::BedSide;
use crate::common::serial::{DeviceMode, SerialError, create_framed_port};
use crate::config::{Config, SidesConfig};
use crate::sensor::command::AlarmCommand;
use crate::sensor::presence::PresenseManager;
use crate::sensor::state::{PIEZO_FREQ, PIEZO_GAIN, SensorState};
use crate::sensor::{SensorCommand, SensorPacket};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use jiff::civil::Time;
use jiff::{Span, Timestamp};
use rumqttc::AsyncClient;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, interval, timeout};
use tokio_serial::SerialStream;
use tokio_util::codec::Framed;

pub const PORT: &str = "/dev/ttymxc0";
const BOOTLOADER_BAUD: u32 = 38400;
const FIRMWARE_BAUD: u32 = 115200;

type Reader = SplitStream<Framed<SerialStream, PacketCodec<SensorPacket>>>;
type Writer = SplitSink<Framed<SerialStream, PacketCodec<SensorPacket>>, SensorCommand>;
type CommandCheck = fn(&SensorState, &Time, &bool, &SidesConfig) -> Option<SensorCommand>;

struct CommandScheduler {
    cmds: Vec<RegisteredCommand>,
    away_mode: bool,
    sides_config: SidesConfig,
    writer: Writer,
}

struct RegisteredCommand {
    name: &'static str,
    interval: Duration,
    last_run: Instant,
    can_run: CommandCheck,
}

pub async fn run(
    port: &'static str,
    config_tx: watch::Sender<Config>,
    mut config_rx: watch::Receiver<Config>,
    mut calibrate_rx: mpsc::Receiver<()>,
    mut client: AsyncClient,
) -> Result<(), SerialError> {
    log::info!("Initializing Sensor Subsystem...");

    let mut state = SensorState::default();
    let mut presense_man = PresenseManager::new(config_tx, config_rx.clone(), client.clone());

    let (writer, mut reader) = run_discovery(port, &mut client, &mut state).await.unwrap();
    log::info!("Connected");

    let cfg = config_rx.borrow_and_update();
    let timezone = cfg.timezone.clone();
    let mut scheduler = CommandScheduler::new(cfg.away_mode, cfg.profile.clone(), writer);
    drop(cfg);

    let mut interval = interval(Duration::from_millis(50));

    loop {
        tokio::select! {
            Some(result) = reader.next() => match result {
                Ok(packet) => {
                    if let SensorPacket::Capacitance(data) = &packet {
                        presense_man.update(data);
                    }

                    state.handle_packet(&mut client, packet);
                }
                Err(e) => {
                    log::error!("Packet decode error: {e}");
                }
            },

            _ = interval.tick() => {
                // this is not expensive so its fine to do at 20hz
                let now = Timestamp::now().to_zoned(timezone.clone()).time();
                scheduler.update(&state, &now).await;
            }

            Some(_) = calibrate_rx.recv() => presense_man.start_calibration(),

            Ok(_) = config_rx.changed() => {
                let cfg = config_rx.borrow();
                scheduler.away_mode = cfg.away_mode;
                scheduler.sides_config = cfg.profile.clone();
            }
        }
    }
}

impl CommandScheduler {
    fn new(away_mode: bool, sides_config: SidesConfig, writer: Writer) -> Self {
        let now = Instant::now();
        const CONFIG_RES_TIME: Duration = Duration::from_millis(800);
        Self {
            away_mode,
            sides_config,
            writer,
            cmds: vec![
                RegisteredCommand {
                    name: "ping",
                    interval: Duration::from_secs(4),
                    last_run: now,
                    can_run: |_, _, _, _| Some(SensorCommand::Ping),
                },
                RegisteredCommand {
                    name: "probe_temperature",
                    interval: Duration::from_secs(4),
                    // stagger
                    last_run: now + Duration::from_millis(2500),
                    can_run: |_, _, _, _| Some(SensorCommand::ProbeTemperature),
                },
                RegisteredCommand {
                    name: "hwinfo",
                    interval: CONFIG_RES_TIME,
                    last_run: now,
                    can_run: |state, _, _, _| {
                        if state.hardware_info.is_none() {
                            Some(SensorCommand::GetHardwareInfo)
                        } else {
                            None
                        }
                    },
                },
                RegisteredCommand {
                    name: "enable_vibration",
                    interval: CONFIG_RES_TIME,
                    last_run: now,
                    can_run: |s, _, _, _| {
                        if !s.vibration_enabled {
                            Some(SensorCommand::EnableVibration)
                        } else {
                            None
                        }
                    },
                },
                RegisteredCommand {
                    name: "piezo_gain",
                    interval: CONFIG_RES_TIME,
                    last_run: now,
                    can_run: |state, _, _, _| {
                        if !state.piezo_gain_ok() {
                            Some(SensorCommand::SetPiezoGain(PIEZO_GAIN, PIEZO_GAIN))
                        } else {
                            None
                        }
                    },
                },
                RegisteredCommand {
                    name: "piezo_freq",
                    interval: CONFIG_RES_TIME,
                    last_run: now,
                    can_run: |state, _, _, _| {
                        if state.piezo_enabled && !state.piezo_freq_ok() {
                            Some(SensorCommand::SetPiezoFreq(PIEZO_FREQ))
                        } else {
                            None
                        }
                    },
                },
                RegisteredCommand {
                    name: "enable_piezo",
                    interval: CONFIG_RES_TIME,
                    last_run: now,
                    can_run: |s, _, _, _| {
                        if !s.piezo_enabled {
                            Some(SensorCommand::EnablePiezo)
                        } else {
                            None
                        }
                    },
                },
                RegisteredCommand {
                    name: "left_alarm",
                    interval: Duration::from_secs(5),
                    last_run: now,
                    can_run: |state, now, away, sides_cfg| {
                        if state.vibration_enabled && !away {
                            get_alarm_cmd(state, now, sides_cfg, &BedSide::Left)
                        } else {
                            None
                        }
                    },
                },
                RegisteredCommand {
                    name: "right_alarm",
                    interval: Duration::from_secs(5),
                    last_run: now,
                    can_run: |state, now, away, sides_cfg| {
                        if state.vibration_enabled && !away {
                            get_alarm_cmd(state, now, sides_cfg, &BedSide::Right)
                        } else {
                            None
                        }
                    },
                },
            ],
        }
    }

    async fn update(&mut self, state: &SensorState, time: &Time) -> Option<SensorCommand> {
        let now = Instant::now();

        for reg_cmd in &mut self.cmds {
            if now.duration_since(reg_cmd.last_run) > reg_cmd.interval
                && let Some(sen_cmd) =
                    (reg_cmd.can_run)(state, time, &self.away_mode, &self.sides_config)
            {
                reg_cmd.last_run = now;
                log::debug!(" -> {:?} (from {})", sen_cmd, reg_cmd.name);
                if let Err(e) = self.writer.send(sen_cmd).await {
                    log::error!("Failed to send {}: {e}", reg_cmd.name);
                }
            }
        }

        None
    }
}

/// alarm runs from (wake - alarm_offset) to ((wake - alarm_offset) + alarm_duration)
fn get_alarm_cmd(
    state: &SensorState,
    now: &Time,
    sides_config: &SidesConfig,
    side: &BedSide,
) -> Option<SensorCommand> {
    let cfg = sides_config.get_side(side);
    let alarm_cfg = cfg.alarm.as_ref()?;
    let alarm_start = cfg.wake - Span::new().seconds(alarm_cfg.offset);
    let alarm_end = alarm_start + Span::new().seconds(alarm_cfg.duration);
    let alarm_running = state.get_alarm_for_side(side);

    if now > &alarm_start && now < &alarm_end {
        if !alarm_running {
            log::info!("Alarm[{side}] requesting to start");
            return Some(SensorCommand::SetAlarm(AlarmCommand {
                side: *side,
                intensity: alarm_cfg.intensity,
                duration: alarm_cfg.duration,
                pattern: alarm_cfg.pattern.clone(),
            }));
        }
    } else if alarm_running {
        log::info!("Alarm[{side}] should NOT be running, but is. Cancelling now.");
        // FIXME TODO not working
        return Some(SensorCommand::ClearAlarm);
        // return Some(SensorCommand::SetAlarm(AlarmCommand {
        //     side: *side,
        //     intensity: 0,
        //     duration: 0,
        //     pattern: AlarmPattern::Double,
        // }));
    }

    None
}

/// tries to connect to the Sensor subsystem at either bootloader baud or firmware baud
async fn run_discovery(
    port: &'static str,
    client: &mut AsyncClient,
    state: &mut SensorState,
) -> Result<(Writer, Reader), SerialError> {
    // try bootloader first
    if let Ok((mut writer, mut reader)) =
        ping_device(port, client, state, DeviceMode::Bootloader).await
    {
        writer
            .send(SensorCommand::JumpToFirmware)
            .await
            .map_err(|e| SerialError::Io(std::io::Error::other(e)))?;

        // wait for mode switch
        wait_for_mode(&mut reader, client, state, DeviceMode::Firmware).await?;

        return Ok(create_framed_port::<SensorPacket>(port, FIRMWARE_BAUD)?.split());
    }

    // try firmware (happens if program was recently running)
    log::info!("Trying Firmware mode");
    ping_device(port, client, state, DeviceMode::Firmware).await
}

async fn ping_device(
    port: &'static str,
    client: &mut AsyncClient,
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
            state.set_device_mode(client, mode);
            state.handle_packet(client, packet);
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
    client: &mut AsyncClient,
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
            state.handle_packet(client, packet);
        }
    }

    Ok(())
}
