use crate::config::{
    Config, HeatConfig, LedConfig, LedPattern, Profile, VibrationConfig, VibrationPattern,
};
use crate::mqtt::MqttError;
use jiff::civil::Time;
use jiff::tz::TimeZone;
use tokio::sync::{mpsc, watch};

enum TimeField {
    Sleep,
    Wake,
}

pub async fn handle_command(
    topic: String,
    payload: bytes::Bytes,
    config_tx: &watch::Sender<Config>,
    config_rx: &watch::Receiver<Config>,
    calibrate_tx: &mut mpsc::Sender<()>,
) -> Result<(), MqttError> {
    let payload = String::from_utf8_lossy(&payload);
    match validate_extract_command(&topic)? {
        "set_sleep_time" => handle_set_time(&payload, config_tx, config_rx, TimeField::Sleep),
        "set_wake_time" => handle_set_time(&payload, config_tx, config_rx, TimeField::Wake),
        "set_timezone" => {
            let timezone = payload.trim();
            let tz =
                TimeZone::get(timezone).map_err(|e| MqttError::InvalidTimezone(e.to_string()))?;
            update_config(config_tx, config_rx, |cfg| cfg.timezone = tz)
        }
        "set_away_mode" => {
            let away_mode = parse_or_invalid::<bool>(&payload, "boolean value")?;
            update_config(config_tx, config_rx, |cfg| cfg.away_mode = away_mode)
        }
        "set_prime_time" => {
            let time = parse_time(payload.trim())?;
            update_config(config_tx, config_rx, |cfg| cfg.prime = time)
        }
        "set_led_config" => {
            let (idle_pattern, active_pattern) = parse_led_params(payload.trim())?;
            update_config(config_tx, config_rx, |cfg| {
                cfg.led = LedConfig {
                    idle: idle_pattern,
                    active: active_pattern,
                }
            })
        }
        "set_temp_profile" => {
            update_profile_field(&payload, config_tx, config_rx, |temps_str, profile| {
                let temps: Vec<i32> = temps_str
                    .split(',')
                    .map(|s| s.trim().parse::<i32>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| {
                        MqttError::InvalidCommand("Invalid temperature values".to_string())
                    })?;
                profile.temp_profile = temps;
                Ok(())
            })
        }
        "set_vibration_config" => {
            update_profile_field(&payload, config_tx, config_rx, |config_str, profile| {
                let (pattern, intensity, duration, offset) = parse_vibration_params(config_str)?;
                profile.vibration = VibrationConfig {
                    pattern,
                    intensity,
                    duration,
                    offset,
                };
                Ok(())
            })
        }
        "set_heat_config" => {
            update_profile_field(&payload, config_tx, config_rx, |config_str, profile| {
                let (temp, offset) = parse_heat_params(config_str)?;
                profile.heat = HeatConfig { temp, offset };
                Ok(())
            })
        }
        "calibrate" => Ok(calibrate_tx.send(()).await?),
        command_name => Err(MqttError::InvalidCommand(command_name.to_string())),
    }
}

fn validate_extract_command(topic: &str) -> Result<&str, MqttError> {
    let topic_bytes = topic.as_bytes();

    let first_slash = memchr::memchr(b'/', topic_bytes)
        .ok_or_else(|| MqttError::InvalidCommand("Topic missing first slash".to_string()))?;

    if &topic_bytes[..first_slash] != b"opensleep" {
        return Err(MqttError::InvalidCommand(
            "Topic doesn't begin with 'opensleep/'".to_string(),
        ));
    }

    let second_slash = memchr::memchr(b'/', &topic_bytes[first_slash + 1..])
        .map(|pos| first_slash + 1 + pos)
        .ok_or_else(|| MqttError::InvalidCommand("Topic missing second slash".to_string()))?;

    if &topic_bytes[first_slash + 1..second_slash] != b"command" {
        return Err(MqttError::InvalidCommand(
            "Topic missing 'opensleep/command'".to_string(),
        ));
    }

    if memchr::memchr(b'/', &topic_bytes[second_slash + 1..]).is_some() {
        return Err(MqttError::InvalidCommand(
            "Topic contains extra slash".to_string(),
        ));
    }

    Ok(&topic[second_slash + 1..])
}

fn update_profile_field<F>(
    payload: &str,
    config_tx: &watch::Sender<Config>,
    config_rx: &watch::Receiver<Config>,
    field_updater: F,
) -> Result<(), MqttError>
where
    F: FnOnce(&str, &mut Profile) -> Result<(), MqttError>,
{
    let (side, value_str) = parse_side_prefix(payload.trim());

    let mut cfg = config_rx.borrow().clone();
    let profile = cfg
        .profile
        .get_profile_mut(side)
        .ok_or(MqttError::ProfileSide)?;

    field_updater(value_str, profile)?;
    config_tx.send(cfg)?;
    Ok(())
}

fn handle_set_time(
    payload: &str,
    config_tx: &watch::Sender<Config>,
    config_rx: &watch::Receiver<Config>,
    field: TimeField,
) -> Result<(), MqttError> {
    update_profile_field(payload, config_tx, config_rx, |time_str, profile| {
        let time = parse_time(time_str)?;
        match field {
            TimeField::Sleep => profile.sleep = time,
            TimeField::Wake => profile.wake = time,
        }
        Ok(())
    })
}

fn update_config<F>(
    tx: &watch::Sender<Config>,
    rx: &watch::Receiver<Config>,
    updater: F,
) -> Result<(), MqttError>
where
    F: FnOnce(&mut Config),
{
    let mut cfg = rx.borrow().clone();
    updater(&mut cfg);
    tx.send(cfg)?;
    Ok(())
}

fn parse_or_invalid<T: std::str::FromStr>(s: &str, field_name: &str) -> Result<T, MqttError> {
    s.trim()
        .parse::<T>()
        .map_err(|_| MqttError::InvalidCommand(format!("Invalid {field_name}")))
}

fn parse_time(time_str: &str) -> Result<Time, MqttError> {
    Time::strptime("%H:%M", time_str).map_err(|e| MqttError::InvalidTime(e.to_string()))
}

fn parse_side_prefix(payload: &str) -> (Option<&str>, &str) {
    if let Some(colon_pos) = payload.find(':') {
        let prefix = &payload[..colon_pos];
        if prefix == "left" || prefix == "right" {
            return (Some(prefix), &payload[colon_pos + 1..]);
        }
    }
    (None, payload)
}

fn parse_vibration_params(s: &str) -> Result<(VibrationPattern, u8, u32, u32), MqttError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(MqttError::InvalidCommand(
            "Expected format: pattern,intensity,duration,offset".to_string(),
        ));
    }

    Ok((
        parts[0]
            .trim()
            .parse::<VibrationPattern>()
            .map_err(|e| MqttError::InvalidCommand(format!("Invalid vibration pattern: {e}")))?,
        parse_or_invalid::<u8>(parts[1], "intensity")?,
        parse_or_invalid::<u32>(parts[2], "duration")?,
        parse_or_invalid::<u32>(parts[3], "offset")?,
    ))
}

fn parse_heat_params(s: &str) -> Result<(u8, u32), MqttError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(MqttError::InvalidCommand(
            "Expected format: temp,offset".to_string(),
        ));
    }

    Ok((
        parse_or_invalid::<u8>(parts[0], "temperature")?,
        parse_or_invalid::<u32>(parts[1], "offset")?,
    ))
}

fn parse_led_params(s: &str) -> Result<(LedPattern, LedPattern), MqttError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(MqttError::InvalidCommand(
            "Expected format: idle:pattern,active:pattern".to_string(),
        ));
    }

    Ok((
        parts[0]
            .trim_start_matches("idle:")
            .parse::<LedPattern>()
            .map_err(|e| MqttError::InvalidCommand(format!("Invalid LED pattern: {e}")))?,
        parts[1]
            .trim_start_matches("active:")
            .parse::<LedPattern>()
            .map_err(|e| MqttError::InvalidCommand(format!("Invalid LED pattern: {e}")))?,
    ))
}
