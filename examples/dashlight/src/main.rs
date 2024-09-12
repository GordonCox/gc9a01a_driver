#![no_std]
#![no_main]

mod simple_rng;
mod waveshare_rp2040_lcd_1_28;

use simple_rng::SimpleRng;

use cortex_m::delay::Delay;
use embedded_hal::delay::DelayNs;

use fugit::RateExtU32;
use gc9a01a_driver::{FrameBuffer, Orientation, Region, GC9A01A};
use panic_halt as _; // for using write! macro

use embedded_hal::spi::SpiBus;
use embedded_hal::digital::OutputPin;

use rp2040_hal::timer::Timer;
use waveshare_rp2040_lcd_1_28::entry;
use waveshare_rp2040_lcd_1_28::{
    hal::{
        self,
        clocks::{init_clocks_and_plls, Clock},
        pac,
        pio::PIOExt,
        watchdog::Watchdog,
        Sio,
    },
    Pins, XOSC_CRYSTAL_FREQ,
};

use embedded_graphics::{
    image::{Image, ImageRaw},
    pixelcolor::Rgb565,
    prelude::*,
};

const LCD_WIDTH: u32 = 240;
const LCD_HEIGHT: u32 = 240;
// Define static buffers
const BUFFER_SIZE: usize = (LCD_WIDTH * LCD_HEIGHT * 2) as usize;
// 16 FPS  Is as fast as I can update the arrow smoothly so all frames are as fast as the slowest.
const DESIRED_FRAME_DURATION_US: u32 = 1_000_000 / 16;
//const DESIRED_FRAME_DURATION_US: u32 = 1_000_000 / 24;

const CHECK_ENGINE_REGION: Region = Region { x: 72, y: 185, width: 40, height: 40 };
const RIGHT_TURN_REGION: Region = Region { x: 190, y: 100, width: 40, height: 40 };
const LEFT_TURN_REGION: Region = Region { x: 10, y: 100, width: 40, height: 40 };
const HIGH_BEAM_REGION: Region = Region { x: 27, y: 47, width: 40, height: 40 };
const EMERGENCY_BRAKE_REGION: Region = Region { x: 72, y: 14, width: 40, height: 40 };
const SEAT_BELT_REGION: Region = Region { x: 172, y: 47, width: 40, height: 40 };
const HAZARD_REGION: Region = Region { x: 127, y: 14, width: 40, height: 40 };
const SHIFT_UP_REGION: Region = Region { x: 127, y: 185, width: 40, height: 40 };
const MASTER_LIGHTING_REGION: Region = Region { x: 172, y: 152, width: 40, height: 40 };
const MAINTENANCE_REQUIRED_REGION: Region = Region { x: 27, y: 152, width: 40, height: 40 };
pub struct DelayWrapper<'a> {
    delay: &'a mut Delay,
}

impl<'a> DelayWrapper<'a> {
    pub fn new(delay: &'a mut Delay) -> Self {
        DelayWrapper { delay }
    }
}

impl<'a> DelayNs for DelayWrapper<'a> {
    fn delay_ns(&mut self, ns: u32) {
        let us = (ns + 999) / 1000; // Convert nanoseconds to microseconds
        self.delay.delay_us(us); // Use microsecond delay
    }
}

/// Main entry point for the application
#[entry]
fn main() -> ! {
    // Take ownership of peripheral instances
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    // Initialize watchdog
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    // Initialize clocks and PLLs
    let clocks = init_clocks_and_plls(XOSC_CRYSTAL_FREQ, pac.XOSC, pac.CLOCKS, pac.PLL_SYS, pac.PLL_USB, &mut pac.RESETS, &mut watchdog).ok().unwrap();

    // Print the system clock frequency
    // Assuming no prescaler, timer runs at system clock frequency

    // Initialize SIO
    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);

    // Set up the delay for the first core
    let sys_freq = clocks.system_clock.freq().to_Hz();
    let mut delay = Delay::new(core.SYST, sys_freq);

    let (mut _pio, _sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);

    // Initialize LCD pins
    let lcd_dc = pins.gp8.into_push_pull_output();
    let lcd_cs = pins.gp9.into_push_pull_output();
    let lcd_clk = pins.gp10.into_function::<hal::gpio::FunctionSpi>();
    let lcd_mosi = pins.gp11.into_function::<hal::gpio::FunctionSpi>();
    let lcd_rst = pins.gp12.into_push_pull_output_in_state(hal::gpio::PinState::High);
    let mut _lcd_bl = pins.gp25.into_push_pull_output_in_state(hal::gpio::PinState::Low);

    // Initialize SPI
    let spi = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (lcd_mosi, lcd_clk));
    let spi = spi.init(&mut pac.RESETS, clocks.peripheral_clock.freq(), 40.MHz(), embedded_hal::spi::MODE_0);

    // Initialize the display
    let mut display = GC9A01A::new(spi, lcd_dc, lcd_cs, lcd_rst, false, LCD_WIDTH, LCD_HEIGHT);

    let mut delay_wrapper = DelayWrapper::new(&mut delay);

    // Use the wrapper when initializing the display
    display.init(&mut delay_wrapper).unwrap();

    display.set_orientation(&Orientation::Portrait).unwrap();

    // Allocate the buffer in main and pass it to the FrameBuffer
    let mut background_buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
    let mut background_framebuffer = FrameBuffer::new(&mut background_buffer, LCD_WIDTH, LCD_HEIGHT);

    let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
    let mut framebuffer = FrameBuffer::new(&mut buffer, LCD_WIDTH, LCD_HEIGHT);
    background_framebuffer.clear(Rgb565::BLACK);

    display.clear_screen(Rgb565::BLACK.into_storage()).unwrap();
    _lcd_bl.into_push_pull_output_in_state(hal::gpio::PinState::High);

    // Initialize the timer
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // Load image data
    let image_data = include_bytes!("background_no_dashlight_240.raw");
    let raw_image: ImageRaw<Rgb565> = ImageRaw::new(image_data, LCD_WIDTH);
    let image = Image::new(&raw_image, Point::zero());
    // Draw the image on both frame buffers
    image.draw(&mut background_framebuffer).unwrap();
    display.show(background_framebuffer.get_buffer()).unwrap();
    //Initial draw of the background image to the framebuffer.
    image.draw(&mut framebuffer).unwrap();

    let dashlight_image_data = include_bytes!("background_dashlight_240.raw");
    let raw_dashlight_image: ImageRaw<Rgb565> = ImageRaw::new(dashlight_image_data, LCD_WIDTH);
    let dashlight_image = Image::new(&raw_dashlight_image, Point::zero());
    // Draw the image on the background frame buffers
    dashlight_image.draw(&mut background_framebuffer).unwrap();

    //delay.delay_ms(1000);

    let mut check_engine_region: Region = Region::default();
    let mut is_check_engine: bool;

    let mut right_turn_region: Region = Region::default();
    let mut is_right_turn: bool;

    let mut left_turn_region: Region = Region::default();
    let mut is_left_turn: bool;

    let mut high_beam_region: Region = Region::default();
    let mut is_high_beam: bool;

    let mut emergency_brake_region: Region = Region::default();
    let mut is_emergency_brake: bool;

    let mut seat_belt_region: Region = Region::default();
    let mut is_seat_belt: bool;

    let mut hazard_region: Region = Region::default();
    let mut is_hazard: bool;

    let mut shift_up_region: Region = Region::default();
    let mut is_shift_up: bool;

    let mut master_lighting_region: Region = Region::default();
    let mut is_master_lighting: bool;

    let mut maintenance_required_region: Region = Region::default();
    let mut is_maintenance_required: bool;

    let mut rng = SimpleRng::new([0x1, 0x2, 0x3, 0x4]);
    //let is_check_engine = rng.gen_bool(0.5);

    loop {
        let start_ticks = timer.get_counter_low();

        //#####################################################################################################
        //1. Start Loop Check Multiplexer (This is just example code right now that will be replaced with checking the Multiplexer)
        is_check_engine = rng.gen_bool(0.5);
        is_right_turn = rng.gen_bool(0.5);
        is_left_turn = rng.gen_bool(0.5);
        is_high_beam = rng.gen_bool(0.5);
        is_emergency_brake = rng.gen_bool(0.5);
        is_seat_belt = rng.gen_bool(0.5);
        is_hazard = rng.gen_bool(0.5);
        is_shift_up = rng.gen_bool(0.5);
        is_master_lighting = rng.gen_bool(0.5);
        is_maintenance_required = rng.gen_bool(0.5);

        //#####################################################################################################
        //2. Turn on any dashlights that are not on and need to be on.
        //    Set the background_framebuffer to the dashlight_image
        dashlight_image.draw(&mut background_framebuffer).unwrap();

        //Check Engine is not on but it should be.
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut check_engine_region, CHECK_ENGINE_REGION, is_check_engine);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut right_turn_region, RIGHT_TURN_REGION, is_right_turn);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut left_turn_region, LEFT_TURN_REGION, is_left_turn);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut high_beam_region, HIGH_BEAM_REGION, is_high_beam);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut emergency_brake_region, EMERGENCY_BRAKE_REGION, is_emergency_brake);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut seat_belt_region, SEAT_BELT_REGION, is_seat_belt);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut hazard_region, HAZARD_REGION, is_hazard);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut shift_up_region, SHIFT_UP_REGION, is_shift_up);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut master_lighting_region, MASTER_LIGHTING_REGION, is_master_lighting);
        update_region_on(&mut display, &mut framebuffer, &background_framebuffer, &mut maintenance_required_region, MAINTENANCE_REQUIRED_REGION, is_maintenance_required);

        //#####################################################################################################
        //3. Turn off any dashlights that are on that need to be off.
        //    Set the background_framebuffer to the background image
        image.draw(&mut background_framebuffer).unwrap();

        //Check Engine is on but it should not be.
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut check_engine_region, is_check_engine);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut right_turn_region, is_right_turn);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut left_turn_region, is_left_turn);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut high_beam_region, is_high_beam);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut emergency_brake_region, is_emergency_brake);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut seat_belt_region, is_seat_belt);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut hazard_region, is_hazard);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut shift_up_region, is_shift_up);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut master_lighting_region, is_master_lighting);
        update_region_off(&mut display, &mut framebuffer, &background_framebuffer, &mut maintenance_required_region, is_maintenance_required);

        //#####################################################################################################
        //4. Write to the Display.
        display.show_regions(framebuffer.get_buffer()).unwrap();

        //#####################################################################################################
        //5. Clear the Regions
        display.clear_regions();

        //#####################################################################################################
        //6. Delay
        //This delay is temporary it will be delayed by the code at the end of the loop to determine frequency.
        delay.delay_ms(1000);

        // Ensure each frame takes the exact same amount of time
        let end_ticks = timer.get_counter_low();
        let frame_ticks = end_ticks - start_ticks;
        if frame_ticks < DESIRED_FRAME_DURATION_US {
            delay.delay_us(DESIRED_FRAME_DURATION_US - frame_ticks);
        }

        //#####################################################################################################
        //7. End Loop
    }
}
/*
//Disabled as we swap the image in the framebuffer for on and off so this function does not work.
fn update_region(
    display: &mut GC9A01A,
    framebuffer: &mut FrameBuffer,
    background_framebuffer: &FrameBuffer,
    region: &mut Region,
    region_name: &str,
    is_on: bool
) {
    if *region == Region::default() && is_on {
        *region = *get_region(region_name).unwrap();
        framebuffer.copy_region(background_framebuffer.get_buffer(), region.x, region.y, region.width, region.height, region.x, region.y);
        display.store_region(*region).unwrap();
    } else if *region != Region::default() && !is_on {
        framebuffer.copy_region(background_framebuffer.get_buffer(), region.x, region.y, region.width, region.height, region.x, region.y);
        display.store_region(*region).unwrap();
        *region = Region::default();
    }
}
*/
fn update_region_on<SPI, DC, CS, RST>(display: &mut GC9A01A<SPI, DC, CS, RST>, framebuffer: &mut FrameBuffer, background_framebuffer: &FrameBuffer, region: &mut Region, region_const: Region, is_on: bool)
where
    SPI: SpiBus<u8>,
    DC: OutputPin,
    CS: OutputPin,
    RST: OutputPin,
{
    if *region == Region::default() && is_on {
        *region = region_const;
        framebuffer.copy_region(background_framebuffer.get_buffer(), region.x, region.y, region.width, region.height, region.x, region.y);
        display.store_region(*region).unwrap();
    }
}

fn update_region_off<SPI, DC, CS, RST>(display: &mut GC9A01A<SPI, DC, CS, RST>, framebuffer: &mut FrameBuffer, background_framebuffer: &FrameBuffer, region: &mut Region, is_on: bool)
where
    SPI: SpiBus<u8>,
    DC: OutputPin,
    CS: OutputPin,
    RST: OutputPin,
{
    if *region != Region::default() && !is_on {
        framebuffer.copy_region(background_framebuffer.get_buffer(), region.x, region.y, region.width, region.height, region.x, region.y);
        display.store_region(*region).unwrap();
        *region = Region::default();
    }
}
