use crate::common::serial::DeviceMode;
use crate::config::{Config, ProfileType};
use crate::frozen::state::FrozenUpdate;
use crate::mqtt::MqttError;
use crate::presence::PresenceState;
use crate::sensor::state::SensorUpdate;
use rumqttc::{AsyncClient, QoS};
use std::fmt::Display;

#[async_trait::async_trait]
trait MqttPublish {
    async fn publish(&self, client: &AsyncClient, topic: &str, qos: QoS) -> Result<(), MqttError>;
}

#[async_trait::async_trait]
impl<T: Display + Send + Sync> MqttPublish for T {
    async fn publish(&self, client: &AsyncClient, topic: &str, qos: QoS) -> Result<(), MqttError> {
        client
            .publish(topic, qos, false, self.to_string())
            .await
            .map_err(|e| e.into())
    }
}

async fn publish_array_csv<T: Display>(
    client: &AsyncClient,
    topic: &str,
    array: &[T],
    qos: QoS,
) -> Result<(), MqttError> {
    let csv = array
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(",");
    csv.publish(client, topic, qos).await
}

pub struct StatePublisher {
    client: AsyncClient,
}

impl StatePublisher {
    pub fn new(client: AsyncClient) -> Self {
        Self { client }
    }

    pub async fn publish_frozen_update(&self, update: FrozenUpdate) -> Result<(), MqttError> {
        let base = "opensleep/subsystems/frozen";

        match update {
            FrozenUpdate::DeviceMode(mode) => {
                mode.to_string()
                    .publish(
                        &self.client,
                        &format!("{base}/device_mode"),
                        QoS::AtLeastOnce,
                    )
                    .await?;
            }
            FrozenUpdate::HardwareInfo(hw_info) => {
                let json = serde_json::to_string(&hw_info)?;
                json.publish(
                    &self.client,
                    &format!("{base}/hardware_info"),
                    QoS::AtLeastOnce,
                )
                .await?;
            }
            FrozenUpdate::Temperature(temp) => {
                temp.left_temp
                    .publish(&self.client, &format!("{base}/temp/left"), QoS::AtMostOnce)
                    .await?;
                temp.right_temp
                    .publish(&self.client, &format!("{base}/temp/right"), QoS::AtMostOnce)
                    .await?;
                temp.heatsink_temp
                    .publish(
                        &self.client,
                        &format!("{base}/temp/heatsink"),
                        QoS::AtMostOnce,
                    )
                    .await?;
                temp.error
                    .publish(&self.client, &format!("{base}/temp/state"), QoS::AtMostOnce)
                    .await?;
            }
            FrozenUpdate::LeftTarget(target) => {
                target
                    .state
                    .publish(
                        &self.client,
                        &format!("{base}/target/left/enabled"),
                        QoS::AtMostOnce,
                    )
                    .await?;
                target
                    .temp
                    .publish(
                        &self.client,
                        &format!("{base}/target/left/temp"),
                        QoS::AtMostOnce,
                    )
                    .await?;
            }
            FrozenUpdate::RightTarget(target) => {
                target
                    .state
                    .publish(
                        &self.client,
                        &format!("{base}/target/right/enabled"),
                        QoS::AtMostOnce,
                    )
                    .await?;
                target
                    .temp
                    .publish(
                        &self.client,
                        &format!("{base}/target/right/temp"),
                        QoS::AtMostOnce,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn publish_sensor_update(&self, update: SensorUpdate) -> Result<(), MqttError> {
        let base = "opensleep/subsystems/sensor";

        match update {
            SensorUpdate::DeviceMode(mode) => {
                let mode_str = match mode {
                    DeviceMode::Unknown => "unknown",
                    DeviceMode::Bootloader => "bootloader",
                    DeviceMode::Firmware => "firmware",
                };
                mode_str
                    .publish(
                        &self.client,
                        &format!("{base}/device_mode"),
                        QoS::AtLeastOnce,
                    )
                    .await?;
            }
            SensorUpdate::HardwareInfo(hw_info) => {
                let json = serde_json::to_string(&hw_info)?;
                json.publish(
                    &self.client,
                    &format!("{base}/hardware_info"),
                    QoS::AtLeastOnce,
                )
                .await?;
            }
            SensorUpdate::VibrationEnabled(enabled) => {
                enabled
                    .publish(
                        &self.client,
                        &format!("{base}/vibration_enabled"),
                        QoS::AtMostOnce,
                    )
                    .await?;
            }
            SensorUpdate::Capacitance(cap) => {
                publish_array_csv(
                    &self.client,
                    &format!("{base}/capacitance"),
                    &cap.values,
                    QoS::AtMostOnce,
                )
                .await?;
            }
            SensorUpdate::Temperature(temp) => {
                publish_array_csv(
                    &self.client,
                    &format!("{base}/temperature/bed"),
                    &temp.bed,
                    QoS::AtMostOnce,
                )
                .await?;
                temp.ambient
                    .publish(
                        &self.client,
                        &format!("{base}/temperature/ambient"),
                        QoS::AtMostOnce,
                    )
                    .await?;
                temp.humidity
                    .publish(
                        &self.client,
                        &format!("{base}/temperature/humidity"),
                        QoS::AtMostOnce,
                    )
                    .await?;
                temp.mcu
                    .publish(
                        &self.client,
                        &format!("{base}/temperature/mcu"),
                        QoS::AtMostOnce,
                    )
                    .await?;
            }
            SensorUpdate::PiezoGain(left, right) => {
                left.publish(
                    &self.client,
                    &format!("{base}/piezo/gain/left"),
                    QoS::AtMostOnce,
                )
                .await?;
                right
                    .publish(
                        &self.client,
                        &format!("{base}/piezo/gain/right"),
                        QoS::AtMostOnce,
                    )
                    .await?;
            }
            SensorUpdate::PiezoFreq(freq) => {
                freq.publish(&self.client, &format!("{base}/piezo/freq"), QoS::AtMostOnce)
                    .await?;
            }
            SensorUpdate::PiezoEnabled(enabled) => {
                enabled
                    .publish(
                        &self.client,
                        &format!("{base}/piezo/sampling"),
                        QoS::AtMostOnce,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn publish_config(&self, config: Config) -> Result<(), MqttError> {
        let base = "opensleep/config";

        config
            .timezone
            .iana_name()
            .unwrap_or("UTC")
            .publish(&self.client, &format!("{base}/timezone"), QoS::AtLeastOnce)
            .await?;

        config
            .away_mode
            .publish(&self.client, &format!("{base}/away_mode"), QoS::AtLeastOnce)
            .await?;

        config
            .prime
            .strftime("%H:%M")
            .to_string()
            .publish(
                &self.client,
                &format!("{base}/prime_time"),
                QoS::AtLeastOnce,
            )
            .await?;

        config
            .led
            .idle
            .to_string()
            .publish(&self.client, &format!("{base}/led/idle"), QoS::AtLeastOnce)
            .await?;
        config
            .led
            .active
            .to_string()
            .publish(
                &self.client,
                &format!("{base}/led/active"),
                QoS::AtLeastOnce,
            )
            .await?;

        config
            .mqtt
            .server
            .publish(
                &self.client,
                &format!("{base}/mqtt/server"),
                QoS::AtLeastOnce,
            )
            .await?;
        config
            .mqtt
            .port
            .publish(&self.client, &format!("{base}/mqtt/port"), QoS::AtLeastOnce)
            .await?;
        config
            .mqtt
            .user
            .publish(&self.client, &format!("{base}/mqtt/user"), QoS::AtLeastOnce)
            .await?;

        match &config.profile {
            ProfileType::Solo(profile) => {
                "solo"
                    .publish(
                        &self.client,
                        &format!("{base}/profile/type"),
                        QoS::AtLeastOnce,
                    )
                    .await?;
                self.publish_profile(profile, &format!("{base}/profile/solo"))
                    .await?;
            }
            ProfileType::Couples { left, right } => {
                "couples"
                    .publish(
                        &self.client,
                        &format!("{base}/profile/type"),
                        QoS::AtLeastOnce,
                    )
                    .await?;
                self.publish_profile(left, &format!("{base}/profile/left"))
                    .await?;
                self.publish_profile(right, &format!("{base}/profile/right"))
                    .await?;
            }
        }

        if let Some(ref presence) = config.presence {
            publish_array_csv(
                &self.client,
                &format!("{base}/presence/baselines"),
                &presence.baselines,
                QoS::AtLeastOnce,
            )
            .await?;
            presence
                .threshold
                .publish(
                    &self.client,
                    &format!("{base}/presence/threshold"),
                    QoS::AtLeastOnce,
                )
                .await?;
            presence
                .debounce_count
                .publish(
                    &self.client,
                    &format!("{base}/presence/debounce_count"),
                    QoS::AtLeastOnce,
                )
                .await?;
        }

        Ok(())
    }

    async fn publish_profile(
        &self,
        profile: &crate::config::Profile,
        base: &str,
    ) -> Result<(), MqttError> {
        publish_array_csv(
            &self.client,
            &format!("{base}/temp_profile"),
            &profile.temp_profile,
            QoS::AtLeastOnce,
        )
        .await?;

        profile
            .sleep
            .strftime("%H:%M")
            .to_string()
            .publish(&self.client, &format!("{base}/sleep"), QoS::AtLeastOnce)
            .await?;

        profile
            .wake
            .strftime("%H:%M")
            .to_string()
            .publish(&self.client, &format!("{base}/wake"), QoS::AtLeastOnce)
            .await?;

        serde_json::to_string(&profile.vibration)?
            .publish(&self.client, &format!("{base}/vibration"), QoS::AtLeastOnce)
            .await?;

        serde_json::to_string(&profile.heat)?
            .publish(&self.client, &format!("{base}/heat"), QoS::AtLeastOnce)
            .await?;

        Ok(())
    }

    pub async fn publish_presence(&self, state: PresenceState) -> Result<(), MqttError> {
        let base = "opensleep/presence";

        state
            .in_bed
            .publish(&self.client, &format!("{base}/in_bed"), QoS::AtMostOnce)
            .await?;

        state
            .on_left
            .publish(&self.client, &format!("{base}/on_left"), QoS::AtMostOnce)
            .await?;

        state
            .on_right
            .publish(&self.client, &format!("{base}/on_right"), QoS::AtMostOnce)
            .await?;

        Ok(())
    }

    pub async fn publish_reset_values(&self) -> Result<(), MqttError> {
        false
            .publish(&self.client, "opensleep/presence/in_bed", QoS::AtMostOnce)
            .await?;
        false
            .publish(&self.client, "opensleep/presence/on_left", QoS::AtMostOnce)
            .await?;
        false
            .publish(&self.client, "opensleep/presence/on_right", QoS::AtMostOnce)
            .await?;

        "unknown"
            .publish(
                &self.client,
                "opensleep/subsystems/sensor/device_mode",
                QoS::AtLeastOnce,
            )
            .await?;
        false
            .publish(
                &self.client,
                "opensleep/subsystems/sensor/vibration_enabled",
                QoS::AtMostOnce,
            )
            .await?;
        false
            .publish(
                &self.client,
                "opensleep/subsystems/sensor/piezo/sampling",
                QoS::AtMostOnce,
            )
            .await?;

        "unknown"
            .publish(
                &self.client,
                "opensleep/subsystems/frozen/device_mode",
                QoS::AtLeastOnce,
            )
            .await?;

        log::info!("Published reset values to MQTT");
        Ok(())
    }
}
