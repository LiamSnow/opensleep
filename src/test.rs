#[cfg(test)]
mod tests {
    use jiff::{civil::time, tz::TimeZone};

    use crate::settings::{
        BySideSettings, HeatAlarm, Settings, SideSettings, VibrationAlarm, VibrationPattern,
    };

    #[test]
    fn test_deserialize_settings_both() {
        let a = Settings::from_str(
            r#"
            {
                "timezone": "America/New_York",
                "prime": "15:00",
                "led_brightness": 100,
                "both": {
                    "temp_profile": [-10, 10, 20],
                    "sleep": "22:00",
                    "wake": "10:30",
                    "vibration": {
                        "pattern": "rise",
                        "intensity": 80,
                        "duration": 600,
                        "offset": 300
                    },
                    "heat": {
                        "temp": 100,
                        "offset": 1800
                    }
                }
            }
            "#,
        )
        .unwrap();

        let b = Settings {
            timezone: TimeZone::get("America/New_York").unwrap(),
            away_mode: false,
            prime: Some(time(15, 0, 0, 0)),
            led_brightness: Some(100),
            by_side: BySideSettings::Solo {
                both: SideSettings {
                    temp_profile: vec![-10, 10, 20],
                    sleep: time(22, 0, 0, 0),
                    wake: time(10, 30, 0, 0),
                    vibration: Some(VibrationAlarm {
                        pattern: VibrationPattern::Rise,
                        intensity: 80,
                        duration: 600,
                        offset: 300,
                    }),
                    heat: Some(HeatAlarm {
                        temp: 100,
                        offset: 1800,
                    }),
                },
            },
        };

        assert_eq!(a, b);
    }

    #[test]
    fn test_deserialize_settings() {
        let a = Settings::from_str(
            r#"
            {
                "timezone": "America/New_York",
                "prime": "15:00",
                "led_brightness": 100,
                "left": {
                    "temp_profile": [-10, 10, 20],
                    "sleep": "22:00",
                    "wake": "10:30",
                    "vibration": {
                        "pattern": "rise",
                        "intensity": 80,
                        "duration": 600,
                        "offset": 300
                    },
                    "heat": {
                        "temp": 100,
                        "offset": 1800
                    }
                },
                "right": {
                    "temp_profile": [-10, 10, 20],
                    "sleep": "22:00",
                    "wake": "10:30",
                    "vibration": {
                        "pattern": "rise",
                        "intensity": 80,
                        "duration": 600,
                        "offset": 300
                    },
                    "heat": {
                        "temp": 100,
                        "offset": 1800
                    }
                }
            }
            "#,
        )
        .unwrap();

        let s = SideSettings {
            temp_profile: vec![-10, 10, 20],
            sleep: time(22, 0, 0, 0),
            wake: time(10, 30, 0, 0),
            vibration: Some(VibrationAlarm {
                pattern: VibrationPattern::Rise,
                intensity: 80,
                duration: 600,
                offset: 300,
            }),
            heat: Some(HeatAlarm {
                temp: 100,
                offset: 1800,
            }),
        };

        let b = Settings {
            timezone: TimeZone::get("America/New_York").unwrap(),
            away_mode: false,
            prime: Some(time(15, 0, 0, 0)),
            led_brightness: Some(100),
            by_side: BySideSettings::Couples {
                left: s.clone(),
                right: s,
            },
        };

        assert_eq!(a, b);
    }
}
