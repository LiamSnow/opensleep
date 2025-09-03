use std::{borrow::Cow, error::Error};

use crate::mqtt::publish_guaranteed;

use super::{AlarmConfig, CONFIG_FILE, Config, SidesConfig};
use rumqttc::AsyncClient;
use tokio::sync::watch;

const TOPIC_TIMEZONE: &str = "opensleep/config/timezone";
const TOPIC_AWAY_MODE: &str = "opensleep/config/away_mode";
const TOPIC_PRIME_TIME: &str = "opensleep/config/prime_time";

const TOPIC_LED_IDLE: &str = "opensleep/config/led/idle";
const TOPIC_LED_ACTIVE: &str = "opensleep/config/led/active";
const TOPIC_LED_BAND: &str = "opensleep/config/led/band";

const TOPIC_MQTT_SERVER: &str = "opensleep/config/mqtt/server";
const TOPIC_MQTT_PORT: &str = "opensleep/config/mqtt/port";
const TOPIC_MQTT_USER: &str = "opensleep/config/mqtt/user";

const TOPIC_PROFILE_TYPE: &str = "opensleep/config/profile/type";

const TOPIC_PROFILE_LEFT_SLEEP: &str = "opensleep/config/profile/left/sleep";
const TOPIC_PROFILE_LEFT_WAKE: &str = "opensleep/config/profile/left/wake";
const TOPIC_PROFILE_LEFT_TEMPERATURES: &str = "opensleep/config/profile/left/temperatures";
const TOPIC_PROFILE_LEFT_ALARM: &str = "opensleep/config/profile/left/alarm";

const TOPIC_PROFILE_RIGHT_SLEEP: &str = "opensleep/config/profile/right/sleep";
const TOPIC_PROFILE_RIGHT_WAKE: &str = "opensleep/config/profile/right/wake";
const TOPIC_PROFILE_RIGHT_TEMPERATURES: &str = "opensleep/config/profile/right/temperatures";
const TOPIC_PROFILE_RIGHT_ALARM: &str = "opensleep/config/profile/right/alarm";

const TOPIC_PROFILE_SOLO_SLEEP: &str = "opensleep/config/profile/solo/sleep";
const TOPIC_PROFILE_SOLO_WAKE: &str = "opensleep/config/profile/solo/wake";
const TOPIC_PROFILE_SOLO_TEMPERATURES: &str = "opensleep/config/profile/solo/temperatures";
const TOPIC_PROFILE_SOLO_ALARM: &str = "opensleep/config/profile/solo/alarm";

const TOPIC_PRESENCE_BASELINES: &str = "opensleep/config/presence/baselines";
const TOPIC_PRESENCE_THRESHOLD: &str = "opensleep/config/presence/threshold";
const TOPIC_PRESENCE_DEBOUNCE_COUNT: &str = "opensleep/config/presence/debounce_count";

impl Config {
    pub fn publish(&self, client: &mut AsyncClient) {
        publish_guaranteed(
            client,
            TOPIC_TIMEZONE,
            true,
            self.timezone.iana_name().unwrap_or("ERROR"),
        );

        publish_guaranteed(client, TOPIC_AWAY_MODE, true, self.away_mode.to_string());

        publish_guaranteed(client, TOPIC_PRIME_TIME, true, self.prime.to_string());

        // led
        publish_guaranteed(client, TOPIC_LED_IDLE, true, self.led.idle.to_string());
        publish_guaranteed(client, TOPIC_LED_ACTIVE, true, self.led.active.to_string());
        publish_guaranteed(client, TOPIC_LED_BAND, true, self.led.band.to_string());

        // mqtt
        publish_guaranteed(
            client,
            TOPIC_MQTT_SERVER,
            true,
            self.mqtt.server.to_string(),
        );
        publish_guaranteed(client, TOPIC_MQTT_PORT, true, self.mqtt.port.to_string());
        publish_guaranteed(client, TOPIC_MQTT_USER, true, self.mqtt.user.to_string());

        // presence
        if let Some(presence) = &self.presence {
            publish_guaranteed(
                client,
                TOPIC_PRESENCE_BASELINES,
                true,
                presence
                    .baselines
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );

            publish_guaranteed(
                client,
                TOPIC_PRESENCE_THRESHOLD,
                true,
                presence.threshold.to_string(),
            );
            publish_guaranteed(
                client,
                TOPIC_PRESENCE_DEBOUNCE_COUNT,
                true,
                presence.debounce_count.to_string(),
            );
        }

        match &self.profile {
            SidesConfig::Solo(solo) => {
                publish_guaranteed(client, TOPIC_PROFILE_TYPE, true, "solo");

                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_SOLO_SLEEP,
                    true,
                    solo.sleep.to_string(),
                );
                publish_guaranteed(client, TOPIC_PROFILE_SOLO_WAKE, true, solo.wake.to_string());
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_SOLO_TEMPERATURES,
                    true,
                    temps_to_string(&solo.temperatures),
                );
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_SOLO_ALARM,
                    true,
                    alarm_to_string(&solo.alarm),
                );
            }
            SidesConfig::Couples { left, right } => {
                publish_guaranteed(client, TOPIC_PROFILE_TYPE, true, "couples");

                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_LEFT_SLEEP,
                    true,
                    left.sleep.to_string(),
                );
                publish_guaranteed(client, TOPIC_PROFILE_LEFT_WAKE, true, left.wake.to_string());
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_LEFT_TEMPERATURES,
                    true,
                    temps_to_string(&left.temperatures),
                );
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_LEFT_ALARM,
                    true,
                    alarm_to_string(&left.alarm),
                );

                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_RIGHT_SLEEP,
                    true,
                    right.sleep.to_string(),
                );
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_RIGHT_WAKE,
                    true,
                    right.wake.to_string(),
                );
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_RIGHT_TEMPERATURES,
                    true,
                    temps_to_string(&right.temperatures),
                );
                publish_guaranteed(
                    client,
                    TOPIC_PROFILE_RIGHT_ALARM,
                    true,
                    alarm_to_string(&right.alarm),
                );
            }
        }
    }
}

pub fn handle_publish(
    topic: &str,
    payload: Cow<'_, str>,
    config_tx: &mut watch::Sender<Config>,
    config_rx: &mut watch::Receiver<Config>,
) -> Result<(), Box<dyn Error>> {
    let mut cfg = config_rx.borrow().clone();

    // verification
    if topic.starts_with("opensleep/config/profile/right")
        || topic.starts_with("opensleep/config/profile/left") && cfg.profile.is_solo()
    {
        return Err("Publish to `couples` profile, but profile is in `solo` mode. Please edit the config.ron file and restart opensleep.".into());
    }

    if topic.starts_with("opensleep/config/profile/solo") && cfg.profile.is_couples() {
        return Err(
            "Publish to `solo` profile, but profile is in `couples` mode. Please edit the config.ron file and restart opensleep.".into()
        );
    }

    if topic.starts_with("opensleep/config/presence") && cfg.presence.is_none() {
        return Err(
            "Publish to `presence` config, but no presence exists. Please calibrate first!".into(),
        );
    }

    // modify config
    match topic {
        TOPIC_AWAY_MODE => {
            cfg.away_mode = payload.trim().parse()?;
        }

        TOPIC_PRIME_TIME => {
            cfg.prime = payload.trim().parse()?;
        }

        // led
        TOPIC_LED_IDLE => {
            cfg.led.idle = payload.trim().parse()?;
        }
        TOPIC_LED_ACTIVE => {
            cfg.led.active = payload.trim().parse()?;
        }

        // left profile
        TOPIC_PROFILE_LEFT_SLEEP => {
            cfg.profile.unwrap_left_mut().sleep = payload.trim().parse()?;
        }
        TOPIC_PROFILE_LEFT_WAKE => {
            cfg.profile.unwrap_left_mut().wake = payload.trim().parse()?;
        }
        TOPIC_PROFILE_LEFT_TEMPERATURES => {
            cfg.profile.unwrap_left_mut().temperatures = parse_temperatures(&payload)?;
        }
        TOPIC_PROFILE_LEFT_ALARM => {
            cfg.profile.unwrap_left_mut().alarm = parse_alarm(&payload)?;
        }

        // right profile
        TOPIC_PROFILE_RIGHT_SLEEP => {
            cfg.profile.unwrap_right_mut().sleep = payload.trim().parse()?;
        }
        TOPIC_PROFILE_RIGHT_WAKE => {
            cfg.profile.unwrap_right_mut().wake = payload.trim().parse()?;
        }
        TOPIC_PROFILE_RIGHT_TEMPERATURES => {
            cfg.profile.unwrap_right_mut().temperatures = parse_temperatures(&payload)?;
        }
        TOPIC_PROFILE_RIGHT_ALARM => {
            cfg.profile.unwrap_right_mut().alarm = parse_alarm(&payload)?;
        }

        // solo profile
        TOPIC_PROFILE_SOLO_SLEEP => {
            cfg.profile.unwrap_solo_mut().sleep = payload.trim().parse()?;
        }
        TOPIC_PROFILE_SOLO_WAKE => {
            cfg.profile.unwrap_solo_mut().wake = payload.trim().parse()?;
        }
        TOPIC_PROFILE_SOLO_TEMPERATURES => {
            cfg.profile.unwrap_solo_mut().temperatures = parse_temperatures(&payload)?;
        }
        TOPIC_PROFILE_SOLO_ALARM => {
            cfg.profile.unwrap_solo_mut().alarm = parse_alarm(&payload)?;
        }

        // presence
        TOPIC_PRESENCE_BASELINES => {
            cfg.presence.as_mut().unwrap().baselines = parse_baselines(&payload)?;
        }
        TOPIC_PRESENCE_THRESHOLD => {
            cfg.presence.as_mut().unwrap().threshold = payload.trim().parse()?;
        }
        TOPIC_PRESENCE_DEBOUNCE_COUNT => {
            cfg.presence.as_mut().unwrap().debounce_count = payload.trim().parse()?;
        }

        // RO
        TOPIC_TIMEZONE | TOPIC_MQTT_SERVER | TOPIC_MQTT_PORT | TOPIC_MQTT_USER
        | TOPIC_PROFILE_TYPE => {
            return Err(format!("Publish to read-only config topic: {}", topic).into());
        }

        // unknown
        topic => {
            return Err(format!("Publish to unknown config topic: {topic}").into());
        }
    }

    // notify others
    if let Err(e) = config_tx.send(cfg.clone()) {
        return Err(format!("Error sending to config watch channel: {e}").into());
    }

    // save to file
    if let Err(e) = cfg.save(CONFIG_FILE) {
        return Err(format!("Failed to save config: {e}").into());
    }
    log::debug!("Config saved to disk");
    Ok(())
}

fn parse_temperatures(payload: &str) -> Result<Vec<f32>, String> {
    payload
        .trim()
        .split(',')
        .map(|s| s.trim().parse::<f32>().map_err(|e| e.to_string()))
        .collect()
}

fn parse_alarm(payload: &str) -> Result<Option<AlarmConfig>, String> {
    let trimmed = payload.trim();

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

fn parse_baselines(payload: &str) -> Result<[u16; 6], String> {
    let values: Result<Vec<u16>, _> = payload
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
