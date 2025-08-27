use crate::config::{Config, PresenceConfig};
use crate::sensor::packet::CapacitanceData;
use crate::sensor::state::SensorUpdate;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, watch};

const DEFAULT_THRESHOLD: u16 = 50;
const DEFAULT_DEBOUNCE: u8 = 5;
const CALIBRATION_DURATION: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PresenceState {
    pub in_bed: bool,
    pub on_left: bool,
    pub on_right: bool,
}

pub struct PresenseManager {
    config_tx: watch::Sender<Config>,
    config_rx: watch::Receiver<Config>,
    config: Option<PresenceConfig>,
    presence_tx: mpsc::Sender<PresenceState>,
    calibration_end: Option<Instant>,
    calibration_samples: Vec<[u16; 6]>,
    debounce: [u8; 6],
    last_state: PresenceState,
}

impl PresenseManager {
    pub fn new(
        config_tx: watch::Sender<Config>,
        config_rx: watch::Receiver<Config>,
        presence_tx: mpsc::Sender<PresenceState>,
    ) -> Self {
        PresenseManager {
            config: {
                let b = config_rx.borrow();
                if b.presence.is_none() {
                    log::warn!(
                        "No presence config found. Please calibrate using 'opensleep/command/calibrate' endpoint."
                    );
                }
                b.presence.as_ref().cloned()
            },
            config_tx,
            config_rx,
            presence_tx,
            calibration_end: None,
            calibration_samples: Vec::new(),
            debounce: [0u8; 6],
            last_state: PresenceState::default(),
        }
    }

    pub fn update(&mut self, data: &CapacitanceData) {
        if self.config.is_some() {
            self.update_presence(data);
        }

        if self.calibration_end.is_some() {
            self.update_calibration(data);
        }
    }

    fn update_presence(&mut self, data: &CapacitanceData) {
        let config = self.config.as_mut().unwrap();

        for i in 0..6 {
            if data.values[i] > config.baselines[i] + config.threshold {
                self.debounce[i] = self.debounce[i].saturating_add(1);
            } else {
                self.debounce[i] = 0;
            }
        }

        let left_present = self.debounce[0..3]
            .iter()
            .any(|&c| c >= config.debounce_count);
        let right_present = self.debounce[3..6]
            .iter()
            .any(|&c| c >= config.debounce_count);

        let state = PresenceState {
            in_bed: left_present || right_present,
            on_left: left_present,
            on_right: right_present,
        };

        if state != self.last_state {
            self.last_state = state.clone();
            if let Err(e) = self.presence_tx.try_send(state) {
                log::error!("Failed to send presence state: {e}");
            }
        }
    }

    pub fn start_calibration(&mut self) {
        log::info!("Running calibration for {}", CALIBRATION_DURATION.as_secs());
        self.calibration_end = Some(Instant::now() + CALIBRATION_DURATION);
        self.calibration_samples = vec![];
    }

    fn update_calibration(&mut self, data: &CapacitanceData) {
        self.calibration_samples.push(data.values);

        if Instant::now() > self.calibration_end.unwrap() {
            self.calibration_end = None;

            if self.calibration_samples.is_empty() {
                log::error!("Calibration failed, no samples collected.");
                return;
            }

            log::info!("Calibration finished. Updating config..");

            let baselines = Self::calculate_baselines(&self.calibration_samples);
            let new_cfg = PresenceConfig {
                baselines,
                threshold: DEFAULT_THRESHOLD,
                debounce_count: DEFAULT_DEBOUNCE,
            };

            // reset
            self.calibration_samples = vec![];
            self.calibration_end = None;

            // update our config
            self.config = Some(new_cfg.clone());

            // update config file
            let mut config = self.config_rx.borrow_and_update().clone();
            config.presence = Some(new_cfg.clone());
            if let Err(e) = self.config_tx.send(config) {
                log::error!("Failed to update config: {e}");
            } else {
                log::info!("Config updated: {baselines:?}");
            }
        }
    }

    fn calculate_baselines(samples: &[[u16; 6]]) -> [u16; 6] {
        let mut sums = [0u32; 6];
        for sample in samples {
            for (sum, &value) in sums.iter_mut().zip(sample) {
                *sum += value as u32;
            }
        }
        let count = samples.len() as u32;
        sums.map(|sum| (sum / count) as u16)
    }
}
