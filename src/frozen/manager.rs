use crate::config::{Config, LEDConfig, SidesConfig};
use crate::frozen::{FrozenCommand, FrozenPacket, packet::FrozenTarget, state::FrozenState};
use crate::led::{IS31FL3194Config, IS31FL3194Controller};
use crate::{
    common::{
        codec::PacketCodec,
        packet::BedSide,
        serial::{SerialError, create_framed_port},
    },
    led::LedPattern,
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use jiff::{SignedDuration, Timestamp, civil::Time, tz::TimeZone};
use linux_embedded_hal::I2cdev;
use rumqttc::AsyncClient;
use tokio::sync::watch;
use tokio::time::{Duration, Instant, interval, sleep};
use tokio_serial::SerialStream;
use tokio_util::codec::Framed;

pub const PORT: &str = "/dev/ttymxc2";
const BAUD: u32 = 38400;

const HWINFO_INT: Duration = Duration::from_secs(1);
const TEMP_INT: Duration = Duration::from_secs(10);

struct CommandTimers {
    last_wake: Instant,
    last_hwinfo: Instant,
    last_left_temp: Instant,
    last_right_temp: Instant,
    last_prime: Instant,
}

type Writer = SplitSink<Framed<SerialStream, PacketCodec<FrozenPacket>>, FrozenCommand>;

pub async fn run(
    port: &'static str,
    mut config_rx: watch::Receiver<Config>,
    mut led: IS31FL3194Controller<I2cdev>,
    mut client: AsyncClient,
) -> Result<(), SerialError> {
    log::info!("Initializing Frozen Subsystem...");

    let cfg = config_rx.borrow_and_update();
    let mut led_idle = cfg.led.idle.get_config(cfg.led.band.clone());
    let mut led_active = cfg.led.active.get_config(cfg.led.band.clone());
    set_led(&mut led, &led_idle);
    let timezone = cfg.timezone.clone();
    let mut away_mode = cfg.away_mode;
    let mut prime = cfg.prime;
    let mut side_config = cfg.profile.clone();
    drop(cfg);

    let (mut writer, mut reader) = create_framed_port::<FrozenPacket>(port, BAUD)?.split();

    let mut state = FrozenState::default();

    // grab hwinfo @ boot
    send_command(&mut writer, FrozenCommand::Ping).await;
    sleep(Duration::from_millis(200)).await;
    send_command(&mut writer, FrozenCommand::GetHardwareInfo).await;

    let mut interval = interval(Duration::from_millis(20));
    let mut timers = CommandTimers::default();
    let mut was_active = false;

    loop {
        tokio::select! {
            Some(result) = reader.next() => match result {
                Ok(packet) => {
                    state.handle_packet(&mut client, packet);

                    if state.is_active() != was_active {
                        if was_active {
                            log::info!("Profile ended!");
                            set_led(&mut led, &led_idle);
                        } else {
                            log::info!("Starting profile!");
                            set_led(&mut led, &led_active);
                        }
                        was_active = !was_active;
                    }
                }
                Err(e) => {
                    log::error!("Packet decode error: {e}");
                }
            },

            // sends commands separated by 20ms
            // before sending any commands, wakes the device by sending ping + jump fw
            _ = interval.tick() => if let Some(cmd) = get_next_command(
                &mut timers,
                &state,
                &timezone,
                &away_mode,
                &prime,
                &side_config
            ) {
                let now = Instant::now();

                // ready to send command
                if state.is_awake() {
                    send_command(&mut writer, cmd).await;
                }

                // keep trying to wake it up, give it 2 seconds every attempt
                else if now.duration_since(timers.last_wake) > Duration::from_secs(2) {
                    timers.last_wake = now;
                    if let Err(e) = writer.send(FrozenCommand::Ping).await {
                        log::error!("Failed to ping: {e}");
                    }
                    sleep(Duration::from_millis(200)).await;
                    if let Err(e) = writer.send(FrozenCommand::JumpToFirmware).await {
                        log::error!("Failed to send JumpToFirmware: {e}");
                    }
                }
            },

            Ok(_) = config_rx.changed() => {
                let cfg = config_rx.borrow();
                away_mode = cfg.away_mode;
                prime = cfg.prime;
                side_config = cfg.profile.clone();
                led_idle = cfg.led.idle.get_config(cfg.led.band.clone());
                led_active = cfg.led.active.get_config(cfg.led.band.clone());
            }
        }
    }
}

fn get_next_command(
    timers: &mut CommandTimers,
    state: &FrozenState,
    timezone: &TimeZone,
    away_mode: &bool,
    prime_time: &Time,
    side_config: &SidesConfig,
) -> Option<FrozenCommand> {
    let now = Instant::now();

    if state.hardware_info.is_none() && now.duration_since(timers.last_hwinfo) > HWINFO_INT {
        timers.last_hwinfo = now;
        return Some(FrozenCommand::GetHardwareInfo);
    }

    if now.duration_since(timers.last_left_temp) > TEMP_INT {
        let wanted_left =
            FrozenTarget::calc_wanted(timezone, away_mode, side_config, &BedSide::Left);
        timers.last_left_temp = now;
        if state.left_target.as_ref() != Some(&wanted_left) {
            return Some(FrozenCommand::SetTargetTemperature {
                side: BedSide::Left,
                tar: wanted_left,
            });
        }
    }

    if now.duration_since(timers.last_right_temp) > TEMP_INT {
        let wanted_right =
            FrozenTarget::calc_wanted(timezone, away_mode, side_config, &BedSide::Right);
        timers.last_right_temp = now;

        if state.right_target.as_ref() != Some(&wanted_right) {
            return Some(FrozenCommand::SetTargetTemperature {
                side: BedSide::Right,
                tar: wanted_right,
            });
        }
    }

    let now_local = Timestamp::now().to_zoned(timezone.clone()).time();

    // TODO verify it actually started priming
    if !away_mode
        // prime if we are within 30 seconds of prime time AND we havn't tried to prime in the last minute
        && now.duration_since(timers.last_prime) > Duration::from_secs(60)
        && now_local.duration_until(*prime_time).abs() < SignedDuration::from_secs(30)
    {
        timers.last_prime = now;
        return Some(FrozenCommand::Prime);
    }

    None
}

async fn send_command(writer: &mut Writer, cmd: FrozenCommand) {
    let name = cmd.to_string();
    log::debug!(" -> {name}");
    if let Err(e) = writer.send(cmd).await {
        log::error!("Failed to write {name}: {e}");
    }
}

fn set_led(led: &mut IS31FL3194Controller<I2cdev>, cfg: &IS31FL3194Config) {
    if let Err(e) = led.set(cfg) {
        log::error!("Failed to set LED: {e}");
    }
}

impl Default for CommandTimers {
    fn default() -> Self {
        let now = Instant::now();
        let ago = now - Duration::from_secs(60);
        Self {
            last_wake: now,
            last_hwinfo: now,
            last_left_temp: ago,
            last_right_temp: ago,
            last_prime: ago,
        }
    }
}
