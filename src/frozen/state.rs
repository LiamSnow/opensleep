use tokio::sync::mpsc;

use crate::{
    common::{
        packet::{BedSide, HardwareInfo},
        serial::DeviceMode,
    },
    frozen::packet::{FrozenPacket, TargetUpdate, TemperatureUpdate},
};

#[derive(Clone, Debug)]
pub enum FrozenUpdate {
    DeviceMode(DeviceMode),
    HardwareInfo(HardwareInfo),
    Temperature(TemperatureUpdate),
    LeftTarget(TargetUpdate),
    RightTarget(TargetUpdate),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrozenState {
    pub device_mode: DeviceMode,
    pub temp: Option<TemperatureUpdate>,
    pub left_target: Option<TargetUpdate>,
    pub right_target: Option<TargetUpdate>,
    pub hardware_info: Option<HardwareInfo>,
}

impl FrozenState {
    pub fn is_awake(&self) -> bool {
        self.device_mode == DeviceMode::Firmware
    }

    pub fn set_device_mode(
        &mut self,
        update_tx: &mut mpsc::Sender<FrozenUpdate>,
        mode: DeviceMode,
    ) {
        let prev = self.device_mode;
        self.device_mode = mode;

        if prev != mode {
            log::info!("Device mode: {prev:?} -> {mode:?}");
            send_update(update_tx, FrozenUpdate::DeviceMode(mode));
        }
    }

    pub fn handle_packet(
        &mut self,
        update_tx: &mut mpsc::Sender<FrozenUpdate>,
        packet: FrozenPacket,
    ) {
        match packet {
            FrozenPacket::Pong(in_firmware) => {
                self.set_device_mode(update_tx, DeviceMode::from_pong(in_firmware));
            }
            FrozenPacket::TemperatureUpdate(u) => {
                log::debug!(
                    "Temperature update - Left: {:.1}, Right: {:.1}, Heatsink: {:.1}, Power: {}",
                    u.left_temp,
                    u.right_temp,
                    u.heatsink_temp,
                    u.error
                );
                send_update(update_tx, FrozenUpdate::Temperature(u.clone()));
                self.temp = Some(u);
            }
            FrozenPacket::TargetUpdate((side, u)) => {
                log::debug!(
                    "Target update - Side: {:?}, Enabled: {}, Temp: {:.1}",
                    side,
                    u.state,
                    u.temp
                );
                match side {
                    BedSide::Left => {
                        send_update(update_tx, FrozenUpdate::LeftTarget(u.clone()));
                        self.left_target = Some(u);
                    }
                    BedSide::Right => {
                        send_update(update_tx, FrozenUpdate::RightTarget(u.clone()));
                        self.right_target = Some(u);
                    }
                }
            }
            FrozenPacket::HardwareInfo(info) => {
                log::info!("Hardware info: {info}");
                send_update(update_tx, FrozenUpdate::HardwareInfo(info.clone()));
                self.hardware_info = Some(info);
            }
            FrozenPacket::JumpingToFirmware(code) => {
                log::debug!("Jumping to firmware with code: 0x{code:02X}");
                self.set_device_mode(update_tx, DeviceMode::Firmware);
            }
            FrozenPacket::Message(msg) => {
                if msg == "FW: water empty -> full" {
                    log::warn!("Water tank reinserted");
                } else if msg == "FW: water full -> empty" {
                    log::warn!("Water tank removed");
                } else if let Some(stripped) = msg.strip_prefix("FW: [priming] ") {
                    log::info!("Priming Message: {stripped}");
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

fn send_update(update_tx: &mut mpsc::Sender<FrozenUpdate>, update: FrozenUpdate) {
    if let Err(e) = update_tx.try_send(update) {
        log::error!("Failed to send to state_tx: {e}");
    }
}
