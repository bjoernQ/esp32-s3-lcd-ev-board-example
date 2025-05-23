#![no_std]
#![no_main]

extern crate alloc;
use alloc::boxed::Box;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
    Drawable,
};
use embedded_graphics_framebuf::FrameBuf;
use esp32_s3_lcd_ev_board_example::display::{BUFFER_SIZE, HEIGHT, WIDTH};
use esp_backtrace as _;
use esp_hal::peripherals::Peripherals;
use esp_hal::{clock::CpuClock::_240MHz, main};
use esp_println::logger::init_logger_from_env;

#[main]
fn main() -> ! {
    let mut peripherals: Peripherals =
        esp_hal::init(esp_hal::Config::default().with_cpu_clock(_240MHz));
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    init_logger_from_env();

    let mut display = esp32_s3_lcd_ev_board_example::display::Display::new(&mut peripherals);

    // Create a framebuffer
    let mut fb_data: Box<[Rgb565; BUFFER_SIZE]> = Box::new([Rgb565::BLACK; BUFFER_SIZE]);
    let mut frame_buf = FrameBuf::new(fb_data.as_mut(), WIDTH, HEIGHT);

    log::info!("Starting main loop");
    loop {
        frame_buf.clear(Rgb565::BLUE).unwrap();

        // draw something
        Rectangle::new(Point::new(10, 10), Size::new(260, 210))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::new(230, 230, 230)))
            .draw(&mut frame_buf)
            .ok();

        // transmit to display
        display.transmit(&frame_buf);
    }
}
