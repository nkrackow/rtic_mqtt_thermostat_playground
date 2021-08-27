// Thermostat ADC driver
// (AD7172 https://www.analog.com/media/en/technical-documentation/data-sheets/AD7172-2.pdf)
// SingularitySurfer 2021

use byteorder::{BigEndian, ByteOrder};
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

// ADC Register Adresses
const ID: u8 = 0x7;
const ADCMODE: u8 = 0x1;
const IFMODE: u8 = 0x2;
const DATA: u8 = 0x44;
const FILTCON0: u8 = 0x28;
const FILTCON1: u8 = 0x29;
const FILTCON2: u8 = 0x2a;
const FILTCON3: u8 = 0x2b;
const CH0: u8 = 0x10;
const CH1: u8 = 0x11;
const CH2: u8 = 0x12;
const CH3: u8 = 0x13;
const SETUPCON0: u8 = 0x20;
const SETUPCON1: u8 = 0x21;
const SETUPCON2: u8 = 0x22;
const SETUPCON3: u8 = 0x23;
const OFFSET0: u8 = 0x30;
const OFFSET1: u8 = 0x31;
const OFFSET2: u8 = 0x32;
const OFFSET3: u8 = 0x33;
const GAIN0: u8 = 0x38;
const GAIN1: u8 = 0x39;
const GAIN2: u8 = 0x3a;
const GAIN3: u8 = 0x3b;

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

        let before = adc.read_reg(IFMODE, 2);
        adc.write_reg(IFMODE, 2, before|0x20);

        info!("filt con: {:#X}", adc.read_reg(FILTCON0, 2));

        let before = adc.read_reg(FILTCON0, 2);
        adc.write_reg(FILTCON0, 2, (before & 0xffe0) | 0x8); // set data rate CH0 to 1 kSPS

        adc.write_reg(CH1, 2, 0x8043); // enable second channel

        let before = adc.read_reg(FILTCON1, 2);
        adc.write_reg(FILTCON1, 2, (before & 0xffe0) | 0x8); // set data rate CH1 to 1 kSPS

        info!("ch0: {:#X}", adc.read_reg(CH0, 2));
        info!("ch1: {:#X}", adc.read_reg(CH1, 2));


        info!("filt con: {:#X}", adc.read_reg(FILTCON0, 2));

        adc.print_continuous_conversion();


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

    fn print_continuous_conversion(&mut self) {
        loop {
            let mut statreg = 0xff;
            while statreg == 0xff {
                statreg = self.get_status_reg();
                // info!("statreg: {:#X}", self.get_status_reg());
            }
            info!("statreg: {:#X}", self.get_status_reg());
            info!("data: {:#X}", self.read_reg(0x44, 4));

        }
    }

    fn read_reg(&mut self, addr: u8, size: u8) -> u32 {
        let mut addr_buf = [addr | 0x40];
        let _ = self.sync.set_low();
        let _ = self.spi.transfer(&mut addr_buf);
        let data = match size {
            1 => {
                let mut buf = [0];
                let raw = self.spi.transfer(&mut buf);
                raw.unwrap()[0].clone() as u32
            }
            2 => {
                let mut buf = [0, 0];
                let raw = self.spi.transfer(&mut buf);
                BigEndian::read_u16(raw.unwrap()) as u32
            }
            3 => {
                let mut buf = [0, 0, 0];
                let raw = self.spi.transfer(&mut buf);
                BigEndian::read_u24(raw.unwrap()) as u32
            }
            4 => {
                let mut buf = [0, 0, 0, 0];
                let raw = self.spi.transfer(&mut buf);
                BigEndian::read_u32(raw.unwrap()) as u32
            }
            _ => 0,
        };
        let _ = self.sync.set_high();
        return data;
    }

    fn write_reg(&mut self, addr: u8, size: u8, data: u32) {
        let mut addr_buf = [addr];
        let _ = self.sync.set_low();
        let _ = self.spi.transfer(&mut addr_buf);
        match size {
            1 => {
                let mut buf = [data as u8];
                let _ = self.spi.transfer(&mut buf);
            }
            2 => {
                let mut buf = [0, 0];
                BigEndian::write_u16(&mut buf, data as u16);
                let _ = self.spi.transfer(&mut buf);
            }
            3 => {
                let mut buf = [0, 0, 0];
                BigEndian::write_u24(&mut buf, data as u32);
                let _ = self.spi.transfer(&mut buf);
            }
            4 => {
                let mut buf = [0, 0, 0, 0];
                BigEndian::write_u32(&mut buf, data as u32);
                let _ = self.spi.transfer(&mut buf);
            }
            _ => {}
        };
    }

    fn get_status_reg(&mut self) -> u8 {
        let mut addr_buf = [0];
        let _ = self.sync.set_low();
        let _ = self.spi.transfer(&mut addr_buf);
        let _ = self.sync.set_high();
        addr_buf[0]
    }
}