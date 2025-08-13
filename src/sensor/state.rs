use std::sync::Arc;

use tokio::sync::{Mutex, broadcast};

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

#[derive(Clone)]
pub struct SensorStateManager {
    update_tx: broadcast::Sender<SensorUpdate>,
    state_lock: Arc<Mutex<SensorState>>,
}

const PIEZO_GAIN: i16 = 400;
const PIEZO_TOLERANCE: i16 = 6;
const PIEZO_FREQ: u16 = 1000;

impl SensorStateManager {
    pub fn new(update_tx: broadcast::Sender<SensorUpdate>) -> Self {
        Self {
            update_tx,
            state_lock: Arc::new(Mutex::new(SensorState::default())),
        }
    }

    pub async fn piezo_gain_ok(&self) -> bool {
        match self.state_lock.lock().await.piezo_gain {
            Some((l, r)) => {
                (PIEZO_GAIN - l as i16).abs() < PIEZO_TOLERANCE
                    && (PIEZO_GAIN - r as i16).abs() < PIEZO_TOLERANCE
            }
            None => false,
        }
    }

    pub async fn piezo_freq(&self) -> Option<u16> {
        self.state_lock.lock().await.piezo_freq
    }

    pub async fn piezo_ok(&self) -> bool {
        let state = self.state_lock.lock().await;
        state.piezo_freq == Some(PIEZO_FREQ) && state.piezo_enabled
    }

    pub async fn device_mode(&self) -> DeviceMode {
        self.state_lock.lock().await.device_mode
    }

    pub async fn set_device_mode(&self, mode: DeviceMode) {
        let prev;

        {
            let mut state = self.state_lock.lock().await;
            prev = state.device_mode;
            state.device_mode = mode;
        }

        if prev != mode {
            log::info!("Device mode: {prev:?} -> {mode:?}");
            self.send_update(SensorUpdate::DeviceMode(mode));
        }
    }

    pub async fn has_hardware_info(&self) -> bool {
        self.state_lock.lock().await.hardware_info.is_some()
    }

    pub async fn vibration_enabled(&self) -> bool {
        self.state_lock.lock().await.vibration_enabled
    }

    fn send_update(&self, update: SensorUpdate) {
        if let Err(e) = self.update_tx.send(update) {
            log::error!("Failed to send to state_tx: {e}");
        }
    }

    pub async fn handle_packet(&self, packet: SensorPacket) {
        match packet {
            SensorPacket::Pong(in_firmware) => {
                self.set_device_mode(DeviceMode::from_pong(in_firmware))
                    .await;
            }
            SensorPacket::HardwareInfo(info) => {
                log::info!("Hardware info: {info}");
                self.send_update(SensorUpdate::HardwareInfo(info.clone()));
                let mut state = self.state_lock.lock().await;
                state.hardware_info = Some(info);
            }
            SensorPacket::JumpingToFirmware(code) => {
                log::debug!("Jumping to firmware with code: 0x{code:02X}");
                self.set_device_mode(DeviceMode::Firmware).await;
            }
            SensorPacket::Message(msg) => {
                log::debug!("Message: {msg}");
            }
            SensorPacket::PiezoGainSet(l, r) => {
                log::info!("Piezo Gain Set: {l},{r}");
                self.send_update(SensorUpdate::PiezoGain(l, r));
                let mut state = self.state_lock.lock().await;
                state.piezo_gain = Some((l, r));
            }
            SensorPacket::PiezoEnabled(cmd, val) => {
                log::info!("Piezo Enabled {cmd:02X},{val:02X}");
                self.send_update(SensorUpdate::PiezoEnabled(true));
                let mut state = self.state_lock.lock().await;
                state.piezo_enabled = true;
            }
            SensorPacket::VibrationEnabled(_, _) => {
                log::info!("Vibration Enabled");
                self.send_update(SensorUpdate::VibrationEnabled(true));
                let mut state = self.state_lock.lock().await;
                state.vibration_enabled = true;
            }
            SensorPacket::Capacitance(u) => {
                self.send_update(SensorUpdate::Capacitance(u));
            }
            SensorPacket::Temperature(u) => {
                self.send_update(SensorUpdate::Temperature(u));
            }
            SensorPacket::Piezo(u) => {
                let (enabled_changed, gain_changed, freq_changed);
                {
                    let mut state = self.state_lock.lock().await;
                    enabled_changed = !state.piezo_enabled;
                    gain_changed = state.piezo_gain != Some(u.gain);
                    freq_changed = state.piezo_freq != Some(u.freq);
                    state.piezo_enabled = true;
                    state.piezo_gain = Some(u.gain);
                    state.piezo_freq = Some(u.freq);
                }
                if gain_changed {
                    self.send_update(SensorUpdate::PiezoGain(u.gain.0, u.gain.1));
                }
                if freq_changed {
                    self.send_update(SensorUpdate::PiezoFreq(u.freq));
                }
                if enabled_changed {
                    self.send_update(SensorUpdate::PiezoEnabled(true));
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
