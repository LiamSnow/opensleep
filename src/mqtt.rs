use crate::{
    NAME, VERSION,
    config::{
        self, Config,
        mqtt::{TOPIC_SET_AWAY_MODE, TOPIC_SET_PRESENCE, TOPIC_SET_PRIME, TOPIC_SET_PROFILE},
    },
    sensor::presence::TOPIC_CALIBRATE,
};
use rumqttc::{AsyncClient, ConnectionError, Event, EventLoop, MqttOptions, Packet, Publish, QoS};
use std::{fmt::Display, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    time::timeout,
};

const TOPIC_AVAILABILITY: &str = "opensleep/availability";

const TOPIC_DEVICE_NAME: &str = "opensleep/device/name";
const TOPIC_DEVICE_VERSION: &str = "opensleep/device/version";
const TOPIC_DEVICE_LABEL: &str = "opensleep/device/label";

const TOPIC_RESULT_ACTION: &str = "opensleep/result/action";
const TOPIC_RESULT_STATUS: &str = "opensleep/result/status";
const TOPIC_RESULT_MSG: &str = "opensleep/result/message";

const SUCCESS: &str = "success";
const ERROR: &str = "error";

pub struct MqttManager {
    config_tx: watch::Sender<Config>,
    config_rx: watch::Receiver<Config>,
    calibrate_tx: mpsc::Sender<()>,
    pub client: AsyncClient,
    eventloop: EventLoop,
    device_label: String,
}

impl MqttManager {
    pub fn new(
        config_tx: watch::Sender<Config>,
        config_rx: watch::Receiver<Config>,
        calibrate_tx: mpsc::Sender<()>,
        device_label: String,
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
            device_label,
        }
    }

    pub async fn wait_for_conn(&mut self) {
        loop {
            let evt = self.eventloop.poll().await;
            if self.handle_event(evt).await {
                return;
            }
        }
    }

    async fn subscribe_actions(&mut self) {
        subscribe(&mut self.client, TOPIC_CALIBRATE).await;
        subscribe(&mut self.client, TOPIC_SET_AWAY_MODE).await;
        subscribe(&mut self.client, TOPIC_SET_PRIME).await;
        subscribe(&mut self.client, TOPIC_SET_PROFILE).await;
        subscribe(&mut self.client, TOPIC_SET_PRESENCE).await;
    }

    /// this must be in its own task because publishing
    /// topics requires someone polling the event loop
    async fn spawn_publish_task(&mut self) {
        let config = {
            let c = self.config_rx.borrow();
            c.clone()
        };
        let mut client = self.client.clone();
        let device_label = self.device_label.clone();
        tokio::spawn(async move {
            config.publish(&mut client).await;
            publish_guaranteed_wait(&mut client, TOPIC_AVAILABILITY, false, "online").await;
            publish_guaranteed_wait(&mut client, TOPIC_DEVICE_NAME, false, NAME).await;
            publish_guaranteed_wait(&mut client, TOPIC_DEVICE_VERSION, false, VERSION).await;
            publish_guaranteed_wait(&mut client, TOPIC_DEVICE_LABEL, false, device_label).await;
        });
    }

    /// returns true if connected
    async fn handle_event(&mut self, msg: Result<Event, ConnectionError>) -> bool {
        match msg {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                log::info!("MQTT broker connected");
                self.subscribe_actions().await;
                self.spawn_publish_task().await;
                return true;
            }
            Ok(Event::Incoming(Packet::Disconnect)) => {
                log::warn!("MQTT broker disconnected");
            }
            Ok(Event::Incoming(Packet::Publish(publ))) => {
                self.handle_action(publ).await;
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("MQTT event loop error: {e}");
            }
        }
        false
    }

    pub async fn run(mut self) {
        loop {
            let evt = self.eventloop.poll().await;
            self.handle_event(evt).await;
        }
    }

    async fn handle_action(&mut self, publ: Publish) {
        if publ.topic == TOPIC_CALIBRATE {
            if let Err(e) = self.calibrate_tx.try_send(()) {
                let msg = format!("Failed to send to calibrate channel: {e}");
                log::error!("{msg}");
                self.publish_result("calibrate", ERROR, msg).await;
            } else {
                self.publish_result("calibrate", SUCCESS, "started calibration".to_string())
                    .await;
            }
        } else if publ.topic.starts_with("opensleep/actions/set_") {
            self.handle_set_action(publ).await;
        } else {
            log::error!("Unkown action published: {}", publ.topic);
            self.publish_result("unknown", ERROR, format!("unknown action: {}", publ.topic))
                .await;
        }
    }

    async fn publish_result(&mut self, action: &str, status: &str, msg: String) {
        publish_guaranteed_wait(&mut self.client, TOPIC_RESULT_ACTION, false, action).await;
        publish_guaranteed_wait(&mut self.client, TOPIC_RESULT_STATUS, false, status).await;
        publish_guaranteed_wait(&mut self.client, TOPIC_RESULT_MSG, false, msg).await;
    }

    async fn handle_set_action(&mut self, publ: Publish) {
        let action = publ.topic.strip_prefix("opensleep/actions/").unwrap();
        let topic = publ.topic.clone();
        let payload = String::from_utf8_lossy(&publ.payload);

        let (status, msg) = match config::mqtt::handle_action(
            &mut self.client,
            &topic,
            payload.clone(),
            &mut self.config_tx,
            &mut self.config_rx,
        )
        .await
        {
            Ok(_) => (SUCCESS, "successfully edited configuration".to_string()),

            Err(e) => {
                log::error!("Error handling set action: {e}");
                (ERROR, e.to_string())
            }
        };

        self.publish_result(action, status, msg).await;
    }
}

async fn subscribe(client: &mut AsyncClient, topic: &'static str) {
    log::debug!("Subscribing to {topic}");
    match client.subscribe(topic, QoS::AtLeastOnce).await {
        Ok(_) => {
            log::debug!("Subscribed to {topic}");
        }
        Err(e) => {
            log::error!("Failed to subscribe to {topic}: {e}");
        }
    }
}

pub async fn publish_guaranteed_wait<S, V>(
    client: &mut AsyncClient,
    topic: S,
    retain: bool,
    payload: V,
) where
    S: Into<String> + Display + Clone,
    V: Into<Vec<u8>>,
{
    let fut = client.publish(topic.clone(), QoS::ExactlyOnce, retain, payload);

    match timeout(Duration::from_millis(100), fut).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            log::error!("Error publishing {topic}: {e}");
        }
        Err(_) => {
            log::error!("Timed out publishing {topic}");
        }
    }
}

pub fn publish_high_freq<S, V>(client: &mut AsyncClient, topic: S, payload: V)
where
    S: Into<String> + Display + Clone,
    V: Into<Vec<u8>>,
{
    if let Err(e) = client.try_publish(topic.clone(), QoS::AtMostOnce, false, payload) {
        log::error!("Error publishing to {topic}: {e}",);
    }
}
