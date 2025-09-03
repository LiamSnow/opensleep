use crate::config::{self, Config};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, Publish, QoS};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

pub struct MqttManager {
    config_tx: watch::Sender<Config>,
    config_rx: watch::Receiver<Config>,
    calibrate_tx: mpsc::Sender<()>,
    pub client: AsyncClient,
    eventloop: EventLoop,
}

impl MqttManager {
    pub fn new(
        config_tx: watch::Sender<Config>,
        config_rx: watch::Receiver<Config>,
        calibrate_tx: mpsc::Sender<()>,
    ) -> Self {
        log::info!("Initializing MQTT...");

        let cfg = config_rx.borrow().mqtt.clone();

        log::info!(
            "Connecting to MQTT broker at {}:{} as user '{}'",
            cfg.server,
            cfg.port,
            cfg.user
        );

        let mut opts = MqttOptions::new("opensleep", &cfg.server, cfg.port);
        opts.set_keep_alive(Duration::from_secs(60));
        opts.set_credentials(&cfg.user, &cfg.password);

        let (client, eventloop) = AsyncClient::new(opts, 10);

        Self {
            config_tx,
            config_rx,
            calibrate_tx,
            client,
            eventloop,
        }
    }

    pub async fn run(mut self) {
        loop {
            match self.eventloop.poll().await {
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    log::info!("MQTT broker connected");
                    self.subscribe("opensleep/config/+").await;
                    self.subscribe("opensleep/calibrate").await;
                }
                Ok(Event::Incoming(Packet::Disconnect)) => {
                    log::warn!("MQTT broker disconnected");
                }
                Ok(Event::Incoming(Packet::Publish(publ))) => {
                    self.handle_publ(publ);
                }
                Ok(_) => {}
                Err(e) => {
                    log::error!("MQTT event loop error: {e}");
                }
            }
        }
    }

    fn handle_publ(&mut self, publ: Publish) {
        if publ.topic == "opensleep/calibrate" {
            if let Err(e) = self.calibrate_tx.try_send(()) {
                log::error!("Failed to send to calibrate channel: {e}");
            }
        } else if publ.topic.starts_with("opensleep/config") {
            let topic = publ.topic.clone();
            let payload = String::from_utf8_lossy(&publ.payload);
            if let Err(e) = config::mqtt::handle_publish(
                &topic,
                payload.clone(),
                &mut self.config_tx,
                &mut self.config_rx,
            ) {
                log::error!(
                    "Error updating config on topic `{topic}` with payload `{payload}`: {e}"
                );
            }
        } else {
            log::warn!("Publish to unknown topic: {}", publ.topic);
        }
    }

    async fn subscribe(&mut self, topic: &'static str) {
        log::debug!("Subscribing to {topic}");
        match self.client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(_) => {
                log::debug!("Subscribed to {topic}");
            }
            Err(e) => {
                log::error!("Failed to subscribe to {topic}: {e}");
            }
        }
    }
}

pub fn publish<S, V>(client: &mut AsyncClient, topic: S, qos: QoS, retain: bool, payload: V)
where
    S: Into<String>,
    V: Into<Vec<u8>>,
{
    if let Err(e) = client.try_publish(topic, qos, retain, payload) {
        log::error!("Error publishing: {e}",);
    }
}

pub fn publish_guaranteed<S, V>(client: &mut AsyncClient, topic: S, retain: bool, payload: V)
where
    S: Into<String>,
    V: Into<Vec<u8>>,
{
    publish(client, topic, QoS::ExactlyOnce, retain, payload);
}

pub fn publish_high_freq<S, V>(client: &mut AsyncClient, topic: S, payload: V)
where
    S: Into<String>,
    V: Into<Vec<u8>>,
{
    publish(client, topic, QoS::AtMostOnce, false, payload);
}
