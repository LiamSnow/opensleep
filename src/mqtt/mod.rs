mod command;
mod model;
mod publisher;

pub use model::MqttError;
use publisher::StatePublisher;

use crate::frozen::state::FrozenUpdate;
use crate::sensor::presence::PresenceState;
use crate::{config::Config, sensor::state::SensorUpdate};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::time::Duration;
use tokio::{
    sync::{mpsc, watch},
    time::sleep,
};

pub fn spawn(
    config_tx: watch::Sender<Config>,
    config_rx: watch::Receiver<Config>,
    sensor_update_rx: mpsc::Receiver<SensorUpdate>,
    frozen_update_rx: mpsc::Receiver<FrozenUpdate>,
    presense_state_rx: mpsc::Receiver<PresenceState>,
    calibrate_tx: mpsc::Sender<()>,
) {
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

    let (mut client, mut eventloop) = AsyncClient::new(opts, 10);

    tokio::spawn(async move {
        wait_for_connection(&mut eventloop).await.unwrap();
        subscribe_commands(&mut client).await.unwrap();

        tokio::spawn(eventloop_task(
            eventloop,
            config_rx.clone(),
            config_tx,
            calibrate_tx,
        ));

        tokio::spawn(publish_task(
            client,
            config_rx,
            sensor_update_rx,
            frozen_update_rx,
            presense_state_rx,
        ));
    });
}

async fn publish_task(
    client: AsyncClient,
    mut config_rx: watch::Receiver<Config>,
    mut sensor_update_rx: mpsc::Receiver<SensorUpdate>,
    mut frozen_update_rx: mpsc::Receiver<FrozenUpdate>,
    mut presense_state_rx: mpsc::Receiver<PresenceState>,
) {
    log::info!("Starting MQTT publishing task");

    let publisher = StatePublisher::new(client.clone());

    // post config
    let cfg = config_rx.borrow().clone();
    if let Err(e) = publisher.publish_config(cfg).await {
        log::error!("Error publishing initial config: {e}");
    }

    // reset values
    if let Err(e) = publisher.publish_reset_values().await {
        log::error!("Error publishing reset values: {e}");
    }
    if let Err(e) = publisher.publish_presence(PresenceState::default()).await {
        log::error!("Error publishing initial presence state: {e}");
    } else {
        log::info!("Published initial presence state to MQTT");
    }

    loop {
        tokio::select! {
            Ok(()) = config_rx.changed() => {
                let config = config_rx.borrow().clone();
                if let Err(e) = publisher.publish_config(config).await {
                    log::error!("Error publishing config: {e}");
                }
            }
            Some(sensor_update) = sensor_update_rx.recv() => {
                if let Err(e) = publisher.publish_sensor_update(sensor_update).await {
                    log::error!("Error publishing sensor update: {e}");
                }
            }
            Some(frozen_update) = frozen_update_rx.recv() => {
                if let Err(e) = publisher.publish_frozen_update(frozen_update).await {
                    log::error!("Error publishing frozen update: {e}");
                }
            }
            Some(presence_state) = presense_state_rx.recv() => {
                if let Err(e) = publisher.publish_presence(presence_state).await {
                    log::error!("Error publishing presence state: {e}");
                }
            }
        }
    }
}

async fn wait_for_connection(eventloop: &mut EventLoop) -> Result<(), String> {
    for _ in 0..10 {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                log::info!("Successfully connected to MQTT broker");
                return Ok(());
            }
            Ok(_) => continue,
            Err(e) => {
                log::error!("Failed to connect to MQTT broker: {e}");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    log::error!("Failed to connect to MQTT broker");
    Err("Failed to connect to MQTT broker".into())
}

async fn subscribe_commands(client: &mut AsyncClient) -> Result<(), String> {
    log::debug!("Subscribing to command topics...");
    if let Err(e) = client
        .subscribe("opensleep/command/+", QoS::AtLeastOnce)
        .await
    {
        log::error!("Failed to subscribe to command topics: {e}");
        return Err(format!("Failed to subscribe to command topics: {e}"));
    }
    log::debug!("Subscribed to command topics");
    Ok(())
}

async fn eventloop_task(
    mut eventloop: EventLoop,
    config_rx: watch::Receiver<Config>,
    config_tx: watch::Sender<Config>,
    mut calibrate_tx: mpsc::Sender<()>,
) {
    log::info!("Starting MQTT event loop task");

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                log::info!("MQTT reconnected");
            }
            Ok(Event::Incoming(Packet::Disconnect)) => {
                log::warn!("MQTT broker disconnected");
            }
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                if let Err(e) = command::handle_command(
                    publish.topic,
                    publish.payload,
                    &config_tx,
                    &config_rx,
                    &mut calibrate_tx,
                )
                .await
                {
                    log::error!("Error handling command: {e}");
                }
            }
            Ok(_) => {}
            Err(e) => {
                log::error!("MQTT event loop error: {e}");
                // Try to recover from connection errors
                match &e {
                    rumqttc::ConnectionError::Io(_)
                    | rumqttc::ConnectionError::ConnectionRefused(_) => {
                        log::info!("Attempting to reconnect to MQTT broker...");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    _ => {
                        log::error!("Unrecoverable MQTT error: {e}");
                        break;
                    }
                }
            }
        }
    }

    log::error!("MQTT event loop task exiting");
}
