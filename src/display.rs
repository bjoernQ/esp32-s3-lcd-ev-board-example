//! Based on https://github.com/georgik/esp32-conways-game-of-life-rs/blob/main/esp32-s3-lcd-ev-board/src/main.rs

use alloc::boxed::Box;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics_framebuf::FrameBuf;
use esp_hal::{
    delay::Delay,
    dma::{DmaDescriptor, DmaTxBuf, CHUNK_SIZE},
    gpio::{Level, Output, OutputConfig},
    i2c::{self, master::I2c},
    lcd_cam::{
        lcd::{
            dpi::{Dpi, Format, FrameTiming},
            ClockMode, Phase, Polarity,
        },
        LcdCam,
    },
    peripherals::Peripherals,
    time::Rate,
    Blocking,
};

// --- DISPLAY CONFIGURATION ---
pub const WIDTH: usize = 480;
pub const HEIGHT: usize = 480;
pub const BUFFER_SIZE: usize = WIDTH * HEIGHT;

// Define the I2C expander struct (based on working code)
struct Tca9554<'a> {
    i2c: I2c<'a, Blocking>,
    address: u8,
}

impl<'a> Tca9554<'a> {
    pub fn new(i2c: I2c<'a, Blocking>) -> Self {
        Self { i2c, address: 0x20 }
    }

    pub fn write_direction_reg(&mut self, value: u8) -> Result<(), i2c::master::Error> {
        self.i2c.write(self.address, &[0x03, value])
    }

    pub fn write_output_reg(&mut self, value: u8) -> Result<(), i2c::master::Error> {
        self.i2c.write(self.address, &[0x01, value])
    }
}

// Define initialization commands
#[derive(Copy, Clone)]
enum InitCmd {
    Cmd(u8, &'static [u8]),
    Delay(u8),
}

// Initialization commands for the display controller
const INIT_CMDS: &[InitCmd] = &[
    InitCmd::Cmd(0xf0, &[0x55, 0xaa, 0x52, 0x08, 0x00]),
    InitCmd::Cmd(0xf6, &[0x5a, 0x87]),
    InitCmd::Cmd(0xc1, &[0x3f]),
    InitCmd::Cmd(0xc2, &[0x0e]),
    InitCmd::Cmd(0xc6, &[0xf8]),
    InitCmd::Cmd(0xc9, &[0x10]),
    InitCmd::Cmd(0xcd, &[0x25]),
    InitCmd::Cmd(0xf8, &[0x8a]),
    InitCmd::Cmd(0xac, &[0x45]),
    InitCmd::Cmd(0xa0, &[0xdd]),
    InitCmd::Cmd(0xa7, &[0x47]),
    InitCmd::Cmd(0xfa, &[0x00, 0x00, 0x00, 0x04]),
    InitCmd::Cmd(0x86, &[0x99, 0xa3, 0xa3, 0x51]),
    InitCmd::Cmd(0xa3, &[0xee]),
    InitCmd::Cmd(0xfd, &[0x3c, 0x3]),
    InitCmd::Cmd(0x71, &[0x48]),
    InitCmd::Cmd(0x72, &[0x48]),
    InitCmd::Cmd(0x73, &[0x00, 0x44]),
    InitCmd::Cmd(0x97, &[0xee]),
    InitCmd::Cmd(0x83, &[0x93]),
    InitCmd::Cmd(0x9a, &[0x72]),
    InitCmd::Cmd(0x9b, &[0x5a]),
    InitCmd::Cmd(0x82, &[0x2c, 0x2c]),
    InitCmd::Cmd(0xB1, &[0x10]),
    InitCmd::Cmd(
        0x6d,
        &[
            0x00, 0x1f, 0x19, 0x1a, 0x10, 0x0e, 0x0c, 0x0a, 0x02, 0x07, 0x1e, 0x1e, 0x1e, 0x1e,
            0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x08, 0x01, 0x09, 0x0b, 0x0d, 0x0f,
            0x1a, 0x19, 0x1f, 0x00,
        ],
    ),
    InitCmd::Cmd(
        0x64,
        &[
            0x38, 0x05, 0x01, 0xdb, 0x03, 0x03, 0x38, 0x04, 0x01, 0xdc, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x65,
        &[
            0x38, 0x03, 0x01, 0xdd, 0x03, 0x03, 0x38, 0x02, 0x01, 0xde, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x66,
        &[
            0x38, 0x01, 0x01, 0xdf, 0x03, 0x03, 0x38, 0x00, 0x01, 0xe0, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x67,
        &[
            0x30, 0x01, 0x01, 0xe1, 0x03, 0x03, 0x30, 0x02, 0x01, 0xe2, 0x03, 0x03, 0x7a, 0x7a,
            0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(
        0x68,
        &[
            0x00, 0x08, 0x15, 0x08, 0x15, 0x7a, 0x7a, 0x08, 0x15, 0x08, 0x15, 0x7a, 0x7a,
        ],
    ),
    InitCmd::Cmd(0x60, &[0x38, 0x08, 0x7a, 0x7a, 0x38, 0x09, 0x7a, 0x7a]),
    InitCmd::Cmd(0x63, &[0x31, 0xe4, 0x7a, 0x7a, 0x31, 0xe5, 0x7a, 0x7a]),
    InitCmd::Cmd(0x69, &[0x04, 0x22, 0x14, 0x22, 0x14, 0x22, 0x08]),
    InitCmd::Cmd(0x6b, &[0x07]),
    InitCmd::Cmd(0x7a, &[0x08, 0x13]),
    InitCmd::Cmd(0x7b, &[0x08, 0x13]),
    InitCmd::Cmd(
        0xd1,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd2,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd3,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd4,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd5,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(
        0xd6,
        &[
            0x00, 0x00, 0x00, 0x04, 0x00, 0x12, 0x00, 0x18, 0x00, 0x21, 0x00, 0x2a, 0x00, 0x35,
            0x00, 0x47, 0x00, 0x56, 0x00, 0x90, 0x00, 0xe5, 0x01, 0x68, 0x01, 0xd5, 0x01, 0xd7,
            0x02, 0x36, 0x02, 0xa6, 0x02, 0xee, 0x03, 0x48, 0x03, 0xa0, 0x03, 0xba, 0x03, 0xc5,
            0x03, 0xd0, 0x03, 0xe0, 0x03, 0xea, 0x03, 0xfa, 0x03, 0xff,
        ],
    ),
    InitCmd::Cmd(0x36, &[0x00]),
    InitCmd::Cmd(0x2A, &[0x00, 0x00, 0x01, 0xDF]), // 0 to 479 (0x1DF)
    // Set full row address range
    InitCmd::Cmd(0x2B, &[0x00, 0x00, 0x01, 0xDF]), // 0 to 479 (0x1DF)
    InitCmd::Cmd(0x3A, &[0x66]),
    InitCmd::Cmd(0x11, &[]),
    InitCmd::Delay(120),
    InitCmd::Cmd(0x29, &[]),
    InitCmd::Delay(20),
];

// Size of the entire frame in bytes (2 bytes per pixel)
const FRAME_BYTES: usize = BUFFER_SIZE * 2;
// Number of descriptors needed, each up to CHUNK_SIZE (4095)
const NUM_DMA_DESC: usize = (FRAME_BYTES + CHUNK_SIZE - 1) / CHUNK_SIZE;

/// Place the descriptor(s) in DMA-capable RAM.
pub static mut TX_DESCRIPTORS: [DmaDescriptor; NUM_DMA_DESC] = [DmaDescriptor::EMPTY; NUM_DMA_DESC];

pub struct Display<'a> {
    pub dpi: Option<Dpi<'a, Blocking>>,
    pub dma_tx: Option<DmaTxBuf>,
}

impl<'a> Display<'a> {
    pub fn new(peripherals: &'a mut Peripherals) -> Self {
        // Setup I2C for the TCA9554 IO expander
        let i2c = I2c::new(
            peripherals.I2C0.reborrow(),
            i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
        .unwrap()
        .with_sda(peripherals.GPIO47.reborrow())
        .with_scl(peripherals.GPIO48.reborrow());

        // Initialize the IO expander for controlling the display
        let mut expander = Tca9554::new(i2c);
        expander.write_output_reg(0b1111_0011).unwrap();
        expander.write_direction_reg(0b1111_0001).unwrap();

        let delay = Delay::new();
        log::info!("Initializing display...");

        // Set up the write_byte function for sending commands to the display
        let mut write_byte = |b: u8, is_cmd: bool| {
            const SCS_BIT: u8 = 0b0000_0010;
            const SCL_BIT: u8 = 0b0000_0100;
            const SDA_BIT: u8 = 0b0000_1000;

            let mut output = 0b1111_0001 & !SCS_BIT;
            expander.write_output_reg(output).unwrap();

            for bit in core::iter::once(!is_cmd).chain((0..8).map(|i| (b >> i) & 0b1 != 0).rev()) {
                let prev = output;
                if bit {
                    output |= SDA_BIT;
                } else {
                    output &= !SDA_BIT;
                }
                if prev != output {
                    expander.write_output_reg(output).unwrap();
                }

                output &= !SCL_BIT;
                expander.write_output_reg(output).unwrap();

                output |= SCL_BIT;
                expander.write_output_reg(output).unwrap();
            }

            output &= !SCL_BIT;
            expander.write_output_reg(output).unwrap();

            output &= !SDA_BIT;
            expander.write_output_reg(output).unwrap();

            output |= SCS_BIT;
            expander.write_output_reg(output).unwrap();
        };

        // VSYNC must be high during initialization
        let mut vsync_pin = peripherals.GPIO3.reborrow();
        let vsync_must_be_high_during_setup =
            Output::new(vsync_pin.reborrow(), Level::High, OutputConfig::default());

        // Initialize the display by sending the initialization commands
        for &init in INIT_CMDS.iter() {
            match init {
                InitCmd::Cmd(cmd, args) => {
                    write_byte(cmd, true);
                    for &arg in args {
                        write_byte(arg, false);
                    }
                }
                InitCmd::Delay(ms) => {
                    delay.delay_millis(ms as _);
                }
            }
        }
        drop(vsync_must_be_high_during_setup);

        // Set up DMA channel for LCD
        let tx_channel = peripherals.DMA_CH2.reborrow();
        let lcd_cam = LcdCam::new(peripherals.LCD_CAM.reborrow());

        // Configure the RGB display
        let config = esp_hal::lcd_cam::lcd::dpi::Config::default()
            .with_clock_mode(ClockMode {
                polarity: Polarity::IdleLow,
                phase: Phase::ShiftLow,
            })
            .with_frequency(Rate::from_mhz(10))
            .with_format(Format {
                enable_2byte_mode: true,
                ..Default::default()
            })
            .with_timing(FrameTiming {
                // active region
                horizontal_active_width: 480,
                vertical_active_height: 480,
                // extend total timings for larger porch intervals
                horizontal_total_width: 600, // allow long back/front porch
                horizontal_blank_front_porch: 80,
                vertical_total_height: 600, // allow longer vertical blank
                vertical_blank_front_porch: 80,
                // maintain sync widths
                hsync_width: 10,
                vsync_width: 4,
                // place HSYNC pulse well before active data
                hsync_position: 10,
            })
            .with_vsync_idle_level(Level::High)
            .with_hsync_idle_level(Level::High)
            .with_de_idle_level(Level::Low)
            .with_disable_black_region(false);

        // Initialize the DPI interface with all the pins
        let dpi = Dpi::new(lcd_cam.lcd, tx_channel, config)
            .unwrap()
            .with_vsync(peripherals.GPIO3.reborrow())
            .with_hsync(peripherals.GPIO46.reborrow())
            .with_de(peripherals.GPIO17.reborrow())
            .with_pclk(peripherals.GPIO9.reborrow())
            // Blue
            .with_data0(peripherals.GPIO10.reborrow())
            .with_data1(peripherals.GPIO11.reborrow())
            .with_data2(peripherals.GPIO12.reborrow())
            .with_data3(peripherals.GPIO13.reborrow())
            .with_data4(peripherals.GPIO14.reborrow())
            // Green
            .with_data5(peripherals.GPIO21.reborrow())
            .with_data6(peripherals.GPIO8.reborrow())
            .with_data7(peripherals.GPIO18.reborrow())
            .with_data8(peripherals.GPIO45.reborrow())
            .with_data9(peripherals.GPIO38.reborrow())
            .with_data10(peripherals.GPIO39.reborrow())
            // Red
            .with_data11(peripherals.GPIO40.reborrow())
            .with_data12(peripherals.GPIO41.reborrow())
            .with_data13(peripherals.GPIO42.reborrow())
            .with_data14(peripherals.GPIO2.reborrow())
            .with_data15(peripherals.GPIO1.reborrow());

        log::info!("Display initialized");

        const FRAME_BYTES: usize = BUFFER_SIZE * 2;
        let buf_box: Box<[u8; FRAME_BYTES]> = Box::new([0; FRAME_BYTES]);
        // Box::leak turns it into a &'static mut [u8]
        let psram_buf: &'static mut [u8] = Box::leak(buf_box);

        // Tie to descriptor set for one-shot DMA
        let dma_tx: DmaTxBuf =
            unsafe { DmaTxBuf::new(&mut TX_DESCRIPTORS[..], psram_buf).unwrap() };

        Self {
            dpi: Some(dpi),
            dma_tx: Some(dma_tx),
        }
    }

    pub fn transmit(&mut self, frame_buf: &FrameBuf<Rgb565, &mut [Rgb565; BUFFER_SIZE]>) {
        // Pack entire frame into PSRAM DMA buffer
        let mut dma_tx = self.dma_tx.take().unwrap();
        let mut dpi = self.dpi.take().unwrap();

        let dst = dma_tx.as_mut_slice();
        for (i, px) in frame_buf.data.iter().enumerate() {
            let px_data = px.into_storage().to_le_bytes();
            dst[(2 * i)..][..2].copy_from_slice(&px_data);
        }

        // One-shot transfer
        match dpi.send(false, dma_tx) {
            Ok(xfer) => {
                let (res, dpi2, buf2) = xfer.wait();
                dpi = dpi2;
                dma_tx = buf2;
                if let Err(e) = res {
                    log::error!("DMA error: {:?}", e);
                }
            }
            Err((e, dpi2, buf2)) => {
                log::error!("DMA send error: {:?}", e);
                dpi = dpi2;
                dma_tx = buf2;
            }
        }

        self.dma_tx = Some(dma_tx);
        self.dpi = Some(dpi);
    }
}
