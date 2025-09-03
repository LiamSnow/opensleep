use crate::led::model::{
    ColorConfig, ColorRepeat, CurrentBand, Gamma, IS31FL3194Config, OperatingMode, PatternConfig,
    PatternNext, Repeat, Timing,
};

use super::*;
use embedded_hal::i2c::{I2c, Operation};
use std::collections::VecDeque;

struct MockI2c {
    expected_writes: VecDeque<(u8, Vec<u8>)>,
    write_count: usize,
}

impl MockI2c {
    fn new() -> Self {
        Self {
            expected_writes: VecDeque::new(),
            write_count: 0,
        }
    }

    fn expect_write(&mut self, addr: u8, data: Vec<u8>) {
        self.expected_writes.push_back((addr, data));
    }

    fn verify_all_writes_called(&self) {
        assert!(
            self.expected_writes.is_empty(),
            "Not all expected writes were called. Remaining: {:?}",
            self.expected_writes
        );
    }
}

impl I2c for MockI2c {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        self.write_count += 1;

        let expected = self.expected_writes.pop_front().unwrap_or_else(|| {
            panic!(
                "Unexpected write #{} to addr 0x{addr:02x}",
                self.write_count
            )
        });

        assert_eq!(
            expected.0, addr,
            "Write #{}: Wrong address",
            self.write_count
        );
        assert_eq!(expected.1, bytes, "Write #{}: Wrong data", self.write_count);

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

const I2C_ADDR: u8 = 0x53;
const REG_OP_CONFIG: u8 = 0x01;
const REG_OUT_CONFIG: u8 = 0x02;
const REG_CURRENT_BAND: u8 = 0x03;
const REG_COLOR_UPDATE: u8 = 0x40;
const REG_RESET: u8 = 0x4F;

#[test]
fn test_reset() {
    let mut mock = MockI2c::new();
    mock.expect_write(I2C_ADDR, vec![REG_RESET, 0xC5]);

    let mut controller = IS31FL3194Controller::new(mock);
    controller.reset().expect("Reset should succeed");

    controller.dev.verify_all_writes_called();
}

#[test]
fn test_current_level_mode() {
    let mut mock = MockI2c::new();

    // config regs
    mock.expect_write(I2C_ADDR, vec![REG_OP_CONFIG, 0b00000101]); // current level mode, RGB, enabled
    mock.expect_write(I2C_ADDR, vec![REG_OUT_CONFIG, 0b00000111]); // all outputs enabled
    mock.expect_write(I2C_ADDR, vec![REG_CURRENT_BAND, 0b00010101]); // band 2 = 01 for all

    // current level regs
    mock.expect_write(I2C_ADDR, vec![0x21, 100]);
    mock.expect_write(I2C_ADDR, vec![0x32, 200]);
    mock.expect_write(I2C_ADDR, vec![0x10, 128]);

    let mut controller = IS31FL3194Controller::new(mock);

    let config = IS31FL3194Config {
        enabled: true,
        mode: OperatingMode::CurrentLevel(100, 200, 128),
        band: CurrentBand::Two,
    };

    controller
        .set_raw(config)
        .expect("Setting current level should succeed");
    controller.dev.verify_all_writes_called();
}

#[test]
fn test_single_pattern_mode() {
    let mut mock = MockI2c::new();

    // config
    mock.expect_write(I2C_ADDR, vec![REG_OP_CONFIG, 0b01110101]); // pattern mode all, RGB, enabled
    mock.expect_write(I2C_ADDR, vec![REG_OUT_CONFIG, 0b00000111]); // all outputs enabled
    mock.expect_write(I2C_ADDR, vec![REG_CURRENT_BAND, 0b00111111]); // band 4 (11) for all

    // P1
    mock.expect_write(I2C_ADDR, vec![0x1C, 0b00000001]);

    // P1 color repeat
    mock.expect_write(I2C_ADDR, vec![0x1D, 0b00000000]);

    // P1 C1 BRG
    mock.expect_write(I2C_ADDR, vec![0x10, 50]);
    mock.expect_write(I2C_ADDR, vec![0x11, 255]);
    mock.expect_write(I2C_ADDR, vec![0x12, 100]);

    // P1 C2
    mock.expect_write(I2C_ADDR, vec![0x13, 0]);
    mock.expect_write(I2C_ADDR, vec![0x14, 0]);
    mock.expect_write(I2C_ADDR, vec![0x15, 0]);

    // P1 C3
    mock.expect_write(I2C_ADDR, vec![0x16, 0]);
    mock.expect_write(I2C_ADDR, vec![0x17, 0]);
    mock.expect_write(I2C_ADDR, vec![0x18, 0]);

    mock.expect_write(I2C_ADDR, vec![0x1E, 0b00110000]); // 3 loops, gamma 2.4, stop
    mock.expect_write(I2C_ADDR, vec![0x1F, 1]);
    mock.expect_write(I2C_ADDR, vec![0x41, 0xC5]);

    // P1 timing
    mock.expect_write(I2C_ADDR, vec![0x19, 0b00100001]);
    mock.expect_write(I2C_ADDR, vec![0x1A, 0b01000011]);
    mock.expect_write(I2C_ADDR, vec![0x1B, 0b01010110]);

    mock.expect_write(I2C_ADDR, vec![REG_COLOR_UPDATE, 0xC5]);

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

    controller
        .set_raw(config)
        .expect("Setting pattern should succeed");
    controller.dev.verify_all_writes_called();
}

#[test]
fn test_multi_pattern_transitions() {
    let mut mock = MockI2c::new();

    // config
    mock.expect_write(I2C_ADDR, vec![REG_OP_CONFIG, 0b01110101]); // pattern mode, RGB, enabled
    mock.expect_write(I2C_ADDR, vec![REG_OUT_CONFIG, 0b00000111]); // all outputs enabled
    mock.expect_write(I2C_ADDR, vec![REG_CURRENT_BAND, 0b00101010]); // band 3 for all

    // P1 colors
    mock.expect_write(I2C_ADDR, vec![0x1C, 0b00000011]); // enable colors 1 and 2

    // P1 color repeat
    mock.expect_write(I2C_ADDR, vec![0x1D, 0x00]);

    // P1 C1
    mock.expect_write(I2C_ADDR, vec![0x10, 255]);
    mock.expect_write(I2C_ADDR, vec![0x11, 0]);
    mock.expect_write(I2C_ADDR, vec![0x12, 0]);

    // P1 C2
    mock.expect_write(I2C_ADDR, vec![0x13, 0]);
    mock.expect_write(I2C_ADDR, vec![0x14, 255]);
    mock.expect_write(I2C_ADDR, vec![0x15, 0]);

    // P1 C3
    mock.expect_write(I2C_ADDR, vec![0x16, 0]);
    mock.expect_write(I2C_ADDR, vec![0x17, 0]);
    mock.expect_write(I2C_ADDR, vec![0x18, 0]);

    mock.expect_write(I2C_ADDR, vec![0x1E, 0b00000001]); // endless, gamma 2.4, goto next
    mock.expect_write(I2C_ADDR, vec![0x1F, 1]); // repeat once
    mock.expect_write(I2C_ADDR, vec![0x41, 0xC5]);
    mock.expect_write(I2C_ADDR, vec![0x19, 0b00000000]);
    mock.expect_write(I2C_ADDR, vec![0x1A, 0b00000000]);
    mock.expect_write(I2C_ADDR, vec![0x1B, 0b00000000]);

    // P2 colors
    mock.expect_write(I2C_ADDR, vec![0x2C, 0b00000001]);

    // P2 color repeat
    mock.expect_write(I2C_ADDR, vec![0x2D, 0b00000000]);

    // P2 C1
    mock.expect_write(I2C_ADDR, vec![0x20, 0]);
    mock.expect_write(I2C_ADDR, vec![0x21, 0]);
    mock.expect_write(I2C_ADDR, vec![0x22, 255]);

    // P2 C2
    mock.expect_write(I2C_ADDR, vec![0x23, 0]);
    mock.expect_write(I2C_ADDR, vec![0x24, 0]);
    mock.expect_write(I2C_ADDR, vec![0x25, 0]);

    // P2 C3
    mock.expect_write(I2C_ADDR, vec![0x26, 0]);
    mock.expect_write(I2C_ADDR, vec![0x27, 0]);
    mock.expect_write(I2C_ADDR, vec![0x28, 0]);

    mock.expect_write(I2C_ADDR, vec![0x2E, 0b00001010]); // endless, linearity, goto next
    mock.expect_write(I2C_ADDR, vec![0x2F, 1]); //repeat once

    mock.expect_write(I2C_ADDR, vec![0x42, 0xC5]);
    mock.expect_write(I2C_ADDR, vec![0x29, 0b00110010]);
    mock.expect_write(I2C_ADDR, vec![0x2A, 0b00000000]);
    mock.expect_write(I2C_ADDR, vec![0x2B, 0b00000000]);

    mock.expect_write(I2C_ADDR, vec![REG_COLOR_UPDATE, 0xC5]);

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

    controller
        .set_raw(config)
        .expect("Setting multi-pattern should succeed");
    controller.dev.verify_all_writes_called();
}
