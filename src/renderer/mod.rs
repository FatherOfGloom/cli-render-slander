use core::{str, time};
use std::{io::Result, thread};
use terminal_size::{terminal_size, Height, Width};

use crate::ffmpeg::FfmpegReader;
static CHARS_LIGHT: &'static[u8] = b"  .:;=!+*#$8@";

pub struct CLIRenderer {
    w: usize,
    h: usize,
    lazy_buf: Option<Vec<u8>>
}

impl CLIRenderer {
    fn new(w: usize, h: usize) -> Self {
        CLIRenderer { w, h, lazy_buf: None }
    }

    fn setup_console(&self) {
        print!("{}", "\u{001b}[2J");
    }

    fn pixel_to_ascii(r: u8, g: u8, b: u8) -> u8 {
        let brightness = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
        let i = ((CHARS_LIGHT.len() - 1) as f32 * brightness / 255.) as usize;
        let res = CHARS_LIGHT[i];
        res
    }

    fn get_buf(&mut self, size: usize) -> &mut [u8] {
        if self.lazy_buf.is_none() {
            self.lazy_buf = Some(vec![0u8; size]);
        }

        self.lazy_buf.as_mut().unwrap().as_mut_slice()
    }

    fn render_ppm(&mut self, buf: &[u8]) {
        let w = self.w;
        let h = self.h;

        // No need for extra allocations
        let render_buf = self.get_buf(w * h);

        let mut i = 0;

        for y in 0..h {
            for x in (0..(w * 3)).step_by(3) {
                let idx: usize = (y * (w * 3) + x).into();
                let (r, g, b) = (buf[idx], buf[idx + 1], buf[idx + 2]);
                let ascii = Self::pixel_to_ascii(r, g, b);

                // I don't give a fuck if it panics, men shouldn't panic
                render_buf[i] = ascii;
                i += 1;
            }
        }

        print!("{}", "\u{001b}[H");

        unsafe {
            print!("\r{}", str::from_utf8_unchecked(render_buf));
        }
    }
}

pub fn video_to_ascii(file_path: &str) -> Result<()> {
    let (terminal_w, terminal_h) = match terminal_size() {
        Some((Width(w), Height(h))) => (w as usize, h as usize - 1),
        _ => panic!("Couldn't get terminal window size"),
    };

    let mut ffmpeg_reader = FfmpegReader::new(file_path, terminal_w, terminal_h)?;
    let mut renderer = CLIRenderer::new(terminal_w, terminal_h);
    let frame_delay = time::Duration::from_millis(20);

    renderer.setup_console();

    loop {
        match ffmpeg_reader.get_frame_buffer_ppm() {
            Some(b) => renderer.render_ppm(b),
            _ => break,
        }
        thread::sleep(frame_delay);
    }

    ffmpeg_reader.wait_for_child()?;

    Ok(())
}