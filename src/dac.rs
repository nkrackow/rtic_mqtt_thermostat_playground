// Thermostat DAC/PWM driver
//
// This file contains all of the drivers to convert an 18 bit word to an analog current.
// On Thermostat this used the ad5680 DAC and the MAX1968 PWM TEC driver.
// SingularitySurfer 2021

use byteorder::{BigEndian, ByteOrder};
use core::fmt;
use log::{error, info, warn};

use stm32_eth::hal::{
    gpio::{gpioe::*, gpiof::*, gpioc::*, Alternate, Output, PushPull, AF5},
    hal::{blocking::spi::Transfer, digital::v2::OutputPin, PwmPin},
    rcc::Clocks,
    spi,
    spi::{Spi, NoMiso},
    stm32::{SPI4, SPI5, TIM1, TIM3},
    time::{U32Ext, MegaHertz},
    pwm::{self, PwmChannels},
};

/// SPI Mode 1
pub const SPI_MODE: spi::Mode = spi::Mode {
    polarity: spi::Polarity::IdleLow,
    phase: spi::Phase::CaptureOnSecondTransition,
};

pub const SPI_CLOCK: MegaHertz = MegaHertz(30);

pub const MAX_VALUE: u32 = 0x3FFFF;

pub const MAX_DUTY: u16 = 0xffff;

pub type Dac0Spi = Spi<
    SPI4,
    (
        PE2<Alternate<AF5>>,
        NoMiso,
        PE6<Alternate<AF5>>,
    ),
>;

pub struct Dac0Pins {
    pub sck: PE2<Alternate<AF5>>,
    pub mosi: PE6<Alternate<AF5>>,
    pub sync: PE4<Output<PushPull>>,
}

pub type Dac1Spi = Spi<
    SPI5,
    (
        PF7<Alternate<AF5>>,
        NoMiso,
        PF9<Alternate<AF5>>,
    ),
>;

pub struct Dac1Pins {
    pub sck: PF7<Alternate<AF5>>,
    pub mosi: PF9<Alternate<AF5>>,
    pub sync: PF6<Output<PushPull>>,
}

pub struct Pwms {
    pub max_v0: PwmChannels<TIM3, pwm::C1>,
    pub max_v1: PwmChannels<TIM3, pwm::C2>,
    pub max_i_pos0: PwmChannels<TIM1, pwm::C1>,
    pub max_i_pos1: PwmChannels<TIM1, pwm::C2>,
    pub max_i_neg0: PwmChannels<TIM1, pwm::C3>,
    pub max_i_neg1: PwmChannels<TIM1, pwm::C4>,
    pub shdn: PE10<Output<PushPull>>,
}

impl Pwms {
    pub fn new<M1, M2, M3, M4, M5, M6>(
        clocks: Clocks,
        tim1: TIM1,
        tim3: TIM3,
        max_v0: PC6<M1>,
        max_v1: PC7<M2>,
        max_i_pos0: PE9<M3>,
        max_i_pos1: PE11<M4>,
        max_i_neg0: PE13<M5>,
        max_i_neg1: PE14<M6>,
        shdn: PE10<Output<PushPull>>,
    ) -> Pwms {
        let freq = 20u32.khz();

        fn init_pwm_pin<P: PwmPin<Duty=u16>>(pin: &mut P) {
            pin.set_duty(0);
            pin.enable();
        }
        let channels = (
            max_v0.into_alternate_af2(),
            max_v1.into_alternate_af2(),
        );
        let (mut max_v0, mut max_v1) = pwm::tim3(tim3, channels, clocks, freq);
        init_pwm_pin(&mut max_v0);
        init_pwm_pin(&mut max_v1);

        let channels = (
            max_i_pos0.into_alternate_af1(),
            max_i_pos1.into_alternate_af1(),
            max_i_neg0.into_alternate_af1(),
            max_i_neg1.into_alternate_af1(),
        );
        let (mut max_i_pos0, mut max_i_pos1, mut max_i_neg0, mut max_i_neg1) =
            pwm::tim1(tim1, channels, clocks, freq);
        init_pwm_pin(&mut max_i_pos0);
        init_pwm_pin(&mut max_i_neg0);
        init_pwm_pin(&mut max_i_pos1);
        init_pwm_pin(&mut max_i_neg1);

        Pwms {
            max_v0, max_v1,
            max_i_pos0, max_i_pos1,
            max_i_neg0, max_i_neg1,
            shdn
        }
    }

    pub fn set(&mut self, duty: f64, ch: u8) {
        fn set<P: PwmPin<Duty=u16>>(pin: &mut P, duty: f64) {
            let max = pin.get_max_duty();
            let value = ((duty * (max as f64)) as u16).min(max);
            pin.set_duty(value);
        }
        match (ch) {
            0 =>
                set(&mut self.max_v0, duty),
            1 =>
                set(&mut self.max_v1, duty),
            2 =>
                set(&mut self.max_i_pos0, duty),
            3 =>
                set(&mut self.max_i_pos1, duty),
            4 =>
                set(&mut self.max_i_neg0, duty),
            5 =>
                set(&mut self.max_i_neg1, duty),
            _ =>
                unreachable!(),
        }
    }
}

pub fn duty(duty:f64) -> u16 {
    ((duty * (MAX_DUTY as f64)) as u16).min(MAX_DUTY)
}

pub struct Dacs {
    spi0: Dac0Spi,
    sync0: PE4<Output<PushPull>>,
    spi1: Dac1Spi,
    sync1: PF6<Output<PushPull>>,
    
}

impl Dacs {
    pub fn new(clocks: Clocks, spi4: SPI4, spi5: SPI5, mut pins0: Dac0Pins, mut pins1: Dac1Pins) -> Self {
        let spi0 = Spi::spi4(
            spi4,
            (pins0.sck, NoMiso, pins0.mosi),
            SPI_MODE,
            SPI_CLOCK.into(),
            clocks,
        );
        let spi1 = Spi::spi5(
            spi5,
            (pins1.sck, NoMiso, pins1.mosi),
            SPI_MODE,
            SPI_CLOCK.into(),
            clocks,
        );
        
        let mut dacs = Dacs{
            spi0,
            sync0: pins0.sync,
            spi1,
            sync1: pins1.sync
        };
        let _ = dacs.sync0.set_low();
        let _ = dacs.sync1.set_low();

        dacs.set(0, 0);
        dacs.set(0, 1);

        dacs
    }

    pub fn set(&mut self, value: u32, ch: u8) {
        let value = value.min(MAX_VALUE);
        // 24 bit transfer. First 6 bit and last 2 bit are low.
        let mut buf = [
            (value >> 14) as u8,
            (value >> 6) as u8,
            (value << 2) as u8,
        ];
        if ch == 0 {
            let _ = self.sync0.set_high();
            // must be high for >= 33 ns
            let mut n = 1000;
            while n>0 {
                n-=1;
            };
            let _ = self.sync0.set_low();
            let _ = self.spi0.transfer(&mut buf);
        }
        else {
            let _ = self.sync1.set_high();
            // must be high for >= 33 ns
            let mut n = 1000;
            while n>0 {
                n-=1;
            };
            let _ = self.sync1.set_low();
            let _ = self.spi1.transfer(&mut buf);
        }
    }
}
