use rumqttc::AsyncClient;

use crate::{
    common::{
        packet::{BedSide, HardwareInfo},
        serial::DeviceMode,
    },
    frozen::packet::{FrozenPacket, FrozenTarget, TemperatureUpdate},
    mqtt::{publish_guaranteed, publish_high_freq},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrozenState {
    pub device_mode: DeviceMode,
    pub temp: Option<TemperatureUpdate>,
    pub left_target: Option<FrozenTarget>,
    pub right_target: Option<FrozenTarget>,
    pub hardware_info: Option<HardwareInfo>,
    pub is_priming: bool,
}

const TOPIC_MODE: &str = "opensleep/frozen/mode";
const TOPIC_HWINFO: &str = "opensleep/frozen/hwinfo";
const TOPIC_LEFT_TEMP: &str = "opensleep/frozen/left_temp";
const TOPIC_RIGHT_TEMP: &str = "opensleep/frozen/right_temp";
const TOPIC_HEATSINK_TEMP: &str = "opensleep/frozen/heatsink_temp";
const TOPIC_LEFT_TARGET_TEMP: &str = "opensleep/frozen/left_target_temp";
const TOPIC_RIGHT_TARGET_TEMP: &str = "opensleep/frozen/right_target_temp";

impl FrozenState {
    pub fn is_awake(&self) -> bool {
        self.device_mode == DeviceMode::Firmware
    }

    pub fn set_device_mode(&mut self, client: &mut AsyncClient, mode: DeviceMode) {
        let prev = self.device_mode;
        self.device_mode = mode;

        if prev != mode {
            log::info!("Device mode: {prev:?} -> {mode:?}");
            publish_guaranteed(client, TOPIC_MODE, false, mode.to_string());
        }
    }

    pub fn is_active(&self) -> bool {
        self.left_target.as_ref().is_some_and(|t| t.enabled)
            || self.right_target.as_ref().is_some_and(|t| t.enabled)
    }

    pub fn handle_packet(&mut self, client: &mut AsyncClient, packet: FrozenPacket) {
        match packet {
            FrozenPacket::Pong(in_firmware) => {
                self.set_device_mode(client, DeviceMode::from_pong(in_firmware));
            }
            FrozenPacket::TemperatureUpdate(u) => {
                log::debug!(
                    "Temperature update - Left: {:.1}, Right: {:.1}, Heatsink: {:.1}, Error: {}",
                    u.left_temp,
                    u.right_temp,
                    u.heatsink_temp,
                    u.error
                );

                publish_high_freq(client, TOPIC_LEFT_TEMP, u.left_temp.to_string());
                publish_high_freq(client, TOPIC_RIGHT_TEMP, u.right_temp.to_string());
                publish_high_freq(client, TOPIC_HEATSINK_TEMP, u.heatsink_temp.to_string());

                self.temp = Some(u);
            }
            FrozenPacket::TargetUpdate((side, u)) => {
                log::debug!(
                    "Target update - Side: {:?}, Enabled: {}, Temp: {:.1}",
                    side,
                    u.enabled,
                    u.temp
                );
                let payload = match u.enabled {
                    true => &u.temp.to_string(),
                    false => "disabled",
                };
                let topic = match side {
                    BedSide::Left => {
                        self.left_target = Some(u);
                        TOPIC_LEFT_TARGET_TEMP
                    }
                    BedSide::Right => {
                        self.right_target = Some(u);
                        TOPIC_RIGHT_TARGET_TEMP
                    }
                };
                publish_high_freq(client, topic, payload);
            }
            FrozenPacket::HardwareInfo(info) => {
                log::info!("Hardware info: {info}");
                publish_guaranteed(client, TOPIC_HWINFO, true, info.to_string());
                self.hardware_info = Some(info);
            }
            FrozenPacket::JumpingToFirmware(code) => {
                log::debug!("Jumping to firmware with code: 0x{code:02X}");
                self.set_device_mode(client, DeviceMode::Firmware);
            }
            FrozenPacket::Message(msg) => {
                if msg == "FW: water empty -> full" {
                    log::warn!("Water tank reinserted");
                } else if msg == "FW: water full -> empty" {
                    log::warn!("Water tank removed");
                } else if let Some(stripped) = msg.strip_prefix("FW: [priming] ") {
                    // done because empty
                    // done
                    // empty stage pause pumps for %u ms
                    // empty phase (%u remaining; runtime %u ms)
                    // empty stage finished w/ %u successful purge
                    // purge phase
                    // purge.fast (%u ms)
                    // purge_fast stage purged? %u
                    // start
                    // %u consecutive failed purges; %u total failed
                    // purge phase (%u iterations remaining)
                    // purge phase complete. now final empty stage
                    // purge.wait
                    // purge.side (%s: %s)
                    // purge.empty, both pumps at 12v
                    log::info!("Priming Message: {stripped}");

                    match stripped {
                        "done" | "done because empty" => self.is_priming = false,
                        "start" => self.is_priming = true,
                        _ => {}
                    }
                } else {
                    log::debug!("Message: {msg}")
                }
            }
            FrozenPacket::PrimingStarted => {
                log::info!("Priming started!");
            }
            _ => {}
        }
    }
}
