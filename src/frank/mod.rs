use std::{io::ErrorKind, sync::Arc, time::Duration};

use command::FrankCommand;
use error::FrankError;
use log::info;
use state::FrankState;
use tokio::{
    fs,
    net::{UnixListener, UnixStream},
    sync::{mpsc, RwLock},
    time::interval,
};

pub mod command;
pub mod error;
pub mod state;
pub mod vibration;
mod socket;

const SOCKET_PATH: &str = "/deviceinfo/dac.sock";
const UPDATE_STATE_INT: Duration = Duration::from_secs(30);

pub type FrankStateLock = Arc<RwLock<FrankState>>;

/// Starts up the Frank Management process which will:
///  1. Replace the existing Unix Socket
///  2. Wait until Frank connects to us
///  3. Spawns a green thread to send commands, read state, and accept new Franks
///  4. Return a channel to send commands to and a shared state
pub async fn run() -> Result<(mpsc::Sender<FrankCommand>, FrankStateLock), FrankError> {
    remove_socket().await?;
    let mut listener =
        UnixListener::bind(SOCKET_PATH).map_err(|e| FrankError::BindUnixListener(e))?;

    let (cmd_tx, cmd_rx) = mpsc::channel(5);
    let state_lock = Arc::new(RwLock::new(FrankState::default()));

    // wait until we have a valid connection
    let stream = loop {
        if let Some(new_stream) = accept_new_frank(&mut listener).await {
            break new_stream;
        }
    };

    info!("[Frank] Frank is ready to play!");
    tokio::spawn(task(listener, stream, cmd_rx, state_lock.clone()));

    Ok((cmd_tx, state_lock))
}

async fn task(
    mut listener: UnixListener,
    mut stream: UnixStream,
    mut cmd_rx: mpsc::Receiver<FrankCommand>,
    state_lock: FrankStateLock,
) {
    info!("[Frank] Lets crank some frank!");
    let mut interval = interval(UPDATE_STATE_INT);

    loop {
        tokio::select! {
            new_stream = accept_new_frank(&mut listener) => {
                if let Some(new_stream) = new_stream {
                    stream = new_stream;
                }
            }

            cmd = cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    if let Err(e) = cmd.exec(&mut stream).await {
                        log::error!("[Frank] Error exec cmd: {e}")
                    }
                }
            }

            // first tick happens immediately
            _ = interval.tick() => {
                if let Some(new_state) = command::request_new_state(&mut stream).await {
                    let mut state = state_lock.write().await;
                    *state = new_state;
                }
            }
        }
    }
}

/// Removed the existing socket, if it exists
async fn remove_socket() -> Result<(), FrankError> {
    let a = fs::remove_file(SOCKET_PATH).await;
    match a {
        Ok(_) => {
            info!("[Frank] Did not have old socket");
            Ok(())
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            info!("[Frank] Removed old socket");
            Ok(())
        }
        Err(e) => Err(FrankError::RemoveSocket(e)),
    }
}

async fn accept_new_frank(listener: &mut UnixListener) -> Option<UnixStream> {
    match listener.accept().await {
        Ok((stream, _)) => {
            info!("[Frank] New Frank found in the wild");
            command::greet(stream).await
        }
        Err(e) => {
            log::error!("[Frank] New Frank did not work out: {e}");
            None
        }
    }
}
