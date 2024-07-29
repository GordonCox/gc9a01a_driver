#![no_std]

use embedded_hal::blocking::delay::DelayMs;
use embedded_hal::blocking::spi;
use embedded_hal::digital::v2::OutputPin;

/// Enumeration of instructions for the GC9A01A display.
pub enum Instruction {
    NOP = 0x00,
    SWRESET = 0x01,
    RDDID = 0x04,
    RDDST = 0x09,
    SLPIN = 0x10,
    SLPOUT = 0x11,
    PTLON = 0x12,
    NORON = 0x13,
    INVOFF = 0x20,
    INVON = 0x21,
    DISPOFF = 0x28,
    DISPON = 0x29,
    CASET = 0x2A,
    RASET = 0x2B,
    RAMWR = 0x2C,
    RAMRD = 0x2E,
    PTLAR = 0x30,
    COLMOD = 0x3A,
    MADCTL = 0x36,
    FRMCTR1 = 0xB1,
    FRMCTR2 = 0xB2,
    FRMCTR3 = 0xB3,
    INVCTR = 0xB4,
    DISSET5 = 0xB6,
    PWCTR1 = 0xC0,
    PWCTR2 = 0xC1,
    PWCTR3 = 0xC2,
    PWCTR4 = 0xC3,
    PWCTR5 = 0xC4,
    VMCTR1 = 0xC5,
    RDID1 = 0xDA,
    RDID2 = 0xDB,
    RDID3 = 0xDC,
    RDID4 = 0xDD,
    PWCTR6 = 0xFC,
    GMCTRP1 = 0xE0,
    GMCTRN1 = 0xE1,
}

/// Driver for the GC9A01A display.
pub struct GC9A01A<SPI, DC, CS, RST>
where
    SPI: spi::Write<u8>,
    DC: OutputPin,
    CS: OutputPin,
    RST: OutputPin,
{
    /// SPI interface.
    spi: SPI,

    /// Data/command pin.
    dc: DC,

    /// Chip select pin.
    cs: CS,

    /// Reset pin.
    rst: RST,

    /// Whether the display is RGB (true) or BGR (false).
    rgb: bool,

    /// Global image offset.
    dx: u16,
    dy: u16,
    width: u32,
    height: u32,
}

/// Display orientation.
#[derive(Clone, Copy)]
pub enum Orientation {
    Portrait = 0x00,
    Landscape = 0x60,
    PortraitSwapped = 0xC0,
    LandscapeSwapped = 0xA0,
}

impl<SPI, DC, CS, RST> GC9A01A<SPI, DC, CS, RST>
where
    SPI: spi::Write<u8>,
    DC: OutputPin,
    CS: OutputPin,
    RST: OutputPin,
{
    /// Creates a new driver instance that uses hardware SPI.
    ///
    /// # Arguments
    ///
    /// * `spi` - SPI interface.
    /// * `dc` - Data/command pin.
    /// * `cs` - Chip select pin.
    /// * `rst` - Reset pin.
    /// * `rgb` - Whether the display is RGB (true) or BGR (false).
    /// * `width` - Width of the display.
    /// * `height` - Height of the display.
    pub fn new(
        spi: SPI,
        dc: DC,
        cs: CS,
        rst: RST,
        rgb: bool,
        width: u32,
        height: u32,
    ) -> Self {
        GC9A01A {
            spi,
            dc,
            cs,
            rst,
            rgb,
            dx: 0,
            dy: 0,
            width,
            height,
        }
    }

    /// Initializes the display.
    ///
    /// # Arguments
    ///
    /// * `delay` - Delay provider.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn init<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), ()>
    where
        DELAY: DelayMs<u8>,
    {
        self.hard_reset(delay)?;
        self.write_command(0xEF as u8, &[])?;
        self.write_command(0xEB as u8, &[0x14])?;
        self.write_command(0xFE, &[])?;
        self.write_command(0xEF, &[])?;
        self.write_command(0xEB, &[0x14])?;
        self.write_command(0x84, &[0x40])?;
        self.write_command(0x85, &[0xFF])?;
        self.write_command(0x86, &[0xFF])?;
        self.write_command(0x87, &[0xFF])?;
        self.write_command(0x88, &[0x0A])?;
        self.write_command(0x89, &[0x21])?;
        self.write_command(0x8A, &[0x00])?;
        self.write_command(0x8B, &[0x80])?;
        self.write_command(0x8C, &[0x01])?;
        self.write_command(0x8D, &[0x01])?;
        self.write_command(0x8E, &[0xFF])?;
        self.write_command(0x8F, &[0xFF])?;
        self.write_command(0xB6, &[0x00, 0x20])?;
        self.write_command(0x36, &[0x98])?;
        self.write_command(0x3A, &[0x05])?;
        self.write_command(0x90, &[0x08, 0x08, 0x08, 0x08])?;
        self.write_command(0xBD, &[0x06])?;
        self.write_command(0xBC, &[0x00])?;
        self.write_command(0xFF, &[0x60, 0x01, 0x04])?;
        self.write_command(0xC3, &[0x13])?;
        self.write_command(0xC4, &[0x13])?;
        self.write_command(0xC9, &[0x22])?;
        self.write_command(0xBE, &[0x11])?;
        self.write_command(0xE1, &[0x10, 0x0E])?;
        self.write_command(0xDF, &[0x21, 0x0C, 0x02])?;
        self.write_command(0xF0, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A])?;
        self.write_command(0xF1, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F])?;
        self.write_command(0xF2, &[0x45, 0x09, 0x08, 0x08, 0x26, 0x2A])?;
        self.write_command(0xF3, &[0x43, 0x70, 0x72, 0x36, 0x37, 0x6F])?;
        self.write_command(0xED, &[0x1B, 0x0B])?;
        self.write_command(0xAE, &[0x77])?;
        self.write_command(0xCD, &[0x63])?;
        self.write_command(0x70, &[0x07, 0x07, 0x04, 0x0E, 0x0F, 0x09, 0x07, 0x08, 0x03])?;
        self.write_command(0xE8, &[0x34])?;
        self.write_command(0x62, &[0x18, 0x0D, 0x71, 0xED, 0x70, 0x70, 0x18, 0x0F, 0x71, 0xEF, 0x70, 0x70])?;
        self.write_command(0x63, &[0x18, 0x11, 0x71, 0xF1, 0x70, 0x70, 0x18, 0x13, 0x71, 0xF3, 0x70, 0x70])?;
        self.write_command(0x64, &[0x28, 0x29, 0xF1, 0x01, 0xF1, 0x00, 0x07])?;
        self.write_command(0x66, &[0x3C, 0x00, 0xCD, 0x67, 0x45, 0x45, 0x10, 0x00, 0x00, 0x00])?;
        self.write_command(0x67, &[0x00, 0x3C, 0x00, 0x00, 0x00, 0x01, 0x54, 0x10, 0x32, 0x98])?;
        self.write_command(0x74, &[0x10, 0x85, 0x80, 0x00, 0x00, 0x4E, 0x00])?;
        self.write_command(0x98, &[0x3E, 0x07])?;
        self.write_command(0x35, &[])?;
        self.write_command(0x21, &[])?;
        self.write_command(0x11, &[])?;
        self.write_command(0x29, &[])?;

        delay.delay_ms(200);

        Ok(())
    }

    /// Performs a hard reset of the display.
    ///
    /// # Arguments
    ///
    /// * `delay` - Delay provider.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn hard_reset<DELAY>(&mut self, delay: &mut DELAY) -> Result<(), ()>
    where
        DELAY: DelayMs<u8>,
    {
        self.rst.set_high().map_err(|_| ())?;
        delay.delay_ms(10);
        self.rst.set_low().map_err(|_| ())?;
        delay.delay_ms(10);
        self.rst.set_high().map_err(|_| ())?;
        delay.delay_ms(10);

        Ok(())
    }

    /// Writes a command to the display.
    ///
    /// # Arguments
    ///
    /// * `command` - Command to write.
    /// * `params` - Parameters for the command.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    fn write_command(&mut self, command: u8, params: &[u8]) -> Result<(), ()> {
        self.cs.set_high().map_err(|_| ())?;
        self.dc.set_low().map_err(|_| ())?;
        self.cs.set_low().map_err(|_| ())?;
        self.spi.write(&[command]).map_err(|_| ())?;
        if !params.is_empty() {
            self.start_data()?;
            self.write_data(params)?;
        }
        self.cs.set_high().map_err(|_| ())?;
        Ok(())
    }

    /// Starts data transmission.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    fn start_data(&mut self) -> Result<(), ()> {
        self.dc.set_high().map_err(|_| ())
    }

    /// Writes data to the display.
    ///
    /// # Arguments
    ///
    /// * `data` - Data to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    fn write_data(&mut self, data: &[u8]) -> Result<(), ()> {
        self.cs.set_high().map_err(|_| ())?;
        self.dc.set_high().map_err(|_| ())?;
        self.cs.set_low().map_err(|_| ())?;
        self.spi.write(data).map_err(|_| ())?;
        self.cs.set_high().map_err(|_| ())?;
        Ok(())
    }

    /// Writes a data word to the display.
    ///
    /// # Arguments
    ///
    /// * `value` - Data word to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    fn write_word(&mut self, value: u16) -> Result<(), ()> {
        self.write_data(&value.to_be_bytes())
    }

    /// Writes buffered data words to the display.
    ///
    /// # Arguments
    ///
    /// * `words` - Data words to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    fn write_words_buffered(&mut self, words: impl IntoIterator<Item = u16>) -> Result<(), ()> {
        let mut buffer = [0; 32];
        let mut index = 0;
        for word in words {
            let as_bytes = word.to_be_bytes();
            buffer[index] = as_bytes[0];
            buffer[index + 1] = as_bytes[1];
            index += 2;
            if index >= buffer.len() {
                self.write_data(&buffer)?;
                index = 0;
            }
        }
        self.write_data(&buffer[0..index])
    }

    /// Sets the orientation of the display.
    ///
    /// # Arguments
    ///
    /// * `orientation` - Orientation to set.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn set_orientation(&mut self, orientation: &Orientation) -> Result<(), ()> {
        if self.rgb {
            self.write_command(Instruction::MADCTL as u8, &[*orientation as u8])?;
        } else {
            self.write_command(Instruction::MADCTL as u8, &[*orientation as u8 | 0x08])?;
        }
        Ok(())
    }

    /// Sets the global offset of the displayed image.
    ///
    /// # Arguments
    ///
    /// * `dx` - Horizontal offset.
    /// * `dy` - Vertical offset.
    pub fn set_offset(&mut self, dx: u16, dy: u16) {
        self.dx = dx;
        self.dy = dy;
    }

    /// Sets the address window for the display.
    ///
    /// # Arguments
    ///
    /// * `sx` - Start x-coordinate.
    /// * `sy` - Start y-coordinate.
    /// * `ex` - End x-coordinate.
    /// * `ey` - End y-coordinate.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn set_address_window(&mut self, sx: u16, sy: u16, ex: u16, ey: u16) -> Result<(), ()> {
        self.write_command(Instruction::CASET as u8, &[])?;
        self.start_data()?;
        self.write_word(sx + self.dx)?;
        self.write_word(ex + self.dx)?;
        self.write_command(Instruction::RASET as u8, &[])?;
        self.start_data()?;
        self.write_word(sy + self.dy)?;
        self.write_word(ey + self.dy)
    }

    /// Sets a pixel color at the given coordinates.
    ///
    /// # Arguments
    ///
    /// * `x` - X-coordinate.
    /// * `y` - Y-coordinate.
    /// * `color` - Color of the pixel.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn set_pixel(&mut self, x: u16, y: u16, color: u16) -> Result<(), ()> {
        self.set_address_window(x, y, x, y)?;
        self.write_command(Instruction::RAMWR as u8, &[])?;
        self.start_data()?;
        self.write_word(color)
    }

    /// Writes pixel colors sequentially into the current drawing window.
    ///
    /// # Arguments
    ///
    /// * `colors` - Pixel colors to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn write_pixels<P: IntoIterator<Item = u16>>(&mut self, colors: P) -> Result<(), ()> {
        self.write_command(Instruction::RAMWR as u8, &[])?;
        self.start_data()?;
        for color in colors {
            self.write_word(color)?;
        }
        Ok(())
    }

    /// Writes buffered pixel colors sequentially into the current drawing window.
    ///
    /// # Arguments
    ///
    /// * `colors` - Pixel colors to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn write_pixels_buffered<P: IntoIterator<Item = u16>>(
        &mut self,
        colors: P,
    ) -> Result<(), ()> {
        self.write_command(Instruction::RAMWR as u8, &[])?;
        self.start_data()?;
        self.write_words_buffered(colors)
    }

    /// Sets pixel colors at the given drawing window.
    ///
    /// # Arguments
    ///
    /// * `sx` - Start x-coordinate.
    /// * `sy` - Start y-coordinate.
    /// * `ex` - End x-coordinate.
    /// * `ey` - End y-coordinate.
    /// * `colors` - Pixel colors to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn set_pixels<P: IntoIterator<Item = u16>>(
        &mut self,
        sx: u16,
        sy: u16,
        ex: u16,
        ey: u16,
        colors: P,
    ) -> Result<(), ()> {
        self.set_address_window(sx, sy, ex, ey)?;
        self.write_pixels(colors)
    }

    /// Sets buffered pixel colors at the given drawing window.
    ///
    /// # Arguments
    ///
    /// * `sx` - Start x-coordinate.
    /// * `sy` - Start y-coordinate.
    /// * `ex` - End x-coordinate.
    /// * `ey` - End y-coordinate.
    /// * `colors` - Pixel colors to write.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn set_pixels_buffered<P: IntoIterator<Item = u16>>(
        &mut self,
        sx: u16,
        sy: u16,
        ex: u16,
        ey: u16,
        colors: P,
    ) -> Result<(), ()> {
        self.set_address_window(sx, sy, ex, ey)?;
        self.write_pixels_buffered(colors)
    }

    /// Draws an image from a slice of RGB565 data.
    ///
    /// # Arguments
    ///
    /// * `image_data` - Image data to draw.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn draw_image(&mut self, image_data: &[u8]) -> Result<(), ()> {
        // Assuming the image dimensions match the display dimensions
        let width = self.width as u16;
        let height = self.height as u16;

        self.set_address_window(0, 0, width - 1, height - 1)?;
        self.write_command(Instruction::RAMWR as u8, &[])?;
        self.start_data()?;
        
        for chunk in image_data.chunks(32) {
            self.write_data(chunk)?;
        }
        
        Ok(())
    }

    /// Displays the provided buffer on the screen.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to display.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn show(&mut self, buffer: &[u8]) -> Result<(), ()> {
        self.write_command(Instruction::CASET as u8, &[])?;
        self.write_data(&[0x00, 0x00, 0x00, 0xEF])?;

        self.write_command(Instruction::RASET as u8, &[])?;
        self.write_data(&[0x00, 0x00, 0x00, 0xEF])?;

        self.write_command(Instruction::RAMWR as u8, &[])?;

        self.cs.set_high().map_err(|_| ())?;
        self.dc.set_high().map_err(|_| ())?;
        self.cs.set_low().map_err(|_| ())?;
        self.spi.write(buffer).map_err(|_| ())?;
        self.cs.set_high().map_err(|_| ())?;
        
        Ok(())
    }

    /// Updates only the specified region of the display with the provided buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to display.
    /// * `region` - Region to update.
    ///
    /// # Returns
    ///
    /// `Result<(), ()>` indicating success or failure.
    pub fn show_region(&mut self, buffer: &[u8], top_left_x: u16, top_left_y: u16, width: u16, height: u16) -> Result<(), ()> {
        let sx = top_left_x as u16;
        let sy = top_left_y as u16;
        let ex = (top_left_x + width - 1) as u16;
        let ey = (top_left_y + height - 1) as u16;

        // Calculate the buffer offset for the region
        let buffer_width = self.width as usize;
        let bytes_per_pixel = 2; // For RGB565

        self.set_address_window(sx, sy, ex, ey)?;
        self.write_command(Instruction::RAMWR as u8, &[])?;
        self.start_data()?;

        for y in sy..=ey {
            let start_index = ((y as usize) * buffer_width + (sx as usize)) * bytes_per_pixel;
            let end_index = start_index + (width as usize) * bytes_per_pixel;

            for chunk in buffer[start_index..end_index].chunks(32) {
                self.write_data(chunk)?;
            }
        }

        Ok(())
    }
}


