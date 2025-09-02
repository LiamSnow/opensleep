use super::model::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum LedPattern {
    SlowBreath(u8, u8, u8),
    FastBreath(u8, u8, u8),
    Fixed(u8, u8, u8),
    Rainbow,
}

impl LedPattern {
    pub fn get_config(&self) -> IS31FL3194Config {
        match self {
            LedPattern::SlowBreath(r, g, b) => IS31FL3194Config {
                enabled: true,
                mode: OperatingMode::Pattern(
                    Some(PatternConfig {
                        timing: Timing {
                            start: 6,
                            rise: 7,
                            hold: 6,
                            fall: 7,
                            between_pulses: 0,
                            off: 6,
                        },
                        colors: [
                            ColorConfig {
                                enabled: true,
                                r: *r,
                                g: *g,
                                b: *b,
                                repeat: ColorRepeat::Endless,
                            },
                            ColorConfig::default(),
                            ColorConfig::default(),
                        ],
                        next: PatternNext::Next,
                        gamma: Gamma::Gamma3_5,
                        multipulse_repeat: Repeat::Endless,
                        pattern_repeat: Repeat::Endless,
                    }),
                    None,
                    None,
                ),
                band: CurrentBand::Three,
            },

            LedPattern::FastBreath(r, g, b) => IS31FL3194Config {
                enabled: true,
                mode: OperatingMode::Pattern(
                    Some(PatternConfig {
                        timing: Timing {
                            start: 2,
                            rise: 3,
                            hold: 2,
                            fall: 3,
                            between_pulses: 0,
                            off: 2,
                        },
                        colors: [
                            ColorConfig {
                                enabled: true,
                                r: *r,
                                g: *g,
                                b: *b,
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
                band: CurrentBand::Three,
            },

            LedPattern::Fixed(r, g, b) => IS31FL3194Config {
                enabled: true,
                mode: OperatingMode::CurrentLevel(*r, *g, *b),
                band: CurrentBand::Three,
            },

            LedPattern::Rainbow => IS31FL3194Config {
                enabled: true,
                mode: OperatingMode::Pattern(
                    Some(PatternConfig {
                        timing: Timing {
                            start: 0,
                            rise: 4,
                            hold: 2,
                            fall: 4,
                            between_pulses: 0,
                            off: 0,
                        },
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
                        timing: Timing {
                            start: 0,
                            rise: 4,
                            hold: 2,
                            fall: 4,
                            between_pulses: 0,
                            off: 0,
                        },
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
                        timing: Timing {
                            start: 0,
                            rise: 4,
                            hold: 2,
                            fall: 4,
                            between_pulses: 0,
                            off: 0,
                        },
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
                band: CurrentBand::Three,
            },
        }
    }
}
