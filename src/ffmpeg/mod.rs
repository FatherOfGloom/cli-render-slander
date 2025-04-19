use std::io::{BufReader, Read, Result};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdout, Command, Stdio};

pub struct FfmpegReader {
    pipe: Option<Child>,
    buf_reader: Option<BufReader<ChildStdout>>,
    frame_buffer: Vec<u8>,
    frame_size: usize,
    frame_header_size: usize,
}

impl FfmpegReader {
    pub fn new(video_file_path: &str, w: usize, h: usize) -> Result<Self> {
        let mut cmd = Command::new("ffmpeg");

        // using raw_arg since arg() function passes args to ffmpeg with quotation marks on windows
        // https://github.com/rust-lang/rust/issues/92939
        cmd.raw_arg(&format!("-i {}", video_file_path))
            .raw_arg(&format!("-s {}x{}", w, h))
            .raw_arg("-f image2pipe")
            .raw_arg("-pix_fmt rgb24")
            .raw_arg("-vcodec ppm")
            .raw_arg("-nostats")
            .raw_arg("-hide_banner")
            .raw_arg("-");

        let mut ffmpeg = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        let stdout = ffmpeg.stdout.take().unwrap();

        // TODO: implement stderr reading
        // let mut stderr = ffmpeg.stderr.take().unwrap();
        // let mut errs = vec![];
        // let err_bytes_read = stderr.read_to_end(&mut errs).unwrap();

        // if err_bytes_read != 0 {
        //     panic!("FFMPEG error: {}", str::from_utf8(&mut errs).unwrap());
        // }
        
        // TODO: ugly
        let ppm_header_size = 9 + w.to_string().len() + h.to_string().len();
        let frame_size = ppm_header_size + w * h * 3;

        Ok(FfmpegReader {
            pipe: Some(ffmpeg),
            buf_reader: Some(BufReader::new(stdout)),
            frame_buffer: vec![],
            frame_size: frame_size,
            frame_header_size: ppm_header_size,
        })
    }

    pub fn get_frame_buffer_ppm(&mut self) -> Option<&[u8]> {
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

    pub fn wait_for_child(&mut self) -> Result<()> {
        let pipe = self
            .pipe
            .as_mut()
            .expect("Cannot wait for uninitialized pipe");

        pipe.wait()?;
        Ok(())
    }
}