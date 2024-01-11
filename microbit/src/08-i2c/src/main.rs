#![no_main]
#![no_std]

use core::fmt::Write;
use cortex_m_rt::entry;
use embedded_hal::serial::Read;
use heapless::Vec;
use microbit::hal::uarte::{self, Baudrate, Parity};
use microbit::pac::UARTE0;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

#[cfg(feature = "v1")]
use microbit::{hal::twi, pac::twi0::frequency::FREQUENCY_A};

#[cfg(feature = "v2")]
use microbit::{hal::twim, pac::twim0::frequency::FREQUENCY_A};

use lsm303agr::{AccelOutputDataRate, Lsm303agr};

mod serial_setup;
use serial_setup::UartePort;

#[derive(Debug)]
enum FillBufferError {
    PushError(u8),
    UarteError(microbit::hal::uarte::Error),
    Write(core::fmt::Error),
}

impl From<u8> for FillBufferError {
    fn from(value: u8) -> Self {
        FillBufferError::PushError(value)
    }
}

impl From<core::fmt::Error> for FillBufferError {
    fn from(value: core::fmt::Error) -> Self {
        FillBufferError::Write(value)
    }
}

impl From<microbit::hal::uarte::Error> for FillBufferError {
    fn from(value: microbit::hal::uarte::Error) -> Self {
        FillBufferError::UarteError(value)
    }
}

#[derive(Debug)]
enum Error<'a> {
    Uarte(microbit::hal::uarte::Error),
    Push(u8),
    Unrecognized(&'a str),
    Utf8(core::str::Utf8Error),
    Write(core::fmt::Error),
}

impl<'a> From<u8> for Error<'a> {
    fn from(value: u8) -> Self {
        return Error::Push(value);
    }
}

impl<'a> From<microbit::hal::uarte::Error> for Error<'a> {
    fn from(value: microbit::hal::uarte::Error) -> Self {
        return Error::Uarte(value);
    }
}

impl<'a> From<core::str::Utf8Error> for Error<'a> {
    fn from(value: core::str::Utf8Error) -> Self {
        return Error::Utf8(value);
    }
}

impl<'a> From<FillBufferError> for Error<'a> {
    fn from(value: FillBufferError) -> Self {
        match value {
            FillBufferError::PushError(err) => Error::Push(err),
            FillBufferError::UarteError(err) => Error::Uarte(err),
            FillBufferError::Write(err) => Error::Write(err),
        }
    }
}

impl<'a> core::fmt::Display for Error<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Uarte(err) => write!(f, "serial communication: {:?}", err),
            Error::Push(_) => write!(f, "command word too long"),
            Error::Unrecognized(err) => write!(f, "unrecognized command: {}", err),
            Error::Utf8(err) => write!(f, "utf8 conversion: {}", err),
            Error::Write(err) => write!(f, "formatted write: {}", err),
        }
    }
}

enum Command {
    Magnetometer,
    Accelerometer,
}

fn try_fill_buffer_with_echo(
    serial: &mut UartePort<UARTE0>,
    buffer: &mut Vec<u8, 16>,
) -> Result<(), FillBufferError> {
    buffer.clear();
    loop {
        let byte = nb::block!(serial.read())?;
        if byte == b'\r' {
            writeln!(serial, "\r")?;
            return Ok(());
        }
        nb::block!(embedded_hal::serial::Write::write(serial, byte))?;
        nb::block!(embedded_hal::serial::Write::flush(serial))?;
        buffer.push(byte)?;
    }
}

fn try_read_command<'a>(
    serial: &mut UartePort<UARTE0>,
    buffer: &'a mut Vec<u8, 16>,
) -> Result<Command, Error<'a>> {
    try_fill_buffer_with_echo(serial, buffer)?;
    let word = core::str::from_utf8(buffer)?;
    match word {
        "magnetometer" => Ok(Command::Magnetometer),
        "accelerometer" => Ok(Command::Accelerometer),
        _ => Err(Error::Unrecognized(word)),
    }
}

fn read_command(serial: &mut UartePort<UARTE0>) -> Result<Command, core::fmt::Error> {
    let mut buffer: Vec<u8, 16> = Vec::new();
    loop {
        writeln!(
            serial,
            "Available commands: \"magnetometer\" and \"accelerometer\": \r"
        )?;
        match try_read_command(serial, &mut buffer) {
            Ok(cmd) => return Ok(cmd),
            Err(err) => writeln!(serial, "*** error ***\r\n{}\r", err)?,
        }
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = microbit::Board::take().unwrap();

    #[cfg(feature = "v1")]
    let mut i2c = { twi::Twi::new(board.TWI0, board.i2c.into(), FREQUENCY_A::K100) };

    #[cfg(feature = "v2")]
    let i2c = { twim::Twim::new(board.TWIM0, board.i2c_internal.into(), FREQUENCY_A::K100) };

    let mut uarte = {
        let serial = uarte::Uarte::new(
            board.UARTE0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );
        UartePort::new(serial)
    };

    let mut sensor = Lsm303agr::new_with_i2c(i2c);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz50).unwrap();
    sensor
        .set_mag_odr(lsm303agr::MagOutputDataRate::Hz50)
        .unwrap();

    loop {
        match read_command(&mut uarte).unwrap() {
            Command::Magnetometer => {
                rprintln!("reading magnetometer");
                loop {
                    if sensor.mag_status().unwrap().xyz_new_data {
                        rprintln!("got value:");
                        let data = sensor.mag_data().unwrap();
                        writeln!(
                            uarte,
                            "Magnetic field (nT): x {} y {} z {}\r",
                            data.x, data.y, data.z
                        )
                        .unwrap();
                        break;
                    }
                }
            }
            Command::Accelerometer => {
                rprintln!("reading accelerometer");
                loop {
                    if sensor.accel_status().unwrap().xyz_new_data {
                        rprintln!("got value:");
                        let data = sensor.accel_data().unwrap();
                        writeln!(
                            uarte,
                            "Acceleration (mg): x {} y {} z {}\r",
                            data.x, data.y, data.z
                        )
                        .unwrap();
                        break;
                    }
                }
            }
        }
        nb::block!(embedded_hal::serial::Write::flush(&mut uarte)).unwrap();
    }
}
