use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Clone)]
pub struct IS31FL3194Config {
    pub enabled: bool,
    pub mode: OperatingMode,
    pub band: CurrentBand,
}

#[derive(Clone)]
pub struct PatternConfig {
    pub timing: Timing,
    pub colors: [ColorConfig; 3],
    pub next: PatternNext,
    pub gamma: Gamma,
    /// how many pulses to do
    pub multipulse_repeat: Repeat,
    /// how many times to repeat entire pattern
    pub pattern_repeat: Repeat,
}

#[derive(Clone, Default)]
pub struct ColorConfig {
    pub enabled: bool,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub repeat: ColorRepeat,
}

#[derive(Clone)]
#[repr(u8)]
pub enum OperatingMode {
    /// $I_{OUT}=\frac{I_{MAX}}{256}\cdot\sum_{n=0}^{7}{D[n]\cdot2^n}$
    ///   where $D[n]$ stands for the individual bit value, $I_{MAX}$ is set by `.band`
    ///   Ex: 0b10110101 -> $I_{OUT}=I_{MAX}\frac{2^7+2^5+2^4+2^2+2^0}{256}$
    CurrentLevel(u8, u8, u8),
    Pattern(
        Option<PatternConfig>,
        Option<PatternConfig>,
        Option<PatternConfig>,
    ),
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug, EnumString, Display)]
#[repr(u8)]
pub enum CurrentBand {
    /// 0mA\~10mA, Imax=10mA
    #[allow(dead_code)]
    One = 0b00,
    /// 0mA\~20mA, Imax=20mA
    #[allow(dead_code)]
    Two = 0b01,
    /// 0mA\~30mA, Imax=30mA
    #[default]
    Three = 0b10,
    /// 0mA\~40mA, Imax=40mA
    #[allow(dead_code)]
    Four = 0b11,
}

#[derive(Clone, Default)]
#[repr(u8)]
pub enum ColorRepeat {
    #[default]
    Endless = 0b00,
    Once = 0b01,
    #[allow(dead_code)]
    Twice = 0b10,
    #[allow(dead_code)]
    Thrice = 0b11,
}

#[derive(Clone)]
pub enum PatternNext {
    #[allow(dead_code)]
    Stop,
    /// goto next pattern
    Next,
    /// goto previous pattern, NOT valid for P1
    #[allow(dead_code)]
    Prev,
}

#[derive(Clone)]
#[repr(u8)]
pub enum Gamma {
    /// gamma = 2.4
    Gamma2_4 = 0b00,
    /// gamma = 3.5
    #[allow(dead_code)]
    Gamma3_5 = 0b01,
    #[allow(dead_code)]
    Linearity = 0b10,
}

#[derive(Clone)]
#[repr(u8)]
pub enum Repeat {
    Endless,
    /// 1-15
    Count(u8),
}

/// 0000 0.03s
/// 0001 0.13s
/// 0010 0.26s
/// 0011 0.38s
/// 0100 0.51s
/// 0101 0.77s
/// 0110 1.04s
/// 0111 1.60s
/// 1000 2.10s
/// 1001 2.60s
/// 1010 3.10s
/// 1011 4.20s
/// 1100 5.20s
/// 1101 6.20s
/// 1110 7.30s
/// 1111 8.30s
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Timing {
    pub start: u8,
    pub rise: u8,
    pub hold: u8,
    pub fall: u8,
    /// fix this name
    pub between_pulses: u8,
    pub off: u8,
}
