use tokio::sync::broadcast;

use crate::{
    common::{packet::HardwareInfo, serial::DeviceMode},
    sensor::packet::{CapacitanceData, SensorPacket, TemperatureData},
};

#[derive(Clone, Debug)]
pub enum SensorUpdate {
    DeviceMode(DeviceMode),
    HardwareInfo(HardwareInfo),
    VibrationEnabled(bool),
    Capacitance(CapacitanceData),
    Temperature(TemperatureData),
    PiezoGain(u16, u16),
    PiezoFreq(u16),
    PiezoEnabled(bool),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SensorState {
    pub device_mode: DeviceMode,
    pub hardware_info: Option<HardwareInfo>,
    pub vibration_enabled: bool,
    pub piezo_gain: Option<(u16, u16)>,
    pub piezo_freq: Option<u16>,
    pub piezo_enabled: bool,
}

const PIEZO_GAIN: i16 = 400;
const PIEZO_TOLERANCE: i16 = 6;
const PIEZO_FREQ: u16 = 1000;

impl SensorState {
    pub fn piezo_gain_ok(&self) -> bool {
        match self.piezo_gain {
            Some((l, r)) => {
                (PIEZO_GAIN - l as i16).abs() < PIEZO_TOLERANCE
                    && (PIEZO_GAIN - r as i16).abs() < PIEZO_TOLERANCE
            }
            None => false,
        }
    }

    pub fn piezo_freq_ok(&self) -> bool {
        self.piezo_freq == Some(PIEZO_FREQ)
    }

    pub fn set_device_mode(
        &mut self,
        update_tx: &mut broadcast::Sender<SensorUpdate>,
        mode: DeviceMode,
    ) {
        let prev = self.device_mode;
        self.device_mode = mode;

        if prev != mode {
            log::info!("Device mode: {prev:?} -> {mode:?}");
            send_update(update_tx, SensorUpdate::DeviceMode(mode));
        }
    }

    pub fn handle_packet(
        &mut self,
        update_tx: &mut broadcast::Sender<SensorUpdate>,
        packet: SensorPacket,
    ) {
        match packet {
            SensorPacket::Pong(in_firmware) => {
                self.set_device_mode(update_tx, DeviceMode::from_pong(in_firmware));
            }
            SensorPacket::HardwareInfo(info) => {
                log::info!("Hardware info: {info}");
                send_update(update_tx, SensorUpdate::HardwareInfo(info.clone()));
                self.hardware_info = Some(info);
            }
            SensorPacket::JumpingToFirmware(code) => {
                log::debug!("Jumping to firmware with code: 0x{code:02X}");
                self.set_device_mode(update_tx, DeviceMode::Firmware);
            }
            SensorPacket::Message(msg) => {
                log::debug!("Message: {msg}");
            }
            SensorPacket::PiezoGainSet(l, r) => {
                log::info!("Piezo Gain Set: {l},{r}");
                send_update(update_tx, SensorUpdate::PiezoGain(l, r));
                self.piezo_gain = Some((l, r));
            }
            SensorPacket::PiezoEnabled(cmd, val) => {
                log::info!("Piezo Enabled {cmd:02X},{val:02X}");
                send_update(update_tx, SensorUpdate::PiezoEnabled(true));
                self.piezo_enabled = true;
            }
            SensorPacket::VibrationEnabled(_, _) => {
                log::info!("Vibration Enabled");
                send_update(update_tx, SensorUpdate::VibrationEnabled(true));
                self.vibration_enabled = true;
            }
            SensorPacket::Capacitance(u) => {
                send_update(update_tx, SensorUpdate::Capacitance(u));
            }
            SensorPacket::Temperature(u) => {
                send_update(update_tx, SensorUpdate::Temperature(u));
            }
            SensorPacket::Piezo(u) => {
                let (enabled_changed, gain_changed, freq_changed);
                {
                    enabled_changed = !self.piezo_enabled;
                    gain_changed = self.piezo_gain != Some(u.gain);
                    freq_changed = self.piezo_freq != Some(u.freq);
                    self.piezo_enabled = true;
                    self.piezo_gain = Some(u.gain);
                    self.piezo_freq = Some(u.freq);
                }
                if gain_changed {
                    send_update(update_tx, SensorUpdate::PiezoGain(u.gain.0, u.gain.1));
                }
                if freq_changed {
                    send_update(update_tx, SensorUpdate::PiezoFreq(u.freq));
                }
                if enabled_changed {
                    send_update(update_tx, SensorUpdate::PiezoEnabled(true));
                }
            }
            SensorPacket::AlarmSet(v) => {
                log::info!("Alarm Set: {v}");
            }
            SensorPacket::Init(v) => {
                log::warn!("Init: {v}");
            }
            _ => {}
        }
    }
}

fn send_update(update_tx: &mut broadcast::Sender<SensorUpdate>, update: SensorUpdate) {
    if let Err(e) = update_tx.send(update) {
        log::error!("Failed to send to state_tx: {e}");
    }
}
