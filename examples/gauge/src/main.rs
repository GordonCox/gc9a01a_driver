#![no_std]
#![no_main]

mod waveshare_rp2040_lcd_1_28;

use embedded_hal::digital::OutputPin;

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

use profont::PROFONT_18_POINT;

use libm::{cos, sin};

use core::fmt::Write;
use heapless::String; // Import the Write trait for using the write! macro

const LCD_WIDTH: u32 = 240;
const LCD_HEIGHT: u32 = 240;
// Define static buffers
const BUFFER_SIZE: usize = (LCD_WIDTH * LCD_HEIGHT * 2) as usize;
// 16 FPS  Is as fast as I can update the arrow smoothly so all frames are as fast as the slowest.
const DESIRED_FRAME_DURATION_US: u32 = 1_000_000 / 16;
//const DESIRED_FRAME_DURATION_US: u32 = 1_000_000 / 24;

// Calculate the center of the image
const ARROW_ROTATE_POINT_X: i32 = 240 / 2;
const ARROW_ROTATE_POINT_Y: i32 = (240 / 10) * 8;

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

#[derive(Debug)]
enum Mode {
    EXAMPLE,
    _DEBUG,
}

const GAUGE_MODE: Option<Mode> = Some(Mode::EXAMPLE);

struct Measurement {
    _adc_value: f32,
    converted_value: f32,
    calculated_average: f32,
    mapped_angle: f32,
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

    //Initialize the Analog to Digital ADC pin for reading the resistance input.

    // Set up the ADC
    //let mut adc = Adc::new(pac.ADC, &mut pac.RESETS);
    // Configure pin 26 as an ADC pin
    //let mut adc_pin_26 = AdcPin::new(pins.gp26.into_floating_input()).unwrap();
    //let mut adc_pin = pins.gp26.into_floating_input();

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

    // Initialize SPI
    // Initialize SPI Bus
    let spi_bus = hal::Spi::<_, _, _, 8>::new(pac.SPI1, (lcd_mosi, lcd_clk));
    let spi_bus = spi_bus.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        40.MHz(),
        embedded_hal::spi::MODE_0,
    );

    // Initialize the display
    let mut display = GC9A01A::new(
        spi_bus, lcd_dc, lcd_cs, lcd_rst, false, LCD_WIDTH, LCD_HEIGHT,
    );
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
    let image_data = get_background(GAUGE_MODE);

    let raw_image: ImageRaw<Rgb565> = ImageRaw::new(image_data, LCD_WIDTH);
    let image = Image::new(&raw_image, Point::zero());

    // Draw the image on both frame buffers
    image.draw(&mut background_framebuffer).unwrap();
    display.show(background_framebuffer.get_buffer()).unwrap();
    delay.delay_ms(1000);

    // Define a rectangle at (0, 0) with width 0 and height 0
    let mut angle: f32 = 45.0;
    let increment: f32 = 1.0;
    let mut increasing: bool = true;

    image.draw(&mut framebuffer).unwrap();
    // Variables to store the minimum and maximum frame rate

    let mut measurement = Measurement {
        _adc_value: 0.0,
        converted_value: 0.0,
        calculated_average: 0.0,
        mapped_angle: 45.0,
    };
    //known resistor for voltage divider
    //The known resistor is connected between the positive and the adc_pin_26.
    //The unknown resistor (return value) is connected between adc_pin_26 and ground.
    //let known_resistor: f32 = 220.0;

    loop {
        let start_ticks = timer.get_counter_low();

        //Example Read the Input Pin
        //let r2_ohms = measure_resistance(&mut adc, &mut adc_pin_26, known_resistor);
        //let voltage = measure_voltage(&mut adc, &mut adc_pin_26);

        if let Some(Mode::EXAMPLE) = GAUGE_MODE {
            // Increment the angle
            if increasing {
                angle += increment;
                if angle >= 135.0 {
                    angle = 135.0;
                    increasing = false;
                }
            } else {
                angle -= increment;
                if angle <= 45.0 {
                    angle = 45.0;
                    increasing = true;
                }
            }
            measurement = Measurement {
                _adc_value: 0.0,
                converted_value: 0.0,
                calculated_average: 0.0,
                mapped_angle: angle,
            };
        }

        // Draw the arrow and return the new bounding box
        let bounding_region = create_arrow(
            &mut framebuffer,
            measurement.mapped_angle as i32,
            ARROW_ROTATE_POINT_X,
            ARROW_ROTATE_POINT_Y,
        );
        display.store_region(bounding_region).unwrap();

        // Draw the center button on top of the arrow.
        create_button(&mut framebuffer, ARROW_ROTATE_POINT_X, ARROW_ROTATE_POINT_Y);

        //Create a String that can hold the number and write it to the screen.
        let mut west_number_str: String<32> = String::new(); // Create a heapless String with a capacity of 32
        write!(west_number_str, "{:.0}", measurement.converted_value).unwrap(); // Write the number into the string

        let west_text_bounding_region = draw_text_with_background(
            &mut framebuffer,
            &west_number_str,
            Point::new(35, 35),
            Rgb565::BLACK,
        );
        display.store_region(west_text_bounding_region).unwrap();

        let mut east_number_str: String<32> = String::new(); // Create a heapless String with a capacity of 32
        write!(east_number_str, "{:.1}", measurement.calculated_average).unwrap(); // Write the number into the string

        let east_text_bounding_region = draw_text_with_background(
            &mut framebuffer,
            &east_number_str,
            Point::new(173, 35),
            Rgb565::BLACK,
        );

        //Clear the background
        //let background_differences = background_framebuffer.diff_with(&framebuffer);
        //display.draw_iter(background_differences).unwrap();
        //Now Draw the changes.
        //let differences = framebuffer.diff_with(&background_framebuffer);
        //display.draw_iter(differences).unwrap();

        display.store_region(east_text_bounding_region).unwrap();

        //Display the next set of regions.
        display.show_regions(framebuffer.get_buffer()).unwrap();
        //reset the display frame buffer from the background for the regions just displayed.
        framebuffer.copy_regions(background_framebuffer.get_buffer(), display.get_regions());
        //clear out the regions from the display so its ready to start again.
        display.clear_regions();
        //Set this bounding region as the next arrow may not overlap the current arrow.
        //This logic can be removed if I loop through the arrows to get to the target location.
        display.store_region(bounding_region).unwrap();
        // Ensure each frame takes the exact same amount of time
        let end_ticks = timer.get_counter_low();
        let frame_ticks = end_ticks - start_ticks;
        if frame_ticks < DESIRED_FRAME_DURATION_US {
            delay.delay_us(DESIRED_FRAME_DURATION_US - frame_ticks);
        }
    }
}

fn get_background(mode: Option<Mode>) -> &'static [u8] {
    match mode {
        Some(Mode::EXAMPLE) => include_bytes!("rust-logo-240x240.raw"),
        Some(Mode::_DEBUG) => include_bytes!("rust-logo-240x240.raw"),
        None => include_bytes!("rust-logo-240x240.raw"),
    }
}

//This can be used for various mapping
//You can map the return voltage or resistance to an angle for the arrow.
fn map_value(input_value: f32, in_min: f32, in_max: f32, out_min: f32, out_max: f32) -> f32 {
    if in_max == in_min {
        //panic!("in_max and in_min cannot be equal");
        return 0.0;
    }

    let mapped_output_value: f32;

    if input_value <= in_min {
        mapped_output_value = out_min;
    } else if input_value >= in_max {
        mapped_output_value = out_max;
    } else {
        mapped_output_value =
            (input_value - in_min) * (out_max - out_min) / (in_max - in_min) + out_min;
    }

    mapped_output_value
}

fn draw_text_with_background(
    framebuffer: &mut FrameBuffer,
    text: &str,
    position: Point,
    text_color: Rgb565,
) -> Region {
    let character_style = MonoTextStyleBuilder::new()
        .font(&PROFONT_18_POINT)
        .text_color(text_color)
        .build();

    // Calculate the size of the text
    let text_area = Rectangle::new(
        position,
        Size::new(
            text.len() as u32 * PROFONT_18_POINT.character_size.width,
            PROFONT_18_POINT.character_size.height,
        ),
    );

    // Draw the text
    Text::with_baseline(text, position, character_style, Baseline::Top)
        .draw(framebuffer)
        .unwrap();

    // Return the bounding box
    //Added 22 width on the Region to accomidate larger numbers
    Region {
        x: text_area.top_left.x as u16,
        y: text_area.top_left.y as u16,
        width: text_area.size.width + 22,
        height: text_area.size.height,
    }
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

    let circle_1 = 128;
    let circle_2 = 125;
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

    let primary_color = PrimitiveStyleBuilder::new().fill_color(red).build();
    let complementary_color = PrimitiveStyleBuilder::new().fill_color(red_9).build();

    draw_polygon(framebuffer, &merged_points, primary_color);
    draw_polygon(framebuffer, &left_points[0..4], complementary_color);
    draw_polygon(framebuffer, &right_points[0..4], primary_color);

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
