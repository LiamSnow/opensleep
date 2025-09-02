use rumqttc::AsyncClient;

use crate::{
    common::{
        packet::{BedSide, HardwareInfo},
        serial::DeviceMode,
    },
    mqtt::{publish_guaranteed, publish_high_freq},
    sensor::packet::SensorPacket,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SensorState {
    pub device_mode: DeviceMode,
    pub hardware_info: Option<HardwareInfo>,
    pub vibration_enabled: bool,
    pub piezo_gain: Option<(u16, u16)>,
    pub piezo_freq: Option<u32>,
    pub piezo_enabled: bool,
    pub alarm_left_running: bool,
    pub alarm_right_running: bool,
}

pub const PIEZO_GAIN: u16 = 400;
const PIEZO_TOLERANCE: i16 = 6;
pub const PIEZO_FREQ: u32 = 1000;

const TOPIC_MODE: &str = "opensleep/sensor/mode";
const TOPIC_HWINFO: &str = "opensleep/sensor/hwinfo";
const TOPIC_PIEZO_OK: &str = "opensleep/sensor/piezo_ok";
const TOPIC_VIBRATION_ENABLED: &str = "opensleep/sensor/vibration_enabled";
const TOPIC_BED_TEMP: &str = "opensleep/sensor/bed_temp";
const TOPIC_AMBIENT_TEMP: &str = "opensleep/sensor/ambient_temp";
const TOPIC_HUMIDITY: &str = "opensleep/sensor/humidity";
const TOPIC_MCU_TEMP: &str = "opensleep/sensor/mcu_temp";

impl SensorState {
    pub fn piezo_gain_ok(&self) -> bool {
        match self.piezo_gain {
            Some((l, r)) => {
                (PIEZO_GAIN as i16 - l as i16).abs() < PIEZO_TOLERANCE
                    && (PIEZO_GAIN as i16 - r as i16).abs() < PIEZO_TOLERANCE
            }
            None => false,
        }
    }

    pub fn piezo_freq_ok(&self) -> bool {
        self.piezo_freq == Some(PIEZO_FREQ)
    }

    pub fn piezo_ok(&self) -> bool {
        self.piezo_enabled && self.piezo_gain_ok() && self.piezo_freq_ok()
    }

    pub fn set_device_mode(&mut self, client: &mut AsyncClient, mode: DeviceMode) {
        let prev = self.device_mode;
        self.device_mode = mode;

        if prev != mode {
            log::info!("Device mode: {prev:?} -> {mode:?}");
            publish_guaranteed(client, TOPIC_MODE, false, mode.to_string());
        }
    }

    pub fn get_alarm_for_side(&self, side: &BedSide) -> bool {
        match side {
            BedSide::Left => self.alarm_left_running,
            BedSide::Right => self.alarm_right_running,
        }
    }

    fn publish_piezo_ok(&self, client: &mut AsyncClient) {
        publish_guaranteed(client, TOPIC_PIEZO_OK, false, self.piezo_ok().to_string());
    }

    /// [%s] off
    /// [%s] start: power %u, pattern %u, dur %u ms
    /// [%s] no longer running (max duration)
    /// [%s] new sequence run. ramp power to %u
    fn handle_alarm_msg(&mut self, msg: &str) {
        // TODO test
        let (bedside, rest) = if let Some(start) = msg.find('[') {
            if let Some(end) = msg.find(']') {
                let bedside = &msg[start + 1..end];
                let remaining = &msg[end + 1..];
                if bedside != "left" && bedside != "right" {
                    log::warn!("Unknown bedside in alarm message: {}", bedside);
                    return;
                }
                (bedside.to_string(), remaining.trim())
            } else {
                log::warn!("Alarm message missing closing bracket: {}", msg);
                return;
            }
        } else {
            log::warn!("Alarm message missing opening bracket: {}", msg);
            return;
        };

        let alarm_running = if bedside == "left" {
            &mut self.alarm_left_running
        } else {
            &mut self.alarm_right_running
        };

        if rest == "off" {
            log::info!("Alarm[{bedside}] off");
            *alarm_running = false;
        } else if rest == "no longer running (max duration)" {
            log::info!("Alarm[{bedside}] duration complete");
            *alarm_running = false;
        } else if let Some(rest) = rest.strip_prefix("start: ") {
            log::info!("Alarm[{bedside}] started: {rest}");
            *alarm_running = true;
        } else if let Some(val) = rest.strip_prefix("new sequence run. ramp power to ") {
            log::debug!("Alarm[{bedside}] ramping power to {val}");
            *alarm_running = true;
        } else {
            log::warn!("Unknown alarm message: {msg}");
        }
    }

    pub fn handle_packet(&mut self, client: &mut AsyncClient, packet: SensorPacket) {
        match packet {
            SensorPacket::Pong(in_firmware) => {
                self.set_device_mode(client, DeviceMode::from_pong(in_firmware));
            }
            SensorPacket::HardwareInfo(info) => {
                log::info!("Hardware info: {info}");
                publish_guaranteed(client, TOPIC_HWINFO, true, info.to_string());
                self.hardware_info = Some(info);
            }
            SensorPacket::JumpingToFirmware(code) => {
                log::debug!("Jumping to firmware with code: 0x{code:02X}");
                self.set_device_mode(client, DeviceMode::Firmware);
            }
            SensorPacket::Message(msg) => {
                if let Some(stripped) = msg.strip_prefix("FW: alarm") {
                    self.handle_alarm_msg(stripped);
                } else {
                    log::debug!("Message: {msg}");
                }
            }
            SensorPacket::PiezoGainSet(l, r) => {
                log::info!("Piezo Gain Set: {l},{r}");
                self.publish_piezo_ok(client);
                self.piezo_gain = Some((l, r));
            }
            SensorPacket::PiezoEnabled(val) => {
                log::info!("Piezo Enabled {val:02X}");
                self.publish_piezo_ok(client);
                self.piezo_enabled = true;
            }
            SensorPacket::VibrationEnabled(_, _) => {
                log::info!("Vibration Enabled");
                publish_guaranteed(client, TOPIC_VIBRATION_ENABLED, false, "true");
                self.vibration_enabled = true;
            }
            SensorPacket::Capacitance(_) => {}
            SensorPacket::Temperature(u) => {
                publish_high_freq(
                    client,
                    TOPIC_BED_TEMP,
                    format!(
                        "{}{}{}{}{}{}",
                        u.bed[0], u.bed[1], u.bed[2], u.bed[3], u.bed[4], u.bed[5]
                    ),
                );
                publish_high_freq(client, TOPIC_AMBIENT_TEMP, u.ambient.to_string());
                publish_high_freq(client, TOPIC_HUMIDITY, u.humidity.to_string());
                publish_high_freq(client, TOPIC_MCU_TEMP, u.microcontroller.to_string());
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
                if gain_changed || freq_changed || enabled_changed {
                    self.publish_piezo_ok(client);
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
