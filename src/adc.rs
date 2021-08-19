// Thermostat ADC driver
// SingularitySurfer 2021

use core::fmt;
use log::{error, info, warn};

use stm32_eth::hal::{
    gpio::{gpiob::*, Alternate, GpioExt, Output, PushPull, AF5},
    hal::{blocking::spi::Transfer, digital::v2::OutputPin},
    rcc::Clocks,
    spi,
    spi::Spi,
    stm32::SPI2,
    time::MegaHertz,
};

/// SPI Mode 3
pub const SPI_MODE: spi::Mode = spi::Mode {
    polarity: spi::Polarity::IdleHigh,
    phase: spi::Phase::CaptureOnSecondTransition,
};

pub const SPI_CLOCK: MegaHertz = MegaHertz(2);

pub type AdcSpi = Spi<
    SPI2,
    (
        PB10<Alternate<AF5>>,
        PB14<Alternate<AF5>>,
        PB15<Alternate<AF5>>,
    ),
>;

pub struct Adc_pins {
    pub sck: PB10<Alternate<AF5>>,
    pub miso: PB14<Alternate<AF5>>,
    pub mosi: PB15<Alternate<AF5>>,
    pub sync: PB12<Output<PushPull>>,
}

pub struct Adc {
    spi: AdcSpi,
    sync: PB12<Output<PushPull>>,
}

impl Adc {
    pub fn new(clocks: Clocks, spi2: SPI2, mut pins: Adc_pins) -> Self {
        pins.sync.set_high();
        let spi = Spi::spi2(
            spi2,
            (pins.sck, pins.miso, pins.mosi),
            SPI_MODE,
            SPI_CLOCK.into(),
            clocks,
        );
        let mut adc = Adc {
            spi,
            sync: pins.sync,
        };

        adc.reset();

        adc
    }

    pub fn reset(&mut self) {
        let mut buf = [0xFFu8; 8];
        self.sync.set_low();
        let result = self.spi.transfer(&mut buf);
        self.sync.set_high();
        match result {
            Err(e) => {
                warn!("ADC reset failed! {:?}", e)
            }
            Ok(_) => {
                info!("ADC reset succeeded")
            }
        };
    }
}
