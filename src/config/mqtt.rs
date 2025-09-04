use std::{borrow::Cow, error::Error};

use crate::{
    config::{PresenceConfig, SideConfig},
    mqtt::publish_guaranteed_wait,
};

use super::{AlarmConfig, CONFIG_FILE, Config, SidesConfig};
use jiff::civil::Time;
use rumqttc::AsyncClient;
use tokio::sync::watch;

const TOPIC_TIMEZONE: &str = "opensleep/state/config/timezone";
const TOPIC_AWAY_MODE: &str = "opensleep/state/config/away_mode";
const TOPIC_PRIME: &str = "opensleep/state/config/prime";

const TOPIC_LED_IDLE: &str = "opensleep/state/config/led/idle";
const TOPIC_LED_ACTIVE: &str = "opensleep/state/config/led/active";
const TOPIC_LED_BAND: &str = "opensleep/state/config/led/band";

const TOPIC_PROFILE_TYPE: &str = "opensleep/state/config/profile/type";

const TOPIC_PROFILE_LEFT_SLEEP: &str = "opensleep/state/config/profile/left/sleep";
const TOPIC_PROFILE_LEFT_WAKE: &str = "opensleep/state/config/profile/left/wake";
const TOPIC_PROFILE_LEFT_TEMPERATURES: &str = "opensleep/state/config/profile/left/temperatures";
const TOPIC_PROFILE_LEFT_ALARM: &str = "opensleep/state/config/profile/left/alarm";

const TOPIC_PROFILE_RIGHT_SLEEP: &str = "opensleep/state/config/profile/right/sleep";
const TOPIC_PROFILE_RIGHT_WAKE: &str = "opensleep/state/config/profile/right/wake";
const TOPIC_PROFILE_RIGHT_TEMPERATURES: &str = "opensleep/state/config/profile/right/temperatures";
const TOPIC_PROFILE_RIGHT_ALARM: &str = "opensleep/state/config/profile/right/alarm";

const TOPIC_PRESENCE_BASELINES: &str = "opensleep/state/config/presence/baselines";
const TOPIC_PRESENCE_THRESHOLD: &str = "opensleep/state/config/presence/threshold";
const TOPIC_PRESENCE_DEBOUNCE_COUNT: &str = "opensleep/state/config/presence/debounce_count";

pub const TOPIC_SET_AWAY_MODE: &str = "opensleep/actions/set_away_mode";
pub const TOPIC_SET_PRIME: &str = "opensleep/actions/set_prime";
pub const TOPIC_SET_PROFILE: &str = "opensleep/actions/set_profile";
pub const TOPIC_SET_PRESENCE: &str = "opensleep/actions/set_presence_config";

impl PresenceConfig {
    async fn publish(&self, client: &mut AsyncClient) {
        publish_guaranteed_wait(
            client,
            TOPIC_PRESENCE_BASELINES,
            true,
            self.baselines
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(","),
        )
        .await;

        publish_guaranteed_wait(
            client,
            TOPIC_PRESENCE_THRESHOLD,
            true,
            self.threshold.to_string(),
        )
        .await;
        publish_guaranteed_wait(
            client,
            TOPIC_PRESENCE_DEBOUNCE_COUNT,
            true,
            self.debounce_count.to_string(),
        )
        .await;
    }
}

impl SidesConfig {
    async fn publish(&self, client: &mut AsyncClient) {
        match &self {
            SidesConfig::Solo(solo) => {
                publish_guaranteed_wait(client, TOPIC_PROFILE_TYPE, true, "solo").await;
                publish_left_profile(client, solo).await;
            }
            SidesConfig::Couples { left, right } => {
                publish_guaranteed_wait(client, TOPIC_PROFILE_TYPE, true, "couples").await;

                publish_left_profile(client, left).await;

                publish_profile(
                    client,
                    right,
                    TOPIC_PROFILE_RIGHT_SLEEP,
                    TOPIC_PROFILE_RIGHT_WAKE,
                    TOPIC_PROFILE_RIGHT_TEMPERATURES,
                    TOPIC_PROFILE_RIGHT_ALARM,
                )
                .await;
            }
        }
    }
}

impl Config {
    pub async fn publish(&self, client: &mut AsyncClient) {
        log::debug!("Publishing config..");
        publish_guaranteed_wait(
            client,
            TOPIC_TIMEZONE,
            true,
            self.timezone.iana_name().unwrap_or("ERROR"),
        )
        .await;

        publish_away_mode(client, self.away_mode).await;

        publish_prime(client, self.prime).await;

        // led
        publish_guaranteed_wait(client, TOPIC_LED_IDLE, true, format!("{:?}", self.led.idle)).await;
        publish_guaranteed_wait(
            client,
            TOPIC_LED_ACTIVE,
            true,
            format!("{:?}", self.led.active),
        )
        .await;
        publish_guaranteed_wait(client, TOPIC_LED_BAND, true, self.led.band.to_string()).await;

        // presence
        if let Some(presence) = &self.presence {
            presence.publish(client).await;
        }

        self.profile.publish(client).await;

        log::debug!("Published config");
    }
}

async fn publish_prime(client: &mut AsyncClient, value: Time) {
    publish_guaranteed_wait(client, TOPIC_PRIME, true, value.to_string()).await;
}

async fn publish_away_mode(client: &mut AsyncClient, mode: bool) {
    publish_guaranteed_wait(client, TOPIC_AWAY_MODE, true, mode.to_string()).await;
}

async fn publish_left_profile(client: &mut AsyncClient, side: &SideConfig) {
    publish_profile(
        client,
        side,
        TOPIC_PROFILE_LEFT_SLEEP,
        TOPIC_PROFILE_LEFT_WAKE,
        TOPIC_PROFILE_LEFT_TEMPERATURES,
        TOPIC_PROFILE_LEFT_ALARM,
    )
    .await;
}

async fn publish_profile(
    client: &mut AsyncClient,
    side: &SideConfig,
    topic_sleep: &'static str,
    topic_wake: &'static str,
    topic_temps: &'static str,
    topic_alarm: &'static str,
) {
    publish_guaranteed_wait(client, topic_sleep, true, side.sleep.to_string()).await;
    publish_guaranteed_wait(client, topic_wake, true, side.wake.to_string()).await;
    publish_guaranteed_wait(
        client,
        topic_temps,
        true,
        temps_to_string(&side.temperatures),
    )
    .await;
    publish_guaranteed_wait(client, topic_alarm, true, alarm_to_string(&side.alarm)).await;
}

pub async fn handle_action(
    client: &mut AsyncClient,
    topic: &str,
    payload: Cow<'_, str>,
    config_tx: &mut watch::Sender<Config>,
    config_rx: &mut watch::Receiver<Config>,
) -> Result<(), Box<dyn Error>> {
    let mut cfg = config_rx.borrow().clone();

    // modify config
    match topic {
        TOPIC_SET_AWAY_MODE => {
            cfg.away_mode = payload.trim().parse()?;
            log::info!("Set away_mode to {}", cfg.away_mode);
            publish_away_mode(client, cfg.away_mode).await;
        }

        TOPIC_SET_PRIME => {
            cfg.prime = payload.trim().parse()?;
            log::info!("Set prime time to {}", cfg.prime);
            publish_prime(client, cfg.prime).await;
        }

        TOPIC_SET_PROFILE => {
            // TARGET.FIELD=VALUE
            let (target, rhs) = payload
                .trim()
                .split_once('.')
                .ok_or("Invalid input. Requires `TARGET.FIELD=VALUE`")?;

            let (field, value) = rhs
                .trim()
                .split_once('=')
                .ok_or("Invalid input. Requires `TARGET.FIELD=VALUE`")?;

            if ["left", "right"].contains(&target) && cfg.profile.is_solo() {
                return Err(
                    "Cannot modify profile in `couples` mode (currently in `solo` mode)".into(),
                );
            }

            let profile = match target {
                "left" => cfg.profile.unwrap_left_mut(),
                "right" => cfg.profile.unwrap_right_mut(),
                "both" => {
                    if cfg.profile.is_couples() {
                        return Err(
                            "Cannot modify profile in `solo` mode (currently in `couples` mode)"
                                .into(),
                        );
                    }

                    cfg.profile.unwrap_solo_mut()
                }
                _ => return Err("Invalid TARGET. Must be `left`, `right`, or `both`".into()),
            };

            match field {
                "sleep" => {
                    profile.sleep = value.parse()?;
                }
                "wake" => {
                    profile.wake = value.parse()?;
                }
                "temperatures" => {
                    profile.temperatures = parse_temperatures(value)?;
                }
                "alarm" => {
                    profile.alarm = parse_alarm(value)?;
                }
                _ => {
                    return Err(
                        "Invalid FIELD. Must be `sleep`, `wake`, `temperatures`, or `alarm`".into(),
                    );
                }
            }

            log::info!("Updated profile ({target}::{field} -> {value})");
            cfg.profile.publish(client).await;
        }

        TOPIC_SET_PRESENCE => {
            if cfg.presence.is_none() {
                return Err("Cannot modify non-existant presense configuration. Please call `actions/calibrate` first!".into());
            }

            let (field, value) = payload
                .trim()
                .split_once('=')
                .ok_or("Invalid input. Requires `FIELD=VALUE`")?;

            match field {
                "baselines" => {
                    cfg.presence.as_mut().unwrap().baselines = parse_baselines(value)?;
                }
                "threshold" => {
                    cfg.presence.as_mut().unwrap().threshold = value.trim().parse()?;
                }
                "debounce_count" => {
                    cfg.presence.as_mut().unwrap().debounce_count = value.trim().parse()?;
                }
                _ => return Err("Unknown field".into()),
            }

            log::info!("Update presence config ({field} -> {value})");
            cfg.presence.as_ref().unwrap().publish(client).await;
        }

        topic => {
            return Err(format!("Publish to unknown config topic: {topic}").into());
        }
    }

    // notify others
    if let Err(e) = config_tx.send(cfg.clone()) {
        return Err(format!("Error sending to config watch channel: {e}").into());
    }

    // save to file
    if let Err(e) = cfg.save(CONFIG_FILE).await {
        return Err(format!("Failed to save config: {e}").into());
    }
    log::debug!("Config saved to disk");

    Ok(())
}

fn parse_temperatures(value: &str) -> Result<Vec<f32>, String> {
    value
        .trim()
        .split(',')
        .map(|s| s.trim().parse::<f32>().map_err(|e| e.to_string()))
        .collect()
}

fn parse_alarm(value: &str) -> Result<Option<AlarmConfig>, String> {
    let trimmed = value.trim();

    if trimmed == "disabled" {
        return Ok(None);
    }

    let parts: Vec<&str> = trimmed.split(',').collect();
    if parts.len() != 4 {
        return Err(format!(
            "Expected 4 comma-separated values or 'disabled', got {}",
            parts.len()
        ));
    }

    let pattern = parts[0]
        .trim()
        .parse()
        .map_err(|e| format!("Invalid pattern: {e}"))?;
    let intensity = parts[1]
        .trim()
        .parse()
        .map_err(|e| format!("Invalid intensity: {e}"))?;
    let duration = parts[2]
        .trim()
        .parse()
        .map_err(|e| format!("Invalid duration: {e}"))?;
    let offset = parts[3]
        .trim()
        .parse()
        .map_err(|e| format!("Invalid offset: {e}"))?;

    Ok(Some(AlarmConfig {
        pattern,
        intensity,
        duration,
        offset,
    }))
}

fn parse_baselines(value: &str) -> Result<[u16; 6], String> {
    let values: Result<Vec<u16>, _> = value
        .trim()
        .split(',')
        .map(|s| s.trim().parse::<u16>().map_err(|e| e.to_string()))
        .collect();

    let values = values?;

    if values.len() != 6 {
        return Err(format!(
            "Expected exactly 6 baseline values, got {}",
            values.len()
        ));
    }

    Ok([
        values[0], values[1], values[2], values[3], values[4], values[5],
    ])
}

fn alarm_to_string(alarm: &Option<AlarmConfig>) -> String {
    match alarm {
        Some(a) => {
            format!("{},{},{},{}", a.pattern, a.intensity, a.duration, a.offset)
        }
        None => "disabled".to_string(),
    }
}

fn temps_to_string(temps: &Vec<f32>) -> String {
    temps
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
