use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

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

#[derive(Clone)]
pub struct FrozenStateManager {
    update_tx: mpsc::Sender<FrozenUpdate>,
    state_lock: Arc<Mutex<FrozenState>>,
}

impl FrozenStateManager {
    pub fn new(update_tx: mpsc::Sender<FrozenUpdate>) -> Self {
        Self {
            update_tx,
            state_lock: Arc::new(Mutex::new(FrozenState::default())),
        }
    }

    pub async fn is_ready(&self) -> bool {
        self.state_lock.lock().await.device_mode == DeviceMode::Firmware
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
            self.send_update(FrozenUpdate::DeviceMode(mode));
        }
    }

    fn send_update(&self, update: FrozenUpdate) {
        if let Err(e) = self.update_tx.try_send(update) {
            log::error!("Failed to send to state_tx: {e}");
        }
    }

    pub async fn handle_packet(&self, packet: FrozenPacket) {
        match packet {
            FrozenPacket::Pong(in_firmware) => {
                self.set_device_mode(DeviceMode::from_pong(in_firmware))
                    .await;
            }
            FrozenPacket::TemperatureUpdate(u) => {
                log::debug!(
                    "Temperature update - Left: {:.1}, Right: {:.1}, Heatsink: {:.1}, Power: {}",
                    u.left_temp,
                    u.right_temp,
                    u.heatsink_temp,
                    u.error
                );
                self.send_update(FrozenUpdate::Temperature(u.clone()));
                let mut state = self.state_lock.lock().await;
                state.temp = Some(u);
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
                        self.send_update(FrozenUpdate::LeftTarget(u.clone()));
                        let mut state = self.state_lock.lock().await;
                        state.left_target = Some(u);
                    }
                    BedSide::Right => {
                        self.send_update(FrozenUpdate::RightTarget(u.clone()));
                        let mut state = self.state_lock.lock().await;
                        state.right_target = Some(u);
                    }
                }
            }
            FrozenPacket::HardwareInfo(info) => {
                log::info!("Hardware info: {info}");
                self.send_update(FrozenUpdate::HardwareInfo(info.clone()));
                let mut state = self.state_lock.lock().await;
                state.hardware_info = Some(info);
            }
            FrozenPacket::JumpingToFirmware(code) => {
                log::debug!("Jumping to firmware with code: 0x{code:02X}");
                self.set_device_mode(DeviceMode::Firmware).await;
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
