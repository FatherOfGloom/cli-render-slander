use core::panic::PanicMessage;
use std::error::Error;
use std::fmt::{Debug, Display};

use std::fs::File;
use std::{
    io::{Read, Write},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    result,
};

use terminal_size::{terminal_size, Height, Width};

const MESSAGE: &'static str = "kill me\n";

pub enum FFMPegError {
    SpawnError(String),
    WriteToStdError(String),
    ReadFromStdError(String),
}

impl Display for FFMPegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FFMPegError::SpawnError(val) => write!(f, "{}", val),
            FFMPegError::WriteToStdError(val) => write!(f, "{}", val),
            FFMPegError::ReadFromStdError(val) => write!(f, "{}", val),
        }
    }
}

impl Debug for FFMPegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnError(arg0) => f.debug_tuple("SpawnError").field(arg0).finish(),
            Self::WriteToStdError(arg0) => f.debug_tuple("WriteToStdError").field(arg0).finish(),
            Self::ReadFromStdError(arg0) => f.debug_tuple("ReadFromStdError").field(arg0).finish(),
        }
    }
}

// ffmpeg -i C:/Users/anton/dev/rust/ascii/VID-20240804-WA0009.mp4

use std::process::Child;

struct ConsoleVideoRenderer {
    ffmpeg_pipe: Option<Child>,
    frame_buffer: Vec<u8>,
    frame_header_size: u8,
    frame_dimens: (usize, usize),
}

use std::io::Result;

impl ConsoleVideoRenderer {
    fn get_frame_buffer(&mut self) -> Result<&[u8]> {
        let pipe = match &mut self.ffmpeg_pipe {
            Some(val) => val,
            None => panic!("cannot get a frame buffer with uninitialized ffmpeg pipe"),
        };

        pipe.stdout
            .take()
            .unwrap()
            .read_exact(&mut self.frame_buffer[..])?;

        Ok(self.frame_buffer.as_slice())
    }

    fn test_frame_buffer(&mut self) -> Result<&[u8]> {
        let pipe = match &mut self.ffmpeg_pipe {
            Some(val) => val,
            None => panic!("cannot get a frame buffer with uninitialized ffmpeg pipe"),
        };

        pipe.stdout
            .as_mut()
            .unwrap()
            .read_to_end(&mut self.frame_buffer)?;

        Ok(&self.frame_buffer)
    }

    fn wait_for_child(&mut self) -> Result<()> {
        self.ffmpeg_pipe.as_mut().unwrap().wait()?;
        Ok(())
    }
}

fn init_ffmpeg_pipe(video_file_path: &str, w: usize, h: usize) -> Result<ConsoleVideoRenderer> {
    let mut cmd = Command::new("ffmpeg");

    // using raw_arg since arg() function passes args to ffmpeg with quotation marks on windows
    // https://github.com/rust-lang/rust/issues/92939
    cmd.raw_arg(&format!("-i {}", video_file_path))
        .raw_arg(&format!("-s {}x{}", w, h))
        .raw_arg("-vframes 1")
        // .raw_arg("-pix_fmt rgb 24")
        .raw_arg("-vcodec ppm")
        .raw_arg("-f image2pipe")
        .raw_arg("-");
    // ffmpeg -i - -ss 00:00:3 -s 650x390 -vframes 1 -c:v png -f image2pipe -
    // cmd.raw_arg(&format!("-i {}", video_file_path))
    //     .raw_arg(&format!("-s {}x{}", w, h))
    //     // .raw_arg("-f image2pipe -")
    //     .raw_arg("-f image2pipe")
    //     .raw_arg("-pix_fmt rgb 24")
    //     .raw_arg("-vcodec ppm")
    //     .raw_arg("-");

    let ffmpeg = cmd
        // .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let ppm_header_size = 9 + w.to_string().len() + h.to_string().len();
    let frame_buffer_alloc_size = ppm_header_size + (w * h * 3) as usize;

    Ok(ConsoleVideoRenderer {
        ffmpeg_pipe: Some(ffmpeg),
        frame_buffer: vec![0u8; frame_buffer_alloc_size],
        // frame_buffer: vec![],
        frame_header_size: ppm_header_size as u8,
        frame_dimens: (w, h),
    })
}

static CHARS_LIGHT: &'static [u8] = b"  .:!+*e$@8";
// static CHARS_LIGHT: &'static [u8] = b"`.-':_,^=;><+!rc*/z?sLTv)J7(|Fi{C}fI31tlu[neoZ5Yxjya]2ESwqkP6h9d4VpOGbUAKXHm8RD#$Bg0MNWQ%&@";
static CHARS_COLOR: &'static [u8] = b".*es@";

fn draw_frame_console(buffer: &[u8], w: usize, h: usize) {
    // print!({}, buffer);

    let mut v: Vec<u8> = Vec::new();

    print!("{}", "\x1B[2J");
    print!("{}", "\x1b[1;1H");

    let ppm_header_size = 9 + w.to_string().len() + h.to_string().len();

    for y in 0..h {
        for x in (0..(w * 3)).step_by(3) {
            let idx: usize = (y * (w * 3) + x + ppm_header_size).into();
            let r = buffer[idx];
            let g = buffer[idx + 1];
            let b = buffer[idx + 2];
            let a = pixel_to_ascii(r, g, b);
            v.push(a);
        }

        v.push(b'\n');
    }

    print!("{}", core::str::from_utf8(v.as_slice()).unwrap());
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

fn video_to_ascii_first_frame(file_path: &str) -> Result<()> {
    let (terminal_w, terminal_h) = match terminal_size() {
        Some((Width(w), Height(h))) => (w, h),
        // _ => panic!("Couldn't get terminal window size"),
        _ => (188, 39),
    };

    println!("console w: {} h: {}", terminal_w, terminal_h);

    let mut ffmpeg = init_ffmpeg_pipe(file_path, terminal_w as usize, terminal_h as usize).unwrap();

    // let buffer = ffmpeg.get_frame_buffer().unwrap();
    let buffer = ffmpeg.test_frame_buffer().unwrap();

    let out_path = "output.ppm";

    let mut f = File::create(out_path)?;

    draw_frame_console(buffer, terminal_w as usize, terminal_h as usize);

    if let Err(msg) = f.write_all(&buffer) {
        println!("Error at writing buffer to a file: {}", msg);
    };

    return Ok(());

    println!("buffer len: {}", buffer.len());

    if buffer.len() == 0 {
        return Ok(());
    }

    if let Err(msg) = f.write_all(&buffer) {
        println!("Error at writing buffer to a file: {}", msg);
    };

    // draw_frame_console(buffer);

    Ok(())
}

fn video_to_ascii(file_path: &str) -> Result<()> {
    let (terminal_w, terminal_h) = match terminal_size() {
        Some((Width(w), Height(h))) => (w, h),
        // _ => panic!("Couldn't get terminal window size"),
        _ => (209, 52),
    };

    println!("console w: {} h: {}", terminal_w, terminal_h);

    let mut ffmpeg = init_ffmpeg_pipe(file_path, terminal_w as usize, terminal_h as usize).unwrap();

    loop {
        match ffmpeg.get_frame_buffer() {
            Ok(b) => {
                draw_frame_console(b, terminal_w as usize, terminal_h as usize);
            }
            Err(e) => {
                println!("{}", e);
                break;
            }
        }
    }

    ffmpeg.wait_for_child()?;

    Ok(())
}

const FILE_PATH: &'static str = "C:/Users/anton/dev/rust/ascii/vid.mp4";

fn main() {
    if let Err(err) = video_to_ascii(&FILE_PATH) {
        println!("{}", err);
    };
}
