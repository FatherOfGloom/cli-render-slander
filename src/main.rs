use core::time;
use std::io::{BufReader, Result};
use std::process::{Child, ChildStdout};

use std::thread;
use std::{
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

use terminal_size::{terminal_size, Height, Width};

static CHARS_LIGHT: &'static [u8] = b"  .:!+*e$@8";

struct CLIRenderer {
    w: usize,
    h: usize,
}

impl CLIRenderer {
    fn new(w: usize, h: usize) -> Self {
        CLIRenderer { w: w, h: h }
    }

    fn setup_console(&self) {
        print!("{}", "\u{001b}[2J");
    }

    fn render_ppm(&self, buf: &[u8]) {
        let w = self.w;
        let h = self.h;

        let mut v: Vec<u8> = Vec::new();

        for y in 0..h {
            for x in (0..(w * 3)).step_by(3) {
                let idx: usize = (y * (w * 3) + x).into();
                let r = buf[idx];
                let g = buf[idx + 1];
                let b = buf[idx + 2];
                let ascii = pixel_to_ascii(r, g, b);
                v.push(ascii);
            }
        }

        print!("{}", "\u{001b}[H");
        print!("\r{}", core::str::from_utf8(v.as_slice()).unwrap());
    }
}

struct FfmpegReader {
    pipe: Option<Child>,
    buf_reader: Option<BufReader<ChildStdout>>,
    frame_buffer: Vec<u8>,
    frame_size: usize,
    frame_header_size: usize,
}

impl FfmpegReader {
    fn new(video_file_path: &str, w: usize, h: usize) -> Result<Self> {
        let mut cmd = Command::new("ffmpeg");

        // using raw_arg since arg() function passes args to ffmpeg with quotation marks on windows
        // https://github.com/rust-lang/rust/issues/92939
        cmd.raw_arg(&format!("-i {}", video_file_path))
            .raw_arg(&format!("-s {}x{}", w, h))
            .raw_arg("-f image2pipe")
            .raw_arg("-pix_fmt rgb24")
            .raw_arg("-vcodec ppm")
            .raw_arg("-");

        let mut ffmpeg = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        let stdout = ffmpeg.stdout.take().unwrap();

        // TODO
        let ppm_header_size = 9 + w.to_string().len() + h.to_string().len();
        let frame_buffer_alloc_size = ppm_header_size + (w * h * 3) as usize;

        Ok(FfmpegReader {
            pipe: Some(ffmpeg),
            buf_reader: Some(BufReader::new(stdout)),
            frame_buffer: vec![],
            frame_size: frame_buffer_alloc_size,
            frame_header_size: ppm_header_size,
        })
    }

    fn get_frame_buffer_ppm(&mut self) -> Option<&[u8]> {
        let b = &mut self.frame_buffer;

        if b.len() > 0 {
            b.clear();
        }

        let m = "Cannot get a frame buffer with uninitialized BufReader.";

        let bytes_read = self
            .buf_reader
            .as_mut()
            .expect(m)
            .by_ref()
            .take(self.frame_size as u64)
            .read_to_end(b)
            .unwrap(); 

        if bytes_read == 0 {
            return None;
        }

        Some(&b[self.frame_header_size..])
    }

    fn wait_for_child(&mut self) -> Result<()> {
        let pipe = self
            .pipe
            .as_mut()
            .expect("Cannot wait for uninitialized pipe");

        pipe.wait()?;
        Ok(())
    }
}

fn pixel_to_ascii(r: u8, g: u8, b: u8) -> u8 {
    let b = rgb_to_brightness(r, g, b);
    let c = brightness_to_ascii(b);
    c
}

fn rgb_to_brightness(r: u8, g: u8, b: u8) -> f32 {
    0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
}

fn brightness_to_ascii(b: f32) -> u8 {
    let i = ((CHARS_LIGHT.len() - 1) as f32 * b / 255.) as usize;
    let res = CHARS_LIGHT[i];
    res
}

fn video_to_ascii(file_path: &str) -> Result<()> {
    let (terminal_w, terminal_h) = match terminal_size() {
        Some((Width(w), Height(h))) => (w as usize, h as usize - 1),
        _ => panic!("Couldn't get terminal window size"),
    };

    let mut ffmpeg_reader = FfmpegReader::new(file_path, terminal_w, terminal_h)?;
    let renderer = CLIRenderer::new(terminal_w, terminal_h);
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

const FILE_PATH: &'static str = "C:/Users/anton/dev/rust/ascii/vid.mp4";

fn main() {
    if let Err(err) = video_to_ascii(&FILE_PATH) {
        println!("{}", err);
    };
}