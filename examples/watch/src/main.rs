#![no_std]
#![no_main]

mod waveshare_rp2040_lcd_1_28;

use core::fmt::Debug;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{ErrorType, SpiBus, SpiDevice};

use cortex_m::delay::Delay;
use embedded_hal::delay::DelayNs;
use fugit::RateExtU32;
use gc9a01a_driver::{FrameBuffer, Orientation, Region, GC9A01A};
use panic_halt as _; // for using write! macro

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
    mono_font::MonoTextStyleBuilder,
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, Triangle},
    text::{Baseline, Text},
};

use profont::PROFONT_12_POINT;

use libm::{cos, sin};

const LCD_WIDTH: u32 = 240;
const LCD_HEIGHT: u32 = 240;
// Define static buffers
const BUFFER_SIZE: usize = (LCD_WIDTH * LCD_HEIGHT * 2) as usize;
// 16 FPS  Is as fast as I can update the arrow smoothly so all frames are as fast as the slowest.
const DESIRED_FRAME_DURATION_US: u32 = 1_000_000 / 16;

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
    let clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Print the system clock frequency
    // Assuming no prescaler, timer runs at system clock frequency
    /*
    let sys_freq = clocks.system_clock.freq().to_Hz();
    let timer_freq = sys_freq;
    */

    // Initialize SIO
    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Set up the delay for the first core
    let sys_freq = clocks.system_clock.freq().to_Hz();
    let mut delay = Delay::new(core.SYST, sys_freq);

    let (mut _pio, _sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);

    // Initialize LCD pins
    let lcd_dc = pins.gp8.into_push_pull_output();
    let lcd_cs = pins.gp9.into_push_pull_output();
    let lcd_clk = pins.gp10.into_function::<hal::gpio::FunctionSpi>();
    let lcd_mosi = pins.gp11.into_function::<hal::gpio::FunctionSpi>();
    let lcd_rst = pins
        .gp12
        .into_push_pull_output_in_state(hal::gpio::PinState::High);
    let mut _lcd_bl = pins
        .gp25
        .into_push_pull_output_in_state(hal::gpio::PinState::Low);
    /*
        // Initialize SPI
        let spi = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (lcd_mosi, lcd_clk));
        let spi = spi.init(
            &mut pac.RESETS,
            clocks.peripheral_clock.freq(),
            40.MHz(),
            embedded_hal::spi::MODE_0,
        );
        use embedded_hal::spi::SpiDevice;
    */
    // Initialize SPI Bus
    let spi_bus = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (lcd_mosi, lcd_clk));
    let spi_bus = spi_bus.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        40.MHz(),
        embedded_hal::spi::MODE_0,
    );

    // Wrap the SPI bus in SpiDeviceImpl
    //let spi_device = SpiDeviceImpl::new(spi_bus, lcd_cs);

    // Initialize the display
    let mut display = GC9A01A::new(spi_bus, lcd_dc, lcd_cs, lcd_rst, false, LCD_WIDTH, LCD_HEIGHT);
    //display.init(&mut delay).unwrap();

    //let mut delay = Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());
    let mut delay_wrapper = DelayWrapper::new(&mut delay);

    // Use the wrapper when initializing the display
    display.init(&mut delay_wrapper).unwrap();

    display.set_orientation(&Orientation::Portrait).unwrap();

    // Allocate the buffer in main and pass it to the FrameBuffer
    let mut background_buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
    let mut background_framebuffer =
        FrameBuffer::new(&mut background_buffer, LCD_WIDTH, LCD_HEIGHT);

    let mut buffer: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
    let mut framebuffer = FrameBuffer::new(&mut buffer, LCD_WIDTH, LCD_HEIGHT);
    background_framebuffer.clear(Rgb565::BLACK);

    display.clear_screen(Rgb565::BLACK.into_storage()).unwrap();
    _lcd_bl.into_push_pull_output_in_state(hal::gpio::PinState::High);

    // Initialize the timer
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // Load image data
    let image_data = include_bytes!("rust-logo-240x240.raw");

    let raw_image: ImageRaw<Rgb565> = ImageRaw::new(image_data, LCD_WIDTH);
    let image = Image::new(&raw_image, Point::zero());

    // Draw the image on both frame buffers
    image.draw(&mut background_framebuffer).unwrap();
    display.show(background_framebuffer.get_buffer()).unwrap();
    delay.delay_ms(1000);

    // Calculate the center of the image
    let image_center = Point::new(240 / 2, 240 / 2);
    //let mut bounding_box: Rectangle;
    //let mut previous_bounding_box = Rectangle::new(Point::new(0, 0), Size::new(0, 0));

    //let mut bounding_region: Region;
    //let mut previous_bounding_region = Region::new(0,0,0,0);
    // Define a rectangle at (0, 0) with width 0 and height 0
    let mut angle: f32 = 90.0;

    image.draw(&mut framebuffer).unwrap();
    // Variables to store the minimum and maximum frame rate

    // Minute Hand
    loop {
        let start_ticks = timer.get_counter_low();

        // Draw the arrow and return the new bounding box
        let bounding_region = create_arrow(
            &mut framebuffer,
            angle as i32,
            image_center.x,
            image_center.y,
        );
        // Draw the center button
        create_button(&mut framebuffer, image_center.x, image_center.y);

        // Increment the angle
        //At 16 Frames per second I need to incrment the angle by 0.375 in order for 1 loop to take 60 seconds.
        angle += 0.375;
        if angle >= 360.0 {
            angle = 0.0;
        }

        // The bounding box has a pixel padding of 5 pixels around the arrow to prevent the need to draw the background buffer before the next arrow is drawn.
        // This improves performance as only one draw operation occurs instead of 2.
        display.store_region(bounding_region).unwrap();
        //show all regions by writing them to the display
        display.show_regions(framebuffer.get_buffer()).unwrap();
        //reset the framebuffer by copying the regions from the background buffer
        framebuffer.copy_regions(background_framebuffer.get_buffer(), display.get_regions());
        // Clear all regions and start again.
        display.clear_regions();

        // Ensure each frame takes the exact same amount of time
        let end_ticks = timer.get_counter_low();
        let frame_ticks = end_ticks - start_ticks;
        if frame_ticks < DESIRED_FRAME_DURATION_US {
            delay.delay_us(DESIRED_FRAME_DURATION_US - frame_ticks);
        }
    }
}

fn draw_text_with_background(
    framebuffer: &mut FrameBuffer,
    text: &str,
    position: Point,
    text_color: Rgb565,
    background_color: Rgb565,
) -> Rectangle {
    let character_style = MonoTextStyleBuilder::new()
        .font(&PROFONT_12_POINT)
        .text_color(text_color)
        .background_color(background_color)
        .build();

    // Calculate the size of the text
    let text_area = Rectangle::new(
        position,
        Size::new(
            text.len() as u32 * PROFONT_12_POINT.character_size.width + 10,
            PROFONT_12_POINT.character_size.height,
        ),
    );

    // Draw the background
    Rectangle::new(position, text_area.size)
        .into_styled(PrimitiveStyle::with_fill(background_color))
        .draw(framebuffer)
        .unwrap();

    // Draw the text
    Text::with_baseline(text, position, character_style, Baseline::Top)
        .draw(framebuffer)
        .unwrap();

    // Return the bounding box
    text_area
}

/// Create an arrow image at a specified angle and position
fn create_arrow(
    framebuffer: &mut FrameBuffer,
    angle: i32,
    compass_center_x: i32,
    compass_center_y: i32,
) -> Region {
    let compass_center = Point::new(compass_center_x, compass_center_y);
    let north_angle = angle - 180;
    let south_angle = angle;
    let north_left_angle = north_angle - 2;
    let north_right_angle = north_angle + 2;
    let south_left_angle = south_angle + 10;
    let south_right_angle = south_angle - 10;

    let circle_1 = 88;
    let circle_2 = 85;
    let circle_3 = 36;
    let circle_4 = 32;

    let north = get_coordinates(compass_center, circle_1, north_angle);
    let south = get_coordinates(compass_center, circle_4, south_angle);
    let north_left = get_coordinates(compass_center, circle_2, north_left_angle);
    let north_right = get_coordinates(compass_center, circle_2, north_right_angle);
    let south_left = get_coordinates(compass_center, circle_3, south_left_angle);
    let south_right = get_coordinates(compass_center, circle_3, south_right_angle);

    let merged_points = [
        north,
        north_left,
        south_left,
        south,
        south_right,
        north_right,
    ];

    let left_points = [
        north,
        north_left,
        south_left,
        south,
        Point::zero(), // unused but needed to keep array size fixed
        Point::zero(), // unused but needed to keep array size fixed
    ];

    let right_points = [
        north,
        north_right,
        south_right,
        south,
        Point::zero(), // unused but needed to keep array size fixed
        Point::zero(), // unused but needed to keep array size fixed
    ];

    let red = Rgb565::new(255, 0, 0);
    let red_9 = Rgb565::new(19, 1, 1);

    let style_red = PrimitiveStyleBuilder::new().fill_color(red).build();
    let style_red_9 = PrimitiveStyleBuilder::new().fill_color(red_9).build();

    draw_polygon(framebuffer, &merged_points, style_red_9);
    draw_polygon(framebuffer, &left_points[0..4], style_red);
    draw_polygon(framebuffer, &right_points[0..4], style_red_9);

    // Calculate the bounding box of the arrow
    let bounding_box = calculate_bounding_box(&merged_points, 10);

    bounding_box
}

/// Draw a polygon on the frame buffer
fn draw_polygon(framebuffer: &mut FrameBuffer, points: &[Point], style: PrimitiveStyle<Rgb565>) {
    if points.len() < 3 {
        return; // Not enough points to form a polygon
    }

    // Use fan triangulation from the first point
    let first_point = points[0];
    for i in 1..points.len() - 1 {
        let triangle = Triangle::new(first_point, points[i], points[i + 1]).into_styled(style);
        triangle.draw(framebuffer).unwrap();
    }
}

// Helper function to calculate coordinates based on angle and radius
fn get_coordinates(center: Point, radius: i32, angle: i32) -> Point {
    let angle_rad = (angle as f32).to_radians() as f64;
    let x = center.x + (radius as f32 * cos(angle_rad) as f32) as i32;
    let y = center.y + (radius as f32 * sin(angle_rad) as f32) as i32;
    Point::new(x, y)
}

/// Draws a circle on the frame buffer.
fn draw_circle(framebuffer: &mut FrameBuffer, color: Rgb565, center: Point, radius: i32) {
    let style = PrimitiveStyleBuilder::new().fill_color(color).build();
    // Calculate the top-left corner of the circle based on the center point and radius
    let top_left = Point::new(center.x - radius, center.y - radius);
    let diameter = (radius * 2) as u32;

    Circle::new(top_left, diameter as u32)
        .into_styled(style)
        .draw(framebuffer)
        .unwrap();
}

/// Creates a button image on the frame buffer.
fn create_button(framebuffer: &mut FrameBuffer, center_x: i32, center_y: i32) {
    let circle_radius = 14;
    draw_circle(
        framebuffer,
        Rgb565::BLACK,
        Point::new(center_x, center_y),
        circle_radius,
    );
}

/// Helper function to calculate the bounding box of a set of points with an optional padding.
fn calculate_bounding_box(points: &[Point], padding: u16) -> Region {
    let mut min_x = points[0].x as i32;
    let mut max_x = points[0].x as i32;
    let mut min_y = points[0].y as i32;
    let mut max_y = points[0].y as i32;

    for point in points.iter().skip(1) {
        if point.x < min_x {
            min_x = point.x;
        }
        if point.x > max_x {
            max_x = point.x;
        }
        if point.y < min_y {
            min_y = point.y;
        }
        if point.y > max_y {
            max_y = point.y;
        }
    }

    let padding = padding as i32;
    Region {
        x: (min_x - padding) as u16,
        y: (min_y - padding) as u16,
        width: (max_x - min_x + 2 * padding) as u32,
        height: (max_y - min_y + 2 * padding) as u32,
    }
}
