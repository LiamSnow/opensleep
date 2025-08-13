use crate::common::serial::SerialError;
use crate::common::serial::create_framed_port;
use crate::frozen::state::{FrozenState, FrozenUpdate};
use crate::frozen::{FrozenCommand, FrozenPacket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, interval, sleep};

pub const PORT: &str = "/dev/ttymxc2";
const BAUD: u32 = 38400;

pub async fn run(
    port: &'static str,
    mut command_rx: mpsc::Receiver<FrozenCommand>,
    mut update_tx: mpsc::Sender<FrozenUpdate>,
) -> Result<(), SerialError> {
    log::info!("Initializing Frozen Subsystem...");

    let (mut writer, mut reader) = create_framed_port::<FrozenPacket>(port, BAUD)?.split();

    let mut state = FrozenState::default();

    // grab hwinfo @ boot
    if let Err(e) = writer.send(FrozenCommand::Ping).await {
        log::error!("Failed to ping: {e}");
    }
    sleep(Duration::from_millis(200)).await;
    if let Err(e) = writer.send(FrozenCommand::GetHardwareInfo).await {
        log::error!("Failed to send GetHardwareInfo: {e}");
    }

    let mut interval = interval(Duration::from_millis(20));
    let mut last_hwinfo = Instant::now();
    let mut last_wake = Instant::now();

    loop {
        tokio::select! {
            Some(result) = reader.next() => match result {
                Ok(packet) => {
                    state.handle_packet(&mut update_tx, packet);
                }
                Err(e) => {
                    log::error!("Packet decode error: {e}");
                }
            },

            // sends commands separated by 20ms
            // before sending any commands, wakes the device by sending ping + jump fw
            _ = interval.tick() => if !command_rx.is_empty() {
                let now = Instant::now();

                // ready to send command
                if state.is_awake() {
                    // get hwinfo if first attempt didn't work
                    if now.duration_since(last_hwinfo) > Duration::from_secs(1) && state.hardware_info.is_none() {
                        last_hwinfo = now;
                        if let Err(e) = writer.send(FrozenCommand::GetHardwareInfo).await {
                            log::error!("Failed to send GetHardwareInfo: {e}");
                        }
                    }

                    // send out command
                    else if let Ok(cmd) = command_rx.try_recv() {
                        if let Err(e) = writer.send(cmd).await {
                            log::error!("Failed to send command: {e}");
                        }
                    }
                }

                // keep trying to wake it up, give it 2 seconds every attempt
                else if now.duration_since(last_wake) > Duration::from_secs(2) {
                    last_wake = now;
                    if let Err(e) = writer.send(FrozenCommand::Ping).await {
                        log::error!("Failed to ping: {e}");
                    }
                    sleep(Duration::from_millis(200)).await;
                    if let Err(e) = writer.send(FrozenCommand::JumpToFirmware).await {
                        log::error!("Failed to send GetHardwareInfo: {e}");
                    }
                }
            }
        }
    }
}
