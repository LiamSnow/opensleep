use crate::common::serial::SerialError;
use crate::frozen::state::FrozenUpdate;
use crate::frozen::{FrozenCommand, FrozenPacket};
use crate::{
    common::{codec::PacketCodec, serial::create_framed_port},
    frozen::state::FrozenStateManager,
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};
use tokio_serial::SerialStream;
use tokio_util::codec::Framed;

pub const PORT: &str = "/dev/ttymxc2";
const MAX_WAKE_ATTEMPTS: u8 = 3;
const BAUD: u32 = 38400;

type Reader = SplitStream<Framed<SerialStream, PacketCodec<FrozenPacket>>>;
type Writer = SplitSink<Framed<SerialStream, PacketCodec<FrozenPacket>>, FrozenCommand>;

pub fn spawn(
    port: &'static str,
    command_rx: mpsc::Receiver<FrozenCommand>,
    update_tx: mpsc::Sender<FrozenUpdate>,
) -> Result<(), SerialError> {
    log::info!("Initializing Frozen Subsystem...");

    let (writer, reader) = create_framed_port::<FrozenPacket>(port, BAUD)?.split();

    let state_manager = FrozenStateManager::new(update_tx);

    tokio::spawn(read_task(reader, state_manager.clone()));
    tokio::spawn(write_task(writer, state_manager.clone(), command_rx));

    Ok(())
}

async fn read_task(mut reader: Reader, state_manager: FrozenStateManager) {
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

async fn write_task(
    mut writer: Writer,
    state_manager: FrozenStateManager,
    mut command_rx: mpsc::Receiver<FrozenCommand>,
) {
    // first wake up to get hardware info
    wake(&mut writer, &state_manager, true).await;

    // forward commands
    while let Some(cmd) = command_rx.recv().await {
        // make sure device is awake
        wake(&mut writer, &state_manager, false).await;

        log::debug!("TX: {cmd:?}");
        if let Err(e) = writer.send(cmd).await {
            log::error!("Failed to send command: {e}");
        }
    }
}

async fn wake(writer: &mut Writer, state_manager: &FrozenStateManager, get_hardware_info: bool) {
    let mut wake_attempts = 0;
    while !state_manager.is_ready().await {
        if wake_attempts >= MAX_WAKE_ATTEMPTS {
            log::error!("Failed to wake after {MAX_WAKE_ATTEMPTS} attempts");
            break;
        }

        // ping device
        if let Err(e) = writer.send(FrozenCommand::Ping).await {
            log::error!("Failed to ping: {e}");
            continue;
        }

        sleep(Duration::from_millis(200)).await;

        if get_hardware_info {
            if let Err(e) = writer.send(FrozenCommand::GetHardwareInfo).await {
                log::error!("Failed to send GetHardwareInfo: {e}");
            }
            sleep(Duration::from_millis(200)).await;
        }

        // jump to firmware mode
        if let Err(e) = writer.send(FrozenCommand::JumpToFirmware).await {
            log::error!("Failed to send JumpToFirmware: {e}");
            break;
        }

        // let it have a second to switch modes
        for _ in 0..10 {
            sleep(Duration::from_millis(200)).await;
            if state_manager.is_ready().await {
                break;
            }
        }

        wake_attempts += 1;
    }
}
