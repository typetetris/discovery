#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;
use microbit::board::Board;
use microbit::display::blocking::Display;
use microbit::hal::prelude::*;
use microbit::hal::timer::Timer;
use panic_rtt_target as _;
use rtt_target::{rprintln, rtt_init_print};

const STATE_TIME: u32 = 50;

enum State {
    Row1GoingRight { col: u16 },
    Col5GoingDown { row: u16 },
    Row5GoingLeft { col: u16 },
    Col1GoingUp { row: u16 },
}

impl State {
    fn start() -> State {
        State::Row1GoingRight { col: 1 }
    }
    fn next(&self) -> State {
        match self {
            State::Row1GoingRight { col } if *col == 5 => State::Col5GoingDown {
                row: 2, /* row1 col5 did light up in the previous state */
            },
            State::Row1GoingRight { col } if *col != 5 => State::Row1GoingRight { col: col + 1 },
            State::Col5GoingDown { row } if *row == 5 => State::Row5GoingLeft {
                col: 4, /* row5 col5 did light up in the previous state */
            },
            State::Col5GoingDown { row } if *row != 5 => State::Col5GoingDown { row: row + 1 },
            State::Row5GoingLeft { col } if *col == 1 => State::Col1GoingUp { row: 4 },
            State::Row5GoingLeft { col } if *col != 1 => State::Row5GoingLeft { col: col - 1 },
            State::Col1GoingUp { row } if *row == 1 => State::Row1GoingRight { col: 2 },
            State::Col1GoingUp { row } if *row != 1 => State::Col1GoingUp { row: row - 1 },
            _ => {
                rprintln!("unexpected state encountered, resetting");
                State::start()
            }
        }
    }
    fn assign(&self, image_buffer: &mut [[u8; 5]; 5], val: u8) {
        match self {
            State::Row1GoingRight { col } if 1 <= *col && *col <= 5 => {
                image_buffer[0][(col - 1) as usize] = val
            }
            State::Col5GoingDown { row } if 1 <= *row && *row <= 5 => {
                image_buffer[(row - 1) as usize][4] = val
            }
            State::Row5GoingLeft { col } if 1 <= *col && *col <= 5 => {
                image_buffer[4][(col - 1) as usize] = val
            }
            State::Col1GoingUp { row } if 1 <= *row && *row <= 5 => {
                image_buffer[(row - 1) as usize][0] = val
            }
            _ => {
                rprintln!("unexpected state encountered, clearing all");
                *image_buffer = [[0; 5]; 5];
            }
        }
    }
    fn set(&self, image_buffer: &mut [[u8; 5]; 5]) {
        self.assign(image_buffer, 1u8);
    }
    fn reset(&self, image_buffer: &mut [[u8; 5]; 5]) {
        self.assign(image_buffer, 0u8);
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let board = Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let mut display = Display::new(board.display_pins);
    let mut image_buffer = [[0; 5]; 5];
    let mut state = State::start();

    state.set(&mut image_buffer);
    display.show(&mut timer, image_buffer, STATE_TIME);

    // infinite loop; just so we don't leave this stack frame
    loop {
        state.reset(&mut image_buffer);
        state = state.next();
        state.set(&mut image_buffer);
        display.show(&mut timer, image_buffer, STATE_TIME);
    }
}
