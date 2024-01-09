#![no_main]
#![no_std]

use core::fmt::Write;
use cortex_m_rt::entry;
use heapless::Vec;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

#[cfg(feature = "v1")]
use microbit::{
    hal::prelude::*,
    hal::uart,
    hal::uart::{Baudrate, Parity},
};

#[cfg(feature = "v2")]
use microbit::{
    hal::prelude::*,
    hal::uarte,
    hal::uarte::{Baudrate, Parity},
};

#[cfg(feature = "v2")]
mod serial_setup;
#[cfg(feature = "v2")]
use serial_setup::UartePort;

#[derive(Debug)]
enum Error {
    UarteError(microbit::hal::uarte::Error),
    WriteError(core::fmt::Error),
    PushError(u8),
}

impl From<u8> for Error {
    fn from(value: u8) -> Error {
        return Error::PushError(value);
    }
}

impl From<core::fmt::Error> for Error {
    fn from(value: core::fmt::Error) -> Error {
        return Error::WriteError(value);
    }
}

impl From<microbit::hal::uarte::Error> for Error {
    fn from(value: microbit::hal::uarte::Error) -> Error {
        return Error::UarteError(value);
    }
}

fn echo_one_word<T: microbit::hal::uarte::Instance>(
    serial: &mut UartePort<T>,
    buffer: &mut Vec<u8, 32>,
) -> Result<(), Error> {
    buffer.clear();
    loop {
        let byte = nb::block!(serial.read())?;
        rprintln!("Received {}", byte);
        rprintln!("Buffer length so far: {}", buffer.len());
        if byte == b'\r' {
            rprintln!("Enter received, sending!");
            buffer.reverse();
            serial.bwrite_all(buffer.as_slice())?;
            writeln!(serial, "\r")?;
            nb::block!(serial.flush())?;
            return Ok(());
        } else if let Err(_) = buffer.push(byte) {
            writeln!(serial, "ERROR: Entered string too long, resetting!\r")?;
            nb::block!(serial.flush())?;
            return Ok(());
        }
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = microbit::Board::take().unwrap();

    #[cfg(feature = "v1")]
    let mut serial = {
        uart::Uart::new(
            board.UART0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        )
    };

    #[cfg(feature = "v2")]
    let mut serial = {
        let serial = uarte::Uarte::new(
            board.UARTE0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );
        UartePort::new(serial)
    };

    let mut buffer: Vec<u8, 32> = Vec::new();

    loop {
        echo_one_word(&mut serial, &mut buffer).unwrap()
    }
}
