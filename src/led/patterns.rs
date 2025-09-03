use super::model::*;
use serde::{Deserialize, Serialize};

// plz make a PR if you find some more cool LED patterns!!
// certainly a lot more room for expansion here

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LedPattern {
    SlowBreath(u8, u8, u8),
    FastBreath(u8, u8, u8),

    Off,

    Fixed(u8, u8, u8),

    FastRainbowBreath,
    SlowRainbowBreath,
    FreakyRainbow,

    CustomBasic(u8, u8, u8, Timing),
    CustomRainbow(Timing),

    // these are mostly troll
    FastPulse(u8, u8, u8),
    Pulse(u8, u8, u8),
    SlowPulse(u8, u8, u8),
}

impl LedPattern {
    pub fn get_config(&self, band: CurrentBand) -> IS31FL3194Config {
        match self {
            LedPattern::Off => IS31FL3194Config {
                enabled: false,
                mode: OperatingMode::CurrentLevel(0, 0, 0),
                band,
            },

            LedPattern::CustomBasic(r, g, b, timing) => {
                make_basic(*r, *g, *b, timing.clone(), band)
            }
            LedPattern::CustomRainbow(timing) => make_rainbow(timing.clone(), band),

            LedPattern::SlowPulse(r, g, b) => make_basic(
                *r,
                *g,
                *b,
                Timing {
                    start: 0,
                    rise: 0,
                    hold: 0,
                    fall: 0,
                    between_pulses: 6,
                    off: 0,
                },
                band,
            ),

            LedPattern::Pulse(r, g, b) => make_basic(
                *r,
                *g,
                *b,
                Timing {
                    start: 0,
                    rise: 0,
                    hold: 0,
                    fall: 0,
                    between_pulses: 2,
                    off: 0,
                },
                band,
            ),

            LedPattern::FastPulse(r, g, b) => make_basic(
                *r,
                *g,
                *b,
                Timing {
                    start: 0,
                    rise: 0,
                    hold: 0,
                    fall: 0,
                    between_pulses: 1,
                    off: 0,
                },
                band,
            ),

            LedPattern::SlowBreath(r, g, b) => make_basic(
                *r,
                *g,
                *b,
                Timing {
                    start: 6,
                    rise: 7,
                    hold: 6,
                    fall: 7,
                    between_pulses: 0,
                    off: 6,
                },
                band,
            ),

            LedPattern::FastBreath(r, g, b) => make_basic(
                *r,
                *g,
                *b,
                Timing {
                    start: 1,
                    rise: 3,
                    hold: 1,
                    fall: 3,
                    between_pulses: 0,
                    off: 2,
                },
                band,
            ),

            LedPattern::Fixed(r, g, b) => IS31FL3194Config {
                enabled: true,
                mode: OperatingMode::CurrentLevel(*r, *g, *b),
                band,
            },

            LedPattern::SlowRainbowBreath => make_rainbow(
                Timing {
                    start: 0,
                    rise: 7,
                    hold: 0,
                    fall: 7,
                    between_pulses: 0,
                    off: 0,
                },
                band,
            ),

            LedPattern::FastRainbowBreath => make_rainbow(
                Timing {
                    start: 0,
                    rise: 4,
                    hold: 0,
                    fall: 4,
                    between_pulses: 0,
                    off: 0,
                },
                band,
            ),

            LedPattern::FreakyRainbow => make_rainbow(
                Timing {
                    start: 1,
                    rise: 1,
                    hold: 0,
                    fall: 1,
                    between_pulses: 0,
                    off: 0,
                },
                band,
            ),
        }
    }
}

fn make_basic(r: u8, g: u8, b: u8, timing: Timing, band: CurrentBand) -> IS31FL3194Config {
    IS31FL3194Config {
        enabled: true,
        mode: OperatingMode::Pattern(
            Some(PatternConfig {
                timing,
                colors: [
                    ColorConfig {
                        enabled: true,
                        r,
                        g,
                        b,
                        repeat: ColorRepeat::Endless,
                    },
                    ColorConfig::default(),
                    ColorConfig::default(),
                ],
                next: PatternNext::Next,
                gamma: Gamma::Gamma2_4,
                multipulse_repeat: Repeat::Endless,
                pattern_repeat: Repeat::Endless,
            }),
            None,
            None,
        ),
        band,
    }
}

fn make_rainbow(timing: Timing, band: CurrentBand) -> IS31FL3194Config {
    IS31FL3194Config {
        enabled: true,
        mode: OperatingMode::Pattern(
            Some(PatternConfig {
                timing: timing.clone(),
                colors: [
                    ColorConfig {
                        enabled: true,
                        r: 255,
                        g: 0,
                        b: 0,
                        repeat: ColorRepeat::Once,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 255,
                        g: 128,
                        b: 0,
                        repeat: ColorRepeat::Once,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 255,
                        g: 255,
                        b: 0,
                        repeat: ColorRepeat::Once,
                    },
                ],
                next: PatternNext::Next,
                gamma: Gamma::Gamma2_4,
                multipulse_repeat: Repeat::Count(1),
                pattern_repeat: Repeat::Count(1),
            }),
            Some(PatternConfig {
                timing: timing.clone(),
                colors: [
                    ColorConfig {
                        enabled: true,
                        r: 128,
                        g: 255,
                        b: 0,
                        repeat: ColorRepeat::Once,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 0,
                        g: 255,
                        b: 0,
                        repeat: ColorRepeat::Once,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 0,
                        g: 255,
                        b: 128,
                        repeat: ColorRepeat::Once,
                    },
                ],
                next: PatternNext::Next,
                gamma: Gamma::Gamma2_4,
                multipulse_repeat: Repeat::Count(1),
                pattern_repeat: Repeat::Count(1),
            }),
            Some(PatternConfig {
                timing,
                colors: [
                    ColorConfig {
                        enabled: true,
                        r: 0,
                        g: 128,
                        b: 255,
                        repeat: ColorRepeat::Once,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 0,
                        g: 0,
                        b: 255,
                        repeat: ColorRepeat::Once,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 128,
                        g: 0,
                        b: 255,
                        repeat: ColorRepeat::Once,
                    },
                ],
                next: PatternNext::Next,
                gamma: Gamma::Gamma2_4,
                multipulse_repeat: Repeat::Count(1),
                pattern_repeat: Repeat::Count(1),
            }),
        ),
        band,
    }
}
