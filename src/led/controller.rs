use embedded_hal::i2c::I2c;

use super::model::*;
use super::patterns::LedPattern;

/// I2C wrapper for the IS31FL3194 LED controller
/// Forced to RGB mode
pub struct IS31FL3194Controller<T: I2c> {
    pub(crate) dev: T,
}

impl<T: I2c> IS31FL3194Controller<T> {
    pub fn new(dev: T) -> IS31FL3194Controller<T> {
        Self { dev }
    }

    fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), T::Error> {
        const ADDR: u8 = 0x53;
        self.dev.write(ADDR, &[reg, value])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) -> Result<(), T::Error> {
        const REG_RESET: u8 = 0x4F;
        const RESET_VALUE: u8 = 0xC5;
        self.write_reg(REG_RESET, RESET_VALUE)?;
        Ok(())
    }

    pub fn set(&mut self, pattern: &LedPattern) -> Result<(), T::Error> {
        self.set_raw(pattern.get_config())
    }

    pub(crate) fn set_raw(&mut self, cfg: IS31FL3194Config) -> Result<(), T::Error> {
        self.set_mode(cfg.mode.get_reg_value())?;
        self.set_out_enabled(cfg.enabled)?;
        self.set_current_band(cfg.band)?;

        match cfg.mode {
            OperatingMode::CurrentLevel(r, g, b) => self.current_level(r, g, b),
            OperatingMode::Pattern(p1, p2, p3) => self.patterns([p1, p2, p3]),
        }
    }

    fn set_current_band(&mut self, band: CurrentBand) -> Result<(), T::Error> {
        const REG_CURRENT_BAND: u8 = 0x03;
        let band = band as u8;
        self.write_reg(REG_CURRENT_BAND, (band << 4) | (band << 2) | band)
    }

    fn set_out_enabled(&mut self, enabled: bool) -> Result<(), T::Error> {
        const REG_OUT_CONFIG: u8 = 0x02;
        self.write_reg(
            REG_OUT_CONFIG,
            ((enabled as u8) << 2) | ((enabled as u8) << 1) | (enabled as u8),
        )
    }

    fn set_mode(&mut self, mode: u8) -> Result<(), T::Error> {
        const REG_OP_CONFIG: u8 = 0x01;
        self.write_reg(
            REG_OP_CONFIG,
            (mode << 6) |
            (mode << 5) |
            (mode << 4) |
            // RGB mode
            (0b10 << 1) |
            // 0 = software shutdown, 1 = enabled
            0b1,
        )
    }

    fn patterns(&mut self, patterns: [Option<PatternConfig>; 3]) -> Result<(), T::Error> {
        for (pn, pattern) in patterns.into_iter().enumerate() {
            let pn = pn as u8;

            if let Some(pattern) = pattern {
                self.pattern_enable_colors(
                    pn,
                    pattern.colors[0].enabled,
                    pattern.colors[1].enabled,
                    pattern.colors[2].enabled,
                )?;

                self.pattern_color_repeat(
                    pn,
                    pattern.colors[0].repeat.clone(),
                    pattern.colors[1].repeat.clone(),
                    pattern.colors[2].repeat.clone(),
                )?;

                for (cn, color) in pattern.colors.into_iter().enumerate() {
                    self.pattern_color(pn, cn as u8, color.r, color.g, color.b)?;
                }

                self.pattern_nxt(pn, pattern.next, pattern.gamma, pattern.multipulse_repeat)?;
                self.pattern_repeat(pn, pattern.pattern_repeat)?;

                self.pattern_update_run(pn)?;

                self.pattern_timing(pn, pattern.timing)?;
            }
        }

        // self.pattern_update_run(0)?;

        self.update_colors()
    }

    fn pattern_repeat(&mut self, pattern: u8, repeat: Repeat) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        let reg = 0x1F + (pattern * 0x10);
        self.write_reg(
            reg,
            match repeat {
                Repeat::Endless => 0,
                Repeat::Count(n) => n,
            },
        )
    }

    pub(crate) fn pattern_color(
        &mut self,
        pattern: u8,
        color_number: u8,
        r: u8,
        g: u8,
        b: u8,
    ) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        assert!(color_number <= 2, "`color_number` must be 0-2");
        // pattern 1, color 1: 10~12
        // pattern 1, color 2: 13~15
        // pattern 2, color 1: 20~22
        // eight sleep messed up PCB so its BRG
        let offset = (pattern * 0x10) + (color_number * 3);
        let reg_b = offset + 0x10;
        let reg_r = offset + 0x11;
        let reg_g = offset + 0x12;
        self.write_reg(reg_b, b)?;
        self.write_reg(reg_r, r)?;
        self.write_reg(reg_g, g)
    }

    /// pattern 0-2
    pub(crate) fn pattern_timing(&mut self, pattern: u8, timing: Timing) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        let offset = pattern * 0x10;
        let reg_pn_start_rise = offset + 0x19;
        let reg_pn_hold_fall = offset + 0x1A;
        let reg_pn_pulse_off = offset + 0x1B;
        // [7:3 rise time], [4:0 start time]
        self.write_reg(reg_pn_start_rise, (timing.rise << 4) | timing.start)?;
        // [7:3 fall time], [4:0 hold time]
        self.write_reg(reg_pn_hold_fall, (timing.fall << 4) | timing.hold)?;
        // [7:3 off time], [4:0 btw pulses]
        self.write_reg(reg_pn_pulse_off, (timing.off << 4) | timing.between_pulses)
    }

    pub(crate) fn pattern_enable_colors(
        &mut self,
        pattern: u8,
        c1_en: bool,
        c2_en: bool,
        c3_en: bool,
    ) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        let reg = (pattern * 0x10) + 0x1C;
        self.write_reg(
            reg,
            ((c3_en as u8) << 2) | ((c2_en as u8) << 1) | (c1_en as u8),
        )
    }

    fn pattern_color_repeat(
        &mut self,
        pattern: u8,
        c1_repeat: ColorRepeat,
        c2_repeat: ColorRepeat,
        c3_repeat: ColorRepeat,
    ) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        let reg = (pattern * 0x10) + 0x1D;
        // [5:4] c3, [3:2] c2, [1:0] c1
        self.write_reg(
            reg,
            ((c3_repeat as u8) << 4) | ((c2_repeat as u8) << 2) | (c1_repeat as u8),
        )
    }

    fn pattern_nxt(
        &mut self,
        pattern: u8,
        next: PatternNext,
        gamma: Gamma,
        repeat: Repeat,
    ) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        let reg = (pattern * 0x10) + 0x1E;

        let mtply = match repeat {
            Repeat::Endless => 0,
            Repeat::Count(n) => n,
        };

        let next = match next {
            PatternNext::Stop => 0b00,
            PatternNext::Next => {
                if pattern == 1 {
                    0b10
                } else {
                    0b01
                }
            }
            PatternNext::Prev => match pattern {
                0 => panic!("Pattern 0 cannot have Prev"),
                1 => 0b01,
                2 => 0b10,
                _ => unreachable!(),
            },
        };

        // [7:4] Multy, [3:2] Gam, [1:0] Next
        self.write_reg(reg, (mtply << 4) | ((gamma as u8) << 2) | next)
    }

    fn pattern_update_run(&mut self, pattern: u8) -> Result<(), T::Error> {
        assert!(pattern <= 2, "`pattern` must be 0-2");
        const UPDATE_VALUE: u8 = 0xC5;
        let reg = 0x41 + pattern;
        self.write_reg(reg, UPDATE_VALUE)
    }

    fn update_colors(&mut self) -> Result<(), T::Error> {
        const REG_COLOR_UPDATE: u8 = 0x40;
        const UPDATE_VALUE: u8 = 0xC5;
        self.write_reg(REG_COLOR_UPDATE, UPDATE_VALUE)
    }

    fn current_level(&mut self, r: u8, g: u8, b: u8) -> Result<(), T::Error> {
        const REG_B_CURRENT_LEVEL: u8 = 0x10;
        const REG_R_CURRENT_LEVEL: u8 = 0x21;
        const REG_G_CURRENT_LEVEL: u8 = 0x32;
        self.write_reg(REG_R_CURRENT_LEVEL, r)?;
        self.write_reg(REG_G_CURRENT_LEVEL, g)?;
        self.write_reg(REG_B_CURRENT_LEVEL, b)
    }
}
