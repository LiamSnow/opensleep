use crate::led::model::{
    ColorConfig, ColorRepeat, CurrentBand, Gamma, IS31FL3194Config, OperatingMode, PatternConfig,
    PatternNext, Repeat, Timing,
};

use super::*;
use embedded_hal::i2c::{I2c, Operation};
use std::collections::HashMap;

const I2C_ADDR: u8 = 0x53;
const REG_OP_CONFIG: u8 = 0x01;
const REG_OUT_CONFIG: u8 = 0x02;
const REG_CURRENT_BAND: u8 = 0x03;
const REG_COLOR_UPDATE: u8 = 0x40;
const REG_RESET: u8 = 0x4F;

#[derive(Default)]
struct MockI2c {
    regs: HashMap<u8, u8>,
}

impl MockI2c {
    fn expect_all(&self, rv_pairs: Vec<(u8, u8)>) {
        for (reg, value) in rv_pairs {
            self.expect(reg, value);
        }
    }

    fn expect(&self, reg: u8, value: u8) {
        assert_eq!(
            self.regs.get(&reg),
            Some(&value),
            "Expected register {reg:02X}h to be {value:08b}"
        );
    }
}

impl I2c for MockI2c {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        assert_eq!(addr, I2C_ADDR, "Write to wrong address");
        self.regs.insert(bytes[0], bytes[1]);
        Ok(())
    }

    fn read(&mut self, _addr: u8, _buffer: &mut [u8]) -> Result<(), Self::Error> {
        panic!()
    }

    fn write_read(
        &mut self,
        _addr: u8,
        _bytes: &[u8],
        _buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        panic!()
    }

    fn transaction(
        &mut self,
        _addr: u8,
        _operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        panic!()
    }
}

impl embedded_hal::i2c::ErrorType for MockI2c {
    type Error = std::convert::Infallible;
}

#[test]
fn test_reset() {
    let mock = MockI2c::default();

    let mut controller = IS31FL3194Controller::new(mock);
    controller.reset().unwrap();

    controller.dev.expect(REG_RESET, 0xC5);
}

#[test]
fn test_current_level_mode() {
    let mock = MockI2c::default();

    let mut controller = IS31FL3194Controller::new(mock);

    let config = IS31FL3194Config {
        enabled: true,
        mode: OperatingMode::CurrentLevel(100, 200, 128),
        band: CurrentBand::Two,
    };

    controller.set(&config).unwrap();

    controller.dev.expect_all(vec![
        // config regs
        (REG_OP_CONFIG, 0b00000001), // current level mode, single led mode, enabled
        (REG_OUT_CONFIG, 0b00000111), // all outputs enabled
        (REG_CURRENT_BAND, 0b00010101), // band 2 = 01 for all
        // current level regs
        (0x21, 100),
        (0x32, 200),
        (0x10, 128),
    ]);
}

#[test]
fn test_single_pattern_mode() {
    let mock = MockI2c::default();

    let mut controller = IS31FL3194Controller::new(mock);

    let config = IS31FL3194Config {
        enabled: true,
        mode: OperatingMode::Pattern(
            Some(PatternConfig {
                timing: Timing {
                    start: 1,
                    rise: 2,
                    hold: 3,
                    fall: 4,
                    off: 5,
                    between_pulses: 6,
                },
                colors: [
                    ColorConfig {
                        enabled: true,
                        r: 255,
                        g: 100,
                        b: 50,
                        repeat: ColorRepeat::Endless,
                    },
                    ColorConfig::default(),
                    ColorConfig::default(),
                ],
                next: PatternNext::Stop,
                gamma: Gamma::Gamma2_4,
                multipulse_repeat: Repeat::Count(3),
                pattern_repeat: Repeat::Count(1),
            }),
            None,
            None,
        ),
        band: CurrentBand::Four,
    };

    controller.set(&config).unwrap();

    controller.dev.expect_all(vec![
        // config
        (REG_OP_CONFIG, 0b01110101),    // pattern mode all, RGB, enabled
        (REG_OUT_CONFIG, 0b00000111),   // all outputs enabled
        (REG_CURRENT_BAND, 0b00111111), // band 4 (11) for all
        // P1
        (0x1C, 0b00000001),
        // P1 color repeat
        (0x1D, 0b00000000),
        // P1 C1 BRG
        (0x10, 50),
        (0x11, 255),
        (0x12, 100),
        // P1 C2
        (0x13, 0),
        (0x14, 0),
        (0x15, 0),
        // P1 C3
        (0x16, 0),
        (0x17, 0),
        (0x18, 0),
        (0x1E, 0b00110000), // 3 loops, gamma 2.4, stop
        (0x1F, 1),
        (0x41, 0xC5),
        // P1 timing
        (0x19, 0b00100001),
        (0x1A, 0b01000011),
        (0x1B, 0b01010110),
        (REG_COLOR_UPDATE, 0xC5),
    ]);
}

#[test]
fn test_multi_pattern_transitions() {
    let mock = MockI2c::default();

    let mut controller = IS31FL3194Controller::new(mock);

    let config = IS31FL3194Config {
        enabled: true,
        mode: OperatingMode::Pattern(
            Some(PatternConfig {
                timing: Timing {
                    start: 0,
                    rise: 0,
                    hold: 0,
                    fall: 0,
                    off: 0,
                    between_pulses: 0,
                },
                colors: [
                    ColorConfig {
                        enabled: true,
                        r: 0,
                        g: 0,
                        b: 255,
                        repeat: ColorRepeat::Endless,
                    },
                    ColorConfig {
                        enabled: true,
                        r: 255,
                        g: 0,
                        b: 0,
                        repeat: ColorRepeat::Endless,
                    },
                    ColorConfig::default(),
                ],
                next: PatternNext::Next,
                gamma: Gamma::Gamma2_4,
                multipulse_repeat: Repeat::Endless,
                pattern_repeat: Repeat::Count(1),
            }),
            Some(PatternConfig {
                timing: Timing {
                    start: 2,
                    rise: 3,
                    hold: 0,
                    fall: 0,
                    off: 0,
                    between_pulses: 0,
                },
                colors: [
                    ColorConfig {
                        enabled: true,
                        r: 0,
                        g: 255,
                        b: 0,
                        repeat: ColorRepeat::Endless,
                    },
                    ColorConfig::default(),
                    ColorConfig::default(),
                ],
                next: PatternNext::Next,
                gamma: Gamma::Linearity,
                multipulse_repeat: Repeat::Endless,
                pattern_repeat: Repeat::Count(1),
            }),
            None,
        ),
        band: CurrentBand::Three,
    };

    controller.set(&config).unwrap();

    controller.dev.expect_all(vec![
        // config
        (REG_OP_CONFIG, 0b01110101),    // pattern mode, RGB, enabled
        (REG_OUT_CONFIG, 0b00000111),   // all outputs enabled
        (REG_CURRENT_BAND, 0b00101010), // band 3 for all
        // P1 colors
        (0x1C, 0b00000011), // enable colors 1 and 2
        // P1 color repeat
        (0x1D, 0x00),
        // P1 C1
        (0x10, 255),
        (0x11, 0),
        (0x12, 0),
        // P1 C2
        (0x13, 0),
        (0x14, 255),
        (0x15, 0),
        // P1 C3
        (0x16, 0),
        (0x17, 0),
        (0x18, 0),
        (0x1E, 0b00000001), // endless, gamma 2.4, goto next
        (0x1F, 1),          // repeat once
        (0x41, 0xC5),
        (0x19, 0b00000000),
        (0x1A, 0b00000000),
        (0x1B, 0b00000000),
        // P2 colors
        (0x2C, 0b00000001),
        // P2 color repeat
        (0x2D, 0b00000000),
        // P2 C1
        (0x20, 0),
        (0x21, 0),
        (0x22, 255),
        // P2 C2
        (0x23, 0),
        (0x24, 0),
        (0x25, 0),
        // P2 C3
        (0x26, 0),
        (0x27, 0),
        (0x28, 0),
        (0x2E, 0b00001010), // endless, linearity, goto next
        (0x2F, 1),          //repeat once
        (0x42, 0xC5),
        (0x29, 0b00110010),
        (0x2A, 0b00000000),
        (0x2B, 0b00000000),
        (REG_COLOR_UPDATE, 0xC5),
    ]);
}
